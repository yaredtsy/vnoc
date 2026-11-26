//! A tour as one document, for the commands and the requests (`docs/26-tours/02-commands.md` §10): `vnoc tour export | import` and `vnoc/tourExport |
//! tourImport` call these and nothing else, so the rules and the words are here. The nesting itself is [`super::doc`]; the checks of every stop are
//! [`super::add::plan_tour`]. What this module adds is what a caller needs round them: the name each stop has now, the write (a new tour, or
//! `--replace` under the tours lock), the sentence for what was written, where a refusal is in the text the person wrote, and the sentence a person
//! pastes to an agent to ask for a tour.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::act::{self, ActError};
use super::doc;
use super::find::find;
use super::fold::State;
use super::record::{Detail, EndRecord, Record, StopId, TourId};
use super::rules::Refusal;
use super::store::{self, StartParams, StoreError};
use super::{Tour, time};
use crate::index::{Index, Node};
use crate::node_ref::{self, Found, Why};
use crate::project::Project;
use crate::userdata::ToursLock;

// ------------------------------------------------------------------------------------------------------------------------------------ export

/// A stop's name as the document says it, for a stop that is not found now: the name it was written with, in the opened folder's terms (the path is the
/// repo's; a project opened below its git root shows paths without the prefix).
pub fn written_name(project: &Project, stop: &super::TourStop) -> String {
    let name = doc::node_name(stop);
    match project.prefix() {
        "" => name,
        prefix => name
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_prefix('/'))
            .map(str::to_string)
            .unwrap_or(name),
    }
}

/// The nested document of the tour as it is now, as text (`--flat`: V-NOC's own shape). With the index, a stop that is found is written with the name it
/// has now; with none (`None`: no driver, or no index yet) every stop keeps the name it was written with.
pub fn export(
    project: &Project,
    index: Option<&Index>,
    vnoc_root: &Path,
    id: &TourId,
    flat: bool,
) -> Result<String, ActError> {
    act::not_in_both(vnoc_root, id)?;
    let tour = act::read_tour(vnoc_root, id)?;
    let mut now: BTreeMap<StopId, String> = tour
        .stops
        .iter()
        .filter(|s| s.written)
        .map(|s| (s.id, written_name(project, s)))
        .collect();
    if let Some(index) = index {
        let tree = index.tree(&project.name(), project.prefix());
        for stop in tour.stops.iter().filter(|s| s.written) {
            if let Some(node) = find(index, project, &tree, stop) {
                now.insert(stop.id, node.location());
            }
        }
    }
    Ok(doc::print(&doc::nest(&tour, &now, flat)))
}

// ------------------------------------------------------------------------------------------------------------------------------------ import

/// Why a document was not written: every refusal of it at once (nothing was written), or the store's and the disk's.
#[derive(Debug)]
pub enum ImportError {
    Refused(Vec<Refusal>),
    Act(ActError),
}

impl From<ActError> for ImportError {
    fn from(e: ActError) -> ImportError {
        ImportError::Act(e)
    }
}

impl From<StoreError> for ImportError {
    fn from(e: StoreError) -> ImportError {
        ImportError::Act(ActError::Store(e))
    }
}

/// What an import wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub id: TourId,
    pub replaced: bool,
    /// with `--replace`: the written stops of the old tour that are in the new one, and the ones that are not
    pub kept: Option<usize>,
    pub gone: Option<usize>,
}

