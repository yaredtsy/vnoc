//! The words of every doc, ranked (`docs/20-search/03-doc-text.md`).
//!
//! A text is cut into words at anything that is not a letter or a digit; a `camelCase` token is a word whole **and** in its parts, cut as a name is
//! (`words.rs`), each at its own byte range of the text. The index maps a word to the docs that hold it, with a count each, the title's counted three
//! times; the words are kept in sorted order, so the last word of a query can open into the words that start with it. A doc is scored with BM25, every
//! word of the query must be in it, and a phrase must be held as written.
//!
//! The index keeps no positions. What the snippet and the bold ranges need is found again by cutting the one doc's text a second time, and only the
//! docs that are shown are cut: at most `limits.docs` of them.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

use crate::utf16;
use crate::words::words_into;
use crate::{DocEntry, Query};

/// A word of the title counts this many times, in its count and in the doc's length.
const TITLE_WEIGHT: u32 = 3;
/// A prefix that opens more words than this uses the ones that are in the most docs.
pub(crate) const PREFIX_CAP: usize = 200;
const K1: f64 = 1.2;
const B: f64 = 0.75;
/// A snippet starts at most this many chars before the first hit, and is at most [`SNIPPET_LEN`] chars long.
const SNIPPET_BEFORE: usize = 40;
const SNIPPET_LEN: usize = 120;

/// `text` lower-cased, each char on its own as the names are (a few letters change length, and a whole-string lower-casing reads some in context).
fn lower_into(text: &str, out: &mut String) {
    out.clear();
    out.extend(text.chars().flat_map(char::to_lowercase));
}

/// Every word of `text` with its byte range: each run of letters and digits, lower-cased, and where a run is a `camelCase` token or has a digit next
/// to a letter, its parts as well, each at its own range. `word` is only valid for the call.
///
/// An ASCII token is lower-cased in place in one buffer that is reused, and its parts are slices of that (lower-casing ASCII never changes a length).
/// A token with any other char is lower-cased char by char, the whole and each part on its own.
pub(crate) fn postings(text: &str, mut word: impl FnMut(&str, usize, usize)) {
    let mut buffer = String::new();
    let mut parts: Vec<(u32, u32)> = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((from, c)) = chars.next() {
        if !c.is_alphanumeric() {
            continue;
        }
        let mut to = from + c.len_utf8();
        // a token that has no capital and no digit has no parts to cut
        let cuttable = |c: char| c.is_uppercase() || c.is_numeric();
        let (mut ascii, mut cut) = (c.is_ascii(), cuttable(c));
        while let Some(&(i, c)) = chars.peek() {
            if !c.is_alphanumeric() {
                break;
            }
            to = i + c.len_utf8();
            ascii &= c.is_ascii();
            cut |= cuttable(c);
            chars.next();
        }
        let token = &text[from..to];
        if ascii {
            buffer.clear();
            buffer.push_str(token);
            buffer.make_ascii_lowercase();
            word(&buffer, from, to);
            if cut {
                parts.clear();
                words_into(token, &mut parts);
                if parts.len() > 1 {
                    for &(s, e) in &parts {
                        word(
                            &buffer[s as usize..e as usize],
                            from + s as usize,
                            from + e as usize,
                        );
                    }
                }
            }
        } else {
            lower_into(token, &mut buffer);
            word(&buffer, from, to);
            if cut {
                parts.clear();
                words_into(token, &mut parts);
                if parts.len() > 1 {
                    for &(s, e) in &parts {
                        lower_into(&token[s as usize..e as usize], &mut buffer);
                        word(&buffer, from + s as usize, from + e as usize);
                    }
                }
            }
        }
    }
}

/// The pieces of a query word: split at anything that is not a letter or a digit, lower-cased. `billing/` is `billing`; `orderservice.par` is two.
fn pieces(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut out = String::new();
            lower_into(p, &mut out);
            out
        })
        .collect()
}

/// What a query asks of the docs.
pub(crate) struct Terms {
    /// Whole words the doc must hold: every piece of every word and phrase but the prefix.
    pub exact: Vec<String>,
    /// The last piece of the last word: a doc holds it as a word that starts with it.
    pub prefix: Option<String>,
    /// The phrases, as chars: lower case, one space for any run of white space.
    pub phrases: Vec<Vec<char>>,
}

