//! `vnoc/planIds`: where the `ID:` line goes, for the IDs the core picked. The edits are returned, never applied: the editor
//! applies them (the buffer may have unsaved changes; only the editor can edit it safely).
//!
//! The placement rules (extend a docstring, or add `"""ID: …"""`) are pylspt's `plan_ids`; this file finds the defs the
//! core named, and puts the answer in the wire's shape.

use pylspt::defs::{DefNode, scan_defs};
use pylspt::edit::{Edit, apply, plan_ids};
use ruff_python_ast::AnyNodeRef;
use ruff_text_size::TextRange;
use vnoc_driver_protocol::{PlanId, PlanIdsParams, PlanIdsResult, Skipped, TextEdit, Written};

use crate::outline::{outline_parsed, parse};
use crate::positions::Lines;

pub fn plan(params: PlanIdsParams) -> PlanIdsResult {
    let text = params.text.as_str();
    let parsed = parse(text);
    let outlined = outline_parsed(text, &parsed);
    let errors: Vec<TextRange> = parsed.errors().iter().map(|e| e.location).collect();
    let root = AnyNodeRef::from(parsed.syntax());
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };

    let mut defs = Vec::new();
    flatten(&scan_defs(&parsed.syntax().body), &mut defs);

    // which def is `qualname#occurrence`: the n-th def of that name, in source order (the outline is in source order)
    let find = |want: &PlanId| {
        let same_name = outlined.result.defs.iter().zip(&outlined.ranges).filter(|(d, _)| d.qualname == want.qualname);
        same_name.skip(want.occurrence.saturating_sub(1) as usize).map(|(_, range)| *range).next()
    };

    let mut planned: Vec<(&PlanId, Edit)> = Vec::new();
    let (mut skipped, mut unmatched) = (Vec::new(), Vec::new());
    let skip = |want: &PlanId, reason: &str| Skipped { qualname: want.qualname.clone(), occurrence: want.occurrence, reason: reason.to_string() };

    for want in &params.ids {
        let Some(range) = find(want) else {
            unmatched.push(want.clone());
            continue;
        };
        let Some(def) = defs.iter().find(|d| d.range == range) else {
            unmatched.push(want.clone());
            continue;
        };
        if errors.iter().any(|e| range.intersect(*e).is_some()) {
            skipped.push(skip(want, "syntax error inside this def"));
        } else if def.id.is_some() {
            skipped.push(skip(want, if want.replace_existing { "replacing an ID is not supported" } else { "already has an ID" }));
        } else {
            // pylspt's planner takes the ID from a closure that does not know which def asks, so it gets one def at a time
            let id = want.id.clone();
            let edits = plan_ids(std::slice::from_ref(def), root, text, newline, &mut || id.clone());
            match edits.into_iter().next() {
                Some(edit) => planned.push((want, edit)),
                None => skipped.push(skip(want, "no place for an ID")),
            }
        }
    }

    // All edits apply against the ORIGINAL text, in order.
    planned.sort_by_key(|(_, edit)| edit.offset);
    let old = Lines::new(text);
    let edits = planned
        .iter()
        .map(|(_, e)| {
            let at = old.position(e.offset);
            TextEdit { range: vnoc_driver_protocol::Range { start: at, end: at }, new_text: e.text.clone() }
        })
        .collect();

    // Where each inserted text ends up in the NEW text: every earlier edit pushes it right.
    let new_text = apply(text, planned.iter().map(|(_, e)| e.clone()).collect());
    let new = Lines::new(&new_text);
    let mut shift = 0;
    let written = planned
        .iter()
        .map(|(want, e)| {
            let start = e.offset.to_usize() + shift;
            let end = start + e.text.len();
            shift += e.text.len();
            let at = |byte: usize| new.position(ruff_text_size::TextSize::try_from(byte).expect("a text under 4 GB"));
            Written { id: want.id.clone(), qualname: want.qualname.clone(), occurrence: want.occurrence, span: vnoc_driver_protocol::Range { start: at(start), end: at(end) }, text: e.text.clone() }
        })
        .collect();

    PlanIdsResult { version: params.version, edits, written, skipped, unmatched, text_hash: blake3::hash(new_text.as_bytes()).to_hex().to_string() }
}

/// Every def and class, children included, as single nodes (what the planner takes one at a time).
fn flatten(defs: &[DefNode], out: &mut Vec<DefNode>) {
    for def in defs {
        out.push(DefNode { children: vec![], ..def.clone() });
        flatten(&def.children, out);
    }
}
