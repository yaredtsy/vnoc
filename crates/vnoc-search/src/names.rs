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
use crate::words::words_into;

/// The chars of `text`, each lower-cased on its own.
pub(crate) fn fold(text: &str) -> Vec<char> {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// The folded form of a name that is not ASCII: its chars, and for each the byte range of the char of the name it came from.
struct Spanned {
    chars: Vec<char>,
    spans: Vec<(usize, usize)>,
}

/// What matching keeps of a node, so no query has to split or fold again.
///
/// A name, qualified name or path that is ASCII keeps nothing: it is compared in place, in the [`NodeEntry`] the corpus holds beside it, with
/// ASCII case folding (lower-casing ASCII never changes a length, so a byte offset is a char offset and a range is the byte range itself). One
/// that is not ASCII keeps its folded chars, and the name its spans too.
pub(crate) struct Prepared {
    /// The folded name; none when it is ASCII.
    name: Option<Spanned>,
    /// The words of the name: byte ranges when the name is ASCII, ranges of folded chars when it is not.
    words: Vec<(u32, u32)>,
    /// The folded qualified name; none when it is ASCII or when the node has none.
    qualname: Option<Vec<char>>,
    /// The folded path; none when it is ASCII.
    path: Option<Vec<char>>,
    /// The name's length in chars, for the order.
    pub(crate) name_len: usize,
}

impl Prepared {
    pub(crate) fn new(entry: &NodeEntry) -> Prepared {
        Prepared::build(entry, false)
    }

    /// The same, with every text folded even when it is ASCII: the tests compare the two paths.
    #[cfg(test)]
    pub(crate) fn folded(entry: &NodeEntry) -> Prepared {
        Prepared::build(entry, true)
    }

    fn build(entry: &NodeEntry, fold_all: bool) -> Prepared {
        let mut words = Vec::new();
        words_into(&entry.name, &mut words);
        let name = if entry.name.is_ascii() && !fold_all {
            None
        } else {
            let mut chars = Vec::new();
            let mut spans = Vec::new();
            for (at, c) in entry.name.char_indices() {
                for lower in c.to_lowercase() {
                    chars.push(lower);
                    spans.push((at, at + c.len_utf8()));
                }
            }
            // the words are byte ranges so far; they become ranges of folded chars
            for word in &mut words {
                *word = (
                    spans.partition_point(|s| s.0 < word.0 as usize) as u32,
                    spans.partition_point(|s| s.0 < word.1 as usize) as u32,
                );
            }
            Some(Spanned { chars, spans })
        };
        let fold_if_needed = |text: &str| (fold_all || !text.is_ascii()).then(|| fold(text));
        Prepared {
            name,
            words,
            qualname: entry.qualname.as_deref().and_then(fold_if_needed),
            path: fold_if_needed(&entry.path),
            name_len: entry.name.chars().count(),
        }
    }

    /// The node's texts as matching reads them; `entry` is the one this was prepared from.
    pub(crate) fn view<'a>(&'a self, entry: &'a NodeEntry) -> View<'a> {
        let text = |text: &'a str, folded: &'a Option<Vec<char>>| match folded {
            Some(chars) => Text::Chars(chars),
            None => Text::Ascii(text.as_bytes()),
        };
        let (name, spans) = match &self.name {
            Some(n) => (Text::Chars(&n.chars), Some(&n.spans[..])),
            None => (Text::Ascii(entry.name.as_bytes()), None),
        };
        View {
            name,
            spans,
            words: &self.words,
            // a node with no qualified name has no folded one either: `Some` only when there is a text to read
            qualname: entry.qualname.as_deref().map(|q| text(q, &self.qualname)),
            path: text(&entry.path, &self.path),
        }
    }
}

/// A text as it is compared: ASCII as written (each byte lower-cased as it is read), or already folded.
#[derive(Clone, Copy)]
enum Text<'a> {
    Ascii(&'a [u8]),
    Chars(&'a [char]),
}

impl Text<'_> {
    fn len(self) -> usize {
        match self {
            Text::Ascii(bytes) => bytes.len(),
            Text::Chars(chars) => chars.len(),
        }
    }

    /// The folded char at `i`. An ASCII text gives ASCII chars only, so a word that has any other never equals it.
    fn at(self, i: usize) -> char {
        match self {
            Text::Ascii(bytes) => char::from(bytes[i].to_ascii_lowercase()),
            Text::Chars(chars) => chars[i],
        }
    }

    fn starts_with(self, needle: &[char]) -> bool {
        needle.len() <= self.len() && needle.iter().enumerate().all(|(j, &c)| self.at(j) == c)
    }

    /// The first place `needle` is in the text.
    fn find(self, needle: &[char]) -> Option<usize> {
        let last = self.len().checked_sub(needle.len())?;
        (0..=last).find(|&at| {
            needle
                .iter()
                .enumerate()
                .all(|(j, &c)| self.at(at + j) == c)
        })
    }
}

/// One node's texts, ready to be compared with a word.
pub(crate) struct View<'a> {
    name: Text<'a>,
    /// For a name that is not ASCII, the byte range each folded char came from.
    spans: Option<&'a [(usize, usize)]>,
    words: &'a [(u32, u32)],
    qualname: Option<Text<'a>>,
    path: Text<'a>,
}