impl Terms {
    pub(crate) fn of(query: &Query) -> Terms {
        let mut all: Vec<String> = query.words.iter().flat_map(|w| pieces(w)).collect();
        let prefix = all.pop();
        let mut exact: Vec<String> = Vec::new();
        let phrase_pieces = query.phrases.iter().flat_map(|p| pieces(p));
        for piece in all.into_iter().chain(phrase_pieces) {
            if !exact.contains(&piece) {
                exact.push(piece);
            }
        }
        let phrases = query
            .phrases
            .iter()
            .map(|p| squeezed(p).into_iter().map(|c| c.0).collect::<Vec<_>>())
            .filter(|p| !p.is_empty())
            .collect();
        Terms {
            exact,
            prefix,
            phrases,
        }
    }

    /// No word to look for: no doc is found.
    pub(crate) fn is_empty(&self) -> bool {
        self.exact.is_empty() && self.prefix.is_none()
    }
}

/// A small hasher for the words: they are short, and SipHash costs more than the rest of the lookup.
#[derive(Default)]
struct WordHasher(u64);

impl Hasher for WordHasher {
    fn write(&mut self, bytes: &[u8]) {
        // FNV-1a
        let mut h = if self.0 == 0 {
            0xcbf2_9ce4_8422_2325
        } else {
            self.0
        };
        for &b in bytes {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        self.0 = h;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type WordIds = HashMap<Box<str>, u32, BuildHasherDefault<WordHasher>>;

/// The words of every doc: which docs hold each, how many times, and how long each doc is. It reads only the docs' titles and bodies, so it depends
/// on their text alone: a caller that keeps it can build a new [`Corpus`](crate::Corpus) around it when the nodes change.
///
/// A word has a number, given once in the order the words are met; the postings are by that number, and `sorted` lists the numbers in the order of
/// the words' text, for the prefix.
#[derive(Default)]
pub struct DocIndex {
    /// The text of each word, by number.
    words: Vec<Box<str>>,
    /// The docs that hold each word, by number, as `(doc, count)` in doc order.
    postings: Vec<Vec<(u32, u32)>>,
    /// The word numbers, ordered by the words' text.
    sorted: Vec<u32>,
    /// Each doc's length: its words, the title's counted three times.
    lengths: Vec<u32>,
    average: f64,
}

impl DocIndex {
    /// The index of `docs`, in their order. Only each doc's `title` and `body` are read.
    pub fn new(docs: &[DocEntry]) -> DocIndex {
        let mut ids = WordIds::default();
        let mut lists: Vec<Vec<(u32, u32)>> = Vec::new();
        // one doc's counts by word number, and the numbers that have one: reused across docs
        let mut counts: Vec<u32> = Vec::new();
        let mut touched: Vec<u32> = Vec::new();
        let mut lengths = Vec::with_capacity(docs.len());
        for (doc, entry) in docs.iter().enumerate() {
            let mut length = 0;
            for (text, weight) in [(&entry.title, TITLE_WEIGHT), (&entry.body, 1)] {
                postings(text, |word, _, _| {
                    length += weight;
                    let id = match ids.get(word) {
                        Some(&id) => id,
                        None => {
                            let id = ids.len() as u32;
                            ids.insert(word.into(), id);
                            counts.push(0);
                            lists.push(Vec::new());
                            id
                        }
                    };
                    if counts[id as usize] == 0 {
                        touched.push(id);
                    }
                    counts[id as usize] += weight;
                });
            }
            lengths.push(length);
            for id in touched.drain(..) {
                lists[id as usize].push((doc as u32, std::mem::take(&mut counts[id as usize])));
            }
        }
        let mut words: Vec<Box<str>> = vec![Box::from(""); ids.len()];
        for (word, id) in ids {
            words[id as usize] = word;
        }
        let mut sorted: Vec<u32> = (0..words.len() as u32).collect();
        sorted.sort_unstable_by(|&a, &b| words[a as usize].cmp(&words[b as usize]));
        let total: u64 = lengths.iter().map(|&l| u64::from(l)).sum();
        DocIndex {
            words,
            postings: lists,
            sorted,
            average: total as f64 / docs.len().max(1) as f64,
            lengths,
        }
    }

    /// How many docs it was built from.
    pub fn len(&self) -> usize {
        self.lengths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lengths.is_empty()
    }

    /// The number of the word `word`, by a binary search over the sorted words.
    fn id_of(&self, word: &str) -> Option<u32> {
        let at = self
            .sorted
            .binary_search_by(|&id| (*self.words[id as usize]).cmp(word))
            .ok()?;
        Some(self.sorted[at])
    }

    /// The numbers of the words that start with `prefix` (itself too): all of them, or when more than [`PREFIX_CAP`] the ones in the most docs, ties
    /// by the word.
    fn expand_ids(&self, prefix: &str) -> Vec<u32> {
        let from = self
            .sorted
            .partition_point(|&id| *self.words[id as usize] < *prefix);
        let mut ids: Vec<u32> = self.sorted[from..]
            .iter()
            .copied()
            .take_while(|&id| self.words[id as usize].starts_with(prefix))
            .collect();
        if ids.len() > PREFIX_CAP {
            // `ids` is in the words' order, so a stable sort by df leaves ties by the word
            ids.sort_by_key(|&id| std::cmp::Reverse(self.postings[id as usize].len()));
            ids.truncate(PREFIX_CAP);
        }
        ids
    }

    /// [`DocIndex::expand_ids`] as the words.
    #[cfg(test)]
    pub(crate) fn expand(&self, prefix: &str) -> Vec<&str> {
        self.expand_ids(prefix)
            .into_iter()
            .map(|id| &*self.words[id as usize])
            .collect()
    }

    /// The docs that hold every term, each with its BM25 score, in doc order; and the words the prefix opened into.
    pub(crate) fn find(&self, terms: &Terms) -> (Vec<(u32, f64)>, HashSet<String>) {
        let mut hit_words: HashSet<String> = terms.exact.iter().cloned().collect();
        let mut lists: Vec<Cow<[(u32, u32)]>> = Vec::new();
        for term in &terms.exact {
            match self.id_of(term) {
                Some(id) => lists.push(Cow::Borrowed(&self.postings[id as usize])),
                None => return (Vec::new(), hit_words),
            }
        }
        if let Some(prefix) = &terms.prefix {
            let ids = self.expand_ids(prefix);
            let mut merged: HashMap<u32, u32> = HashMap::new();
            for &id in &ids {
                for &(doc, count) in &self.postings[id as usize] {
                    *merged.entry(doc).or_default() += count;
                }
                hit_words.insert(self.words[id as usize].to_string());
            }
            if merged.is_empty() {
                return (Vec::new(), hit_words);
            }
            let mut list: Vec<(u32, u32)> = merged.into_iter().collect();
            list.sort_unstable();
            lists.push(Cow::Owned(list));
        }
        if lists.is_empty() {
            return (Vec::new(), hit_words);
        }
        let n = self.lengths.len() as f64;
        let idf: Vec<f64> = lists
            .iter()
            .map(|l| {
                let df = l.len() as f64;
                (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
            })
            .collect();
        let driver = (0..lists.len()).min_by_key(|&i| lists[i].len()).unwrap();
        let mut found = Vec::new();
        'docs: for &(doc, _) in lists[driver].iter() {
            let length = f64::from(self.lengths[doc as usize]);
            let norm = K1 * (1.0 - B + B * length / self.average.max(f64::MIN_POSITIVE));
            let mut score = 0.0;
            // summed in the order of the terms, whichever list drives, so equal docs get equal scores
            for (list, idf) in lists.iter().zip(&idf) {
                let Ok(at) = list.binary_search_by_key(&doc, |p| p.0) else {
                    continue 'docs;
                };
                let tf = f64::from(list[at].1);
                score += idf * tf * (K1 + 1.0) / (tf + norm);
            }
            found.push((doc, score));
        }
        found.sort_unstable_by_key(|f| f.0);
        (found, hit_words)
    }
}

/// `text` as chars, lower-cased, each with the byte range of the char it came from, a run of white space as one space and none at either end.
fn squeezed(text: &str) -> Vec<(char, usize, usize)> {
    let mut out: Vec<(char, usize, usize)> = Vec::new();
    for (at, c) in text.char_indices() {
        let end = at + c.len_utf8();
        if c.is_whitespace() {
            match out.last_mut() {
                Some(last) if last.0 == ' ' => last.2 = end,
                Some(_) => out.push((' ', at, end)),
                None => {}
            }
        } else {
            out.extend(c.to_lowercase().map(|l| (l, at, end)));
        }
    }
    if out.last().is_some_and(|l| l.0 == ' ') {
        out.pop();
    }
    out
}

/// Where `phrase` (as [`Terms`] keeps it) is in `text`, as byte ranges: case ignored, a run of white space as one space.
fn phrase_spans(text: &str, phrase: &[char]) -> Vec<(usize, usize)> {
    let folded = squeezed(text);
    let mut spans = Vec::new();
    let mut at = 0;
    while at + phrase.len() <= folded.len() {
        if folded[at..at + phrase.len()]
            .iter()
            .zip(phrase)
            .all(|(f, p)| f.0 == *p)
        {
            spans.push((folded[at].1, folded[at + phrase.len() - 1].2));
            at += phrase.len();
        } else {
            at += 1;
        }
    }
    spans
}

/// Whether the title or the body of `doc` holds every phrase.
pub(crate) fn holds_phrases(doc: &DocEntry, phrases: &[Vec<char>]) -> bool {
    phrases
        .iter()
        .all(|p| !phrase_spans(&doc.title, p).is_empty() || !phrase_spans(&doc.body, p).is_empty())
}

/// The hits in `text`: the byte ranges of the words of `hit_words` and of the phrases, sorted and merged where they touch or overlap.
fn hits_in(text: &str, hit_words: &HashSet<String>, phrases: &[Vec<char>]) -> Vec<(usize, usize)> {
    let mut hits = Vec::new();
    postings(text, |word, start, end| {
        if hit_words.contains(word) {
            hits.push((start, end));
        }
    });
    for phrase in phrases {
        hits.extend(phrase_spans(text, phrase));
    }
    hits.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(hits.len());
    for hit in hits {
        match merged.last_mut() {
            Some(last) if hit.0 <= last.1 => last.1 = last.1.max(hit.1),
            _ => merged.push(hit),
        }
    }
    merged
}

/// What a shown doc draws: the hits in the title, and the snippet of the body with its hits, all in UTF-16 units.
pub(crate) struct Shown {
    pub title_ranges: Vec<(u32, u32)>,
    pub snippet: String,
    pub snippet_ranges: Vec<(u32, u32)>,
}

pub(crate) fn show(doc: &DocEntry, hit_words: &HashSet<String>, phrases: &[Vec<char>]) -> Shown {
    let title_ranges = hits_in(&doc.title, hit_words, phrases)
        .into_iter()
        .map(|(s, e)| utf16::range(&doc.title, s, e))
        .collect();
    let (snippet, snippet_ranges) = snippet(&doc.body, &hits_in(&doc.body, hit_words, phrases));
    Shown {
        title_ranges,
        snippet,
        snippet_ranges,
    }
}

/// About [`SNIPPET_LEN`] chars of `body` around the first of `hits` (byte ranges, sorted), or from the start when there is none: it begins at most
/// [`SNIPPET_BEFORE`] chars before the hit, moved back to the start of a word, and ends at most [`SNIPPET_LEN`] chars after its start, moved back
/// to the end of a word. White space runs are one space, a cut end has `…`. The ranges are the hits wholly inside, in UTF-16 units of the snippet
/// as returned.
fn snippet(body: &str, hits: &[(usize, usize)]) -> (String, Vec<(u32, u32)>) {
    let chars: Vec<char> = body.chars().collect();
    let mut offsets: Vec<usize> = body.char_indices().map(|(at, _)| at).collect();
    offsets.push(body.len());
    let n = chars.len();
    let space = |i: usize| chars[i].is_whitespace();
    let Some(lead) = (0..n).find(|&i| !space(i)) else {
        return (String::new(), Vec::new());
    };
    let char_of = |byte: usize| offsets.binary_search(&byte).unwrap_or_else(|at| at);

    let mut start = hits
        .first()
        .map_or(0, |h| char_of(h.0).saturating_sub(SNIPPET_BEFORE));
    while start > 0 && !space(start - 1) {
        start -= 1;
    }
    start = start.max(lead);
    while start < n && space(start) {
        start += 1;
    }
    let mut end = (start + SNIPPET_LEN).min(n);
    if end < n && !space(end) {
        let mut back = end;
        while back > start && !space(back - 1) {
            back -= 1;
        }
        // one word longer than the snippet: cut it where the snippet ends
        if back > start {
            end = back;
        }
    }
    while end > start && space(end - 1) {
        end -= 1;
    }

    let mut out = String::new();
    if start > lead {
        out.push('…');
    }
    // where each char of the window starts in `out`, and one more for its end
    let mut at = vec![0usize; end - start + 1];
    let mut i = start;
    while i < end {
        at[i - start] = out.len();
        if space(i) {
            out.push(' ');
            i += 1;
            while i < end && space(i) {
                at[i - start] = out.len();
                i += 1;
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    at[end - start] = out.len();
    if chars[end..].iter().any(|c| !c.is_whitespace()) {
        out.push('…');
    }

    let ranges = hits
        .iter()
        .map(|&(s, e)| (char_of(s), char_of(e)))
        .filter(|&(s, e)| s >= start && e <= end)
        .map(|(s, e)| utf16::range(&out, at[s - start], at[e - start]))
        .collect();
    (out, ranges)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        postings(text, |w, s, e| {
            out.push((w.to_string(), text[s..e].to_string()))
        });
        out
    }

    fn doc(title: &str, body: &str) -> DocEntry {
        DocEntry {
            node: None,
            node_id: String::new(),
            node_name: String::new(),
            node_kind: None,
            node_path: String::new(),
            missing: false,
            file: String::new(),
            doc_path: String::new(),
            title: title.to_string(),
            body: body.to_string(),
        }
    }

    #[test]
    fn a_camel_case_token_is_a_word_whole_and_in_parts_at_their_own_ranges() {
        let got: Vec<_> = words("a retryPolicy, x2y café")
            .into_iter()
            .map(|(w, t)| format!("{w}={t}"))
            .collect();
        assert_eq!(
            got,
            [
                "a=a",
                "retrypolicy=retryPolicy",
                "retry=retry",
                "policy=Policy",
                "x2y=x2y",
                "x=x",
                "2=2",
                "y=y",
                "café=café"
            ]
        );
    }

    /// The `i`th of a sorted run of letters-only words: `aaaa`, `aaab`, … (a digit would be a word of its own, as in names).
    fn letters(first: char, i: usize) -> String {
        let at = |n: usize| char::from(b'a' + (n % 26) as u8);
        format!("{first}{}{}{}", at(i / 676), at(i / 26), at(i))
    }

    #[test]
    fn the_prefix_uses_the_most_common_200_of_300_words() {
        // doc `d` holds the words `d` to 299, so word `i` is in the docs `0..=i`: its df is i + 1, and the most common are the highest
        let docs: Vec<DocEntry> = (0..300)
            .map(|d| {
                let body: Vec<String> = (d..300).map(|i| letters('a', i)).collect();
                doc("", &body.join(" "))
            })
            .collect();
        let index = DocIndex::new(&docs);
        let got = index.expand("a");
        let want: Vec<String> = (100..300).rev().map(|i| letters('a', i)).collect();
        assert_eq!(got.len(), PREFIX_CAP);
        assert_eq!(got, want.iter().map(String::as_str).collect::<Vec<_>>());
        // few enough words: all of them, in sorted order
        let few = index.expand(&letters('a', 290)[..3]);
        assert!(
            few.len() <= 26 && few.windows(2).all(|w| w[0] < w[1]),
            "{few:?}"
        );
        assert!(index.expand("b").is_empty());
    }

    #[test]
    fn equal_df_ties_go_by_the_word() {
        let docs: Vec<DocEntry> = (0..250).map(|d| doc("", &letters('w', d))).collect();
        let index = DocIndex::new(&docs);
        let got = index.expand("w");
        assert_eq!(got.len(), PREFIX_CAP);
        assert_eq!(got[0], letters('w', 0));
        assert_eq!(got[199], letters('w', 199));
    }

    #[test]
    fn a_phrase_ignores_case_and_reads_white_space_as_one_space() {
        let phrase: Vec<char> = "retry policy".chars().collect();
        assert_eq!(phrase_spans("Retry\n  \tPOLICY!", &phrase), [(0, 15)]);
        assert_eq!(phrase_spans("retry-policy", &phrase), []);
        assert_eq!(
            phrase_spans("x retry policy retry policy", &phrase).len(),
            2
        );
    }

    #[test]
    fn the_snippet_of_a_blank_body_is_empty() {
        assert_eq!(snippet("", &[]), (String::new(), vec![]));
        assert_eq!(snippet(" \n\t ", &[]), (String::new(), vec![]));
    }
}
