//! Times what one `vnoc/callTree` costs, with opening the project timed apart from the walk (docs/14 step 7.6).
//!
//!   cargo run --example measure_call_tree -- <root> <file::qualname[#n]>...      each entry: open, first walk, second walk, fresh again
//!   cargo run --example measure_call_tree -- <root> --sweep                      every def of every .py file, on one open project
//!
//! "Open" is `Project::open`; "walk" is `call_wire`. The driver does both for every request today (T6); a kept-open project would pay the
//! walk only, and the second walk of one entry on one project is what a memo that survives the request would give.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use analyzer_core::graph::Entry;
use python_analysis::{Frame, Limits, Options, Project, call_wire};

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn nodes(frame: &Frame) -> usize {
    1 + frame.children.iter().map(nodes).sum::<usize>()
}

fn options() -> Options {
    // what the driver runs with
    Options { limits: Limits { deadline: Some(Duration::from_secs(20)), ..Limits::default() }, ..Options::default() }
}

/// `file::qualname#n`, n counted from 1 like the protocol (default 1).
fn parse(spec: &str) -> (String, String, u16) {
    let (file, rest) = spec.split_once("::").expect("file::qualname");
    let (qualname, nth) = rest.rsplit_once('#').map_or((rest, 1), |(q, n)| (q, n.parse().expect("#n")));
    (file.to_string(), qualname.to_string(), (nth - 1) as u16)
}

fn walk(project: &Project, file: &str, qualname: &str, occurrence: u16) -> (Duration, Option<usize>) {
    let started = Instant::now();
    let frame = call_wire(project, file, &Entry { qualname, occurrence }, &options(), false);
    (started.elapsed(), frame.as_ref().map(nodes))
}

fn open(root: &str) -> (Project, Duration) {
    let started = Instant::now();
    let project = Project::open(root).expect("the project opens");
    (project, started.elapsed())
}

fn entries(root: &Path, specs: &[String]) {
    println!("{:<46} {:>7} {:>9} {:>9} {:>9} {:>11}", "entry", "nodes", "open", "1st walk", "2nd walk", "fresh again");
    for spec in specs {
        let (file, qualname, occurrence) = parse(spec);
        let (project, opened) = open(root.to_str().unwrap());
        let (first, count) = walk(&project, &file, &qualname, occurrence);
        let (second, _) = walk(&project, &file, &qualname, occurrence);
        drop(project);
        // what the next request costs the driver today: a new project, then the same walk
        let (again_project, again_open) = open(root.to_str().unwrap());
        let (again_walk, _) = walk(&again_project, &file, &qualname, occurrence);
        let nodes = count.map_or("none".to_string(), |n| n.to_string());
        println!("{:<46} {:>7} {:>7.1}ms {:>7.1}ms {:>7.1}ms {:>9.1}ms", spec, nodes, ms(opened), ms(first), ms(second), ms(again_open + again_walk));
    }
}

fn python_files(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = read.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy();
        if path.is_dir() {
            if !name.starts_with('.') && name != "__pycache__" && name != "node_modules" {
                python_files(&path, root, out);
            }
        } else if name.ends_with(".py") {
            out.push(path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
        }
    }
}

/// Every def, one after the other, on ONE open project: the cost of asking about everything once the project is open.
fn sweep(root: &Path) {
    let mut files = Vec::new();
    python_files(root, root, &mut files);
    let (project, opened) = open(root.to_str().unwrap());
    let mut timings: Vec<(Duration, usize, String)> = Vec::new();
    let mut unanswered = 0;
    let all = Instant::now();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(root.join(file)) else { continue };
        let outline = python_syntax::outline(&text, python_analysis::MarkerStyle::docstring("ID"));
        let mut seen = std::collections::HashMap::<String, u16>::new();
        for def in &outline.defs {
            let nth = seen.entry(def.qualname.clone()).or_insert(0);
            let occurrence = *nth;
            *nth += 1;
            if !matches!(def.kind, python_syntax::DefKind::Function | python_syntax::DefKind::Method) {
                continue;
            }
            let (took, count) = walk(&project, file, &def.qualname, occurrence);
            match count {
                Some(n) => timings.push((took, n, format!("{file}::{}{}", def.qualname, if occurrence > 0 { format!("#{}", occurrence + 1) } else { String::new() }))),
                None => unanswered += 1,
            }
        }
    }
    let total = all.elapsed();
    timings.sort_by_key(|(d, _, _)| *d);
    let at = |q: f64| ms(timings[((timings.len() - 1) as f64 * q) as usize].0);
    println!("{} files, {} functions walked ({} unanswered), open {:.1} ms, all walks {:.0} ms on one open project", files.len(), timings.len(), unanswered, ms(opened), ms(total));
    println!("per walk: median {:.2} ms, p90 {:.2} ms, p99 {:.2} ms, max {:.1} ms", at(0.5), at(0.9), at(0.99), at(1.0));
    println!("the slowest five:");
    for (took, n, name) in timings.iter().rev().take(5) {
        println!("  {:>8.1} ms  {:>6} nodes  {name}", ms(*took), n);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((root, rest)) = args.split_first() else { return eprintln!("usage: measure_call_tree <root> --sweep | <file::qualname[#n]>...") };
    let root = std::fs::canonicalize(root).expect("the root exists");
    if rest.first().map(String::as_str) == Some("--sweep") {
        sweep(&root);
    } else {
        entries(&root, rest);
    }
}
