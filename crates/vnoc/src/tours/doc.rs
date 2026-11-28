//! A tour as one JSON document, nested the way a person reads it (`docs/26-tours/02-commands.md` §10, `01-on-disk.md` §3).
//!
//! V-NOC keeps a tour **flat** (stops with a `from`, blocks with a `parent`); the document is the same tour as a tree: a stop's `stops` are the stops nested
//! in it, a block's `blocks` its details. [`nest`] makes the tree from a tour, [`flatten`] makes the flat lists from a tree, and nothing nested is ever
//! stored. `export --flat` prints the other shape (every stop and block at one level, with `from` and `parent`); [`flatten`] takes both, and a document
//! that says a nesting and a `from` that disagree is refused.
//!
//! Nothing here touches the disk or the index. The document is read from a `serde_json::Value` **by hand**, so that every refusal (an unknown key, a text
//! that is a number, a `same_as` that names nothing) carries its place as written: `stops[0].stops[1].blocks[0].text`. A typo (`blokcs`) is refused with its
//! place and never dropped.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Map, Value};

use super::fold::{Tour, TourBlock, TourStop};
use super::record::{Detail, StopId};
use super::rules::Refusal;

/// The most a document is, in bytes: the most a tour's file is (01 §10). Over it, nothing is parsed.
pub const MAX_BYTES: usize = 1024 * 1024;

const DOCUMENT_KEYS: &[&str] = &["schema", "title", "question", "author", "detail", "stops"];
const STOP_KEYS: &[&str] = &[
    "id", "from", "line", "same_as", "node", "text", "blocks", "stops",
];
const BLOCK_KEYS: &[&str] = &["id", "parent", "lines", "title", "text", "blocks"];

// ------------------------------------------------------------------------------------------------------------------------- the document

