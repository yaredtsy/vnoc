//! `vnoc/outline`: every def of a document, flat, in source order. The shape is `plan_vs/07-drivers/01-driver-protocol.md` §5. The defs are the
//! analyzer's (`bend_syntax::outline`); this file puts them in the protocol's shape. Bend has no nesting: every `parent` is `""`.

use bend_syntax::{DefKind as Kind, MarkerStyle, Outline};
use vnoc_driver_protocol::{Def, DefKind, OutlineResult, SyntaxError};

use crate::file_id;
use crate::positions::Lines;

/// How a def's marker is written: an `ID: …` comment line directly above the declaration.
pub fn style() -> MarkerStyle<'static> {
    MarkerStyle::above("ID")
}

/// The analyzer's outline of a text, as it is: `plan_ids` needs its defs.
pub fn scan(text: &str) -> Outline {
    bend_syntax::outline(text, style())
}

/// The protocol counts same-named defs from 1, the analyzer from 0. `None` for 0 (there is no such def) and for a number the analyzer
/// cannot hold.
pub fn analyzer_occurrence(protocol: u32) -> Option<u32> {
    protocol.checked_sub(1)
}

/// The protocol knows two kinds: a class, and everything else. A `type` is the analyzer's `Enum` and is what gets built, so it is a class;
/// a def and a law are functions (the core's enum is open, hence the `_`).
pub fn kind(kind: &Kind) -> DefKind {
    match kind {
        Kind::Enum => DefKind::Class,
        _ => DefKind::Function,
    }
}

pub fn outline(text: &str) -> OutlineResult {
    let found = scan(text);
    let lines = Lines::new(text);
    let defs = found
        .defs
        .iter()
        .map(|d| Def {
            qualname: d.qualname.clone(),
            name: d.name.clone(),
            kind: kind(&d.kind),
            parent: String::new(),
            parent_kind: None,
            range: lines.range(d.range),
            params: d.params.clone(),
            id: d.marker.clone(),
            syntax_error: d.syntax_error,
        })
        .collect();
    let syntax_errors = found
        .syntax_errors
        .iter()
        .map(|e| SyntaxError {
            range: lines.range(e.range),
            message: e.message.clone(),
        })
        .collect();
    // the outline's own `file_marker` is not used: the header has its own label (`FileID`)
    OutlineResult {
        defs,
        file_id: file_id::read(text),
        syntax_errors,
    }
}
