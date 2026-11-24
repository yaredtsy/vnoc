//! The lines of a tour's file: one serde type per record of `docs/26-tours/01-on-disk.md` §4, `parse_line` and `print_line`, and the three ids.
//!
//! A line is one JSON object with a `type`. [`parse_line`] reads one and never panics: what does not fit its type (an id of the wrong form, a path
//! with `..`, a commit that is not 40 hex characters) is a [`LineError`], and the replay (`fold.rs`) makes that one problem and goes on (§10).
//! A `type` this version does not know is [`Record::Other`], and a key it does not know in a known record is dropped. [`print_line`] writes keys in
//! the order of §4's table, with no white space, and ends with `\n`. A line read is never written back over itself (the log only grows), so what
//! is dropped on reading is never lost from the file.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU32;
use std::str::FromStr;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use vnoc_domain::{NodeId, NodeKind, RepoPath};

// ------------------------------------------------------------------------------------------------------------------ ids

/// Why a text is not an id of its kind.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct IdError(String);

/// The four characters after the slug of a tour's id: `0-9` and `a-z` without `i`, `l`, `o`, `u`.
const SUFFIX_ALPHABET: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";
const MAX_SLUG: usize = 40;

/// A tour's id, `checkout-order-7k2q`: the slug of its title, `-`, four random characters (§5). It is the file's name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TourId(String);

impl TourId {
    /// The slug is 1 to 40 characters of `a-z`, `0-9` and `-`, with no `-` at either end and none twice in a row.
    pub fn parse(text: &str) -> Result<TourId, IdError> {
        let bad = |why: &str| IdError(format!("`{text}` is not a tour id: {why}"));
        let (slug, suffix) = text
            .rsplit_once('-')
            .ok_or_else(|| bad("it is a slug, `-`, and four characters"))?;
        if suffix.len() != 4 || !suffix.bytes().all(|b| SUFFIX_ALPHABET.contains(&b)) {
            return Err(bad(
                "the last four characters are 0-9 and a-z, without i, l, o and u",
            ));
        }
        if slug.is_empty() || slug.len() > MAX_SLUG {
            return Err(bad("the slug is 1 to 40 characters"));
        }
        if !slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(bad("the slug is a-z, 0-9 and `-`"));
        }
        if slug.starts_with('-') || slug.ends_with('-') || slug.contains("--") {
            return Err(bad(
                "the slug has a single `-` between words, and none at its ends",
            ));
        }
        Ok(TourId(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The part before the last `-` and the four characters.
    pub fn slug(&self) -> &str {
        &self.0[..self.0.len() - 5]
    }
}

/// A number written without a sign, a plus or a leading zero, from 1 up.
fn number(digits: &str) -> Option<NonZeroU32> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || digits.starts_with('0') {
        return None;
    }
    digits.parse().ok()
}

/// A stop's id: `s1`, `s2`, … (§5). It is also the id of the stop's intro, its first step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StopId(NonZeroU32);

impl StopId {
    /// `None` for 0.
    pub fn new(number: u32) -> Option<StopId> {
        NonZeroU32::new(number).map(StopId)
    }

    pub fn parse(text: &str) -> Result<StopId, IdError> {
        text.strip_prefix('s')
            .and_then(number)
            .map(StopId)
            .ok_or_else(|| {
                IdError(format!(
                    "`{text}` is not a stop id: `s` and a number from 1, with no leading zero"
                ))
            })
    }

    pub fn number(self) -> u32 {
        self.0.get()
    }
}

/// A step's id: a stop's own id (its intro), or one of its blocks, `s3.b2` (§5). The depth of a block is not in its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StepId {
    stop: StopId,
    block: Option<NonZeroU32>,
}

impl StepId {
    pub fn intro(stop: StopId) -> StepId {
        StepId { stop, block: None }
    }

    /// `None` for block number 0.
    pub fn block(stop: StopId, number: u32) -> Option<StepId> {
        Some(StepId {
            stop,
            block: Some(NonZeroU32::new(number)?),
        })
    }