impl View<'_> {
    /// The byte range of the name that folded chars `from..to` came from.
    fn bytes(&self, from: usize, to: usize) -> (usize, usize) {
        match self.spans {
            Some(spans) => (spans[from].0, spans[to - 1].1),
            None => (from, to),
        }
    }

    /// The `w`th word of the name, as a range of its folded chars.
    fn word(&self, w: usize) -> (usize, usize) {
        let (start, end) = self.words[w];
        (start as usize, end as usize)
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

/// The best tier `word` (folded, not empty) reaches in `node`, or none.
pub(crate) fn match_word(node: &View, word: &[char]) -> Option<WordMatch> {
    if let Some(at) = node.name.find(word) {
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
    let tier = if node.qualname.is_some_and(|q| q.find(word).is_some()) {
        3
    } else if node.path.starts_with(word) {
        4
    } else if node.path.find(word).is_some() {
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
fn fuzzy(node: &View, word: &[char]) -> Option<(u32, Vec<(usize, usize)>)> {
    let m = word.len();
    let k = node.words.len();
    // the prefixes are of the name's own letters, so a longer word is no cut, and no table is built for it
    if m > node.name.len() || (0..k).all(|w| node.name.at(node.word(w).0) != word[0]) {
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
        let start = node.word(w).0;
        ranges.push(node.bytes(start, start + len));
        (i, j) = (i + len, w + 1);
    }
    Some((skipped, ranges))
}

const UNKNOWN: u32 = u32::MAX;
const FAILS: u32 = u32::MAX - 1;

struct Fuzzy<'a> {
    node: &'a View<'a>,
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
            let (start, end) = self.node.word(w);
            let shared = self.word[i..]
                .iter()
                .zip(start..end)
                .take_while(|&(&a, at)| a == self.node.name.at(at))
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
pub(crate) fn match_node(node: &View, words: &[Vec<char>]) -> Option<NodeMatch> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Kind;

    /// A small deterministic generator (xorshift): the same names on every run.
    struct Rng(u64);

    impl Rng {
        fn below(&mut self, n: usize) -> usize {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % n as u64) as usize
        }

        fn text(&mut self, alphabet: &[char], min: usize, max: usize) -> String {
            let len = min + self.below(max - min + 1);
            (0..len)
                .map(|_| alphabet[self.below(alphabet.len())])
                .collect()
        }
    }

    const ALPHABET: &[char] = &[
        'a', 'b', 'c', 'd', 'A', 'B', 'C', 'D', 'H', 'T', 'P', '_', '_', '.', '-', '0', '1', '9',
        'x', 'X',
    ];

    /// A part of `text` or its word starts, so the words match often enough to reach every tier.
    fn word_from(rng: &mut Rng, text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            return rng.text(ALPHABET, 1, 3);
        }
        match rng.below(3) {
            0 => {
                let from = rng.below(chars.len());
                let to = from + 1 + rng.below(chars.len() - from);
                chars[from..to].iter().collect()
            }
            1 => {
                // the first letters of some words: what tier 2 is for
                let mut word = String::new();
                for (start, _) in crate::words::name_words(text) {
                    if rng.below(3) > 0 {
                        word.push(text[start..].chars().next().unwrap());
                        if rng.below(2) == 0 {
                            word.extend(text[start..].chars().skip(1).take(1));
                        }
                    }
                }
                word
            }
            _ => rng.text(ALPHABET, 1, 4),
        }
    }

    type Outcome = Option<(u8, u32, Vec<(usize, usize)>)>;

    fn outcome(node: &Prepared, entry: &NodeEntry, words: &[Vec<char>]) -> Outcome {
        match_node(&node.view(entry), words).map(|m| (m.tier, m.skipped, m.ranges))
    }

    #[test]
    fn the_in_place_path_and_the_folded_path_agree_on_ascii() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut tiers = [0usize; 6];
        let mut tier_2_with_skips = 0;
        for _ in 0..4000 {
            let name = rng.text(ALPHABET, 0, 14);
            let entry = NodeEntry {
                id: String::new(),
                kind: Kind::Function,
                qualname: (rng.below(4) > 0)
                    .then(|| format!("{}.{name}", rng.text(ALPHABET, 1, 6))),
                path: format!(
                    "{}/{}.py",
                    rng.text(ALPHABET, 1, 6),
                    rng.text(ALPHABET, 1, 6)
                ),
                name,
                line: None,
                in_class: false,
                docs: 0,
            };
            assert!(entry.name.is_ascii());
            let in_place = Prepared::new(&entry);
            let folded = Prepared::folded(&entry);
            assert!(
                in_place.name.is_none() && in_place.path.is_none(),
                "ASCII is kept in place"
            );
            assert!(
                folded.name.is_some() && folded.path.is_some(),
                "the other path folds it"
            );

            for _ in 0..3 {
                let from = [
                    &entry.name,
                    entry.qualname.as_ref().unwrap_or(&entry.path),
                    &entry.path,
                ][rng.below(3)];
                let mut query = vec![fold(&word_from(&mut rng, from))];
                if rng.below(4) == 0 {
                    query.push(fold(&rng.text(ALPHABET, 1, 3)));
                }
                query.retain(|w| !w.is_empty());
                if query.is_empty() {
                    continue;
                }
                let (a, b) = (
                    outcome(&in_place, &entry, &query),
                    outcome(&folded, &entry, &query),
                );
                assert_eq!(a, b, "{entry:?} against {query:?}");
                if let Some((tier, skipped, _)) = a {
                    tiers[tier as usize] += 1;
                    tier_2_with_skips += usize::from(tier == 2 && skipped > 0);
                }
            }
        }
        // the check is only worth something if every tier was reached
        assert!(tiers.iter().all(|&n| n >= 50), "tiers reached: {tiers:?}");
        assert!(
            tier_2_with_skips >= 20,
            "{tier_2_with_skips} fuzzy matches that skip words"
        );
    }

    #[test]
    fn an_ascii_text_never_matches_a_word_that_is_not_ascii() {
        let entry = NodeEntry {
            id: String::new(),
            kind: Kind::Function,
            name: "cafe_order".into(),
            qualname: Some("Shop.cafe_order".into()),
            path: "shop/cafe.py".into(),
            line: None,
            in_class: false,
            docs: 0,
        };
        let node = Prepared::new(&entry);
        for word in ["é", "café", "éa", "ordér"] {
            assert!(outcome(&node, &entry, &[fold(word)]).is_none(), "{word}");
        }
    }

    #[test]
    fn a_word_longer_than_the_name_is_no_tier_2_match() {
        for name in ["co_op", "É_a"] {
            let entry = NodeEntry {
                id: String::new(),
                kind: Kind::Function,
                name: name.into(),
                qualname: None,
                path: "p.py".into(),
                line: None,
                in_class: false,
                docs: 0,
            };
            let node = Prepared::new(&entry);
            let view = node.view(&entry);
            let first = fold(name).remove(0);
            // as long as the name's letters it can be cut; one letter more it cannot
            let long: Vec<char> = std::iter::once(first)
                .chain(std::iter::repeat_n('o', fold(name).len()))
                .collect();
            assert!(fuzzy(&view, &long).is_none(), "{name}");
            let huge: Vec<char> = std::iter::once(first)
                .chain(std::iter::repeat_n('o', 20_000))
                .collect();
            assert!(match_word(&view, &huge).is_none(), "{name}");
        }
        let entry = NodeEntry {
            id: String::new(),
            kind: Kind::Function,
            name: "co_op".into(),
            qualname: None,
            path: "p.py".into(),
            line: None,
            in_class: false,
            docs: 0,
        };
        let node = Prepared::new(&entry);
        assert!(fuzzy(&node.view(&entry), &fold("coop")).is_some());
    }
}
