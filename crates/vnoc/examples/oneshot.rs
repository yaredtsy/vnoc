//! What one command of the agent CLI would cost: open the project, start the driver, build the whole index, answer, exit
//! (`docs/21-agent-cli/05-how-it-runs.md` §3). Built for step A0 and kept: A16 measures again with it.
//!
//!   oneshot generate <dir>                      a fresh project: 1,000 Python files in 20 folders, ~10 defs each, then 1,000 docs of ~2 KB
//!   oneshot docs <root> <driver> <n>            n docs of ~2 KB on defs of an existing project (found through the driver), spread evenly
//!   oneshot measure <root> <driver> [--calls <repo/path.py::Qual.name>] [--word <w>] [--runs N]
//!   oneshot lock <dir>                          what a second `File::try_lock` on one lock file sees (04 §7)
//!
//! Release builds: `cargo build --release -p vnoc --examples`, and `cargo build --release` in drivers/python for the driver.
//! `measure` runs N times (default 5) after one warm-up run that is thrown away; every run starts from nothing (a new `Project`,
//! `DriverHost`, `Index` and two empty search slots), as a new process would.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use vnoc::driver_host::DriverHost;
use vnoc::index::{Index, Kind, Node, Touched, refresh};
use vnoc::project::Project;
use vnoc::scan;
use vnoc::search::{self, CorpusSlot};
use vnoc::userdata::{DocTexts, UserData};
use vnoc_domain::{IdInCode, LocationKey, NodeKind, NodeToml, RepoPath, first_free_id};
use vnoc_driver_protocol::{CallEntry, CallNode, CallTreeParams};
use vnoc_search::Limits;

/// `index::build` caps the walk at this many files; the example's own walk must match it.
const MAX_FILES: usize = 20_000;

// ---------------------------------------------------------------------------------------------------------------- main

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("generate") if args.len() == 2 => generate(Path::new(&args[1])),
        Some("docs") if args.len() == 4 => match args[3].parse() {
            Ok(n) => add_docs(Path::new(&args[1]), Path::new(&args[2]), n),
            Err(_) => Err("<n> is a number".into()),
        },
        Some("measure") if args.len() >= 3 => measure(&args[1..]),
        Some("lock") if args.len() == 2 => lock(Path::new(&args[1])),
        _ => Err(USAGE.into()),
    };
    if let Err(message) = result {
        eprintln!("{message}");
        std::process::exit(2);
    }
}

const USAGE: &str = "usage:\n  oneshot generate <dir>\n  oneshot docs <root> <driver> <n>\n  oneshot measure <root> <driver> [--calls <repo/path.py::Qual.name>] [--word <w>] [--runs N]\n  oneshot lock <dir>";

type Fail = Box<dyn std::error::Error>;

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

// ---------------------------------------------------------------------------------------------------------- generate

/// A fixed-seed generator: the same project every time.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn pick<'a>(&mut self, of: &[&'a str]) -> &'a str {
        of[self.next() as usize % of.len()]
    }
}

const VERBS: [&str; 16] = [
    "parse", "build", "load", "send", "check", "merge", "split", "render", "update", "resolve",
    "format", "collect", "apply", "read", "write", "compute",
];
const NOUNS: [&str; 16] = [
    "order", "invoice", "user", "payment", "report", "event", "token", "route", "cache", "queue",
    "record", "session", "item", "batch", "schema", "config",
];

/// A word of the doc bodies: a syllable word from a vocabulary of 4,000 with a few common ones (as `doc_text.rs`'s timing has it), and now and
/// then a word the def names use, so a search for one of those finds docs as well.
fn body_word(rng: &mut Rng) -> String {
    let r = rng.next();
    if r.is_multiple_of(9) {
        return (if r.is_multiple_of(2) {
            VERBS[(r / 2) as usize % 16]
        } else {
            NOUNS[(r / 2) as usize % 16]
        })
        .to_string();
    }
    let mut n = if r.is_multiple_of(4) {
        r % 20
    } else {
        r % 4000
    };
    let (letters, vowels) = (b"bcdfghklmnprstvz", b"aeiou");
    let mut w = String::new();
    for _ in 0..2 {
        w.push(letters[(n % 16) as usize] as char);
        n /= 16;
        w.push(vowels[(n % 5) as usize] as char);
        n /= 5;
    }
    w
}