    pub fn parse(text: &str) -> Result<StepId, IdError> {
        let bad = || {
            IdError(format!(
                "`{text}` is not a step id: `s3` (a stop's intro) or `s3.b2` (a block), numbers from 1 with no leading zero"
            ))
        };
        match text.split_once('.') {
            None => StopId::parse(text).map(StepId::intro).map_err(|_| bad()),
            Some((stop, block)) => {
                let stop = StopId::parse(stop).map_err(|_| bad())?;
                let block = block.strip_prefix('b').and_then(number).ok_or_else(bad)?;
                Ok(StepId {
                    stop,
                    block: Some(block),
                })
            }
        }
    }

    /// The stop this step belongs to.
    pub fn stop(self) -> StopId {
        self.stop
    }

    /// The block's number under its stop; `None` for an intro.
    pub fn block_number(self) -> Option<u32> {
        self.block.map(NonZeroU32::get)
    }

    pub fn is_intro(self) -> bool {
        self.block.is_none()
    }
}

/// A commit, as a pin keeps it: exactly 40 lower-case hexadecimal characters, so that it can never be taken for an option by git (§10).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Commit(String);

impl Commit {
    pub fn parse(text: &str) -> Result<Commit, IdError> {
        if text.len() == 40 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            Ok(Commit(text.to_string()))
        } else {
            Err(IdError(format!(
                "`{text}` is not a commit: 40 lower-case hexadecimal characters"
            )))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TourId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for StopId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "s{}", self.0)
    }
}

impl fmt::Display for StepId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.block {
            None => write!(f, "{}", self.stop),
            Some(n) => write!(f, "{}.b{n}", self.stop),
        }
    }
}

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `FromStr` and serde (a JSON string) for a type with `parse` and `Display`.
macro_rules! text_type {
    ($($type:ident),*) => {$(
        impl FromStr for $type {
            type Err = IdError;
            fn from_str(text: &str) -> Result<Self, IdError> {
                $type::parse(text)
            }
        }
        impl Serialize for $type {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }
        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let text = String::deserialize(d)?;
                $type::parse(&text).map_err(de::Error::custom)
            }
        }
    )*};
}
text_type!(TourId, StopId, StepId, Commit);

// ----------------------------------------------------------------------------------------------------- small value types

/// How much a tour says: the first line's `detail`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Detail {
    Quick,
    Normal,
    Detailed,
}

/// `[first, last]`: file lines, 1-based, `first <= last`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lines {
    pub first: u32,
    pub last: u32,
}

impl Lines {
    pub fn new(first: u32, last: u32) -> Option<Lines> {
        (first >= 1 && first <= last).then_some(Lines { first, last })
    }
}

impl Serialize for Lines {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        [self.first, self.last].serialize(s)
    }
}

impl<'de> Deserialize<'de> for Lines {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let [first, last] = <[u32; 2]>::deserialize(d)?;
        Lines::new(first, last).ok_or_else(|| {
            de::Error::custom(format!(
                "[{first}, {last}] are not lines: 1-based, the first not after the last"
            ))
        })
    }
}

/// Where one step's lines were when a pin was made: `[path, first, last]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhereAt {
    pub path: RepoPath,
    pub lines: Lines,
}

impl Serialize for WhereAt {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        (self.path.as_str(), self.lines.first, self.lines.last).serialize(s)
    }
}

impl<'de> Deserialize<'de> for WhereAt {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (path, first, last) = <(String, u32, u32)>::deserialize(d)?;
        Ok(WhereAt {
            path: RepoPath::new(path).map_err(de::Error::custom)?,
            lines: Lines::new(first, last).ok_or_else(|| {
                de::Error::custom(format!(
                    "[{first}, {last}] are not lines: 1-based, the first not after the last"
                ))
            })?,
        })
    }
}

/// A JSON object kept in the order it was written: `ids.nodes` and `pin.where`, whose keys are ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ordered<K, V>(pub Vec<(K, V)>);

impl<K: fmt::Display, V: Serialize> Serialize for Ordered<K, V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(&key.to_string(), value)?;
        }
        map.end()
    }
}

impl<'de, K, V> Deserialize<'de> for Ordered<K, V>
where
    K: FromStr,
    K::Err: fmt::Display,
    V: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Entries<K, V>(std::marker::PhantomData<(K, V)>);
        impl<'de, K, V> Visitor<'de> for Entries<K, V>
        where
            K: FromStr,
            K::Err: fmt::Display,
            V: Deserialize<'de>,
        {
            type Value = Ordered<K, V>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    let key = key.parse::<K>().map_err(de::Error::custom)?;
                    entries.push((key, map.next_value()?));
                }
                Ok(Ordered(entries))
            }
        }
        d.deserialize_map(Entries(std::marker::PhantomData))
    }
}

