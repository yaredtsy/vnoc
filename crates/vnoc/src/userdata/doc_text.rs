//! The text of every doc, kept in memory for search (`docs/20-search/03-doc-text.md` §5).
//!
//! A record in the index is the `node.toml` and the **names** of its `.md` files, so an edited body is not a change to it. The text has its own store:
//! for each (node ID, file name) the file's stamp (size and modified time), its title and its body. [`DocTexts::refresh`] stats every doc and reads
//! only the ones whose stamp moved. Its [`generation`](DocTexts::generation) is its own counter and never the index's revision (Q2).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use vnoc_domain::NodeId;

use super::layout;
use super::read::{NodeRecord, split_doc};

/// A doc larger than this is not a note, and is not kept.
pub const MAX_DOC_BYTES: u64 = 1_000_000;

/// What a file was when it was read: if neither moved, the text is the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    size: u64,
    modified: Option<SystemTime>,
}

/// One doc as search reads it.
#[derive(Debug)]
pub struct DocText {
    /// absolute path of the `.md` file
    pub path: PathBuf,
    /// the first `# heading`, else the file name without `.md`
    pub title: String,
    /// the text after the frontmatter, without the title's own line
    pub body: String,
    stamp: Stamp,
}

type Key = (NodeId, String);

/// Every doc of every node, sorted by (node ID, file name): the order is the same for one generation, so a doc index built from it stays valid.
#[derive(Debug, Default)]
pub struct DocTexts {
    generation: u64,
    docs: BTreeMap<Key, Arc<DocText>>,
    /// docs left out (not UTF-8, too big) with the stamp they had, so that one warning is logged per change of the file, not one per update
    skipped: BTreeMap<Key, Stamp>,
}

/// What one [`DocTexts::refresh`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DocStats {
    /// docs listed by the records, each looked at on disk once
    pub stat_count: usize,
    /// files read again (a new doc, or one whose stamp moved)
    pub read_count: usize,
    /// a doc was added, dropped or read again: the generation moved
    pub changed: bool,
}

impl DocTexts {
    /// +1 for each [`refresh`](DocTexts::refresh) that changed something. Not the index's revision.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// The docs in order: the node's permanent ID, the file name, the doc.
    pub fn iter(&self) -> impl Iterator<Item = (&NodeId, &str, &DocText)> {
        self.docs
            .iter()
            .map(|((id, file), doc)| (id, file.as_str(), &**doc))
    }

