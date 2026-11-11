//! `vnoc/callTree`: the calls made by one function, or by a class's `__init__`, one node per call site. The tree is not merged: that is
//! the core's (docs/14 T2). The solving is pylspt's `call_wire_at`; this file finds the entry, and puts the answer in the wire's shape
//! (paths relative to `root`, 0-based lines, UTF-16 columns).

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use lsp_server::ErrorCode;
use pylspt::db::{open, open_project};
use pylspt::interp::{FrameJson, Limits, Mark, Mode, Options, TargetKind, call_wire_at};
use ruff_db::system::SystemPath;
use vnoc_driver_protocol::{CallMark, CallNode, CallTarget, CallTreeParams, CallTreeResult, Def, DefKind};

use crate::positions::Lines;
use crate::{Failure, invalid, outline};

/// A solve that runs this long ends by itself: the tree then carries `incomplete` marks.
const DEADLINE: Duration = Duration::from_secs(20);

fn internal(message: impl Into<String>) -> Failure {
    Failure { code: ErrorCode::InternalError, message: message.into() }
}

/// `canonicalize` gives `\\?\C:\…` on Windows; the paths pylspt hands back have no such prefix.
fn plain(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(|p| p.strip_prefix(r"\\?\")) {
        Some(rest) => PathBuf::from(rest),
        None => path,
    }
}

pub fn call_tree(params: CallTreeParams) -> Result<CallTreeResult, Failure> {
    let root = std::fs::canonicalize(&params.root).map(plain).map_err(|e| internal(format!("cannot open {}: {e}", params.root)))?;
    let path = root.join(&params.file);
    let text = std::fs::read_to_string(&path).map_err(|e| internal(format!("cannot read {}: {e}", params.file)))?;

    let no_def = || invalid(format!("no function {} in {}", params.entry.qualname, params.file));
    let outlined = outline::outline(&text).defs;
    let entry = nth_def(&outlined, &params.entry.qualname, params.entry.occurrence).ok_or_else(no_def)?;

    let opts = Options { mode: Mode::Summary, require_ids: false, limits: Limits { deadline: Some(DEADLINE), render_depth: params.depth, ..Limits::default() } };
    let db = open_project(system_path(&root)?).map_err(|e| internal(e.to_string()))?;
    let (file, _) = open(&db, system_path(&path)?).map_err(|e| internal(format!("cannot open {}: {e}", params.file)))?;

    // A class has no calls of its own: it stands for its `__init__`, the last one in its body (the one Python runs).
    // Its tree is the class, with what `__init__` calls under it; there is no node for `__init__` itself.
    let (entry_qualname, entry_nth) = match entry.kind {
        DefKind::Function => (entry.qualname.clone(), params.entry.occurrence),
        DefKind::Class => {
            let init = format!("{}.__init__", entry.qualname);
            let inits = outlined.iter().filter(|d| d.qualname == init && d.kind == DefKind::Function).count();
            if inits == 0 {
                return Ok(CallTreeResult { root: class_root(&params, entry, Vec::new(), Vec::new()) });
            }
            (init, inits as u32)
        }
    };
    let (wire, _) = call_wire_at(&db, file, &entry_qualname, entry_nth, &opts).ok_or_else(|| match entry.kind {
        DefKind::Function => no_def(),
        DefKind::Class => internal(format!("cannot solve {entry_qualname} in {}", params.file)),
    })?;

    let lines = Callers::read(&wire)?;
    let convert = Convert { root: &root, lines: lines.lines() };
    let tree = convert.node(&wire, None);
    let root = match entry.kind {
        DefKind::Function => tree,
        DefKind::Class => class_root(&params, entry, tree.children, tree.marks),
    };
    Ok(CallTreeResult { root })
}

fn system_path(path: &Path) -> Result<&SystemPath, Failure> {
    SystemPath::from_std_path(path).ok_or_else(|| internal(format!("{} is not valid UTF-8", path.display())))
}

/// The `occurrence`-th def (1-based) with this qualname, in source order: the outline's own order.
fn nth_def<'a>(defs: &'a [Def], qualname: &str, occurrence: u32) -> Option<&'a Def> {
    defs.iter().filter(|d| d.qualname == qualname).nth(occurrence.checked_sub(1)? as usize)
}

fn class_root(params: &CallTreeParams, class: &Def, children: Vec<CallNode>, marks: Vec<CallMark>) -> CallNode {
    let target = CallTarget {
        file: Some(params.file.replace('\\', "/")),
        qualname: class.qualname.clone(),
        occurrence: params.entry.occurrence,
        kind: DefKind::Class,
        id: class.id.clone(),
    };
    CallNode { target, call_site: None, marks, children }
}

/// The text of every file a call is made from: a node's `call_site` is read in its parent's file.
struct Callers {
    texts: Vec<(String, String)>,
}

impl Callers {
    fn read(wire: &FrameJson) -> Result<Callers, Failure> {
        let mut files = BTreeSet::new();
        fn collect(node: &FrameJson, files: &mut BTreeSet<String>) {
            if let (Some(file), false) = (&node.target_file, node.children.is_empty()) {
                files.insert(file.clone());
            }
            node.children.iter().for_each(|c| collect(c, files));
        }
        collect(wire, &mut files);
        let texts = files
            .into_iter()
            .map(|file| std::fs::read_to_string(&file).map(|text| (file.clone(), text)).map_err(|e| internal(format!("cannot read {file}: {e}"))))
            .collect::<Result<_, _>>()?;
        Ok(Callers { texts })
    }

    fn lines(&self) -> HashMap<&str, Lines<'_>> {
        self.texts.iter().map(|(file, text)| (file.as_str(), Lines::new(text))).collect()
    }
}

struct Convert<'a> {
    root: &'a Path,
    lines: HashMap<&'a str, Lines<'a>>,
}

impl Convert<'_> {
    fn node(&self, node: &FrameJson, parent: Option<&FrameJson>) -> CallNode {
        let call_site = node.call_site.as_ref().zip(parent.and_then(|p| p.target_file.as_deref())).and_then(|(span, file)| {
            let lines = self.lines.get(file)?;
            Some(vnoc_driver_protocol::Range { start: lines.position_1based(span.start), end: lines.position_1based(span.end) })
        });
        let marks = node
            .marks
            .iter()
            .map(|m| match m {
                Mark::Recursive => CallMark::Recursive,
                Mark::Cut => CallMark::Cut,
                Mark::Incomplete => CallMark::Incomplete,
            })
            .collect();
        CallNode { target: self.target(node), call_site, marks, children: node.children.iter().map(|c| self.node(c, Some(node))).collect() }
    }

    fn target(&self, node: &FrameJson) -> CallTarget {
        // `FunctionSchema/<id>` or `ClassSchema/<id>`: the core is given the ID as the code spells it
        let id = node.target_id.split_once('/').map_or(node.target_id.as_str(), |(_, id)| id);
        CallTarget {
            file: node.target_file.as_deref().and_then(|f| Path::new(f).strip_prefix(self.root).ok()).map(|rel| rel.to_string_lossy().replace('\\', "/")),
            qualname: node.target_qualname.clone(),
            occurrence: node.target_nth,
            kind: match node.target_kind {
                TargetKind::Function => DefKind::Function,
                TargetKind::Class => DefKind::Class,
            },
            id: (!id.is_empty()).then(|| id.to_string()),
        }
    }
}
