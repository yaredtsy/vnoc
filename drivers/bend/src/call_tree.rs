//! `vnoc/callTree`: the calls made by one def, one node per call site. The tree is not merged: that is the core's (docs/14 T2). The solving is
//! `bend_analysis::Session::graph`; this file runs it, and puts the answer in the protocol's shape (paths relative to `root`, 0-based lines, UTF-16
//! columns).
//!
//! The sessions are kept: `Sessions` holds one open `Session` per (root, entry file), and each request first brings it up to date with the disk
//! (`refresh`), so an edit, a new file or a deleted one is seen without opening it again. It lives on the worker thread (`worker.rs`).
//!
//! A Bend file has no code of its own: the entry is always a def (`Entry::File` is answered with nothing), and a def with the name of a law
//! and its proof is one of two (occurrence 1 and 2).

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use analyzer_core::graph::Entry;
use bend_analysis::{
    Budget, Cancel, Frame, MarkerStyle, Options, RenderOptions, Session, WireOptions, WireStyle,
    wire,
};
use lsp_server::ErrorCode;
use vnoc_driver_protocol::{
    CallMark, CallNode, CallTarget, CallTreeParams, CallTreeResult, DefKind,
};

use crate::outline::{analyzer_occurrence, kind};
use crate::positions::Lines;
use crate::source;
use crate::{Failure, invalid};

/// The wire format versions of the analyzer (`analyzer_core::wire::WIRE_FORMAT_VERSION`, "bumped when a field is added that changes what a node
/// means") that this driver has been checked against. 2 is what the pinned rev reports. Any other number fails
/// `the_wire_format_is_one_this_driver_was_checked_against`: a bump is a reason to read what changed, not to pass.
pub const WIRE_FORMATS_READ: [u32; 1] = [2];

/// What the worker has done, for the test hooks (`vnoc/test/stats`) and the log.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    /// the solver was started (a request that was cancelled in the queue never gets here)
    pub solves: u64,
    /// `Session::open` calls: one for each (root, file) the first time, and again after a crash dropped it
    pub opens: u64,
    /// a refresh found a file changed on the disk and built the book again
    pub reopens: u64,
}

/// One open `Session` per (root, entry file). Kept on the worker thread that made them.
#[derive(Default)]
pub struct Sessions {
    by_entry: HashMap<(String, String), Session>,
    pub counts: Counts,
}

impl Sessions {
    /// The session of this root and file, opened on the first use, brought up to date with the disk before each request. A session whose entry
    /// file is gone has no book (`lost`): that is an error to the caller, and the next request that finds the file again brings it back.
    fn ensure(&mut self, root: &str, file: &str) -> Result<(), Failure> {
        let key = (root.to_string(), file.to_string());
        if !self.by_entry.contains_key(&key) {
            let session = Session::open(
                Path::new(root),
                Path::new(file),
                Options::new(MarkerStyle::above("ID")),
            )
            .map_err(|e| internal(format!("cannot open {file}: {e}")))?;
            self.counts.opens += 1;
            self.by_entry.insert(key.clone(), session);
        }
        let session = self.by_entry.get_mut(&key).expect("just opened");
        let refreshed = session.refresh();
        if refreshed.reopened {
            self.counts.reopens += 1;
        }
        if let Some(lost) = refreshed.lost {
            return Err(internal(format!("cannot read {file}: {lost}")));
        }
        Ok(())
    }

    /// Drops the sessions of a root that may be in a bad state (a request panicked inside one): the next request opens new ones.
    pub fn forget(&mut self, root: &str) {
        self.by_entry.retain(|(r, _), _| r != root);
    }

    pub fn len(&self) -> usize {
        self.by_entry.len()
    }
}

fn internal(message: impl Into<String>) -> Failure {
    Failure {
        code: ErrorCode::InternalError,
        message: message.into(),
    }
}

/// `cancel` is the request's: its flag is the core's `$/cancelRequest`, its deadline is counted from when the request arrived (so waiting in the
/// queue counts). A fired one cuts what is not walked yet, and the tree says so with `incomplete`.
pub fn call_tree(
    sessions: &mut Sessions,
    params: CallTreeParams,
    cancel: &Cancel,
) -> Result<CallTreeResult, Failure> {
    let no_def = || {
        invalid(format!(
            "no def {} in {}",
            params.entry.qualname, params.file
        ))
    };
    // the protocol counts from 1: a 0 is a malformed request, not a def that is not there
    if params.entry.occurrence == 0 {
        return Err(invalid(format!(
            "occurrence of {} starts at 1, not 0",
            params.entry.qualname
        )));
    }
    let entry = Entry::Def {
        qualname: &params.entry.qualname,
        occurrence: analyzer_occurrence(params.entry.occurrence)
            .and_then(|n| u16::try_from(n).ok())
            .ok_or_else(no_def)?,
    };
    let budget = Budget::new(params.depth, None);

    sessions.ensure(&params.root, &params.file)?;
    sessions.counts.solves += 1;
    let session = &sessions.by_entry[&(params.root.clone(), params.file.clone())];
    let graph = session
        .graph(&params.file, &entry, &budget, cancel)
        .ok_or_else(no_def)?;
    let book = session.book().ok_or_else(|| internal("the book is gone"))?;
    let wire = wire(
        &graph,
        graph.entries.first().ok_or_else(no_def)?,
        book,
        &WireOptions::new(WireStyle::default(), false, RenderOptions::default()),
    );

    let callers = Callers::read(Path::new(&params.root), &wire)?;
    let convert = Convert {
        lines: callers.lines(),
    };
    Ok(CallTreeResult {
        root: convert.node(&wire),
    })
}