    /// The docs of `records` as they are on disk now, given what was held before. Disk work only: no lock is needed, and `self` is not changed.
    /// `root` is the `.vnoc/` folder.
    pub fn refresh(&self, root: &Path, records: &[(NodeId, NodeRecord)]) -> (DocTexts, DocStats) {
        let mut stats = DocStats::default();
        let mut docs = BTreeMap::new();
        let mut skipped = BTreeMap::new();
        for (id, record) in records {
            let folder = layout::node_dir(root, id);
            for file in &record.docs {
                stats.stat_count += 1;
                let path = folder.join(file);
                let Ok(meta) = fs::metadata(&path) else {
                    continue;
                };
                let stamp = Stamp {
                    size: meta.len(),
                    modified: meta.modified().ok(),
                };
                let key = (*id, file.clone());
                if let Some(held) = self.docs.get(&key).filter(|d| d.stamp == stamp) {
                    docs.insert(key, held.clone());
                    continue;
                }
                // left out before, and the file is as it was: not tried again, not warned about again
                if self.skipped.get(&key) == Some(&stamp) {
                    skipped.insert(key, stamp);
                    continue;
                }
                if meta.len() > MAX_DOC_BYTES {
                    tracing::warn!(
                        "{}: not searched, it is larger than {MAX_DOC_BYTES} bytes",
                        path.display()
                    );
                    skipped.insert(key, stamp);
                    continue;
                }
                stats.read_count += 1;
                match fs::read_to_string(&path) {
                    Ok(text) => {
                        let (title, body) = split_doc(&text);
                        let title =
                            title.unwrap_or_else(|| file.trim_end_matches(".md").to_string());
                        let doc = DocText {
                            path,
                            title,
                            body: body.into_owned(),
                            stamp,
                        };
                        docs.insert(key, Arc::new(doc));
                    }
                    Err(e) => {
                        tracing::warn!("{}: not searched, {e}", path.display());
                        skipped.insert(key, stamp);
                    }
                }
            }
        }
        // a doc is the same when its stamp is (that is what "keep" means above): so the stamps tell whether anything moved
        stats.changed = docs.len() != self.docs.len()
            || docs
                .iter()
                .any(|(key, new)| self.docs.get(key).is_none_or(|old| old.stamp != new.stamp));
        let generation = self.generation + u64::from(stats.changed);
        (
            DocTexts {
                generation,
                docs,
                skipped,
            },
            stats,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::time::Duration;
    use vnoc_domain::NodeToml;

    pub(crate) struct Scratch {
        pub root: PathBuf,
    }

    impl Scratch {
        pub fn new(label: &str) -> Scratch {
            let root =
                std::env::temp_dir().join(format!("vnoc-doctext-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Scratch { root }
        }

        /// A record for node number `n` whose folder holds the given files (written); returns the record.
        pub fn node(&self, n: u32, files: &[(&str, &[u8])]) -> (NodeId, NodeRecord) {
            let id = NodeId::parse(&format!("{n:026}")).unwrap();
            let folder = layout::node_dir(&self.root, &id);
            fs::create_dir_all(&folder).unwrap();
            for (name, bytes) in files {
                fs::write(folder.join(name), bytes).unwrap();
            }
            let mut docs: Vec<String> = files.iter().map(|(n, _)| n.to_string()).collect();
            docs.sort();
            let toml = NodeToml::parse("path = \"a.py\"\nqualname = \"f\"\n").unwrap();
            (id, NodeRecord { toml, docs })
        }

        pub fn file(&self, id: &NodeId, name: &str) -> PathBuf {
            layout::node_dir(&self.root, id).join(name)
        }
    }

    fn stamp_at(file: &Path, secs: u64) {
        let f = File::options().write(true).open(file).unwrap();
        f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
            .unwrap();
    }

    fn texts(held: &DocTexts) -> Vec<(String, String, String)> {
        held.iter()
            .map(|(id, file, d)| {
                (
                    format!("{}/{file}", &id.to_string()[24..]),
                    d.title.clone(),
                    d.body.clone(),
                )
            })
            .collect()
    }

    #[test]
    fn the_first_refresh_reads_every_doc_and_the_second_reads_none() {
        let s = Scratch::new("first");
        let records = vec![
            s.node(
                1,
                &[
                    ("b.md", b"---\nnode: X\n---\n# Title B\nbody b".as_slice()),
                    ("a.md", b"no heading here"),
                ],
            ),
            s.node(2, &[("c.md", b"# C\n")]),
        ];
        let (first, stats) = DocTexts::default().refresh(&s.root, &records);
        assert_eq!(
            stats,
            DocStats {
                stat_count: 3,
                read_count: 3,
                changed: true
            }
        );
        assert_eq!(first.generation(), 1);
        assert_eq!(
            texts(&first),
            [
                ("01/a.md".into(), "a".into(), "no heading here".into()),
                ("01/b.md".into(), "Title B".into(), "body b".into()),
                ("02/c.md".into(), "C".into(), "".into()),
            ],
            "sorted by (id, file); the title is the heading or the file name; the body has neither the frontmatter nor the heading"
        );

        let (second, stats) = first.refresh(&s.root, &records);
        assert_eq!(
            stats,
            DocStats {
                stat_count: 3,
                read_count: 0,
                changed: false
            }
        );
        assert_eq!(
            second.generation(),
            1,
            "nothing changed: the same generation"
        );
        assert!(
            first
                .iter()
                .zip(second.iter())
                .all(|((_, _, a), (_, _, b))| std::ptr::eq(a, b)),
            "the texts are kept, not copied"
        );
    }

    #[test]
    fn a_changed_doc_is_read_again_even_when_the_size_is_the_same() {
        let s = Scratch::new("changed");
        let records = vec![s.node(1, &[("a.md", b"alpha".as_slice()), ("b.md", b"bravo")])];
        for (name, secs) in [("a.md", 1_000), ("b.md", 1_000)] {
            stamp_at(&s.file(&records[0].0, name), secs);
        }
        let (first, _) = DocTexts::default().refresh(&s.root, &records);

        // the same size and a new time
        let a = s.file(&records[0].0, "a.md");
        fs::write(&a, b"ALPHA").unwrap();
        stamp_at(&a, 2_000);
        let (second, stats) = first.refresh(&s.root, &records);
        assert_eq!(
            (stats.stat_count, stats.read_count, stats.changed),
            (2, 1, true)
        );
        assert_eq!(second.generation(), first.generation() + 1);
        assert_eq!(texts(&second)[0].2, "ALPHA");

        // the same size and a later time again
        fs::write(&a, b"alpha").unwrap();
        stamp_at(&a, 3_000);
        let (fourth, stats) = second.refresh(&s.root, &records);
        assert_eq!((stats.read_count, stats.changed), (1, true));
        assert_eq!(texts(&fourth)[0].2, "alpha");
        assert_eq!(fourth.generation(), second.generation() + 1);
    }

    #[test]
    fn a_deleted_doc_is_dropped_and_a_new_one_is_added() {
        let s = Scratch::new("gone");
        let mut records = vec![s.node(1, &[("a.md", b"alpha".as_slice()), ("b.md", b"bravo")])];
        let (first, _) = DocTexts::default().refresh(&s.root, &records);

        // `b.md` is deleted but its record still lists it (a stale record): dropped; `c.md` is new
        fs::remove_file(s.file(&records[0].0, "b.md")).unwrap();
        fs::write(s.file(&records[0].0, "c.md"), b"charlie").unwrap();
        records[0].1.docs.push("c.md".into());
        let (second, stats) = first.refresh(&s.root, &records);
        assert_eq!(
            stats,
            DocStats {
                stat_count: 3,
                read_count: 1,
                changed: true
            }
        );
        assert_eq!(
            texts(&second).into_iter().map(|t| t.0).collect::<Vec<_>>(),
            ["01/a.md", "01/c.md"]
        );
        assert_eq!(second.generation(), first.generation() + 1);

        // a record gone altogether: its docs go
        let (third, stats) = second.refresh(&s.root, &[]);
        assert_eq!((third.len(), stats.changed), (0, true));
    }

    #[test]
    fn a_file_that_is_not_utf8_or_over_a_megabyte_is_skipped_and_the_rest_kept() {
        let s = Scratch::new("skip");
        let big = vec![b'x'; MAX_DOC_BYTES as usize + 1];
        let exactly = vec![b'y'; MAX_DOC_BYTES as usize];
        let records = vec![s.node(
            1,
            &[
                ("big.md", big.as_slice()),
                ("binary.md", &[0xff, 0xfe, b'a']),
                ("exactly.md", exactly.as_slice()),
                ("ok.md", b"fine"),
            ],
        )];
        let (held, stats) = DocTexts::default().refresh(&s.root, &records);
        let names: Vec<_> = texts(&held).into_iter().map(|t| t.0).collect();
        assert_eq!(names, ["01/exactly.md", "01/ok.md"]);
        assert_eq!(stats.stat_count, 4);
        assert_eq!(
            stats.read_count, 3,
            "the big one is not read; the binary one is tried"
        );

        // as they were: the skipped ones are not tried again, and nothing changed
        let (again, stats) = held.refresh(&s.root, &records);
        assert_eq!((stats.read_count, stats.changed), (0, false));
        assert_eq!(again.generation(), held.generation());
    }

    /// By hand: `cargo test -p vnoc --release --lib -- --ignored --nocapture doc_text`. 03 §7's measurement: 1,000 docs of about 2 KB in 1,000 records.
    #[test]
    #[ignore = "a timing, run by hand"]
    fn timing_1000_docs() {
        use std::time::Instant;
        use vnoc_search::{Corpus, DocEntry, Limits, parse, run};

        // words from a vocabulary of 4,000 with a few common ones, as the crate's timing test has them
        let word = |n: u64| {
            let letters = b"bcdfghklmnprstvz";
            let vowels = b"aeiou";
            let mut w = String::new();
            let mut n = n;
            for _ in 0..2 {
                w.push(letters[(n % 16) as usize] as char);
                n /= 16;
                w.push(vowels[(n % 5) as usize] as char);
                n /= 5;
            }
            w
        };
        let mut seed = 12345u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            seed >> 33
        };
        let s = Scratch::new("timing");
        let mut records = Vec::new();
        for n in 0..1000u32 {
            let mut body = String::new();
            while body.len() < 2000 {
                let r = next();
                body.push_str(&word(if r % 4 == 0 { r % 20 } else { r % 4000 }));
                body.push(if r % 11 == 0 { '\n' } else { ' ' });
            }
            let text = format!(
                "---\nnode: X\n---\n# {} {}\n{body}",
                word(next() % 4000),
                word(next() % 4000)
            );
            records.push(s.node(n + 1, &[("doc.md", text.as_bytes())]));
        }

        let started = Instant::now();
        let (first, stats) = DocTexts::default().refresh(&s.root, &records);
        println!("first refresh: {:?}, {stats:?}", started.elapsed());
        let held: usize = first
            .iter()
            .map(|(_, _, d)| d.title.len() + d.body.len())
            .sum();
        println!(
            "held: {} docs, {} bytes of titles and bodies ({} KB)",
            first.len(),
            held,
            held / 1024
        );

        let docs: Vec<DocEntry> = first
            .iter()
            .map(|(id, file, d)| DocEntry {
                node: None,
                node_id: id.to_string(),
                node_name: String::new(),
                node_kind: None,
                node_path: String::new(),
                missing: false,
                file: file.to_string(),
                doc_path: d.path.to_string_lossy().into_owned(),
                title: d.title.clone(),
                body: d.body.clone(),
            })
            .collect();
        let started = Instant::now();
        let corpus = Corpus::new(Vec::new(), docs);
        println!("Corpus::new (the doc index): {:?}", started.elapsed());
        for query in ["baba", "ba"] {
            let started = Instant::now();
            let results = run(&corpus, &parse(query), None, Limits::default());
            println!(
                "run {query:?}: {:?}, {} docs found, {} shown",
                started.elapsed(),
                results.docs_total,
                results.docs.len()
            );
        }

        let changed = s.file(&records[500].0, "doc.md");
        fs::write(&changed, "# Changed\nnew text").unwrap();
        let started = Instant::now();
        let (second, stats) = first.refresh(&s.root, &records);
        println!(
            "one doc changed, refresh: {:?}, {stats:?}, generation {} to {}",
            started.elapsed(),
            first.generation(),
            second.generation()
        );
        let started = Instant::now();
        let (_, stats) = second.refresh(&s.root, &records);
        println!(
            "nothing changed, refresh: {:?}, {stats:?}",
            started.elapsed()
        );
    }
}
