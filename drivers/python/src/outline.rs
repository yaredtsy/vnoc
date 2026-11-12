//! `vnoc/outline`: every def and class of a document, flat, in source order, parents before children.
//! The shape is `plan_vs/07-drivers/01-driver-protocol.md` §5. The defs are the analyzer's (`python_syntax::outline`); this file puts
//! them in the protocol's shape.

use python_syntax::{DefKind as Kind, MarkerStyle, Outline};
use vnoc_driver_protocol::{Def, DefKind, OutlineResult, SyntaxError};

use crate::file_id;
use crate::positions::Lines;

/// How a def's marker is written: an `ID: …` line in the docstring.
pub fn style() -> MarkerStyle<'static> {
    MarkerStyle::docstring("ID")
}

/// The analyzer's outline of a text, as it is: `plan_ids` needs its defs.
pub fn scan(text: &str) -> Outline {
    python_syntax::outline(text, style())
}

/// The protocol counts same-named defs from 1, the analyzer from 0. `None` for 0 (there is no such def) and for a number the analyzer
/// cannot hold.
pub fn analyzer_occurrence(protocol: u32) -> Option<u16> {
    u16::try_from(protocol.checked_sub(1)?).ok()
}

pub fn protocol_occurrence(analyzer: u16) -> u32 {
    u32::from(analyzer) + 1
}

/// The protocol knows two kinds: a class, and everything else (the core's enum is open, hence the `_`).
pub fn kind(kind: &Kind) -> DefKind {
    match kind {
        Kind::Class => DefKind::Class,
        _ => DefKind::Function,
    }
}

pub fn outline(text: &str) -> OutlineResult {
    let found = scan(text);
    let lines = Lines::new(text);
    let defs = found
        .defs
        .iter()
        .map(|d| {
            let parent = d.parent.and_then(|i| found.defs.get(i));
            Def {
                qualname: d.qualname.clone(),
                name: d.name.clone(),
                kind: kind(&d.kind),
                parent: parent.map_or_else(String::new, |p| p.qualname.clone()),
                parent_kind: parent.map(|p| kind(&p.kind)),
                range: lines.range(d.range),
                params: d.params.clone(),
                id: d.marker.clone(),
                syntax_error: d.syntax_error,
            }
        })
        .collect();
    let syntax_errors = found.syntax_errors.iter().map(|e| SyntaxError { range: lines.range(e.range), message: e.message.clone() }).collect();
    // the outline's own `file_marker` is not filled for Python: the header has its own label
    OutlineResult { defs, file_id: file_id::read(text), syntax_errors }
}
