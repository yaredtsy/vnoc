//! `vnoc/planIds`: where the `ID:` line goes, for the IDs the core picked. The edits are returned, never applied: the editor
//! applies them (the buffer may have unsaved changes; only the editor can edit it safely).
//!
//! The placement rules (extend a docstring, or add `"""ID: …"""`) are `python_syntax::plan_marker`'s; this file finds the defs the
//! core named, and puts the answer in the wire's shape.

use analyzer_core::edit::{Edit, apply};
use lsp_server::ErrorCode;
use python_syntax::Skip;
use vnoc_driver_protocol::{PlanId, PlanIdsParams, PlanIdsResult, Skipped, TextEdit, Written};

use crate::{Failure, invalid};
use crate::outline::{analyzer_occurrence, scan, style};
use crate::positions::Lines;

/// Why a def got no ID, in this driver's words.
fn reason(skip: &Skip, replace_existing: bool) -> String {
    match skip {
        Skip::SyntaxError => "syntax error inside this def".into(),
        Skip::AlreadyMarked { .. } if replace_existing => "replacing an ID is not supported".into(),
        Skip::AlreadyMarked { .. } => "already has an ID".into(),
        Skip::UnsafeDocComment => "no place for an ID".into(),
        Skip::InvalidText => "this ID cannot be written into a docstring".into(),
        other => other.to_string(),
    }
}

pub fn plan(params: PlanIdsParams) -> Result<PlanIdsResult, Failure> {
    // the protocol counts from 1: a 0 is a malformed request, not a def that is not there
    if let Some(want) = params.ids.iter().find(|want| want.occurrence == 0) {
        return Err(invalid(format!("occurrence of {} starts at 1, not 0", want.qualname)));
    }
    let text = params.text.as_str();
    let outline = scan(text);

    // which def is `qualname#occurrence`: the n-th def of that name, in source order (the outline is in source order)
    let find = |want: &PlanId| analyzer_occurrence(want.occurrence).and_then(|n| outline.defs.iter().filter(|d| d.qualname == want.qualname).nth(usize::from(n)));

    let mut planned: Vec<(&PlanId, Edit)> = Vec::new();
    let (mut skipped, mut unmatched) = (Vec::new(), Vec::new());

    for want in &params.ids {
        let Some(def) = find(want) else {
            unmatched.push(want.clone());
            continue;
        };
        match python_syntax::plan_marker(text, def, &want.id, style()) {
            Ok(edit) => planned.push((want, edit)),
            Err(Skip::DefNotFound) => unmatched.push(want.clone()),
            Err(skip) => skipped.push(Skipped { qualname: want.qualname.clone(), occurrence: want.occurrence, reason: reason(&skip, want.replace_existing) }),
        }
    }

    // All edits apply against the ORIGINAL text, in order.
    planned.sort_by_key(|(_, edit)| (edit.at.start, edit.at.end));
    let old = Lines::new(text);
    let edits = planned.iter().map(|(_, e)| TextEdit { range: old.range(e.at), new_text: e.text.clone() }).collect();

    // Where each inserted text ends up in the NEW text: every earlier edit pushes it right.
    let all: Vec<Edit> = planned.iter().map(|(_, e)| e.clone()).collect();
    let new_text = apply(text, &all).ok_or_else(|| Failure { code: ErrorCode::InternalError, message: "the planned edits do not fit the text".into() })?;
    let new = Lines::new(&new_text);
    let (mut added, mut removed) = (0usize, 0usize);
    let written = planned
        .iter()
        .map(|(want, e)| {
            let start = e.at.start as usize + added - removed;
            let end = start + e.text.len();
            added += e.text.len();
            removed += (e.at.end - e.at.start) as usize;
            let at = |byte: usize| new.position(u32::try_from(byte).unwrap_or(u32::MAX));
            Written { id: want.id.clone(), qualname: want.qualname.clone(), occurrence: want.occurrence, span: vnoc_driver_protocol::Range { start: at(start), end: at(end) }, text: e.text.clone() }
        })
        .collect();

    Ok(PlanIdsResult { version: params.version, edits, written, skipped, unmatched, text_hash: blake3::hash(new_text.as_bytes()).to_hex().to_string() })
}
