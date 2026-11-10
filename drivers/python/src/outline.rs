//! `vnoc/outline`: every def and class of a document, flat, in source order, parents before children.
//! The shape is `plan_vs/07-drivers/01-driver-protocol.md` §5.

use std::collections::HashMap;

use pylspt::defs::{DefNode, scan_defs};
use ruff_python_ast::statement_visitor::{StatementVisitor, walk_body, walk_stmt};
use ruff_python_ast::{PythonVersion, Stmt};
use ruff_python_parser::{Mode, ParseOptions, parse_unchecked};
use ruff_text_size::{Ranged, TextRange};
use vnoc_driver_protocol::{Def, DefKind, OutlineResult, SyntaxError};

use crate::positions::Lines;

/// Python's `__qualname__`: a def inside a FUNCTION gets `<locals>` after that function (`top.<locals>.inner`);
/// inside a class or at module level it is just joined with a dot.
fn qualname(stack: &[(String, DefKind)], name: &str) -> String {
    match stack.last() {
        None => name.to_string(),
        Some((q, DefKind::Function)) => format!("{q}.<locals>.{name}"),
        Some((q, _)) => format!("{q}.{name}"),
    }
}

/// The IDs pylspt reads (its one shared reader), keyed by the def's range.
fn ids_by_range(defs: &[DefNode], out: &mut HashMap<TextRange, String>) {
    for d in defs {
        if let Some(id) = &d.id {
            out.insert(d.range, id.clone());
        }
        ids_by_range(&d.children, out);
    }
}

struct Collector<'a> {
    lines: &'a Lines<'a>,
    ids: &'a HashMap<TextRange, String>,
    errors: &'a [TextRange],
    stack: Vec<(String, DefKind)>,
    out: Vec<Def>,
}

impl<'a> Collector<'a> {
    fn record(&mut self, stmt: &'a Stmt, name: &str, kind: DefKind, params: Vec<String>, body: &'a [Stmt]) {
        let range = stmt.range();
        let qualname = qualname(&self.stack, name);
        let (parent, parent_kind) = self.stack.last().map_or(("", None), |(q, k)| (q.as_str(), Some(*k)));
        self.out.push(Def {
            qualname: qualname.clone(),
            name: name.to_string(),
            kind,
            parent: parent.to_string(),
            parent_kind,
            range: self.lines.range(range),
            params,
            id: self.ids.get(&range).cloned(),
            // a def whose lines contain a parse error; it errs toward flagging a neighbour, never toward missing the broken one
            syntax_error: self.errors.iter().any(|e| range.intersect(*e).is_some()),
        });
        self.stack.push((qualname, kind));
        walk_body(self, body);
        self.stack.pop();
    }
}

impl<'a> StatementVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(f) => {
                let mut params: Vec<String> = f.parameters.iter_non_variadic_params().map(|p| p.parameter.name.to_string()).collect();
                if let Some(v) = &f.parameters.vararg {
                    params.push(format!("*{}", v.name));
                }
                if let Some(k) = &f.parameters.kwarg {
                    params.push(format!("**{}", k.name));
                }
                self.record(stmt, f.name.as_str(), DefKind::Function, params, &f.body);
            }
            Stmt::ClassDef(c) => self.record(stmt, c.name.as_str(), DefKind::Class, vec![], &c.body),
            _ => walk_stmt(self, stmt), // defs can sit inside if / try / with / for
        }
    }
}

pub fn outline(text: &str) -> OutlineResult {
    let options = ParseOptions::from(Mode::Module).with_target_version(PythonVersion::PY313);
    let parsed = parse_unchecked(text, options).try_into_module().expect("Mode::Module always produces a module");
    let lines = Lines::new(text);
    let mut ids = HashMap::new();
    ids_by_range(&scan_defs(&parsed.syntax().body), &mut ids);
    let errors: Vec<TextRange> = parsed.errors().iter().map(|e| e.location).collect();

    let mut c = Collector { lines: &lines, ids: &ids, errors: &errors, stack: vec![], out: vec![] };
    walk_body(&mut c, &parsed.syntax().body);
    let syntax_errors = parsed.errors().iter().map(|e| SyntaxError { range: lines.range(e.location), message: e.error.to_string() }).collect();
    OutlineResult { defs: c.out, syntax_errors }
}