/// `#[serde(with = "repo_path")]`: a `RepoPath` as a JSON string, refused when it is not one (§10).
mod repo_path {
    use super::*;

    pub fn serialize<S: Serializer>(path: &RepoPath, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(path.as_str())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<RepoPath, D::Error> {
        RepoPath::new(String::deserialize(d)?).map_err(de::Error::custom)
    }
}

/// `#[serde(with = "node_kind")]`: `function`, `class`, `file`, `folder`; a word a newer V-NOC wrote is kept (`NodeKind::Other`).
mod node_kind {
    use super::*;

    pub fn serialize<S: Serializer>(kind: &NodeKind, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(kind.as_str())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<NodeKind, D::Error> {
        Ok(match String::deserialize(d)?.as_str() {
            "function" => NodeKind::Function,
            "class" => NodeKind::Class,
            "file" => NodeKind::File,
            "folder" => NodeKind::Folder,
            other => NodeKind::Other(other.to_string()),
        })
    }
}

fn one() -> NonZeroU32 {
    NonZeroU32::MIN
}

fn is_one(n: &NonZeroU32) -> bool {
    n.get() == 1
}

// -------------------------------------------------------------------------------------------------------------- records

/// The first line, and only the first. `schema` is the one version gate of the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TourRecord {
    pub schema: u32,
    pub id: TourId,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub at: String,
    pub detail: Detail,
    /// After an `import --replace`: the highest numbers ever used (§5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Next>,
}

/// The highest numbers ever used in a tour: `{"stop": 12, "blocks": {"s3": 4, "s7": 2}}`. A stop with no entry has used no block number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Next {
    pub stop: u32,
    /// stop id → the highest block number used under it; written in stop order
    #[serde(default)]
    pub blocks: BTreeMap<StopId, u32>,
}

/// A later value for some of the first line's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<Detail>,
}

/// A stop on the route that is not written yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStop {
    pub id: StopId,
    #[serde(with = "repo_path")]
    pub path: RepoPath,
    pub qualname: String,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub occurrence: NonZeroU32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<StopId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_as: Option<StopId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanRecord {
    pub stops: Vec<PlanStop>,
}

/// A written stop: its node, where its lines were, what they hashed to, how the tour got here, and the intro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopRecord {
    pub id: StopId,
    #[serde(with = "repo_path")]
    pub path: RepoPath,
    pub qualname: String,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub occurrence: NonZeroU32,
    #[serde(with = "node_kind")]
    pub kind: NodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<Lines>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<StopId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_as: Option<StopId>,
    pub text: String,
}

/// One block of a stop. `parent` is the block it is a detail of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRecord {
    pub id: StepId,
    pub stop: StopId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<StepId>,
    pub lines: Lines,
    pub hash: String,
    pub title: String,
    pub text: String,
}

/// A stop (`s3`, with its blocks and the stops nested in it) or a block (`s3.b1`, with its details) is off the tour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropRecord {
    pub id: StepId,
}

/// These stops' nodes now have an ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsRecord {
    pub nodes: Ordered<StopId, NodeId>,
}

/// The tour is pinned: every stop's lines are in this commit, at these places.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinRecord {
    pub commit: Commit,
    pub at: String,
    #[serde(rename = "where")]
    pub places: Ordered<StepId, WhereAt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndRecord {
    pub at: String,
}

/// A record that has no keys of its own (`halt`, `halted`, `resume`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bare {}

/// One line of a tour's file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Record {
    Tour(TourRecord),
    Meta(MetaRecord),
    Plan(PlanRecord),
    Stop(StopRecord),
    Step(StepRecord),
    Drop(DropRecord),
    Ids(IdsRecord),
    Pin(PinRecord),
    /// the reader asked the agent to stop
    Halt(Bare),
    /// the agent has been told
    Halted(Bare),
    Resume(Bare),
    End(EndRecord),
    /// A `type` this version does not know (a newer V-NOC, or the interactive tour): skipped when read, and it keeps nothing.
    #[serde(other)]
    Other,
}

/// Why a line is not a record.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct LineError(String);