/// Writes the document `flat`: a new tour, or (`replace`, with the tours lock the caller holds) that tour made anew. Every stop is checked against the code in
/// `index` as it is now, all refusals at once; nothing is written if there is one. The tour is written finished: what `finish` does after `end` is the
/// caller's.
///
/// `resolve` is the caller's way of reading a stop's `node` (`file::Name`, `file:line`, an ID, a path): the sentence it gives is the refusal.
pub fn write_import<'t>(
    project: &Project,
    index: &Index,
    vnoc_root: &Path,
    resolve: &dyn Fn(&str) -> Result<&'t Node, String>,
    flat: &doc::Flat,
    replace: Option<(&TourId, &ToursLock)>,
) -> Result<Written, ImportError> {
    match replace {
        Some((id, lock)) => {
            act::not_in_both(vnoc_root, id)?;
            let old = act::read_tour(vnoc_root, id)?;
            if matches!(old.state, State::Writing | State::Stopping) {
                return Err(StoreError::Refused(format!(
                    "the tour {id} is being written ({}): stop it or finish it first",
                    old.state
                ))
                .into());
            }
            let mut records = super::add::plan_tour(index, project, resolve, flat, Some(&old))
                .map_err(ImportError::Refused)?;
            records.push(Record::End(EndRecord { at: time::now() }));
            store::replace_locked(lock, vnoc_root, id, &records)?;
            let kept_ids: Vec<StopId> = records
                .iter()
                .filter_map(|r| match r {
                    Record::Stop(stop) if old.stop(stop.id).is_some_and(|o| o.written) => {
                        Some(stop.id)
                    }
                    _ => None,
                })
                .collect();
            let gone = old
                .stops
                .iter()
                .filter(|s| s.written && !kept_ids.contains(&s.id))
                .count();
            let kept = kept_ids.len();
            Ok(Written {
                id: id.clone(),
                replaced: true,
                kept: Some(kept),
                gone: Some(gone),
            })
        }
        None => {
            let mut records = super::add::plan_tour(index, project, resolve, flat, None)
                .map_err(ImportError::Refused)?;
            records.push(Record::End(EndRecord { at: time::now() }));
            let started = store::start(
                project.git_root(),
                StartParams {
                    title: flat.title.trim().to_string(),
                    question: flat.question.as_ref().map(|q| q.trim().to_string()),
                    author: flat.author.clone(),
                    detail: flat.detail.unwrap_or(Detail::Normal),
                },
            )?;
            store::append(vnoc_root, &started.id, &records)?;
            Ok(Written {
                id: started.id,
                replaced: false,
                kept: None,
                gone: None,
            })
        }
    }
}

/// *"Imported “checkout_order”: 10 stops."* · *"Replaced “checkout_order”: 10 stops (8 kept, 2 new, 1 gone)."*
pub fn import_words(title: &str, written: &Written, stops: usize) -> String {
    let stops_said = format!("{stops} {}", if stops == 1 { "stop" } else { "stops" });
    match (written.replaced, written.kept, written.gone) {
        (true, Some(kept), Some(gone)) => format!(
            "Replaced “{title}”: {stops_said} ({kept} kept, {} new, {gone} gone).",
            stops.saturating_sub(kept)
        ),
        _ => format!("Imported “{title}”: {stops_said}."),
    }
}

/// How the node a request or a document names is read when there is no current folder: the references `node_ref` takes (`path::qualname`, `path:line`, an
/// ID, a path), in `tree`. The sentence is the refusal's: a short one, from the reason the index gives (the command line's gives more, with "did you
/// mean").
pub fn resolve_node<'t>(
    index: &Index,
    project: &Project,
    tree: &'t Node,
    text: &str,
) -> Result<&'t Node, String> {
    let Some(written) = node_ref::parse(text) else {
        return Err("an empty reference names no node".to_string());
    };
    match index.resolve_ref(project, &written) {
        Found::Node(id) => tree
            .find(&id)
            .ok_or_else(|| format!("{text} is not a node of this project's tree")),
        Found::Missing(_) => Err(format!(
            "{text} is Missing: its code is gone. A stop is about code that is there"
        )),
        Found::Nothing => Err(match index.why_not(project, &written) {
            Some(Why::NoDef { path, name }) if name.is_empty() => format!("no node at {path}"),
            Some(Why::NoDef { path, name }) => format!("no node at {path}::{name}"),
            Some(Why::NoFile(path)) => format!("no file {path}"),
            Some(Why::NotRead(path)) => {
                format!("{path} is not read: not a source file V-NOC reads, or ignored by git")
            }
            Some(Why::Failed { path, message }) => format!("{path} could not be read: {message}"),
            Some(Why::Ambiguous { name, files }) => {
                format!(
                    "{name} is in several files: name one of {}",
                    files.join(" · ")
                )
            }
            Some(Why::PastEnd { path, lines }) => {
                format!("{path} has {lines} lines: the reference names a later one")
            }
            Some(Why::NoId(id)) => format!("no node has the ID {id}"),
            Some(Why::Outside(path)) => format!("{path} is not under the project"),
            None => format!("{text} names no node"),
        }),
    }
}