/// A doc body of about 2 KB.
fn body(rng: &mut Rng) -> String {
    let mut text = String::new();
    while text.len() < 2000 {
        text.push_str(&body_word(rng));
        text.push(if rng.next().is_multiple_of(11) {
            '\n'
        } else {
            ' '
        });
    }
    text.push('\n');
    text
}

/// One def of a generated file, as a node would be attached to it.
struct Made {
    qualname: String,
    kind: NodeKind,
}

/// The source of one file: a class with three methods, four functions, two of them with a nested def: ten defs.
fn module_source(rng: &mut Rng) -> (String, Vec<Made>) {
    let mut taken = BTreeSet::new();
    let mut name = |rng: &mut Rng| loop {
        let candidate = format!("{}_{}", rng.pick(&VERBS), rng.pick(&NOUNS));
        if taken.insert(candidate.clone()) {
            return candidate;
        }
    };
    let class = {
        let (noun, verb) = (rng.pick(&NOUNS), rng.pick(&VERBS));
        let mut c = String::new();
        c.push_str(&noun[..1].to_uppercase());
        c.push_str(&noun[1..]);
        c.push_str(&verb[..1].to_uppercase());
        c.push_str(&verb[1..]);
        c
    };
    let mut made = Vec::new();
    let mut out = String::from(
        "\"\"\"A generated module for the one-shot measurement.\"\"\"\n\nimport os\n\n\n",
    );
    let functions: Vec<String> = (0..4).map(|_| name(rng)).collect();
    for (n, f) in functions.iter().enumerate() {
        out.push_str(&format!("def {f}(a, b=None):\n    total = a\n"));
        made.push(Made {
            qualname: f.clone(),
            kind: NodeKind::Function,
        });
        if n < 2 {
            let inner = format!("inner_{}", rng.pick(&NOUNS));
            out.push_str(&format!(
                "\n    def {inner}(x):\n        return x + 1\n\n    total = {inner}(total)\n"
            ));
            made.push(Made {
                qualname: format!("{f}.{inner}"),
                kind: NodeKind::Function,
            });
        }
        // a call to the function before it, so a call tree has something to walk
        if n > 0 {
            out.push_str(&format!("    total = {}(total, b)\n", functions[n - 1]));
        }
        out.push_str("    return total\n\n\n");
    }
    out.push_str(&format!(
        "class {class}:\n    \"\"\"Holds one value.\"\"\"\n\n"
    ));
    made.push(Made {
        qualname: class.clone(),
        kind: NodeKind::Class,
    });
    out.push_str("    def __init__(self, value):\n        self.value = value\n\n");
    made.push(Made {
        qualname: format!("{class}.__init__"),
        kind: NodeKind::Function,
    });
    for _ in 0..2 {
        let method = name(rng);
        out.push_str(&format!(
            "    def {method}(self, extra):\n        return {}(self.value, extra)\n\n",
            functions[0]
        ));
        made.push(Made {
            qualname: format!("{class}.{method}"),
            kind: NodeKind::Function,
        });
    }
    (out, made)
}

/// Attaches doc number `n` to the node at `location`: the ID, the node, the doc, then the body, all through the library. The ID is not in the
/// code (`id_in_code = "no"`): the record finds its def by name.
fn attach(
    data: &UserData,
    location: &LocationKey,
    kind: NodeKind,
    title: &str,
    rng: &mut Rng,
) -> Result<(), Fail> {
    let id = first_free_id(location, |id| data.is_taken(id))?;
    let mut toml = NodeToml::new(location);
    toml.set_id_in_code(IdInCode::No);
    toml.set_kind(&kind);
    data.create_node(&id, &toml)?;
    let hint = location.to_string();
    let path = data.create_doc(
        &id,
        &vnoc::userdata::NewDoc {
            title,
            hint: &hint,
            body: "",
            author: None,
            tags: &[],
        },
    )?;
    OpenOptions::new()
        .append(true)
        .open(path)?
        .write_all(body(rng).as_bytes())?;
    Ok(())
}

fn git_init(dir: &Path) -> Result<(), Fail> {
    let status = Command::new("git")
        .arg("init")
        .arg("-q")
        .arg(dir)
        .status()?;
    if !status.success() {
        return Err("git init failed".into());
    }
    Ok(())
}