/// Reads one line (with or without its `\n` or `\r\n`). Never panics.
pub fn parse_line(line: &str) -> Result<Record, LineError> {
    let record: Record = serde_json::from_str(line).map_err(|e| LineError(e.to_string()))?;
    if let Record::Step(step) = &record {
        if step.id.is_intro() {
            return Err(LineError(format!(
                "a `step` is a block: its id is like s3.b2, not {}",
                step.id
            )));
        }
        if step.id.stop() != step.stop {
            return Err(LineError(format!(
                "the block {} says it belongs to {}",
                step.id, step.stop
            )));
        }
    }
    Ok(record)
}

/// One line as it is written: keys in the order of `01-on-disk.md` §4, no white space, `\n` at the end. [`Record::Other`] has nothing to write and
/// gives an empty string.
pub fn print_line(record: &Record) -> String {
    if matches!(record, Record::Other) {
        return String::new();
    }
    let mut line = serde_json::to_string(record).expect("a record is always JSON");
    line.push('\n');
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    const NODE: &str = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";

    /// Every record, in its written form: each must print back to the same text, and parse to a record that prints the same again.
    fn every_record() -> Vec<&'static str> {
        vec![
            r#"{"type":"tour","schema":1,"id":"checkout-order-7k2q","title":"checkout_order","question":"what happens when the payment times out","author":"claude-code","at":"2026-11-23T18:40:02Z","detail":"normal"}"#,
            r#"{"type":"tour","schema":1,"id":"a-0000","title":"t","at":"2026-11-23T18:40:02Z","detail":"quick"}"#,
            r#"{"type":"tour","schema":1,"id":"a-0000","title":"t","at":"x","detail":"detailed","next":{"stop":9,"blocks":{"s1":3,"s2":1,"s10":4}}}"#,
            r#"{"type":"tour","schema":1,"id":"a-0000","title":"t","at":"x","detail":"quick","next":{"stop":0,"blocks":{}}}"#,
            r#"{"type":"meta","title":"New title"}"#,
            r#"{"type":"meta","title":"t","question":"q","detail":"detailed"}"#,
            r#"{"type":"meta"}"#,
            r#"{"type":"plan","stops":[{"id":"s1","path":"app/checkout.py","qualname":"checkout_order"},{"id":"s2","path":"app/inventory.py","qualname":"check_stock","from":"s1","line":13},{"id":"s3","path":"a.py","qualname":"f","occurrence":2,"same_as":"s1"}]}"#,
            r#"{"type":"stop","id":"s1","path":"app/checkout.py","qualname":"checkout_order","kind":"function","node":"0Q8V5W4T2F7KX9M3B6D1RZ4H7K","lines":[11,30],"hash":"9f2c61a07b3e4d15","text":"intro"}"#,
            r#"{"type":"stop","id":"s2","path":"app/inventory.py","qualname":"check_stock","kind":"function","lines":[12,15],"hash":"0b7e55c1d2f4a960","from":"s1","line":13,"text":"t"}"#,
            r#"{"type":"stop","id":"s4","path":"app/inventory.py","qualname":"check_stock","occurrence":2,"kind":"class","lines":[12,15],"hash":"0b7e55c1d2f4a960","from":"s3","line":21,"same_as":"s2","text":"Called again."}"#,
            r#"{"type":"stop","id":"s5","path":"app","qualname":"","kind":"folder","text":"a folder"}"#,
            r#"{"type":"stop","id":"s6","path":"a.py","qualname":"","kind":"module","text":"a kind this version does not know"}"#,
            r#"{"type":"step","id":"s1.b1","stop":"s1","lines":[11,24],"hash":"41c07d92aa10b6e3","title":"Hold the stock, then charge","text":"Nothing is charged."}"#,
            r#"{"type":"step","id":"s1.b2","stop":"s1","parent":"s1.b1","lines":[11,16],"hash":"7be1a0c4d95f2233","title":"Check","text":"t \"quoted\" é 😀"}"#,
            r#"{"type":"drop","id":"s3"}"#,
            r#"{"type":"drop","id":"s3.b1"}"#,
            r#"{"type":"ids","nodes":{"s2":"3M1X9C7B5N2V8K4J6H0G1F3D5S","s10":"0Q8V5W4T2F7KX9M3B6D1RZ4H7K"}}"#,
            r#"{"type":"pin","commit":"f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a","at":"2026-11-23T18:47:31Z","where":{"s1":["app/checkout.py",11,30],"s1.b1":["app/checkout.py",11,24],"s2":["app/inventory.py",12,15]}}"#,
            r#"{"type":"pin","commit":"f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a","at":"x","where":{}}"#,
            r#"{"type":"halt"}"#,
            r#"{"type":"halted"}"#,
            r#"{"type":"resume"}"#,
            r#"{"type":"end","at":"2026-11-23T18:47:31Z"}"#,
        ]
    }

    #[test]
    fn each_record_prints_and_parses_back_to_itself() {
        for text in every_record() {
            let record = parse_line(text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_ne!(record, Record::Other, "{text}");
            let printed = print_line(&record);
            assert_eq!(printed, format!("{text}\n"), "prints as written");
            assert_eq!(parse_line(&printed).unwrap(), record, "{text}");
        }
    }

    #[test]
    fn the_keys_come_in_the_order_of_the_table_whatever_order_they_were_read_in() {
        // `text` and `kind` first, the others scrambled
        let line = r#"{"text":"t","hash":"h","from":"s1","kind":"class","line":3,"qualname":"q","lines":[2,4],"path":"a.py","id":"s2","type":"stop","same_as":"s1","node":"0Q8V5W4T2F7KX9M3B6D1RZ4H7K","occurrence":2}"#;
        assert_eq!(
            print_line(&parse_line(line).unwrap()),
            format!(
                "{{\"type\":\"stop\",\"id\":\"s2\",\"path\":\"a.py\",\"qualname\":\"q\",\"occurrence\":2,\"kind\":\"class\",\"node\":\"{NODE}\",\"lines\":[2,4],\"hash\":\"h\",\"from\":\"s1\",\"line\":3,\"same_as\":\"s1\",\"text\":\"t\"}}\n"
            )
        );
    }

    #[test]
    fn an_occurrence_of_one_is_not_written_and_a_missing_one_reads_as_one() {
        let with = r#"{"type":"stop","id":"s1","path":"a.py","qualname":"f","occurrence":1,"kind":"function","text":"t"}"#;
        let without = r#"{"type":"stop","id":"s1","path":"a.py","qualname":"f","kind":"function","text":"t"}"#;
        let (a, b) = (parse_line(with).unwrap(), parse_line(without).unwrap());
        assert_eq!(a, b);
        assert_eq!(print_line(&a), format!("{without}\n"));
        let Record::Stop(stop) = a else { panic!() };
        assert_eq!(stop.occurrence.get(), 1);
    }

    #[test]
    fn a_key_this_version_does_not_know_is_accepted_and_dropped() {
        let cases = [
            (
                r#"{"type":"stop","id":"s1","path":"a.py","qualname":"f","kind":"function","text":"t","future":{"a":[1]}}"#,
                r#"{"type":"stop","id":"s1","path":"a.py","qualname":"f","kind":"function","text":"t"}"#,
            ),
            (r#"{"type":"halt","by":"reader"}"#, r#"{"type":"halt"}"#),
            (
                r#"{"type":"end","at":"x","note":1}"#,
                r#"{"type":"end","at":"x"}"#,
            ),
            (
                r#"{"type":"drop","id":"s2","why":"x"}"#,
                r#"{"type":"drop","id":"s2"}"#,
            ),
            (
                r#"{"type":"pin","commit":"f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a","at":"x","where":{},"extra":true}"#,
                r#"{"type":"pin","commit":"f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a","at":"x","where":{}}"#,
            ),
        ];
        for (read, printed) in cases {
            assert_eq!(
                print_line(&parse_line(read).unwrap()),
                format!("{printed}\n"),
                "{read}"
            );
        }
    }

    #[test]
    fn a_type_this_version_does_not_know_is_other_and_prints_nothing() {
        for line in [
            r#"{"type":"question","at":"s1","text":"why?"}"#,
            r#"{"type":"Stop"}"#,
            r#"{"type":""}"#,
        ] {
            assert_eq!(parse_line(line).unwrap(), Record::Other, "{line}");
        }
        assert_eq!(print_line(&Record::Other), "");
    }

    #[test]
    fn a_line_that_is_not_a_record_is_an_error_and_never_a_panic() {
        let cases = [
            ("", "empty"),
            ("   ", "blank"),
            ("not json", "text"),
            ("[]", "an array"),
            ("42", "a number"),
            ("null", "null"),
            ("{", "cut off"),
            (r#"{"id":"s1"}"#, "no type"),
            (r#"{"type":5}"#, "a type that is not text"),
            (r#"{"type":"stop","id":"s1"}"#, "keys missing"),
            (r#"{"type":"drop"}"#, "no id"),
            (
                r#"{"type":"tour","schema":1,"id":"a-0000","title":"t","at":"x"}"#,
                "no detail",
            ),
            (
                r#"{"type":"tour","schema":1,"id":"a-0000","title":"t","at":"x","detail":"huge"}"#,
                "a detail that is not one",
            ),
            (r#"{"type":"end","at":3}"#, "a time that is a number"),
        ];
        for (line, what) in cases {
            assert!(parse_line(line).is_err(), "{what}: {line}");
        }
    }

    #[test]
    fn a_value_that_fails_its_type_makes_the_line_an_error() {
        // (what is wrong, the line)
        let stop = |key: &str, value: &str| {
            let mut keys = vec![
                ("id", r#""s1""#),
                ("path", r#""a.py""#),
                ("qualname", r#""f""#),
                ("kind", r#""function""#),
                ("text", r#""t""#),
            ];
            keys.retain(|(k, _)| *k != key);
            keys.push((key, value));
            let body: Vec<String> = keys.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect();
            format!("{{\"type\":\"stop\",{}}}", body.join(","))
        };
        let cases = [
            ("a path with ..", stop("path", r#""../a.py""#)),
            ("a path with a dot segment", stop("path", r#""a/./b.py""#)),
            ("an absolute path", stop("path", r#""/etc/passwd""#)),
            ("a drive path", stop("path", r#""C:/a.py""#)),
            ("a backslash path", stop("path", r#""a\\b.py""#)),
            ("an empty path", stop("path", r#""""#)),
            ("a path that is a number", stop("path", "3")),
            ("a stop id that is s0", stop("id", r#""s0""#)),
            ("a stop id that is a block's", stop("id", r#""s1.b1""#)),
            ("a node that is not an ID", stop("node", r#""nope""#)),
            ("lines that are backwards", stop("lines", "[9,3]")),
            ("lines from 0", stop("lines", "[0,3]")),
            ("lines that are three numbers", stop("lines", "[1,2,3]")),
            ("lines that are not numbers", stop("lines", r#"["a","b"]"#)),
            ("a negative line", stop("line", "-1")),
            ("an occurrence of 0", stop("occurrence", "0")),
            ("a from that is not a stop", stop("from", r#""x""#)),
            ("a same_as that is a block", stop("same_as", r#""s1.b1""#)),
            ("a text that is a number", stop("text", "7")),
        ];
        for (what, line) in &cases {
            assert!(parse_line(line).is_err(), "{what}: {line}");
        }
        let commit = |c: &str| format!(r#"{{"type":"pin","commit":"{c}","at":"x","where":{{}}}}"#);
        let good = "f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a";
        assert!(parse_line(&commit(good)).is_ok());
        for (what, c) in [
            ("a commit of 39 characters", &good[..39]),
            ("a commit of 41 characters", &format!("{good}0")),
            ("a commit in capitals", &good.to_uppercase()),
            ("an option", "--help"),
            (
                "an option as long as a commit",
                "--output=/etc/passwd-0000000000000000",
            ),
            ("a ref name", "HEAD"),
            (
                "a commit that is not hex",
                "g4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a",
            ),
        ] {
            assert!(parse_line(&commit(c)).is_err(), "{what}: {c}");
        }
        for (what, line) in [
            (
                "a pin path with ..",
                r#"{"type":"pin","commit":"f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a","at":"x","where":{"s1":["../a.py",1,2]}}"#,
            ),
            (
                "a pin key that is not a step",
                r#"{"type":"pin","commit":"f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a","at":"x","where":{"first":["a.py",1,2]}}"#,
            ),
            (
                "a pin place with lines backwards",
                r#"{"type":"pin","commit":"f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a","at":"x","where":{"s1":["a.py",5,2]}}"#,
            ),
            (
                "a pin place of two items",
                r#"{"type":"pin","commit":"f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a","at":"x","where":{"s1":["a.py",5]}}"#,
            ),
            (
                "an ids key that is a block",
                r#"{"type":"ids","nodes":{"s1.b1":"0Q8V5W4T2F7KX9M3B6D1RZ4H7K"}}"#,
            ),
            (
                "an ids value that is not an ID",
                r#"{"type":"ids","nodes":{"s1":"x"}}"#,
            ),
            ("an ids that is a list", r#"{"type":"ids","nodes":[]}"#),
            (
                "a plan path with ..",
                r#"{"type":"plan","stops":[{"id":"s1","path":"../x","qualname":""}]}"#,
            ),
            ("a plan that is not a list", r#"{"type":"plan","stops":{}}"#),
            (
                "a step that is an intro id",
                r#"{"type":"step","id":"s1","stop":"s1","lines":[1,2],"hash":"h","title":"t","text":"t"}"#,
            ),
            (
                "a step in another stop than its id says",
                r#"{"type":"step","id":"s1.b1","stop":"s2","lines":[1,2],"hash":"h","title":"t","text":"t"}"#,
            ),
            (
                "a step with a parent that is not an id",
                r#"{"type":"step","id":"s1.b1","stop":"s1","parent":"b1","lines":[1,2],"hash":"h","title":"t","text":"t"}"#,
            ),
            (
                "a drop that is not an id",
                r#"{"type":"drop","id":"s1.b0"}"#,
            ),
            (
                "a meta with a detail that is not one",
                r#"{"type":"meta","detail":"long"}"#,
            ),
        ] {
            assert!(parse_line(line).is_err(), "{what}: {line}");
        }
    }

    #[test]
    fn a_line_with_its_line_end_parses() {
        for line in [
            "{\"type\":\"halt\"}\n",
            "{\"type\":\"halt\"}\r\n",
            "{\"type\":\"halt\"}",
        ] {
            assert!(matches!(parse_line(line).unwrap(), Record::Halt(_)));
        }
    }

    #[test]
    fn next_is_typed_and_written_stop_first_and_its_blocks_in_stop_order() {
        let head = r#"{"type":"tour","schema":1,"id":"a-0000","title":"t","at":"x","detail":"normal","next":"#;
        // blocks read in any order are written in stop order (s2 before s10), `stop` first
        let read = format!(r#"{head}{{"blocks":{{"s10":4,"s2":1,"s1":3}},"stop":12}}}}"#);
        let Record::Tour(tour) = parse_line(&read).unwrap() else {
            panic!()
        };
        let next = tour.next.clone().unwrap();
        assert_eq!(next.stop, 12);
        assert_eq!(next.blocks.len(), 3);
        assert_eq!(
            print_line(&Record::Tour(tour)),
            format!(r#"{head}{{"stop":12,"blocks":{{"s1":3,"s2":1,"s10":4}}}}}}"#) + "\n"
        );
        // a stop with no entry has used none: no `blocks` at all reads as none, and is written as an empty object
        let Record::Tour(tour) = parse_line(&format!(r#"{head}{{"stop":5}}}}"#)).unwrap() else {
            panic!()
        };
        assert!(tour.next.as_ref().unwrap().blocks.is_empty());
        assert!(print_line(&Record::Tour(tour)).contains(r#""next":{"stop":5,"blocks":{}}"#));
        // what is not a `next` makes the line an error
        for (what, next) in [
            ("a number", "3"),
            ("a list", "[]"),
            ("no stop", r#"{"blocks":{}}"#),
            ("a negative stop", r#"{"stop":-1}"#),
            ("a stop that is text", r#"{"stop":"9"}"#),
            (
                "a block key that is not a stop",
                r#"{"stop":1,"blocks":{"x":1}}"#,
            ),
            (
                "a block key that is a block",
                r#"{"stop":1,"blocks":{"s1.b1":1}}"#,
            ),
            (
                "a block count that is text",
                r#"{"stop":1,"blocks":{"s1":"2"}}"#,
            ),
        ] {
            assert!(parse_line(&format!("{head}{next}}}")).is_err(), "{what}");
        }
    }

    #[test]
    fn tour_ids() {
        let good = [
            "checkout-order-7k2q",
            "a-0000",
            "x-zzzz",
            "tour-9z8y",
            "a1-b2-c3-0k2m",
            &format!("{}-7k2q", "a".repeat(40)),
        ];
        for id in good {
            let parsed = TourId::parse(id).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(parsed.to_string(), id);
            assert_eq!(parsed.as_str(), id);
        }
        assert_eq!(
            TourId::parse("checkout-order-7k2q").unwrap().slug(),
            "checkout-order"
        );
        let bad = [
            ("", "empty"),
            ("7k2q", "no slug"),
            ("-7k2q", "an empty slug"),
            ("checkout-order", "no four characters ('order' is five)"),
            ("a-7k2", "three characters"),
            ("a-7k2qq", "five characters"),
            ("Checkout-7k2q", "a capital in the slug"),
            ("checkout-7K2Q", "a capital in the last four"),
            ("checkout-7l2q", "an l in the last four"),
            ("checkout-7i2q", "an i in the last four"),
            ("checkout-7o2q", "an o in the last four"),
            ("checkout-7u2q", "a u in the last four"),
            ("a--b-7k2q", "two hyphens in a row"),
            ("a--7k2q", "a hyphen at the end of the slug"),
            ("-a-7k2q", "a hyphen at the start of the slug"),
            ("a_b-7k2q", "an underscore"),
            ("a b-7k2q", "a space"),
            ("é-7k2q", "a letter that is not ASCII"),
            ("a-7é2", "a last four with an accent"),
            ("a/b-7k2q", "a slash"),
            ("../a-7k2q", "a way up"),
            (
                &format!("{}-7k2q", "a".repeat(41)),
                "a slug of 41 characters",
            ),
        ];
        for (id, what) in bad {
            assert!(TourId::parse(id).is_err(), "{what}: {id:?}");
        }
    }

    #[test]
    fn stop_ids() {
        for (text, number) in [
            ("s1", 1),
            ("s9", 9),
            ("s10", 10),
            ("s40", 40),
            ("s4294967295", u32::MAX),
        ] {
            let id = StopId::parse(text).unwrap();
            assert_eq!((id.number(), id.to_string()), (number, text.to_string()));
            assert_eq!(StopId::new(number), Some(id));
        }
        assert_eq!(StopId::new(0), None);
        for text in [
            "",
            "s",
            "s0",
            "s01",
            "s00",
            "S1",
            "s-1",
            "s+1",
            "s1.b1",
            "s 1",
            "s1 ",
            " s1",
            "1",
            "b1",
            "ss1",
            "s1a",
            "s4294967296",
            "s٣",
        ] {
            assert!(StopId::parse(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn step_ids() {
        let s3 = StopId::new(3).unwrap();
        for (text, block) in [("s3", None), ("s3.b1", Some(1)), ("s3.b12", Some(12))] {
            let id = StepId::parse(text).unwrap();
            assert_eq!(
                (id.stop(), id.block_number(), id.is_intro()),
                (s3, block, block.is_none())
            );
            assert_eq!(id.to_string(), text);
            assert_eq!(
                id,
                match block {
                    None => StepId::intro(s3),
                    Some(n) => StepId::block(s3, n).unwrap(),
                }
            );
        }
        assert_eq!(StepId::block(s3, 0), None);
        for text in [
            "", "s0", "s3.", "s3.b", "s3.b0", "s3.b01", "s3.1", "s3.B1", "s3.b1.b2", "s3b1",
            "s03.b1", "s3.b-1", "s3.b+1", ".b1", "b1", "s3.c1", "s3. b1",
        ] {
            assert!(StepId::parse(text).is_err(), "{text:?}");
        }
        // an intro is the stop's own id, as the design has it
        assert_eq!(
            StepId::parse("s3").unwrap().to_string(),
            StopId::new(3).unwrap().to_string()
        );
    }

    #[test]
    fn commits() {
        let good = "f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a";
        assert_eq!(Commit::parse(good).unwrap().as_str(), good);
        assert_eq!(Commit::parse(good).unwrap().to_string(), good);
        assert!(Commit::parse(&"0".repeat(40)).is_ok());
        for text in [
            "",
            "abc",
            &good.to_uppercase(),
            &good[1..],
            "-".repeat(40).as_str(),
        ] {
            assert!(Commit::parse(text).is_err(), "{text:?}");
        }
    }
}