/// The document of 02 §10. `from` and `parent` are the flat shape's: a nested document leaves them out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub title: String,
    pub question: Option<String>,
    pub author: Option<String>,
    pub detail: Option<Detail>,
    pub stops: Vec<DocStop>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocStop {
    pub id: Option<String>,
    pub from: Option<String>,
    pub line: Option<u32>,
    pub same_as: Option<String>,
    pub node: String,
    pub text: String,
    pub blocks: Vec<DocBlock>,
    pub stops: Vec<DocStop>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocBlock {
    pub id: Option<String>,
    pub parent: Option<String>,
    pub lines: [u32; 2],
    pub title: String,
    pub text: String,
    pub blocks: Vec<DocBlock>,
}

fn refusal(place: impl Into<String>, problem: impl AsRef<str>) -> Refusal {
    let place = place.into();
    let place = if place.is_empty() {
        "document".to_string()
    } else {
        place
    };
    Refusal {
        problem: format!("{place}: {}", problem.as_ref()),
        place,
    }
}

fn at(base: &str, key: &str) -> String {
    if base.is_empty() {
        key.to_string()
    } else {
        format!("{base}.{key}")
    }
}

// ------------------------------------------------------------------------------------------------------------------------------ reading

struct Reader {
    refusals: Vec<Refusal>,
}

impl Reader {
    fn say(&mut self, place: &str, problem: impl AsRef<str>) {
        self.refusals.push(refusal(place, problem));
    }

    /// The object at `place`, or why not; its keys that are not `keys` are each refused with their place.
    fn object<'v>(
        &mut self,
        value: &'v Value,
        place: &str,
        what: &str,
        keys: &[&str],
    ) -> Option<&'v Map<String, Value>> {
        let Some(map) = value.as_object() else {
            self.say(place, format!("this is not an object: {what} is {{ … }}"));
            return None;
        };
        for key in map.keys() {
            if !keys.contains(&key.as_str()) {
                self.say(
                    &at(place, key),
                    format!(
                        "`{key}` is not a key of {what}: the keys are {}",
                        keys.join(", ")
                    ),
                );
            }
        }
        Some(map)
    }

    fn string(
        &mut self,
        map: &Map<String, Value>,
        key: &str,
        place: &str,
        required: bool,
    ) -> Option<String> {
        let here = at(place, key);
        match map.get(key) {
            None | Some(Value::Null) => {
                if required {
                    self.say(&here, "this is needed: text");
                }
                None
            }
            Some(Value::String(text)) => Some(text.clone()),
            Some(other) => {
                self.say(&here, format!("this is {}, not text", kind(other)));
                None
            }
        }
    }

    fn label(&mut self, map: &Map<String, Value>, key: &str, place: &str) -> Option<String> {
        let here = at(place, key);
        let text = self.string(map, key, place, false)?;
        if text.trim().is_empty() {
            self.say(&here, "this is empty: leave it out, or give a short name");
            return None;
        }
        Some(text)
    }

    fn number(&mut self, value: &Value, place: &str) -> Option<u32> {
        match value.as_u64().and_then(|n| u32::try_from(n).ok()) {
            Some(n) => Some(n),
            None => {
                self.say(
                    place,
                    format!(
                        "this is {}, not a line number: a whole number from 1",
                        describe(value)
                    ),
                );
                None
            }
        }
    }

    fn array<'v>(
        &mut self,
        map: &'v Map<String, Value>,
        key: &str,
        place: &str,
    ) -> Option<&'v Vec<Value>> {
        match map.get(key) {
            None | Some(Value::Null) => None,
            Some(Value::Array(items)) => Some(items),
            Some(other) => {
                self.say(
                    &at(place, key),
                    format!("this is {}, not a list", kind(other)),
                );
                None
            }
        }
    }

    fn block(&mut self, value: &Value, place: &str) -> DocBlock {
        let mut block = DocBlock::default();
        let Some(map) = self.object(value, place, "a block", BLOCK_KEYS) else {
            return block;
        };
        block.id = self.label(map, "id", place);
        block.parent = self.label(map, "parent", place);
        block.title = self.string(map, "title", place, true).unwrap_or_default();
        block.text = self.string(map, "text", place, true).unwrap_or_default();
        let here = at(place, "lines");
        match map.get("lines") {
            None | Some(Value::Null) => self.say(&here, "this is needed: [first line, last line]"),
            Some(Value::Array(pair)) if pair.len() == 2 => {
                let first = self.number(&pair[0], &format!("{here}[0]"));
                let last = self.number(&pair[1], &format!("{here}[1]"));
                if let (Some(first), Some(last)) = (first, last) {
                    if first == 0 || last < first {
                        self.say(&here, format!("[{first}, {last}] are not lines: from 1, the first not after the last"));
                    }
                    block.lines = [first, last];
                }
            }
            Some(_) => self.say(
                &here,
                "this is not a pair of line numbers: [first line, last line]",
            ),
        }
        if let Some(items) = self.array(map, "blocks", place) {
            block.blocks = items
                .iter()
                .enumerate()
                .map(|(i, v)| self.block(v, &format!("{}[{i}]", at(place, "blocks"))))
                .collect();
        }
        block
    }

    fn stop(&mut self, value: &Value, place: &str) -> DocStop {
        let mut stop = DocStop::default();
        let Some(map) = self.object(value, place, "a stop", STOP_KEYS) else {
            return stop;
        };
        stop.id = self.label(map, "id", place);
        stop.from = self.label(map, "from", place);
        stop.same_as = self.label(map, "same_as", place);
        stop.node = self.string(map, "node", place, true).unwrap_or_default();
        stop.text = self.string(map, "text", place, true).unwrap_or_default();
        match map.get("line") {
            None | Some(Value::Null) => {}
            Some(n) => stop.line = self.number(n, &at(place, "line")),
        }
        if let Some(items) = self.array(map, "blocks", place) {
            stop.blocks = items
                .iter()
                .enumerate()
                .map(|(i, v)| self.block(v, &format!("{}[{i}]", at(place, "blocks"))))
                .collect();
        }
        if let Some(items) = self.array(map, "stops", place) {
            stop.stops = items
                .iter()
                .enumerate()
                .map(|(i, v)| self.stop(v, &format!("{}[{i}]", at(place, "stops"))))
                .collect();
        }
        stop
    }
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "true or false",
        Value::Number(_) => "a number",
        Value::String(_) => "text",
        Value::Array(_) => "a list",
        Value::Object(_) => "an object",
    }
}

fn describe(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        other => kind(other).to_string(),
    }
}