fn generate(dir: &Path) -> Result<(), Fail> {
    if dir.exists() && fs::read_dir(dir)?.next().is_some() {
        return Err(format!("{} is not empty", dir.display()).into());
    }
    fs::create_dir_all(dir)?;
    git_init(dir)?;
    let mut rng = Rng(20_251_117);
    let mut modules: Vec<(String, Vec<Made>)> = Vec::new();
    for folder in 0..20 {
        let sub = dir.join(format!("pkg{folder:02}"));
        fs::create_dir_all(&sub)?;
        for file in 0..50 {
            let (source, made) = module_source(&mut rng);
            let name = format!("mod{file:02}.py");
            fs::write(sub.join(&name), source)?;
            modules.push((format!("pkg{folder:02}/{name}"), made));
        }
    }

    let data = UserData::new(dir);
    data.initialize(false)?;
    let mut docs = 0;
    for (n, (path, made)) in modules.iter().enumerate() {
        // one doc per file, on a different kind of def in turn: the class, a function, a method, a nested def
        let target = &made[[6, 0, 8, 1][n % 4]];
        let location =
            LocationKey::new(RepoPath::new(path.as_str())?, target.qualname.as_str(), 1)?;
        let title = format!("Notes on {}", target.qualname);
        attach(&data, &location, target.kind.clone(), &title, &mut rng)?;
        docs += 1;
    }
    println!(
        "{}: {} files in 20 folders, {docs} docs",
        dir.display(),
        modules.len()
    );
    Ok(())
}

// -------------------------------------------------------------------------------------------------------------- docs

fn tokio_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime")
}

fn defs_of(node: &Node, out: &mut Vec<(String, String, NodeKind)>) {
    for child in &node.children {
        if let (Some(qualname), Kind::Function | Kind::Class) = (&child.qualname, child.kind) {
            let kind = if child.kind == Kind::Class {
                NodeKind::Class
            } else {
                NodeKind::Function
            };
            out.push((child.path.clone(), qualname.clone(), kind));
        }
        defs_of(child, out);
    }
}

/// n docs on an existing project, on defs spread evenly through the index (the driver is asked for them: the project is not ours).
fn add_docs(root: &Path, driver: &Path, n: usize) -> Result<(), Fail> {
    let project = Project::open(root)?;
    let host = DriverHost::default();
    host.set_path(Some(driver.to_path_buf()));
    let index = RwLock::new(Index::default());
    tokio_runtime().block_on(refresh(&project, &host, &index, Touched::All))?;
    let mut defs = Vec::new();
    defs_of(
        &index
            .read()
            .unwrap()
            .tree(&project.name(), project.prefix()),
        &mut defs,
    );
    if defs.len() < n {
        return Err(format!("only {} defs, {n} asked", defs.len()).into());
    }
    let data = UserData::new(project.git_root());
    data.initialize(false)?;
    let mut rng = Rng(7);
    for i in 0..n {
        let (path, qualname, kind) = &defs[i * defs.len() / n];
        let location = LocationKey::new(project.repo_path(path)?, qualname.as_str(), 1)?;
        attach(
            &data,
            &location,
            kind.clone(),
            &format!("Notes on {qualname}"),
            &mut rng,
        )?;
    }
    println!("{n} docs on {} defs of {}", defs.len(), root.display());
    Ok(())
}

// ----------------------------------------------------------------------------------------------------------- measure

