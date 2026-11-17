//! A node's name, qualified name and path against the words of a query (02).
//!
//! A word is given the best of six tiers it reaches: the name starts with it (0), the name holds it (1), its letters start the name's words in
//! order (2), the qualified name holds it (3), the path starts with it (4), the path holds it (5). A node with several words is as good as its
//! worst word, and every word must match something.
//!
//! Case is ignored letter by letter: each char is lower-cased on its own and the name is never lower-cased as a whole, because a few letters
//! change length when lower-cased and the ranges must point into the name as it was written.

use std::cmp::Ordering;

use crate::NodeEntry;
use crate::words::name_words;

/// The chars of `text`, each lower-cased on its own.
pub(crate) fn fold(text: &str) -> Vec<char> {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// A node with its text folded and its name split into words, so no query has to do either again.
pub(crate) struct Prepared {
    name: Vec<char>,
    /// For each folded char of the name, the byte range of the char of the name it came from.
    spans: Vec<(usize, usize)>,
    /// The words of the name, as ranges of folded chars.
    words: Vec<(usize, usize)>,
    qualname: Option<Vec<char>>,
    path: Vec<char>,
    /// The name's length in chars, for the order.
    pub(crate) name_len: usize,
}

impl Prepared {
    pub(crate) fn new(entry: &NodeEntry) -> Prepared {
        let mut name = Vec::new();
        let mut spans = Vec::new();
        for (at, c) in entry.name.char_indices() {
            for lower in c.to_lowercase() {
                name.push(lower);
                spans.push((at, at + c.len_utf8()));
            }
        }
        let words = name_words(&entry.name)
            .into_iter()
            .map(|(start, end)| {
                (
                    spans.partition_point(|s| s.0 < start),
                    spans.partition_point(|s| s.0 < end),
                )
            })
            .collect();
        Prepared {
            name,
            spans,
            words,
            qualname: entry.qualname.as_deref().map(fold),
            path: fold(&entry.path),
            name_len: entry.name.chars().count(),
        }
    }

    /// The byte range of the name that folded chars `from..to` came from.
    fn bytes(&self, from: usize, to: usize) -> (usize, usize) {
        (self.spans[from].0, self.spans[to - 1].1)
    }
}

/// How one word matched a node.
pub(crate) struct WordMatch {
    pub tier: u8,
    /// Name words passed over before the last one used; 0 unless the tier is 2.
    pub skipped: u32,
    /// Byte ranges of the name; none for tiers 3 to 5.
    pub ranges: Vec<(usize, usize)>,
}

/// How a node matched all the words.
pub(crate) struct NodeMatch {
    /// The worst tier of the words.
    pub tier: u8,
    /// The sum over the words at tier 2; 0 when the node's tier is not 2, as then it plays no part in the order.
    pub skipped: u32,
    /// Where the words are in the name: sorted, merged where they touch, in bytes.
    pub ranges: Vec<(usize, usize)>,
}

/// The first place `needle` is in `hay`.
fn find(hay: &[char], needle: &[char]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The best tier `word` (folded, not empty) reaches in `node`, or none.
pub(crate) fn match_word(node: &Prepared, word: &[char]) -> Option<WordMatch> {
    if let Some(at) = find(&node.name, word) {
        let tier = if at == 0 { 0 } else { 1 };
        return Some(WordMatch {
            tier,
            skipped: 0,
            ranges: vec![node.bytes(at, at + word.len())],
        });
    }
    if let Some((skipped, ranges)) = fuzzy(node, word) {
        return Some(WordMatch {
            tier: 2,
            skipped,
            ranges,
        });
    }
    let tier = if node
        .qualname
        .as_deref()
        .is_some_and(|q| find(q, word).is_some())
    {
        3
    } else if node.path.starts_with(word) {
        4
    } else if find(&node.path, word).is_some() {
        5
    } else {
        return None;
    };
    Some(WordMatch {
        tier,
        skipped: 0,
        ranges: Vec::new(),
    })
}

/// Tier 2 (02 §3): the word cut, in order, into prefixes of the name's words, each name word giving one prefix or none. Of the cuts that work,
/// the one that passes over the fewest words before its last, then the one with the longest first prefix. Gives the words passed over and the
/// byte ranges of the prefixes.
///
/// The first cut that works is not always the right one (`abcd` in `ab_abc_d` takes `ab` first and is stuck), so every cut is tried, with the
/// result for (letters used, word reached) kept: the work is at most letters x words x the longest word.
fn fuzzy(node: &Prepared, word: &[char]) -> Option<(u32, Vec<(usize, usize)>)> {
    let m = word.len();
    let k = node.words.len();
    if node
        .words
        .iter()
        .all(|&(start, _)| node.name[start] != word[0])
    {
        return None;
    }
    let mut search = Fuzzy {
        node,
        word,
        cost: vec![UNKNOWN; (m + 1) * (k + 1)],
        choice: vec![(0, 0); (m + 1) * (k + 1)],
    };
    let skipped = search.best(0, 0)?;
    let mut ranges = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < m {
        let (w, len) = search.choice[i * (k + 1) + j];
        let start = node.words[w].0;
        ranges.push(node.bytes(start, start + len));
        (i, j) = (i + len, w + 1);
    }
    Some((skipped, ranges))
}

const UNKNOWN: u32 = u32::MAX;
const FAILS: u32 = u32::MAX - 1;

struct Fuzzy<'a> {
    node: &'a Prepared,
    word: &'a [char],
    /// By (letters used, word reached): the fewest words passed over from there on, `FAILS`, or `UNKNOWN`.
    cost: Vec<u32>,
    /// By the same: the word and the length of the prefix taken from it.
    choice: Vec<(usize, usize)>,
}

impl Fuzzy<'_> {
    /// The fewest words passed over to use `word[i..]` on the name's words from `j` on.
    fn best(&mut self, i: usize, j: usize) -> Option<u32> {
        let k = self.node.words.len();
        let slot = i * (k + 1) + j;
        match self.cost[slot] {
            FAILS => return None,
            UNKNOWN => {}
            done => return Some(done),
        }
        if i == self.word.len() {
            self.cost[slot] = 0;
            return Some(0);
        }
        let mut best: Option<(u32, usize, usize)> = None;
        for w in j..k {
            let (start, end) = self.node.words[w];
            let shared = self.word[i..]
                .iter()
                .zip(&self.node.name[start..end])
                .take_while(|(a, b)| a == b)
                .count();
            for len in (1..=shared).rev() {
                let Some(rest) = self.best(i + len, w + 1) else {
                    continue;
                };
                let cost = (w - j) as u32 + rest;
                if best.is_none_or(|(c, _, l)| cost < c || (cost == c && len > l)) {
                    best = Some((cost, w, len));
                }
            }
        }
        match best {
            Some((cost, w, len)) => {
                self.cost[slot] = cost;
                self.choice[slot] = (w, len);
                Some(cost)
            }
            None => {
                self.cost[slot] = FAILS;
                None
            }
        }
    }
}