/// Reads a document from its text, or says everything that is wrong with it, each with its place. A text over [`MAX_BYTES`] is refused before it is
/// parsed.
pub fn parse(text: &str) -> Result<Document, Vec<Refusal>> {
    if text.len() > MAX_BYTES {
        return Err(vec![refusal(
            "document",
            format!(
                "the document is {} bytes: the most one document takes is {MAX_BYTES}",
                text.len()
            ),
        )]);
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value: Value = serde_json::from_str(text)
        .map_err(|e| vec![refusal("document", format!("this is not JSON: {e}"))])?;
    let mut reader = Reader {
        refusals: Vec::new(),
    };
    let Some(map) = reader.object(&value, "", "the document", DOCUMENT_KEYS) else {
        return Err(reader.refusals);
    };
    match map.get("schema") {
        None | Some(Value::Null) => {}
        Some(v) if v.as_u64() == Some(1) => {}
        Some(v) => reader.say(
            "schema",
            format!(
                "{} is not a schema this version reads: it is 1",
                describe(v)
            ),
        ),
    }
    let title = reader.string(map, "title", "", true).unwrap_or_default();
    let question = reader.string(map, "question", "", false);
    let author = reader.string(map, "author", "", false);
    let detail = match reader.string(map, "detail", "", false).as_deref() {
        None => None,
        Some("quick") => Some(Detail::Quick),
        Some("normal") => Some(Detail::Normal),
        Some("detailed") => Some(Detail::Detailed),
        Some(other) => {
            reader.say(
                "detail",
                format!("\"{other}\" is not quick, normal or detailed"),
            );
            None
        }
    };
    let mut stops = Vec::new();
    match map.get("stops") {
        Some(Value::Array(items)) => {
            if items.is_empty() {
                reader.say("stops", "a tour needs at least one stop");
            }
            for (i, item) in items.iter().enumerate() {
                stops.push(reader.stop(item, &format!("stops[{i}]")));
            }
        }
        None | Some(Value::Null) => reader.say("stops", "this is needed: a list of stops"),
        Some(other) => reader.say("stops", format!("this is {}, not a list", kind(other))),
    }
    if !reader.refusals.is_empty() {
        return Err(reader.refusals);
    }
    Ok(Document {
        title,
        question,
        author,
        detail,
        stops,
    })
}

// ------------------------------------------------------------------------------------------------------------------------------- flatten

/// One stop of a document, flat: in the route's order, with the stop it is nested in and the stop it repeats as places in that order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatStop {
    /// where it is in the document as written: `stops[0].stops[1]`
    pub place: String,
    pub id: Option<String>,
    pub node: String,
    /// an earlier stop
    pub from: Option<usize>,
    pub line: Option<u32>,
    /// an earlier stop
    pub same_as: Option<usize>,
    pub text: String,
    /// in reading order: a block, then its details
    pub blocks: Vec<FlatBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatBlock {
    pub place: String,
    pub id: Option<String>,
    /// an earlier block of the stop
    pub parent: Option<usize>,
    pub lines: (u32, u32),
    pub title: String,
    pub text: String,
}

/// A document as V-NOC's flat lists: what `plan_tour` checks and writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flat {
    pub title: String,
    pub question: Option<String>,
    pub author: Option<String>,
    pub detail: Option<Detail>,
    pub stops: Vec<FlatStop>,
}

/// The depth-first order of nodes given each one's parent (an earlier node, or none): roots in their order, each followed by what is nested in it, in
/// order. `out[i]` is the encounter index of the i-th in that order.
fn depth_first(parents: &[Option<usize>]) -> Vec<usize> {
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); parents.len()];
    let mut roots = Vec::new();
    for (i, parent) in parents.iter().enumerate() {
        match parent {
            Some(p) => children[*p].push(i),
            None => roots.push(i),
        }
    }
    fn walk(i: usize, children: &[Vec<usize>], out: &mut Vec<usize>) {
        out.push(i);
        for c in &children[i] {
            walk(*c, children, out);
        }
    }
    let mut out = Vec::new();
    for root in roots {
        walk(root, &children, &mut out);
    }
    out
}

/// Which of the nested lists an item is in, and the nesting says what it is nested in; a `from` or `parent` as well must say the same (or, where
/// nothing is nested, it is the nesting). `earlier` is every item before this one in the list as written. `Err` is the sentence.
fn parent_of(
    label: Option<&String>,
    container: Option<usize>,
    me: usize,
    labels: &HashMap<&str, usize>,
    what: &str,
    key: &str,
) -> Result<Option<usize>, String> {
    let Some(label) = label else {
        return Ok(container);
    };
    let Some(&named) = labels.get(label.as_str()) else {
        return Err(format!(
            "`{label}` is not the id of any {what} of this document"
        ));
    };
    if named >= me {
        return Err(format!(
            "`{label}` comes later in the list: {key} names a {what} that comes earlier"
        ));
    }
    match container {
        Some(inside) if inside != named => Err(format!(
            "`{label}` is not the {what} this one is nested in: a nesting and a {key} must say the same"
        )),
        _ => Ok(Some(named)),
    }
}

struct StopEntry<'a> {
    stop: &'a DocStop,
    place: String,
    container: Option<usize>,
}

fn collect_stops<'a>(
    stops: &'a [DocStop],
    base: &str,
    container: Option<usize>,
    out: &mut Vec<StopEntry<'a>>,
) {
    for (i, stop) in stops.iter().enumerate() {
        let place = format!("{}[{i}]", at(base, "stops"));
        let me = out.len();
        out.push(StopEntry {
            stop,
            place: place.clone(),
            container,
        });
        collect_stops(&stop.stops, &place, Some(me), out);
    }
}

struct BlockEntry<'a> {
    block: &'a DocBlock,
    place: String,
    container: Option<usize>,
}

fn collect_blocks<'a>(
    blocks: &'a [DocBlock],
    base: &str,
    container: Option<usize>,
    out: &mut Vec<BlockEntry<'a>>,
) {
    for (i, block) in blocks.iter().enumerate() {
        let place = format!("{}[{i}]", at(base, "blocks"));
        let me = out.len();
        out.push(BlockEntry {
            block,
            place: place.clone(),
            container,
        });
        collect_blocks(&block.blocks, &place, Some(me), out);
    }
}