// ------------------------------------------------------------------------------------------------------------------------------ where in the text

/// The 1-based line of the text a refusal is about. `place` is a path in the document as the refusals give it (`stops[0].stops[1].blocks[0].text`); one
/// whose key is not in the text (a missing `text`) is the line of the closest thing above it that is; `document` is the first line, except for text that
/// is not JSON, where the parser's own line is taken from its sentence. `None` when the text has nothing at all to point at.
pub fn line_of(text: &str, place: &str, problem: &str) -> Option<u32> {
    if let Some(line) = problem
        .split("at line ")
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .and_then(|n| n.parse().ok())
    {
        return Some(line);
    }
    let mut segments = path_of(place)?;
    loop {
        if let Some(line) = Scan::new(text).find(&segments) {
            return Some(line);
        }
        segments.pop()?;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Seg {
    Key(String),
    At(usize),
}

/// `stops[0].blocks[1].text` as its segments; `None` for a place that is not a path (`document`).
fn path_of(place: &str) -> Option<Vec<Seg>> {
    if place == "document" || place.is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for part in place.split('.') {
        let (name, mut rest) = match part.find('[') {
            Some(i) => (&part[..i], &part[i..]),
            None => (part, ""),
        };
        if !name.is_empty() {
            out.push(Seg::Key(name.to_string()));
        }
        while let Some(inner) = rest.strip_prefix('[') {
            let (n, after) = inner.split_once(']')?;
            out.push(Seg::At(n.parse().ok()?));
            rest = after;
        }
        if !rest.is_empty() {
            return None;
        }
    }
    Some(out)
}

/// A walk over JSON text that counts lines: no values are built.
struct Scan<'a> {
    bytes: &'a [u8],
    at: usize,
    line: u32,
}

impl<'a> Scan<'a> {
    fn new(text: &'a str) -> Scan<'a> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        Scan {
            bytes: text.as_bytes(),
            at: 0,
            line: 1,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn blanks(&mut self) {
        while let Some(b) = self.peek() {
            match b {
                b'\n' => self.line += 1,
                b' ' | b'\t' | b'\r' => {}
                _ => break,
            }
            self.at += 1;
        }
    }

    /// A string at the cursor, its text with the escapes of `\"` and `\\` read (a key is compared with it; nothing else needs them).
    fn string(&mut self) -> Option<String> {
        if self.peek()? != b'"' {
            return None;
        }
        self.at += 1;
        let mut out = Vec::new();
        loop {
            let b = self.peek()?;
            self.at += 1;
            match b {
                b'"' => return Some(String::from_utf8_lossy(&out).into_owned()),
                b'\\' => {
                    let next = self.peek()?;
                    self.at += 1;
                    out.push(next);
                }
                b'\n' => {
                    self.line += 1;
                    out.push(b);
                }
                _ => out.push(b),
            }
        }
    }

    /// Past the value at the cursor.
    fn skip(&mut self) -> Option<()> {
        self.blanks();
        match self.peek()? {
            b'"' => {
                self.string()?;
            }
            open @ (b'{' | b'[') => {
                self.at += 1;
                let close = if open == b'{' { b'}' } else { b']' };
                self.blanks();
                if self.peek()? == close {
                    self.at += 1;
                    return Some(());
                }
                loop {
                    if open == b'{' {
                        self.blanks();
                        self.string()?;
                        self.blanks();
                        if self.peek()? != b':' {
                            return None;
                        }
                        self.at += 1;
                    }
                    self.skip()?;
                    self.blanks();
                    match self.peek()? {
                        b',' => self.at += 1,
                        c if c == close => {
                            self.at += 1;
                            return Some(());
                        }
                        _ => return None,
                    }
                }
            }
            _ => {
                while let Some(b) = self.peek() {
                    if matches!(b, b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n') {
                        break;
                    }
                    self.at += 1;
                }
            }
        }
        Some(())
    }

    /// The line the value at `path` starts on, below the value at the cursor.
    fn find(&mut self, path: &[Seg]) -> Option<u32> {
        self.blanks();
        let Some((first, rest)) = path.split_first() else {
            return Some(self.line);
        };
        match first {
            Seg::Key(wanted) => {
                if self.peek()? != b'{' {
                    return None;
                }
                self.at += 1;
                loop {
                    self.blanks();
                    if self.peek()? == b'}' {
                        return None;
                    }
                    let key = self.string()?;
                    self.blanks();
                    if self.peek()? != b':' {
                        return None;
                    }
                    self.at += 1;
                    if &key == wanted {
                        return self.find(rest);
                    }
                    self.skip()?;
                    self.blanks();
                    match self.peek()? {
                        b',' => self.at += 1,
                        _ => return None,
                    }
                }
            }
            Seg::At(n) => {
                if self.peek()? != b'[' {
                    return None;
                }
                self.at += 1;
                for i in 0.. {
                    self.blanks();
                    if self.peek()? == b']' {
                        return None;
                    }
                    if i == *n {
                        return self.find(rest);
                    }
                    self.skip()?;
                    self.blanks();
                    match self.peek()? {
                        b',' => self.at += 1,
                        _ => return None,
                    }
                }
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------- the ask

/// The sentence for an agent, and the line that says what to do with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub text: String,
    pub words: &'static str,
}

/// How far a tour goes from its node, as the person chose it in the form (`vnoc/tourAsk`'s `reach`). `Agent` says nothing: the agent decides.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reach {
    #[default]
    Agent,
    Only,
    Calls,
    Calls2,
}

/// How much each step says, as the person chose it (`vnoc/tourAsk`'s `detail`). `Agent` says nothing: the agent chooses from the question.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AskDetail {
    #[default]
    Agent,
    Quick,
    Normal,
    Detailed,
}

impl AskDetail {
    /// A tour's own detail as a choice: `normal` is what nobody chose, so it is the agent's.
    pub fn of_tour(detail: Detail) -> AskDetail {
        match detail {
            Detail::Quick => AskDetail::Quick,
            Detail::Normal => AskDetail::Agent,
            Detail::Detailed => AskDetail::Detailed,
        }
    }
}

/// What the form of **Ask for a Tour** has: every part may be empty.
#[derive(Debug, Clone, Copy, Default)]
pub struct Parts<'a> {
    pub address: Option<&'a str>,
    pub question: Option<&'a str>,
    pub reach: Reach,
    pub detail: AskDetail,
}

/// The sentence from its parts (design/07 §7, TR23a): `Walk me through orders/service.py::OrderService.parse_order: why does it retry twice? Cover it and
/// what it calls. Make it a quick tour.`, and *"Copied. Paste it to your agent."*. With no node, the question alone follows `Walk me through `; with neither,
/// the text is `Walk me through ` and the words say *"Add what you want explained."* (a reach and a detail have nothing to be about, so they are not added).
/// Whitespace in the question is one space, so the sentence is one line.
pub fn ask(parts: &Parts) -> Asked {
    let address = parts.address.map(str::trim).filter(|a| !a.is_empty());
    let question = parts
        .question
        .map(|q| q.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|q| !q.is_empty());
    let subject = match (address, &question) {
        (Some(a), Some(q)) => format!("{a}: {q}"),
        (Some(a), None) => a.to_string(),
        (None, Some(q)) => q.clone(),
        (None, None) => {
            return Asked {
                text: "Walk me through ".to_string(),
                words: "Add what you want explained.",
            };
        }
    };
    let mut text = format!("Walk me through {subject}");
    let reach = match parts.reach {
        Reach::Agent => None,
        Reach::Only => Some("Cover only that, not what it calls."),
        Reach::Calls => Some("Cover it and what it calls."),
        Reach::Calls2 => Some("Cover it, what it calls, and what those call."),
    };
    let detail = match parts.detail {
        AskDetail::Agent => None,
        AskDetail::Quick => Some("Make it a quick tour."),
        AskDetail::Normal => Some("Make it a normal tour."),
        AskDetail::Detailed => Some("Make it a detailed tour."),
    };
    for sentence in reach.into_iter().chain(detail) {
        if !text.ends_with(['.', '?', '!']) {
            text.push('.');
        }
        text.push(' ');
        text.push_str(sentence);
    }
    Asked {
        text,
        words: "Copied. Paste it to your agent.",
    }
}

/// A row's **Ask for a New Tour**: what that tour is of (the address of its root), its question and its detail, as the form's first values (for a changed
/// tour: "make this again").
pub fn of_tour<'a>(tour: &'a Tour, address: Option<&'a str>) -> Parts<'a> {
    Parts {
        address,
        question: tour.question.as_deref(),
        reach: Reach::Agent,
        detail: AskDetail::of_tour(tour.detail),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "{\n  \"schema\": 1,\n  \"title\": \"T\",\n  \"stops\": [\n    { \"node\": \"a.py::f\",\n      \"text\": \"one\",\n      \"blocks\": [\n        { \"lines\": [1, 2],\n          \"text\": \"two\" }\n      ],\n      \"stops\": [\n        { \"node\": \"b.py::g\", \"text\": \"three\" }\n      ] }\n  ]\n}\n";

    #[test]
    fn a_place_in_the_document_is_the_line_it_starts_on() {
        let at = |place: &str| line_of(TEXT, place, "");
        assert_eq!(at("title"), Some(3));
        assert_eq!(at("stops[0]"), Some(5));
        assert_eq!(at("stops[0].text"), Some(6));
        assert_eq!(at("stops[0].blocks[0].text"), Some(9));
        assert_eq!(at("stops[0].blocks[0].lines"), Some(8));
        assert_eq!(at("stops[0].stops[0].node"), Some(12));
        assert_eq!(at("document"), Some(1));
    }

    #[test]
    fn a_place_whose_key_is_not_in_the_text_is_the_line_of_what_holds_it() {
        // no `question` in the document: the document; no `title` in the stop's first block: the block
        assert_eq!(line_of(TEXT, "stops[0].blocks[0].title", ""), Some(8));
        assert_eq!(line_of(TEXT, "question", ""), Some(1));
        assert_eq!(line_of(TEXT, "stops[9].text", ""), Some(4));
    }

    #[test]
    fn text_that_is_not_json_has_the_parsers_line() {
        let said = "document: this is not JSON: expected `,` or `}` at line 7 column 3";
        assert_eq!(line_of("{\n\n\n\n\n\n  x", "document", said), Some(7));
    }

    fn said(
        address: Option<&str>,
        question: Option<&str>,
        reach: Reach,
        detail: AskDetail,
    ) -> String {
        ask(&Parts {
            address,
            question,
            reach,
            detail,
        })
        .text
    }

    #[test]
    fn the_ask_sentence_has_the_address_or_waits_for_one() {
        let with = ask(&Parts {
            address: Some("app/inventory.py::reserve_stock"),
            ..Parts::default()
        });
        assert_eq!(with.text, "Walk me through app/inventory.py::reserve_stock");
        assert_eq!(with.words, "Copied. Paste it to your agent.");
        let without = ask(&Parts::default());
        assert_eq!(without.text, "Walk me through ");
        assert_eq!(without.words, "Add what you want explained.");
        // blanks are nothing
        assert_eq!(
            ask(&Parts {
                address: Some("  "),
                question: Some(" \n "),
                ..Parts::default()
            }),
            without
        );
    }

    #[test]
    fn the_question_follows_the_address_after_a_colon_on_one_line() {
        let at = Some("orders/service.py::OrderService.parse_order");
        assert_eq!(
            said(
                at,
                Some("why does it retry twice?"),
                Reach::Agent,
                AskDetail::Agent
            ),
            "Walk me through orders/service.py::OrderService.parse_order: why does it retry twice?"
        );
        assert_eq!(
            said(at, Some("  why\n  twice  "), Reach::Agent, AskDetail::Agent),
            "Walk me through orders/service.py::OrderService.parse_order: why twice"
        );
    }

    #[test]
    fn with_no_node_the_question_alone_follows_walk_me_through() {
        let asked = ask(&Parts {
            question: Some("how does checkout end?"),
            ..Parts::default()
        });
        assert_eq!(asked.text, "Walk me through how does checkout end?");
        assert_eq!(asked.words, "Copied. Paste it to your agent.");
        // a reach and a detail are about a subject: with neither node nor question they are not added
        assert_eq!(
            said(None, None, Reach::Calls, AskDetail::Quick),
            "Walk me through "
        );
        // with only a question they are
        assert_eq!(
            said(None, Some("why twice?"), Reach::Calls, AskDetail::Quick),
            "Walk me through why twice? Cover it and what it calls. Make it a quick tour."
        );
    }

    #[test]
    fn each_reach_says_its_own_sentence_and_the_agents_says_nothing() {
        let at = Some("a.py::f");
        let reach = |r| said(at, None, r, AskDetail::Agent);
        assert_eq!(reach(Reach::Agent), "Walk me through a.py::f");
        assert_eq!(
            reach(Reach::Only),
            "Walk me through a.py::f. Cover only that, not what it calls."
        );
        assert_eq!(
            reach(Reach::Calls),
            "Walk me through a.py::f. Cover it and what it calls."
        );
        assert_eq!(
            reach(Reach::Calls2),
            "Walk me through a.py::f. Cover it, what it calls, and what those call."
        );
    }

    #[test]
    fn each_detail_says_its_own_sentence_and_the_agents_says_nothing() {
        let at = Some("a.py::f");
        let detail = |d| said(at, Some("why?"), Reach::Agent, d);
        assert_eq!(detail(AskDetail::Agent), "Walk me through a.py::f: why?");
        assert_eq!(
            detail(AskDetail::Quick),
            "Walk me through a.py::f: why? Make it a quick tour."
        );
        assert_eq!(
            detail(AskDetail::Normal),
            "Walk me through a.py::f: why? Make it a normal tour."
        );
        assert_eq!(
            detail(AskDetail::Detailed),
            "Walk me through a.py::f: why? Make it a detailed tour."
        );
    }

    #[test]
    fn a_full_stop_is_added_only_where_the_sentence_has_none() {
        assert_eq!(
            said(
                Some("a.py::f"),
                Some("why retry"),
                Reach::Calls,
                AskDetail::Detailed
            ),
            "Walk me through a.py::f: why retry. Cover it and what it calls. Make it a detailed tour."
        );
        assert_eq!(
            said(
                Some("a.py::f"),
                Some("why retry?"),
                Reach::Calls2,
                AskDetail::Quick
            ),
            "Walk me through a.py::f: why retry? Cover it, what it calls, and what those call. Make it a quick tour."
        );
    }

    #[test]
    fn a_tours_detail_is_a_choice_and_normal_is_the_agents() {
        assert_eq!(AskDetail::of_tour(Detail::Quick), AskDetail::Quick);
        assert_eq!(AskDetail::of_tour(Detail::Normal), AskDetail::Agent);
        assert_eq!(AskDetail::of_tour(Detail::Detailed), AskDetail::Detailed);
    }
}