/// The node against every word (folded, none empty): a hit only if each word matches something.
pub(crate) fn match_node(node: &Prepared, words: &[Vec<char>]) -> Option<NodeMatch> {
    let mut tier = 0;
    let mut skipped = 0;
    let mut ranges = Vec::new();
    for word in words {
        let found = match_word(node, word)?;
        tier = tier.max(found.tier);
        skipped += found.skipped;
        ranges.extend(found.ranges);
    }
    if tier != 2 {
        skipped = 0;
    }
    Some(NodeMatch {
        tier,
        skipped,
        ranges: merge(ranges),
    })
}

/// The ranges sorted, with those that touch or overlap joined.
fn merge(mut ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    ranges.sort_unstable();
    let mut out: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        match out.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => out.push((start, end)),
        }
    }
    out
}

/// A node that matched, with what the order needs of it.
pub(crate) struct Found {
    pub index: usize,
    pub matched: NodeMatch,
    pub name_len: usize,
}

/// The order of 02 §5, after the exact reference: the tier, then fewer words skipped (inside tier 2), then the shorter name, then reading order.
pub(crate) fn order(a: &Found, b: &Found) -> Ordering {
    (a.matched.tier, a.matched.skipped, a.name_len, a.index).cmp(&(
        b.matched.tier,
        b.matched.skipped,
        b.name_len,
        b.index,
    ))
}