fn flatten_blocks(stop: &DocStop, place: &str, refusals: &mut Vec<Refusal>) -> Vec<FlatBlock> {
    let mut entries = Vec::new();
    collect_blocks(&stop.blocks, place, None, &mut entries);
    let mut labels: HashMap<&str, usize> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        if let Some(id) = &e.block.id
            && labels.insert(id, i).is_some()
        {
            refusals.push(refusal(
                at(&e.place, "id"),
                format!("`{id}` is the id of another block of this stop"),
            ));
        }
    }
    let mut parents: Vec<Option<usize>> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let parent = match parent_of(
            e.block.parent.as_ref(),
            e.container,
            i,
            &labels,
            "block",
            "parent",
        ) {
            Ok(parent) => parent,
            Err(problem) => {
                refusals.push(refusal(at(&e.place, "parent"), problem));
                None
            }
        };
        parents.push(parent);
    }
    // details nest one level: a block whose parent has a parent is refused
    for (i, e) in entries.iter().enumerate() {
        if let Some(p) = parents[i]
            && parents[p].is_some()
        {
            refusals.push(refusal(
                &e.place,
                "details nest one level: a detail takes no blocks of its own. Make this a block of the stop, or a detail of the block above",
            ));
        }
    }
    let order = depth_first(&parents);
    let mut position = vec![0; entries.len()];
    for (pos, &enc) in order.iter().enumerate() {
        position[enc] = pos;
    }
    order
        .iter()
        .map(|&enc| {
            let e = &entries[enc];
            FlatBlock {
                place: e.place.clone(),
                id: e.block.id.clone(),
                parent: parents[enc].map(|p| position[p]),
                lines: (e.block.lines[0], e.block.lines[1]),
                title: e.block.title.clone(),
                text: e.block.text.clone(),
            }
        })
        .collect()
}

/// The flat lists of a document, in the route's order (a stop, then what is nested in it) and the reading order (a block, then its details), or every
/// refusal, each with its place. Takes the nested shape and the flat one (`from`, `parent`), and a mix: what both say must agree.
pub fn flatten(document: &Document) -> Result<Flat, Vec<Refusal>> {
    let mut refusals = Vec::new();
    let mut entries = Vec::new();
    collect_stops(&document.stops, "", None, &mut entries);
    let mut labels: HashMap<&str, usize> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        if let Some(id) = &e.stop.id
            && labels.insert(id, i).is_some()
        {
            refusals.push(refusal(
                at(&e.place, "id"),
                format!("`{id}` is the id of another stop of this document"),
            ));
        }
    }
    let mut parents: Vec<Option<usize>> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let parent = match parent_of(
            e.stop.from.as_ref(),
            e.container,
            i,
            &labels,
            "stop",
            "from",
        ) {
            Ok(parent) => parent,
            Err(problem) => {
                refusals.push(refusal(at(&e.place, "from"), problem));
                None
            }
        };
        parents.push(parent);
    }
    let order = depth_first(&parents);
    let mut position = vec![0; entries.len()];
    for (pos, &enc) in order.iter().enumerate() {
        position[enc] = pos;
    }
    let mut stops = Vec::new();
    for &enc in &order {
        let e = &entries[enc];
        let same_as = match &e.stop.same_as {
            None => None,
            Some(label) => match labels.get(label.as_str()) {
                None => {
                    refusals.push(refusal(
                        at(&e.place, "same_as"),
                        format!("`{label}` is not the id of any stop of this document"),
                    ));
                    None
                }
                Some(&named) if position[named] >= position[enc] => {
                    refusals.push(refusal(
                        at(&e.place, "same_as"),
                        format!("`{label}` is this stop or comes after it: same_as names a stop that comes earlier in the reading order"),
                    ));
                    None
                }
                Some(&named) => Some(position[named]),
            },
        };
        if e.stop.line.is_some() && parents[enc].is_none() {
            refusals.push(refusal(
                at(&e.place, "line"),
                "a stop that is not nested in another has no line to be called from: leave `line` out",
            ));
        }
        stops.push(FlatStop {
            place: e.place.clone(),
            id: e.stop.id.clone(),
            node: e.stop.node.clone(),
            from: parents[enc].map(|p| position[p]),
            line: e.stop.line,
            same_as,
            text: e.stop.text.clone(),
            blocks: flatten_blocks(e.stop, &e.place, &mut refusals),
        });
    }
    if !refusals.is_empty() {
        return Err(refusals);
    }
    Ok(Flat {
        title: document.title.clone(),
        question: document.question.clone(),
        author: document.author.clone(),
        detail: document.detail,
        stops,
    })
}

// ---------------------------------------------------------------------------------------------------------------------------------- nest

/// A stop's name as the document says it: `path::qualname` (with `#n` for the n-th of a name), a file's or a folder's path.
pub fn node_name(stop: &TourStop) -> String {
    if stop.qualname.is_empty() {
        stop.path.to_string()
    } else if stop.occurrence > 1 {
        format!("{}::{}#{}", stop.path, stop.qualname, stop.occurrence)
    } else {
        format!("{}::{}", stop.path, stop.qualname)
    }
}