/// The text of every file a call is made from: a node's `call_site` is read in `call_site_file`, which is not always its parent's file
/// (a call made in an inherited constructor is in the base class's file), and the node says so on the wire.
struct Callers {
    texts: Vec<(String, String)>,
}

impl Callers {
    fn read(root: &Path, wire: &Frame) -> Result<Callers, Failure> {
        let mut files = BTreeSet::new();
        fn collect(node: &Frame, files: &mut BTreeSet<String>) {
            if let (Some(file), Some(_)) = (&node.call_site_file, &node.call_site) {
                // a file outside `root` is not read: `Convert::node` drops its sites
                if !Path::new(file).is_absolute() {
                    files.insert(file.clone());
                }
            }
            node.children.iter().for_each(|c| collect(c, files));
        }
        collect(wire, &mut files);
        let texts = files
            .into_iter()
            .map(|file| {
                source::read_text(&root.join(&file))
                    .map(|text| (file.clone(), text))
                    .map_err(|e| internal(format!("cannot read {file}: {e}")))
            })
            .collect::<Result<_, _>>()?;
        Ok(Callers { texts })
    }

    fn lines(&self) -> HashMap<&str, Lines<'_>> {
        self.texts
            .iter()
            .map(|(file, text)| (file.as_str(), Lines::new(text)))
            .collect()
    }
}

/// The protocol's word for one of the analyzer's marks. A mark the protocol has no word for is left out (`external` and `unresolved` are the core's
/// business: such a def is not in the project and the core drops it).
fn mark(word: &str) -> Option<CallMark> {
    match word {
        "recursive" => Some(CallMark::Recursive),
        "cut" => Some(CallMark::Cut),
        "incomplete" => Some(CallMark::Incomplete),
        "likely" => Some(CallMark::Likely),
        _ => None,
    }
}

struct Convert<'a> {
    lines: HashMap<&'a str, Lines<'a>>,
}

impl Convert<'_> {
    fn node(&self, frame: &Frame) -> CallNode {
        // the position and the file come from one place, so a site never has a position from one file and the name of another
        let site = frame
            .call_site
            .as_ref()
            .zip(frame.call_site_file.as_deref())
            .and_then(|(span, file)| {
                // a site in a file outside `root` is no site: "a site with no file" then means only a driver from before 7.7 (docs/14 §10.3)
                if Path::new(file).is_absolute() {
                    return None;
                }
                let lines = self.lines.get(file)?;
                Some((
                    vnoc_driver_protocol::Range {
                        start: lines.position_1based(span.start),
                        end: lines.position_1based(span.end),
                    },
                    file.to_string(),
                ))
            });
        let (call_site, call_site_file) = site.unzip();
        let marks = frame.marks.iter().filter_map(|m| mark(m)).collect();
        CallNode {
            target: self.target(frame),
            call_site,
            call_site_file,
            marks,
            children: frame.children.iter().map(|c| self.node(c)).collect(),
        }
    }

    fn target(&self, frame: &Frame) -> CallTarget {
        CallTarget {
            // relative to the root already; a def outside it (library code) keeps its absolute path, and the protocol says `null`
            file: frame
                .target_file
                .clone()
                .filter(|f| !Path::new(f).is_absolute()),
            qualname: frame.target_qualname.clone(),
            occurrence: u32::from(frame.target_occurrence) + 1,
            kind: frame.target_kind.as_ref().map_or(DefKind::Function, kind),
            id: (!frame.target_id.is_empty()).then(|| frame.target_id.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marks_of_the_wire_have_a_word_each_and_the_likely_one_is_kept() {
        assert_eq!(mark("recursive"), Some(CallMark::Recursive));
        assert_eq!(mark("cut"), Some(CallMark::Cut));
        assert_eq!(mark("incomplete"), Some(CallMark::Incomplete));
        assert_eq!(
            mark("likely"),
            Some(CallMark::Likely),
            "it used to be dropped, so a guess looked like a certainty"
        );
        assert_eq!(mark("external"), None);
        assert_eq!(mark("something new"), None);
    }

    #[test]
    fn the_wire_format_is_one_this_driver_was_checked_against() {
        let reported = analyzer_core::wire::WIRE_FORMAT_VERSION;
        assert!(
            WIRE_FORMATS_READ.contains(&reported),
            "the analyzer's wire format is {reported}: read what changed, then add it to WIRE_FORMATS_READ"
        );
    }
}
