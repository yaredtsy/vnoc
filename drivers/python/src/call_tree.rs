//! `vnoc/callTree`: the calls made by one function, or by a class's constructor, one node per call site. The tree is not merged: that is
//! the core's (docs/14 T2). The solving is `python_analysis::call_wire`; this file runs it, and puts the answer in the protocol's shape
//! (paths relative to `root`, 0-based lines, UTF-16 columns).

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::time::Duration;

use analyzer_core::graph::Entry;
use lsp_server::ErrorCode;
use python_analysis::{Frame, Limits, Options, Project, call_wire};
use python_syntax::DefKind as Kind;
use vnoc_driver_protocol::{CallMark, CallNode, CallTarget, CallTreeParams, CallTreeResult, DefKind};

use crate::outline::{analyzer_occurrence, protocol_occurrence};
use crate::positions::Lines;
use crate::{Failure, invalid};

/// A solve that runs this long ends by itself: the tree then carries `incomplete` marks.
const DEADLINE: Duration = Duration::from_secs(20);

fn internal(message: impl Into<String>) -> Failure {
    Failure { code: ErrorCode::InternalError, message: message.into() }
}

pub fn call_tree(params: CallTreeParams) -> Result<CallTreeResult, Failure> {
    let root = Path::new(&params.root);
    std::fs::read_to_string(root.join(&params.file)).map_err(|e| internal(format!("cannot read {}: {e}", params.file)))?;

    let no_def = || invalid(format!("no function {} in {}", params.entry.qualname, params.file));
    // the protocol counts from 1: a 0 is a malformed request, not a def that is not there
    if params.entry.occurrence == 0 {
        return Err(invalid(format!("occurrence of {} starts at 1, not 0", params.entry.qualname)));
    }
    let entry = Entry { qualname: &params.entry.qualname, occurrence: analyzer_occurrence(params.entry.occurrence).ok_or_else(no_def)? };
    let opts = Options { limits: Limits { deadline: Some(DEADLINE), render_depth: params.depth, ..Limits::default() }, ..Options::default() };

    let project = Project::open(&params.root).map_err(|e| internal(e.to_string()))?;
    let wire = call_wire(&project, &params.file, &entry, &opts, false).ok_or_else(no_def)?;

    let callers = Callers::read(root, &wire)?;
    let convert = Convert { lines: callers.lines() };
    Ok(CallTreeResult { root: convert.node(&wire) })
}

/// The text of every file a call is made from: a node's `call_site` is read in `call_site_file`, which is not always its parent's file
/// (a call made in an inherited constructor is in the base class's file).
struct Callers {
    texts: Vec<(String, String)>,
}

impl Callers {
    fn read(root: &Path, wire: &Frame) -> Result<Callers, Failure> {
        let mut files = BTreeSet::new();
        fn collect(node: &Frame, files: &mut BTreeSet<String>) {
            if let (Some(file), Some(_)) = (&node.call_site_file, &node.call_site) {
                files.insert(file.clone());
            }
            node.children.iter().for_each(|c| collect(c, files));
        }
        collect(wire, &mut files);
        let texts = files
            .into_iter()
            .map(|file| std::fs::read_to_string(root.join(&file)).map(|text| (file.clone(), text)).map_err(|e| internal(format!("cannot read {file}: {e}"))))
            .collect::<Result<_, _>>()?;
        Ok(Callers { texts })
    }

    fn lines(&self) -> HashMap<&str, Lines<'_>> {
        self.texts.iter().map(|(file, text)| (file.as_str(), Lines::new(text))).collect()
    }
}

struct Convert<'a> {
    lines: HashMap<&'a str, Lines<'a>>,
}

impl Convert<'_> {
    fn node(&self, frame: &Frame) -> CallNode {
        let call_site = frame.call_site.as_ref().zip(frame.call_site_file.as_deref()).and_then(|(span, file)| {
            let lines = self.lines.get(file)?;
            Some(vnoc_driver_protocol::Range { start: lines.position_1based(span.start), end: lines.position_1based(span.end) })
        });
        // a mark this driver's protocol has no word for is left out
        let marks = frame
            .marks
            .iter()
            .filter_map(|m| match m.as_str() {
                "recursive" => Some(CallMark::Recursive),
                "cut" => Some(CallMark::Cut),
                "incomplete" => Some(CallMark::Incomplete),
                _ => None,
            })
            .collect();
        CallNode { target: self.target(frame), call_site, marks, children: frame.children.iter().map(|c| self.node(c)).collect() }
    }

    fn target(&self, frame: &Frame) -> CallTarget {
        CallTarget {
            // relative to the root already; a def outside it (library code) keeps its absolute path, and the protocol says `null`
            file: frame.target_file.clone().filter(|f| !Path::new(f).is_absolute()),
            qualname: frame.target_qualname.clone(),
            occurrence: protocol_occurrence(frame.target_occurrence),
            kind: match frame.target_kind {
                Some(Kind::Class) => DefKind::Class,
                _ => DefKind::Function,
            },
            id: (!frame.target_id.is_empty()).then(|| frame.target_id.clone()),
        }
    }
}