fn nest_blocks(
    blocks: &[TourBlock],
    flat: bool,
    parent: Option<&TourBlock>,
    out: &mut Vec<DocBlock>,
) {
    for block in blocks {
        let mut one = DocBlock {
            id: Some(block.id.to_string()),
            parent: if flat {
                parent.map(|p| p.id.to_string())
            } else {
                None
            },
            lines: [block.lines.first, block.lines.last],
            title: block.title.clone(),
            text: block.text.clone(),
            blocks: Vec::new(),
        };
        if flat {
            out.push(one);
            nest_blocks(&block.details, flat, Some(block), out);
        } else {
            nest_blocks(&block.details, flat, Some(block), &mut one.blocks);
            out.push(one);
        }
    }
}

fn doc_stop(
    stop: &TourStop,
    now: &BTreeMap<StopId, String>,
    flat: bool,
    parent: Option<StopId>,
) -> DocStop {
    let mut blocks = Vec::new();
    nest_blocks(&stop.blocks, flat, None, &mut blocks);
    DocStop {
        id: Some(stop.id.to_string()),
        from: if flat {
            parent.map(|p| p.to_string())
        } else {
            None
        },
        line: stop.line,
        same_as: stop.same_as.map(|s| s.to_string()),
        node: now
            .get(&stop.id)
            .cloned()
            .unwrap_or_else(|| node_name(stop)),
        text: stop.text.clone().unwrap_or_default(),
        blocks,
        stops: Vec::new(),
    }
}

/// The nearest stop above this one on the route that is written (a planned stop is not in the document), by `from`.
fn written_parent(tour: &Tour, stop: &TourStop) -> Option<StopId> {
    let mut at = stop.from;
    for _ in 0..=tour.stops.len() {
        let above = tour.stop(at?)?;
        if above.written {
            return Some(above.id);
        }
        at = above.from;
    }
    None
}