/// The times of one phase over the runs, in the order they were first recorded.
#[derive(Default)]
struct Phases(Vec<(&'static str, Vec<f64>)>);

impl Phases {
    fn add(&mut self, name: &'static str, took: f64) {
        match self.0.iter_mut().find(|(n, _)| *n == name) {
            Some((_, samples)) => samples.push(took),
            None => self.0.push((name, vec![took])),
        }
    }

    fn print(&self) {
        println!(
            "{:<58} {:>9} {:>9} {:>9}",
            "phase (ms)", "median", "min", "max"
        );
        for (name, samples) in &self.0 {
            let mut s = samples.clone();
            s.sort_by(f64::total_cmp);
            let median = if s.len() % 2 == 1 {
                s[s.len() / 2]
            } else {
                (s[s.len() / 2 - 1] + s[s.len() / 2]) / 2.0
            };
            println!(
                "{name:<58} {median:>9.1} {:>9.1} {:>9.1}",
                s[0],
                s[s.len() - 1]
            );
        }
    }
}

/// What one run counted (the same every run; the last is printed).
#[derive(Default)]
struct Counts {
    files: usize,
    folders: usize,
    classes: usize,
    functions: usize,
    records: usize,
    docs_stat: usize,
    docs_read: usize,
    doc_texts: usize,
    search_nodes: usize,
    search_docs: usize,
    calls_nodes: Option<usize>,
}

struct Options {
    root: PathBuf,
    driver: PathBuf,
    calls: Option<String>,
    word: String,
    runs: usize,
}

fn options(args: &[String]) -> Result<Options, Fail> {
    let mut o = Options {
        root: PathBuf::from(&args[0]),
        driver: PathBuf::from(&args[1]),
        calls: None,
        word: "parse".into(),
        runs: 5,
    };
    let mut rest = args[2..].iter();
    while let Some(flag) = rest.next() {
        let mut value = || {
            rest.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--calls" => o.calls = Some(value()?),
            "--word" => o.word = value()?,
            "--runs" => o.runs = value()?.parse()?,
            other => return Err(format!("unknown flag {other}").into()),
        }
    }
    Ok(o)
}

fn nodes_in(root: &CallNode) -> usize {
    1 + root.children.iter().map(nodes_in).sum::<usize>()
}

/// `server.rs` `driver_tree`'s mapping from a key to the driver's request: the folder is the opened one, the file relative to it.
fn call_params(
    project: &Project,
    location: &LocationKey,
    depth: u32,
) -> Result<CallTreeParams, Fail> {
    let file = location
        .path()
        .as_str()
        .strip_prefix(project.prefix())
        .map(|p| p.trim_start_matches('/'))
        .filter(|p| !p.is_empty())
        .ok_or("the key is not under the opened folder")?;
    Ok(CallTreeParams {
        root: project.open_dir().to_string_lossy().into_owned(),
        file: file.to_string(),
        entry: CallEntry {
            qualname: location.qualname().to_string(),
            occurrence: location.occurrence(),
        },
        depth: Some(depth),
    })
}

fn run_once(o: &Options, phases: &mut Phases, counts: &mut Counts) -> Result<(), Fail> {
    let rt = tokio_runtime();

    // 1. open
    let started = Instant::now();
    let project = Project::open(&o.root)?;
    phases.add("1  Project::open", ms(started.elapsed()));

    // 2. the walk, on its own
    let started = Instant::now();
    // this probe measures Python: `.py` and `.pyi`
    let extensions = ["py", "pyi"].map(String::from).into();
    let walked = scan::files(&project, &extensions, MAX_FILES);
    let walk = ms(started.elapsed());
    phases.add("2  walk (scan::files)", walk);
    counts.files = walked.files.len();
    counts.folders = walked.folders.len();

    // 3. the whole build, starting the driver on its first request
    let host = DriverHost::default();
    host.set_path(Some(o.driver.clone()));
    let index = RwLock::new(Index::default());
    let started = Instant::now();
    rt.block_on(refresh(&project, &host, &index, Touched::All))?;
    let total = ms(started.elapsed());
    phases.add("3  refresh(All) in all   (= `vnoc status`)", total);

    // 4. what is inside it, on its own, after it: `.vnoc/` and the doc texts from an empty `DocTexts`
    let data = UserData::new(project.git_root());
    let started = Instant::now();
    let report = data.load_all();
    let loaded = ms(started.elapsed());
    phases.add("4a .vnoc/ read (UserData::load_all)", loaded);
    let started = Instant::now();
    let (texts, stats) = data.refresh_doc_texts(&DocTexts::default(), &report.records);
    let read_texts = ms(started.elapsed());
    phases.add(
        "4b doc texts read (refresh_doc_texts, from empty)",
        read_texts,
    );
    phases.add(
        "4c outlines ≈ 3 − 2 − 4a − 4b   (a difference)",
        total - walk - loaded - read_texts,
    );
    counts.records = report.records.len();
    counts.docs_stat = stats.stat_count;
    counts.docs_read = stats.read_count;
    counts.doc_texts = texts.len();
    let counted = index
        .read()
        .unwrap()
        .tree(&project.name(), project.prefix())
        .counts();
    counts.classes = counted[2];
    counts.functions = counted[3];

    // 5. one search, in a process that has searched nothing before: the corpus and the doc index are built, then `run`
    let (corpus_slot, doc_slot): (CorpusSlot, Mutex<_>) = (Mutex::new(None), Mutex::new(None));
    let started = Instant::now();
    let prepared = search::prepare(&project, &index, &corpus_slot, &doc_slot, &o.word);
    phases.add(
        "5a search::prepare (corpus + doc index, both empty)",
        ms(started.elapsed()),
    );
    let (corpus_only, kept_docs): (CorpusSlot, _) = (Mutex::new(None), doc_slot);
    let started = Instant::now();
    let _ = search::prepare(&project, &index, &corpus_only, &kept_docs, &o.word);
    phases.add(
        "5b search::prepare (corpus alone: doc index kept)",
        ms(started.elapsed()),
    );
    // what 5a and 5b spend on the tree alone: `prepare` makes it from the index before it makes the corpus
    let started = Instant::now();
    let _ = index
        .read()
        .unwrap()
        .tree(&project.name(), project.prefix());
    phases.add(
        "5c Index::tree alone (inside 5a and 5b)",
        ms(started.elapsed()),
    );
    let started = Instant::now();
    let results = vnoc_search::run(
        &prepared.corpus,
        &prepared.query,
        prepared.exact,
        Limits {
            nodes: usize::MAX,
            docs: usize::MAX,
        },
    );
    phases.add("5d vnoc_search::run (no limits)", ms(started.elapsed()));
    counts.search_nodes = results.nodes_total;
    counts.search_docs = results.docs_total;

    // 6. the call tree, depth 2: on the driver that is already running, then on one that has to start
    if let Some(key) = &o.calls {
        let location: LocationKey = key.parse()?;
        if index.read().unwrap().def_at(&location).is_none() {
            return Err(format!("`{key}` is not in the index").into());
        }
        let asked = call_params(&project, &location, 2)?;
        let started = Instant::now();
        let warm = rt.block_on(host.call_tree(&asked))?;
        phases.add(
            "6a calls depth 2, driver already running",
            ms(started.elapsed()),
        );
        counts.calls_nodes = Some(nodes_in(&warm.root));
        let cold = DriverHost::default();
        cold.set_path(Some(o.driver.clone()));
        let started = Instant::now();
        rt.block_on(cold.call_tree(&asked))?;
        phases.add(
            "6b calls depth 2, cold driver (start included)",
            ms(started.elapsed()),
        );
    }

    // the driver start alone: a new host, one request that reads nothing
    let probe = DriverHost::default();
    probe.set_path(Some(o.driver.clone()));
    let started = Instant::now();
    rt.block_on(probe.outline_text("probe.py", "x = 1\n"))?;
    phases.add(
        "7  driver start + one empty outline (a probe)",
        ms(started.elapsed()),
    );
    Ok(())
}

fn measure(args: &[String]) -> Result<(), Fail> {
    let filter = tracing_subscriber::EnvFilter::try_from_env("VNOC_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,vnoc=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let o = options(args)?;
    let (mut phases, mut counts) = (Phases::default(), Counts::default());
    // the first run only warms the caches of the disk and the binary: its numbers are not kept
    eprintln!("-- warm-up run (discarded)");
    run_once(&o, &mut Phases::default(), &mut Counts::default())?;
    for n in 1..=o.runs {
        eprintln!("-- run {n}");
        run_once(&o, &mut phases, &mut counts)?;
    }
    println!(
        "{} ({} runs after one warm-up, driver {})",
        o.root.display(),
        o.runs,
        o.driver.display()
    );
    phases.print();
    println!(
        "files {} · folders {} · classes {} · functions/methods {} · records {} · doc files looked at {} and read {} (held: {})",
        counts.files,
        counts.folders,
        counts.classes,
        counts.functions,
        counts.records,
        counts.docs_stat,
        counts.docs_read,
        counts.doc_texts
    );
    println!(
        "search {:?}: {} nodes, {} docs found",
        o.word, counts.search_nodes, counts.search_docs
    );
    if let Some(n) = counts.calls_nodes {
        println!("call tree depth 2: {n} nodes (the root included)");
    }
    Ok(())
}

// -------------------------------------------------------------------------------------------------------------- lock

/// 04 §7: two separate handles on one lock file, as two processes would hold it.
fn lock(dir: &Path) -> Result<(), Fail> {
    fs::create_dir_all(dir)?;
    let path = dir.join("write.lock");
    let open = || {
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
    };
    let (first, second): (File, File) = (open()?, open()?);
    first.lock()?;
    println!("first.lock()                   -> Ok");
    match second.try_lock() {
        Ok(()) => println!("second.try_lock()              -> Ok (not expected)"),
        Err(e) => println!("second.try_lock() while held   -> Err({e:?}); Display: \"{e}\""),
    }
    drop(first);
    println!("first dropped");
    match second.try_lock() {
        Ok(()) => println!("second.try_lock()              -> Ok"),
        Err(e) => println!("second.try_lock() after drop   -> Err({e:?})"),
    }
    Ok(())
}