/// The tour as a document: the written stops, nested by `from` (or listed flat with `from` and `parent`), their blocks nested by `parent`. `now` has the
/// name a stop is found under now, where it is found (`tours::find`), for the stops whose name changed or whose node is found by its ID; any other stop
/// keeps the name it was written with. Every id is kept: it is what `import --replace` matches by.
pub fn nest(tour: &Tour, now: &BTreeMap<StopId, String>, flat: bool) -> Document {
    let written: Vec<&TourStop> = tour.stops.iter().filter(|s| s.written).collect();
    let stops = if flat {
        written
            .iter()
            .map(|s| doc_stop(s, now, true, written_parent(tour, s)))
            .collect()
    } else {
        // the route is depth-first, so each stop's parent is above it: build from the bottom up
        fn build(
            tour: &Tour,
            written: &[&TourStop],
            now: &BTreeMap<StopId, String>,
            parent: Option<StopId>,
        ) -> Vec<DocStop> {
            written
                .iter()
                .filter(|s| written_parent(tour, s) == parent)
                .map(|s| {
                    let mut one = doc_stop(s, now, false, None);
                    one.stops = build(tour, written, now, Some(s.id));
                    one
                })
                .collect()
        }
        build(tour, &written, now, None)
    };
    Document {
        title: tour.title.clone(),
        question: tour.question.clone(),
        author: tour.author.clone(),
        detail: Some(tour.detail),
        stops,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------- print

fn s(text: &str) -> String {
    serde_json::to_string(text).expect("text is JSON")
}

fn print_block(out: &mut String, block: &DocBlock, indent: usize) {
    let pad = " ".repeat(indent);
    let mut parts: Vec<String> = Vec::new();
    if let Some(id) = &block.id {
        parts.push(format!("\"id\": {}", s(id)));
    }
    if let Some(parent) = &block.parent {
        parts.push(format!("\"parent\": {}", s(parent)));
    }
    parts.push(format!(
        "\"lines\": [{}, {}]",
        block.lines[0], block.lines[1]
    ));
    parts.push(format!("\"title\": {}", s(&block.title)));
    parts.push(format!("\"text\": {}", s(&block.text)));
    out.push_str(&format!("{pad}{{ {}", parts.join(", ")));
    if block.blocks.is_empty() {
        out.push_str(" }");
        return;
    }
    out.push_str(",\n");
    out.push_str(&format!("{pad}  \"blocks\": [\n"));
    for (i, inner) in block.blocks.iter().enumerate() {
        print_block(out, inner, indent + 4);
        out.push_str(if i + 1 < block.blocks.len() {
            ",\n"
        } else {
            "\n"
        });
    }
    out.push_str(&format!("{pad}  ] }}"));
}

fn print_stop(out: &mut String, stop: &DocStop, indent: usize) {
    let pad = " ".repeat(indent);
    let mut head: Vec<String> = Vec::new();
    if let Some(id) = &stop.id {
        head.push(format!("\"id\": {}", s(id)));
    }
    if let Some(from) = &stop.from {
        head.push(format!("\"from\": {}", s(from)));
    }
    if let Some(line) = stop.line {
        head.push(format!("\"line\": {line}"));
    }
    if let Some(same_as) = &stop.same_as {
        head.push(format!("\"same_as\": {}", s(same_as)));
    }
    head.push(format!("\"node\": {}", s(&stop.node)));
    out.push_str(&format!("{pad}{{ {},\n", head.join(", ")));
    out.push_str(&format!("{pad}  \"text\": {}", s(&stop.text)));
    if !stop.blocks.is_empty() {
        out.push_str(&format!(",\n{pad}  \"blocks\": [\n"));
        for (i, block) in stop.blocks.iter().enumerate() {
            print_block(out, block, indent + 4);
            out.push_str(if i + 1 < stop.blocks.len() {
                ",\n"
            } else {
                "\n"
            });
        }
        out.push_str(&format!("{pad}  ]"));
    }
    if !stop.stops.is_empty() {
        out.push_str(&format!(",\n{pad}  \"stops\": [\n"));
        for (i, inner) in stop.stops.iter().enumerate() {
            print_stop(out, inner, indent + 4);
            out.push_str(if i + 1 < stop.stops.len() {
                ",\n"
            } else {
                "\n"
            });
        }
        out.push_str(&format!("{pad}  ]"));
    }
    out.push_str(" }");
}

/// The document as text: two spaces, the keys in the order of 02 §10's example, a pair of lines on one line, a newline at the end.
pub fn print(document: &Document) -> String {
    let mut out = String::from("{\n  \"schema\": 1,\n");
    out.push_str(&format!("  \"title\": {},\n", s(&document.title)));
    if let Some(question) = &document.question {
        out.push_str(&format!("  \"question\": {},\n", s(question)));
    }
    if let Some(author) = &document.author {
        out.push_str(&format!("  \"author\": {},\n", s(author)));
    }
    if let Some(detail) = document.detail {
        let word = match detail {
            Detail::Quick => "quick",
            Detail::Normal => "normal",
            Detail::Detailed => "detailed",
        };
        out.push_str(&format!("  \"detail\": \"{word}\",\n"));
    }
    out.push_str("  \"stops\": [\n");
    for (i, stop) in document.stops.iter().enumerate() {
        print_stop(&mut out, stop, 4);
        out.push_str(if i + 1 < document.stops.len() {
            ",\n"
        } else {
            "\n"
        });
    }
    out.push_str("  ]\n}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tours::fold::fold;

    const HEAD: &str = r#"{"type":"tour","schema":1,"id":"t-0000","title":"T","question":"q","author":"me","at":"x","detail":"normal"}"#;

    fn stop(n: u32, from: Option<(u32, u32)>, same_as: Option<u32>) -> String {
        let from = from.map_or(String::new(), |(f, l)| {
            format!(r#","from":"s{f}","line":{l}"#)
        });
        let same = same_as.map_or(String::new(), |s| format!(r#","same_as":"s{s}""#));
        format!(
            r#"{{"type":"stop","id":"s{n}","path":"a.py","qualname":"f{n}","kind":"function","lines":[1,20],"hash":"{n:016x}"{from}{same},"text":"intro {n}"}}"#
        )
    }

    fn step(n: u32, m: u32, parent: Option<u32>) -> String {
        let parent = parent.map_or(String::new(), |p| format!(r#","parent":"s{n}.b{p}""#));
        format!(
            r#"{{"type":"step","id":"s{n}.b{m}","stop":"s{n}"{parent},"lines":[{m},{}],"hash":"{m:016x}","title":"b{m}","text":"block {m}"}}"#,
            m + 3
        )
    }

    fn tour(lines: &[String]) -> Tour {
        fold(&format!("{HEAD}\n{}\n", lines.join("\n"))).unwrap()
    }

    /// Stops three deep and details: s1 > s2 > s3, s1 > s4, s5; s1 has b1 > b2, b3; s3 has b1.
    fn deep() -> Tour {
        tour(&[
            stop(1, None, None),
            step(1, 1, None),
            step(1, 2, Some(1)),
            step(1, 3, None),
            stop(2, Some((1, 3)), None),
            stop(3, Some((2, 4)), Some(1)),
            step(3, 1, None),
            stop(4, Some((1, 5)), None),
            stop(5, None, None),
        ])
    }

    #[test]
    fn nest_then_flatten_is_the_identity_on_a_tour_three_deep_with_details() {
        let t = deep();
        for flat in [false, true] {
            let document = nest(&t, &BTreeMap::new(), flat);
            let text = print(&document);
            let read = parse(&text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
            assert_eq!(read, document, "printed and read back, flat: {flat}");
            let f = flatten(&read).unwrap();
            // the route's order, the nesting as places, the blocks in reading order
            let ids: Vec<&str> = f.stops.iter().map(|s| s.id.as_deref().unwrap()).collect();
            assert_eq!(ids, ["s1", "s2", "s3", "s4", "s5"], "flat: {flat}");
            let froms: Vec<Option<usize>> = f.stops.iter().map(|s| s.from).collect();
            assert_eq!(froms, [None, Some(0), Some(1), Some(0), None]);
            assert_eq!(f.stops[2].same_as, Some(0));
            assert_eq!(f.stops[1].line, Some(3));
            let blocks: Vec<(&str, Option<usize>)> = f.stops[0]
                .blocks
                .iter()
                .map(|b| (b.id.as_deref().unwrap(), b.parent))
                .collect();
            assert_eq!(
                blocks,
                [("s1.b1", None), ("s1.b2", Some(0)), ("s1.b3", None)]
            );
            assert_eq!(
                (f.title.as_str(), f.question.as_deref(), f.author.as_deref()),
                ("T", Some("q"), Some("me"))
            );
        }
    }

    #[test]
    fn the_flat_and_the_nested_document_are_one_tour() {
        let t = deep();
        let a = flatten(&nest(&t, &BTreeMap::new(), false)).unwrap();
        let b = flatten(&nest(&t, &BTreeMap::new(), true)).unwrap();
        // the places say where each was written; the rest is the same
        let strip = |f: &Flat| {
            let mut f = f.clone();
            for s in &mut f.stops {
                s.place.clear();
                for b in &mut s.blocks {
                    b.place.clear();
                }
            }
            f
        };
        assert_eq!(strip(&a), strip(&b));
    }

    #[test]
    fn a_flat_document_is_put_in_depth_first_order_and_a_late_nested_stop_sits_after_its_siblings()
    {
        // s1, s2 at the top, then s3 from s1: the route is s1, s3, s2
        let text = r#"{"title":"t","stops":[
            {"id":"a","node":"a.py::f","text":"x"},
            {"id":"b","node":"a.py::g","text":"x"},
            {"id":"c","from":"a","line":3,"node":"a.py::h","text":"x"}]}"#;
        let f = flatten(&parse(text).unwrap()).unwrap();
        let ids: Vec<&str> = f.stops.iter().map(|s| s.id.as_deref().unwrap()).collect();
        assert_eq!(ids, ["a", "c", "b"]);
        assert_eq!(f.stops[1].from, Some(0));
    }

    fn places(text: &str) -> Vec<(String, String)> {
        let err = parse(text)
            .and_then(|d| flatten(&d))
            .expect_err("a refusal");
        err.into_iter().map(|r| (r.place, r.problem)).collect()
    }

    #[test]
    fn a_typo_key_is_refused_with_its_place_at_every_level() {
        let said = places(
            r#"{"title":"t","stops":[{"node":"a.py::f","text":"x","blokcs":[],"stops":[{"node":"a.py::g","text":"x","blocks":[{"lines":[1,2],"title":"t","text":"x","tittle":"y"}]}]}],"questoin":"q"}"#,
        );
        let at: Vec<&str> = said.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            at,
            [
                "questoin",
                "stops[0].blokcs",
                "stops[0].stops[0].blocks[0].tittle"
            ]
        );
        assert!(
            said[1].1.starts_with(
                "stops[0].blokcs: `blokcs` is not a key of a stop: the keys are id, from, line,"
            ),
            "{}",
            said[1].1
        );
    }

    #[test]
    fn what_is_missing_or_the_wrong_kind_is_refused_with_its_place() {
        let said = places(
            r#"{"title":3,"stops":[{"node":"a.py::f"},{"node":7,"text":"x","line":"2","blocks":[{"lines":[3,1],"title":"t","text":"x"},{"lines":[1],"title":"t"}]}]}"#,
        );
        let at: Vec<&str> = said.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            at,
            [
                "title",
                "stops[0].text",
                "stops[1].node",
                "stops[1].line",
                "stops[1].blocks[0].lines",
                "stops[1].blocks[1].text",
                "stops[1].blocks[1].lines"
            ]
        );
        assert_eq!(said[2].1, "stops[1].node: this is a number, not text");
        for bad in [
            "",
            "[]",
            "3",
            r#"{"title":"t"}"#,
            r#"{"title":"t","stops":[]}"#,
            r#"{"title":"t","schema":2,"stops":[{"node":"a","text":"x"}]}"#,
            "not json",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_document_over_one_megabyte_is_refused_before_it_is_parsed() {
        let big = format!(r#"{{"title":"{}"}}"#, "x".repeat(MAX_BYTES));
        let said = parse(&big).unwrap_err();
        assert_eq!(said.len(), 1);
        assert!(
            said[0].problem.starts_with("document: the document is "),
            "{}",
            said[0].problem
        );
        // not JSON would be the other refusal: the size came first
        assert!(!said[0].problem.contains("not JSON"));
    }

    #[test]
    fn a_nesting_and_a_from_that_disagree_are_refused_and_so_is_a_same_as_that_names_a_later_stop()
    {
        let said = places(
            r#"{"title":"t","stops":[
                {"id":"a","node":"a.py::f","text":"x","stops":[
                    {"id":"b","from":"c","line":1,"node":"a.py::g","text":"x"}]},
                {"id":"c","node":"a.py::h","text":"x","same_as":"d"},
                {"id":"d","node":"a.py::i","text":"x","same_as":"nope"},
                {"id":"d","node":"a.py::j","text":"x","from":"later"}]}"#,
        );
        let at: Vec<&str> = said.iter().map(|(p, _)| p.as_str()).collect();
        assert!(at.contains(&"stops[0].stops[0].from"), "{at:?}");
        assert!(at.contains(&"stops[3].id"), "a duplicate id: {at:?}");
        assert!(at.contains(&"stops[3].from"), "{at:?}");
        let same_as: Vec<&String> = said
            .iter()
            .filter(|(p, _)| p == "stops[1].same_as")
            .map(|(_, t)| t)
            .collect();
        assert_eq!(same_as.len(), 1, "{said:?}");
        assert!(
            same_as[0].contains("comes earlier in the reading order"),
            "{}",
            same_as[0]
        );
        assert!(
            said.iter()
                .any(|(p, t)| p == "stops[2].same_as" && t.contains("is not the id of any stop"))
        );
        // a `from` that names the stop it is nested in is no disagreement
        assert!(flatten(&parse(r#"{"title":"t","stops":[{"id":"a","node":"a.py::f","text":"x","stops":[{"from":"a","line":1,"node":"a.py::g","text":"x"}]}]}"#).unwrap()).is_ok());
    }

    #[test]
    fn details_nest_one_level_and_a_block_parent_must_agree_with_its_nesting() {
        let said = places(
            r#"{"title":"t","stops":[{"node":"a.py::f","text":"x","blocks":[
                {"id":"b1","lines":[1,9],"title":"t","text":"x","blocks":[
                    {"id":"b2","lines":[1,5],"title":"t","text":"x","blocks":[
                        {"lines":[1,2],"title":"t","text":"x"}]}]},
                {"id":"b3","parent":"b2","lines":[10,12],"title":"t","text":"x"}]}]}"#,
        );
        let at: Vec<&str> = said.iter().map(|(p, _)| p.as_str()).collect();
        assert!(
            at.contains(&"stops[0].blocks[0].blocks[0].blocks[0]"),
            "{at:?}"
        );
        assert!(
            at.contains(&"stops[0].blocks[1]"),
            "a parent that is a detail: {at:?}"
        );
        let said = places(
            r#"{"title":"t","stops":[{"node":"a.py::f","text":"x","blocks":[
                {"id":"b1","lines":[1,9],"title":"t","text":"x","blocks":[
                    {"parent":"b2","lines":[1,5],"title":"t","text":"x"}]},
                {"id":"b2","lines":[10,12],"title":"t","text":"x"}]}]}"#,
        );
        let at: Vec<&str> = said.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(at, ["stops[0].blocks[0].blocks[0].parent"], "{said:?}");
    }

    #[test]
    fn a_stop_is_printed_with_its_keys_in_the_order_of_the_example() {
        let t = deep();
        let text = print(&nest(&t, &BTreeMap::new(), false));
        assert!(text.starts_with("{\n  \"schema\": 1,\n  \"title\": \"T\",\n  \"question\": \"q\",\n  \"author\": \"me\",\n  \"detail\": \"normal\",\n  \"stops\": [\n    { \"id\": \"s1\", \"node\": \"a.py::f1\",\n      \"text\": \"intro 1\",\n      \"blocks\": [\n        { \"id\": \"s1.b1\", \"lines\": [1, 4], \"title\": \"b1\", \"text\": \"block 1\",\n"), "{text}");
        assert!(text.ends_with("  ]\n}\n"));
        assert!(text.contains("{ \"id\": \"s2\", \"line\": 3, \"node\": \"a.py::f2\","));
        assert!(text.contains(
            "{ \"id\": \"s3\", \"line\": 4, \"same_as\": \"s1\", \"node\": \"a.py::f3\","
        ));
    }

    #[test]
    fn a_planned_stop_is_not_in_the_document_and_what_is_nested_in_it_moves_up() {
        let plan =
            r#"{"type":"plan","stops":[{"id":"s2","path":"a.py","qualname":"p"}]}"#.to_string();
        let t = tour(&[stop(1, None, None), plan, stop(3, Some((2, 4)), None)]);
        let doc = nest(&t, &BTreeMap::new(), false);
        // s3 is nested in s2, which is planned: it has no written stop above it, so it is at the top
        let ids = |d: &Document| {
            d.stops
                .iter()
                .map(|s| s.id.clone().unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&doc), ["s1", "s3"]);
        assert_eq!(doc.stops[1].line, Some(4));
        let flat = nest(&t, &BTreeMap::new(), true);
        assert_eq!(flat.stops.len(), 2);
        assert_eq!(flat.stops[1].from, None);
    }
}
