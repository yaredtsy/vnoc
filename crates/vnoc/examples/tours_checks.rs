//! The seven checks of step TR0 of the tours plan (`docs/26-tours/06-build-order.md` §2; the answers are in `docs/26-tours/README.md` §7).
//! Built for TR0 and kept: a later step measures again with it, on another machine.
//!
//!   tours_checks race <dir>                     check 3: two writers append whole lines under `tours.lock` while a reader reads (200 lines each)
//!   tours_checks markers <root> <driver>...     check 4: the lines of each def that hold its ID text, and a nested def's, from `Def.id` alone
//!   tours_checks git                            check 6: how long git takes from Rust, and what it does in the cases the plan names
//!   tours_checks stat <dir>                     check 7: does a rewrite of the same size change the file's changed-time, and by how much
//!   tours_checks measure-check <dir> [driver]   TR5 [measure]: a generated tour (40 stops, 100 blocks, 12 files): replay, a real check, a recorded one
//!
//! Every subcommand prints a short report to paste into a doc. `git` makes its own repositories in a temp folder and never touches this one.
//! `markers` wants the drivers built (`cargo build` in `drivers/python`, and in `drivers/bend`); a driver path that is not there is skipped, loudly.
//! `race-writer` and `race-reader` are `race`'s own children, started by it.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions, TryLockError};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use vnoc::driver_host::{DriverHost, extension_of};
use vnoc::project::Project;
use vnoc::scan;
use vnoc::source;
use vnoc_driver_protocol::Def;

type Fail = Box<dyn std::error::Error>;

const USAGE: &str = "usage:\n  tours_checks race <dir>\n  tours_checks markers <root> <driver>...\n  tours_checks git\n  tours_checks stat <dir>\n  tours_checks measure-check <dir> [driver]";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("race") if args.len() == 2 => race(Path::new(&args[1])),
        Some("race-writer") if args.len() == 3 => race_writer(Path::new(&args[1]), &args[2]),
        Some("race-reader") if args.len() == 2 => race_reader(Path::new(&args[1])),
        Some("markers") if args.len() >= 3 => markers(Path::new(&args[1]), &args[2..]),
        Some("git") if args.len() == 1 => git_checks(),
        Some("stat") if args.len() == 2 => stat(Path::new(&args[1])),
        Some("measure-check") if (2..=3).contains(&args.len()) => {
            measure_check(Path::new(&args[1]), args.get(2).map(String::as_str))
        }
        _ => Err(USAGE.into()),
    };
    if let Err(message) = result {
        eprintln!("{message}");
        std::process::exit(2);
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// A fresh, empty folder under the system's temp folder.
fn temp_folder(label: &str) -> Result<PathBuf, Fail> {
    let dir =
        std::env::temp_dir().join(format!("vnoc-tours-checks-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn median_min_max(samples: &mut [f64]) -> (f64, f64, f64) {
    samples.sort_by(f64::total_cmp);
    let n = samples.len();
    let median = if n % 2 == 1 {
        samples[n / 2]
    } else {
        (samples[n / 2 - 1] + samples[n / 2]) / 2.0
    };
    (median, samples[0], samples[n - 1])
}

// ------------------------------------------------------------------------------------------------------------- 3: race

const LINES_EACH: u64 = 200;

/// The wait loop of `userdata/lock.rs` `WriteLock::take`: `try_lock` every 50 ms, 5 s at most. Returns the open lock file (the lock lives as long as
/// it does) and how long it waited.
fn take_lock(path: &Path) -> Result<(fs::File, Duration), Fail> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok((file, started.elapsed())),
            Err(TryLockError::Error(e)) => return Err(e.into()),
            Err(TryLockError::WouldBlock) => {
                if started.elapsed() >= Duration::from_secs(5) {
                    return Err("the lock was busy for 5 s".into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// One writer: 200 times, the lock, one whole line in one `write_all`, the lock let go.
fn race_writer(dir: &Path, name: &str) -> Result<(), Fail> {
    let w: u64 = name.parse()?;
    let mut longest = Duration::ZERO;
    for i in 0..LINES_EACH {
        let (lock, waited) = take_lock(&dir.join("tours.lock"))?;
        longest = longest.max(waited);
        let line = format!("{{\"w\":{w},\"i\":{i}}}\n");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("t.jsonl"))?;
        file.write_all(line.as_bytes())?;
        drop(file);
        drop(lock);
    }
    println!("longest_wait_ms={:.1}", ms(longest));
    Ok(())
}

/// What the reader saw in one whole read of the file: the complete lines, those that do not parse, and whether a last line had no `\n`.
struct Seen {
    lines: usize,
    broken: usize,
    torn_tail: bool,
    ids: BTreeSet<(u64, u64)>,
}

fn look(bytes: &[u8]) -> Seen {
    let text = String::from_utf8_lossy(bytes);
    let torn_tail = !text.is_empty() && !text.ends_with('\n');
    let mut parts: Vec<&str> = text.split('\n').collect();
    parts.pop(); // what follows the last `\n`: empty, or a line that is not finished (not an error, not read)
    let mut seen = Seen {
        lines: 0,
        broken: 0,
        torn_tail,
        ids: BTreeSet::new(),
    };
    for part in parts {
        seen.lines += 1;
        let parsed: Option<(u64, u64)> = serde_json::from_str::<serde_json::Value>(part)
            .ok()
            .and_then(|v| Some((v.get("w")?.as_u64()?, v.get("i")?.as_u64()?)));
        match parsed {
            Some(id) => {
                seen.ids.insert(id);
            }
            None => seen.broken += 1,
        }
    }
    seen
}

/// The reader: the whole file, again and again, until `<dir>/done` exists; then once more (the answer).
fn race_reader(dir: &Path) -> Result<(), Fail> {
    let (file, done) = (dir.join("t.jsonl"), dir.join("done"));
    let (mut reads, mut errors, mut broken_mid, mut torn_tails) = (0u64, 0u64, 0u64, 0u64);
    loop {
        let finished = done.exists();
        match fs::read(&file) {
            Ok(bytes) => {
                reads += 1;
                let seen = look(&bytes);
                broken_mid += seen.broken as u64;
                torn_tails += u64::from(seen.torn_tail);
                if finished {
                    println!(
                        "reads={reads} read_errors={errors} broken_in_any_read={broken_mid} reads_with_a_last_line_unfinished={torn_tails} \
                         lines={} broken={} distinct={}",
                        seen.lines,
                        seen.broken,
                        seen.ids.len()
                    );
                    return Ok(());
                }
            }
            // before the first writer made the file: not a failure of what is measured
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !finished => {}
            Err(e) => {
                errors += 1;
                eprintln!("read error: {e}");
                if finished {
                    println!(
                        "reads={reads} read_errors={errors} broken_in_any_read={broken_mid} reads_with_a_last_line_unfinished={torn_tails} lines=0 broken=0 distinct=0"
                    );
                    return Ok(());
                }
            }
        }
        std::thread::sleep(Duration::from_micros(200));
    }
}

fn race(dir: &Path) -> Result<(), Fail> {
    fs::create_dir_all(dir)?;
    for name in ["t.jsonl", "tours.lock", "done"] {
        let _ = fs::remove_file(dir.join(name));
    }
    let exe = std::env::current_exe()?;
    let spawn = |args: &[&str]| Command::new(&exe).args(args).stdout(Stdio::piped()).spawn();
    let d = dir.to_string_lossy().into_owned();
    let reader = spawn(&["race-reader", &d])?;
    let writers = [
        spawn(&["race-writer", &d, "1"])?,
        spawn(&["race-writer", &d, "2"])?,
    ];
    let mut waits = Vec::new();
    let mut writers_ok = true;
    for writer in writers {
        let out = writer.wait_with_output()?;
        writers_ok &= out.status.success();
        waits.push(String::from_utf8_lossy(&out.stdout).trim().to_string());
    }
    fs::write(dir.join("done"), "")?;
    let out = reader.wait_with_output()?;
    let said = String::from_utf8_lossy(&out.stdout);
    let field = |key: &str| -> String {
        said.split_whitespace()
            .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
            .unwrap_or("?")
            .to_string()
    };
    let longest = waits
        .iter()
        .filter_map(|w| w.strip_prefix("longest_wait_ms=")?.parse::<f64>().ok())
        .fold(0.0, f64::max);
    println!("race in {} ({}):", dir.display(), std::env::consts::OS);
    println!("  writers finished ok       {writers_ok}");
    println!(
        "  lines found               {} (expected {})",
        field("lines"),
        2 * LINES_EACH
    );
    println!("  distinct (w, i) found     {}", field("distinct"));
    println!("  broken lines (final read) {}", field("broken"));
    println!(
        "  broken lines, any read    {}",
        field("broken_in_any_read")
    );
    println!("  read errors               {}", field("read_errors"));
    println!("  reads done                {}", field("reads"));
    println!(
        "  reads that ended on a line with no newline  {}",
        field("reads_with_a_last_line_unfinished")
    );
    println!("  longest wait for the lock {longest:.1} ms");
    Ok(())
}

// --------------------------------------------------------------------------------------------------------- 4: markers

/// One def, with where it is.
struct Row {
    language: String,
    path: String,
    qualname: String,
    id: Option<String>,
    /// 1-based lines of the def's own range that contain the ID text
    lines: Vec<u32>,
}

/// The 1-based lines of `def`'s own range that contain `text`.
fn lines_with(file: &Path, def: &Def, text: &str) -> Vec<u32> {
    let Ok(excerpt) = source::excerpt(file, &def.range, 0) else {
        return Vec::new();
    };
    excerpt
        .lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(text))
        .map(|(n, _)| excerpt.first + n as u32)
        .collect()
}

fn inside(inner: &Def, outer: &Def) -> bool {
    inner.range.start.line >= outer.range.start.line && inner.range.end.line <= outer.range.end.line
}

fn markers(root: &Path, drivers: &[String]) -> Result<(), Fail> {
    let mut found: Vec<PathBuf> = Vec::new();
    for driver in drivers {
        let path = PathBuf::from(driver);
        if path.is_file() {
            found.push(path);
        } else {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_prefix("vnoc-driver-"))
                .unwrap_or("?");
            let name = name.strip_suffix(".exe").unwrap_or(name);
            let mut shown = name.to_string();
            if let Some(first) = shown.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            println!("skipped: no {shown} driver ({driver} is not there)");
        }
    }
    if found.is_empty() {
        return Ok(());
    }
    let project = Project::open(root)?;
    let host = DriverHost::default();
    host.set_paths(found);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(host.start_all());
    let languages = rt.block_on(host.languages());
    let language_of = |file: &str| -> String {
        let ext = extension_of(file).unwrap_or("");
        languages
            .iter()
            .find(|l| l.extensions.iter().any(|e| e == ext))
            .map_or("?".to_string(), |l| l.name.clone())
    };
    let walked = scan::files(&project, &host.extensions(), 20_000);

    let mut rows: Vec<Row> = Vec::new();
    // (language, file, outer qualname, nested qualname, the nested ID, lines of the outer def that hold it)
    let mut nested: Vec<(String, String, String, String, String, Vec<u32>)> = Vec::new();
    let mut file_ids: Vec<(String, String, String, Vec<u32>)> = Vec::new();
    for repo_path in &walked.files {
        let file = repo_path.as_str();
        let on_disk = project.git_root().join(file);
        let outline = rt.block_on(host.outline(project.git_root(), file))?;
        let language = language_of(file);
        for def in &outline.defs {
            rows.push(Row {
                language: language.clone(),
                path: file.to_string(),
                qualname: def.qualname.clone(),
                id: def.id.clone(),
                lines: def
                    .id
                    .as_deref()
                    .map(|id| lines_with(&on_disk, def, id))
                    .unwrap_or_default(),
            });
            for other in &outline.defs {
                let Some(id) = other.id.as_deref() else {
                    continue;
                };
                if std::ptr::eq(other, def) || !inside(other, def) {
                    continue;
                }
                nested.push((
                    language.clone(),
                    file.to_string(),
                    def.qualname.clone(),
                    other.qualname.clone(),
                    id.to_string(),
                    lines_with(&on_disk, def, id),
                ));
            }
        }
        if let Some(id) = outline.file_id.as_deref() {
            let text = source::read_text(&on_disk)?;
            let at: Vec<u32> = text
                .lines()
                .enumerate()
                .filter(|(_, l)| l.contains(id))
                .map(|(n, _)| n as u32 + 1)
                .collect();
            file_ids.push((language.clone(), file.to_string(), id.to_string(), at));
        }
    }

    println!("markers in {}", root.display());
    let names: BTreeSet<&str> = rows.iter().map(|r| r.language.as_str()).collect();
    for language in names {
        let of_language: Vec<&Row> = rows.iter().filter(|r| r.language == language).collect();
        let with_id: Vec<&&Row> = of_language.iter().filter(|r| r.id.is_some()).collect();
        let exactly_one = with_id.iter().filter(|r| r.lines.len() == 1).count();
        println!("{language}:");
        println!(
            "  defs {} · with an ID {} · exactly one line holding it {}",
            of_language.len(),
            with_id.len(),
            exactly_one
        );
        for row in with_id.iter().filter(|r| r.lines.len() != 1) {
            println!(
                "  NOT ONE LINE: {}::{}  id {}  lines in its range {:?}",
                row.path,
                row.qualname,
                row.id.as_deref().unwrap_or(""),
                row.lines
            );
            if let Some(id) = row.id.as_deref()
                && let Ok(text) = source::read_text(&project.git_root().join(&row.path))
            {
                let anywhere: Vec<usize> = text
                    .lines()
                    .enumerate()
                    .filter(|(_, l)| l.contains(id))
                    .map(|(n, _)| n + 1)
                    .collect();
                println!("      the ID text is on these lines of the file: {anywhere:?}");
            }
        }
        for row in with_id.iter().filter(|r| r.lines.len() == 1) {
            println!(
                "  ok: {}::{}  id {}  line {}",
                row.path,
                row.qualname,
                row.id.as_deref().unwrap_or(""),
                row.lines[0]
            );
        }
        let pairs: Vec<_> = nested.iter().filter(|n| n.0 == language).collect();
        let one = pairs.iter().filter(|n| n.5.len() == 1).count();
        println!(
            "  a def and a def nested in it, found by the nested one's ID: {} pairs, {} with exactly one line",
            pairs.len(),
            one
        );
        for n in pairs.iter().filter(|n| n.5.len() != 1) {
            println!(
                "  NOT ONE LINE: {}::{} holds the ID {} of {}  on lines {:?}",
                n.1, n.2, n.4, n.3, n.5
            );
        }
        for n in pairs.iter().filter(|n| n.5.len() == 1) {
            println!(
                "  ok: {}::{} holds the ID of {} on line {}",
                n.1, n.2, n.3, n.5[0]
            );
        }
        for f in file_ids.iter().filter(|f| f.0 == language) {
            println!(
                "  FileID {} of {}: on lines {:?} of the file",
                f.2, f.1, f.3
            );
        }
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------------------- 6: git

/// What this program asks git. **Every argument list is built here**, and a path always comes after `--` (or, for `show`, inside `<commit>:<path>`).
enum Git<'a> {
    Init,
    Add(&'a [&'a str]),
    Commit(&'a str),
    CommitOnly(&'a str, &'a [&'a str]),
    RevParseHead,
    RevParseVerifyHead,
    Status(&'a [&'a str]),
    StatusAll,
    Show(&'a str, &'a str),
    /// the same, with the guard `--` in front, to see what git does with it
    ShowAfterDashes(&'a str, &'a str),
    ShowAfterEndOfOptions(&'a str, &'a str),
    CommittedNames,
    WorktreeAdd(&'a Path, &'a str),
}

fn git_args(call: &Git) -> Vec<String> {
    let s = |x: &str| x.to_string();
    let with_paths = |mut head: Vec<String>, paths: &[&str]| {
        head.push(s("--"));
        head.extend(paths.iter().map(|p| p.to_string()));
        head
    };
    match call {
        Git::Init => vec![s("init"), s("-q"), s("-b"), s("main")],
        Git::Add(paths) => with_paths(vec![s("add")], paths),
        Git::Commit(message) => vec![s("commit"), s("-q"), s("-m"), s(message)],
        Git::CommitOnly(message, paths) => with_paths(
            vec![s("commit"), s("-q"), s("--only"), s("-m"), s(message)],
            paths,
        ),
        Git::RevParseHead => vec![s("rev-parse"), s("HEAD")],
        Git::RevParseVerifyHead => vec![s("rev-parse"), s("--verify"), s("-q"), s("HEAD")],
        Git::Status(paths) => with_paths(vec![s("status"), s("--porcelain"), s("-z")], paths),
        Git::StatusAll => vec![s("status"), s("--porcelain")],
        Git::Show(commit, path) => vec![s("show"), format!("{commit}:{path}")],
        Git::ShowAfterDashes(commit, path) => vec![s("show"), s("--"), format!("{commit}:{path}")],
        Git::ShowAfterEndOfOptions(commit, path) => {
            vec![s("show"), s("--end-of-options"), format!("{commit}:{path}")]
        }
        Git::CommittedNames => vec![s("show"), s("--name-only"), s("--format="), s("HEAD")],
        Git::WorktreeAdd(dir, branch) => vec![
            s("worktree"),
            s("add"),
            s("-q"),
            s("-b"),
            s(branch),
            dir.to_string_lossy().into_owned(),
        ],
    }
}

struct Out {
    /// `None`: git could not be started
    code: Option<i32>,
    stdout: String,
    stderr: String,
    spawn_error: Option<String>,
    took: Duration,
}

/// Runs git in `repo` with a configuration that does not depend on this machine's (no global or system file, a fixed author, English).
fn run_in(repo: &Path, call: &Git, path_env: Option<&Path>) -> Out {
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo)
        .args(git_args(call))
        .env("GIT_CONFIG_GLOBAL", null)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .env("LC_ALL", "C");
    if let Some(path) = path_env {
        command.env("PATH", path);
    }
    let started = Instant::now();
    match command.output() {
        Ok(out) => Out {
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            spawn_error: None,
            took: started.elapsed(),
        },
        Err(e) => Out {
            code: None,
            stdout: String::new(),
            stderr: String::new(),
            spawn_error: Some(e.to_string()),
            took: started.elapsed(),
        },
    }
}

fn run(repo: &Path, call: &Git) -> Out {
    run_in(repo, call, None)
}

/// Setup that must work: a failure ends the program.
fn must(repo: &Path, call: &Git) -> Result<(), Fail> {
    let out = run(repo, call);
    if out.code == Some(0) {
        Ok(())
    } else {
        Err(format!("setup failed: git {:?}: {}", git_args(call), out.stderr).into())
    }
}

/// A repository with these files committed (each holds its own name).
fn repository(dir: &Path, files: &[&str]) -> Result<(), Fail> {
    fs::create_dir_all(dir)?;
    must(dir, &Git::Init)?;
    for f in files {
        fs::write(dir.join(f), format!("{f}\n"))?;
    }
    if !files.is_empty() {
        must(dir, &Git::Add(files))?;
        must(dir, &Git::Commit("first"))?;
    }
    Ok(())
}

struct Shown {
    root: PathBuf,
    root_real: PathBuf,
}

impl Shown {
    fn text(&self, s: &str) -> String {
        s.replace(&*self.root_real.to_string_lossy(), "<tmp>")
            .replace(&*self.root.to_string_lossy(), "<tmp>")
            .trim_end()
            .replace('\n', " ⏎ ")
    }

    /// One line: exit code, stdout, stderr.
    fn line(&self, label: &str, out: &Out) {
        match &out.spawn_error {
            Some(e) => println!("  {label:<46} could not be started: {}", self.text(e)),
            None => println!(
                "  {label:<46} exit {} | stdout {:?} | stderr {:?}",
                out.code.map_or("signal".to_string(), |c| c.to_string()),
                self.text(&out.stdout),
                self.text(&out.stderr)
            ),
        }
    }
}

fn git_checks() -> Result<(), Fail> {
    let root = temp_folder("git")?;
    let shown = Shown {
        root_real: dunce::canonicalize(&root)?,
        root: root.clone(),
    };
    let version = run(&root, &Git::StatusAll); // only to make sure git runs at all
    if version.spawn_error.is_some() {
        return Err("git is not on the PATH: nothing to measure".into());
    }
    println!(
        "git from Rust, {} ({}), `{}`",
        std::env::consts::OS,
        std::env::consts::ARCH,
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    );
    println!(
        "(no global or system git configuration is read; the author is fixed; a process is started for each call)"
    );

    // ---- the measure: 12 paths, 9 runs each
    let names: Vec<String> = (0..12).map(|n| format!("f{n:02}.txt")).collect();
    let paths: Vec<&str> = names.iter().map(String::as_str).collect();
    let measured = root.join("measure");
    repository(&measured, &paths)?;
    let commit = run(&measured, &Git::RevParseHead).stdout.trim().to_string();
    for (n, name) in paths.iter().enumerate().take(4) {
        fs::write(measured.join(name), format!("changed {n}\n"))?;
    }
    println!("\n[measure] 9 runs each, in ms (4 of the 12 files modified, the others clean)");
    println!("  {:<46} {:>8} {:>8} {:>8}", "", "median", "min", "max");
    let calls: [(&str, Git); 3] = [
        ("rev-parse HEAD", Git::RevParseHead),
        ("status --porcelain -z -- <12 paths>", Git::Status(&paths)),
        ("show <commit>:<path>", Git::Show(&commit, paths[0])),
    ];
    for (label, call) in &calls {
        let mut samples: Vec<f64> = (0..9)
            .map(|_| {
                let out = run(&measured, call);
                assert_eq!(out.code, Some(0), "{label}: {}", out.stderr);
                ms(out.took)
            })
            .collect();
        let (median, min, max) = median_min_max(&mut samples);
        println!("  {label:<46} {median:>8.1} {min:>8.1} {max:>8.1}");
    }

    // ---- what each gives
    println!("\nwhat git gives (exit code, stdout, stderr):");
    let base = root.join("base");
    repository(&base, &["a.txt", "b.txt", "c.txt"])?;
    let worktree = root.join("wt");
    must(&base, &Git::WorktreeAdd(&worktree, "wt"))?;
    shown.line(
        "rev-parse HEAD, in a linked worktree",
        &run(&worktree, &Git::RevParseHead),
    );
    let empty = root.join("nocommit");
    repository(&empty, &[])?;
    shown.line(
        "rev-parse HEAD, no commit yet",
        &run(&empty, &Git::RevParseHead),
    );
    shown.line(
        "rev-parse --verify -q HEAD, no commit yet",
        &run(&empty, &Git::RevParseVerifyHead),
    );
    let nowhere = root.join("nowhere");
    fs::create_dir_all(&nowhere)?;
    let ceiling = shown.root_real.clone();
    let outside = {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&nowhere)
            .args(git_args(&Git::RevParseHead))
            .env("GIT_CEILING_DIRECTORIES", &ceiling)
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .env("LC_ALL", "C");
        let out = command.output()?;
        Out {
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            spawn_error: None,
            took: Duration::ZERO,
        }
    };
    shown.line("rev-parse HEAD, outside any repository", &outside);
    let no_git = root.join("no-git-on-path");
    fs::create_dir_all(&no_git)?;
    shown.line(
        "rev-parse HEAD, PATH without git",
        &run_in(&base, &Git::RevParseHead, Some(&no_git)),
    );

    let missing = "0123456789abcdef0123456789abcdef01234567";
    let head = run(&base, &Git::RevParseHead).stdout.trim().to_string();
    shown.line(
        "show <40 hex, not a commit>:a.txt",
        &run(&base, &Git::Show(missing, "a.txt")),
    );
    shown.line(
        "show <HEAD's 40 hex>:a.txt",
        &run(&base, &Git::Show(&head, "a.txt")),
    );
    shown.line(
        "show <HEAD>:nope.txt (no such path)",
        &run(&base, &Git::Show(&head, "nope.txt")),
    );
    shown.line(
        "show -- <HEAD>:a.txt (the guard `--`)",
        &run(&base, &Git::ShowAfterDashes(&head, "a.txt")),
    );
    shown.line(
        "show --end-of-options <HEAD>:a.txt",
        &run(&base, &Git::ShowAfterEndOfOptions(&head, "a.txt")),
    );

    // commit --only with a third file staged
    println!("\ncommit --only -m x -- a.txt b.txt, with c.txt staged:");
    for (label, c_is_new) in [
        ("a third file: new, staged", true),
        ("a third file: tracked, modified, staged", false),
    ] {
        let repo = root.join(if c_is_new { "only-new" } else { "only-mod" });
        repository(&repo, &["a.txt", "b.txt", "c.txt"])?;
        fs::write(repo.join("a.txt"), "a2\n")?;
        fs::write(repo.join("b.txt"), "b2\n")?;
        if c_is_new {
            fs::write(repo.join("d.txt"), "d\n")?;
            must(&repo, &Git::Add(&["d.txt"]))?;
        } else {
            fs::write(repo.join("c.txt"), "c2\n")?;
            must(&repo, &Git::Add(&["c.txt"]))?;
        }
        let staged = if c_is_new { "d.txt" } else { "c.txt" };
        shown.line(
            &format!("[{label}] commit --only"),
            &run(&repo, &Git::CommitOnly("x", &["a.txt", "b.txt"])),
        );
        shown.line(
            &format!("[{label}] files in the commit"),
            &run(&repo, &Git::CommittedNames),
        );
        let after = run(&repo, &Git::StatusAll);
        shown.line(
            &format!("[{label}] status after ({staged} was the staged one)"),
            &after,
        );
    }

    // b untracked
    println!("\ncommit --only -m x -- a.txt b.txt, with b.txt untracked:");
    let repo = root.join("only-untracked");
    repository(&repo, &["a.txt", "c.txt"])?;
    fs::write(repo.join("a.txt"), "a2\n")?;
    fs::write(repo.join("b.txt"), "b\n")?;
    shown.line(
        "commit --only, b.txt not added",
        &run(&repo, &Git::CommitOnly("x", &["a.txt", "b.txt"])),
    );
    shown.line(
        "HEAD is still the first commit?",
        &run(&repo, &Git::CommittedNames),
    );
    shown.line("add -- b.txt", &run(&repo, &Git::Add(&["b.txt"])));
    shown.line(
        "commit --only, after add",
        &run(&repo, &Git::CommitOnly("x", &["a.txt", "b.txt"])),
    );
    shown.line("files in the commit", &run(&repo, &Git::CommittedNames));

    // a failing pre-commit hook
    println!(
        "\ncommit --only -m x -- a.txt b.txt, with a pre-commit hook that exits 1 (c.txt staged):"
    );
    let repo = root.join("only-hook");
    repository(&repo, &["a.txt", "b.txt", "c.txt"])?;
    let hook = repo.join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\necho 'hook says no' >&2\nexit 1\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
    }
    fs::write(repo.join("a.txt"), "a2\n")?;
    fs::write(repo.join("b.txt"), "b2\n")?;
    fs::write(repo.join("c.txt"), "c2\n")?;
    must(&repo, &Git::Add(&["c.txt"]))?;
    let before = run(&repo, &Git::RevParseHead).stdout;
    shown.line(
        "commit --only",
        &run(&repo, &Git::CommitOnly("x", &["a.txt", "b.txt"])),
    );
    let after = run(&repo, &Git::RevParseHead).stdout;
    println!("  {:<46} {}", "HEAD moved?", before != after);
    shown.line("status after", &run(&repo, &Git::StatusAll));

    // status for a path of each kind
    println!("\nstatus --porcelain -z -- <path>:");
    let repo = root.join("status");
    fs::create_dir_all(&repo)?;
    repository(&repo, &["m.txt", "s.txt", "k.txt"])?;
    fs::write(repo.join(".gitignore"), "ign.txt\n")?;
    must(&repo, &Git::Add(&[".gitignore"]))?;
    must(&repo, &Git::Commit("ignore"))?;
    fs::write(repo.join("m.txt"), "m2\n")?;
    fs::write(repo.join("s.txt"), "s2\n")?;
    must(&repo, &Git::Add(&["s.txt"]))?;
    fs::write(repo.join("u.txt"), "u\n")?;
    fs::write(repo.join("ign.txt"), "i\n")?;
    for (label, path) in [
        ("a modified file", "m.txt"),
        ("a staged file", "s.txt"),
        ("an untracked file", "u.txt"),
        ("an ignored file", "ign.txt"),
        ("a path that does not exist", "nope.txt"),
        ("a file that is clean", "k.txt"),
    ] {
        shown.line(label, &run(&repo, &Git::Status(&[path])));
    }
    shown.line(
        "all six in one call",
        &run(
            &repo,
            &Git::Status(&["m.txt", "s.txt", "u.txt", "ign.txt", "nope.txt", "k.txt"]),
        ),
    );
    let _ = fs::remove_dir_all(&root);
    Ok(())
}

// ------------------------------------------------------------------------------------------------------------- 7: stat

fn stat_of(path: &Path) -> Result<(u64, u128), Fail> {
    let meta = fs::metadata(path)?;
    let nanos = meta.modified()?.duration_since(UNIX_EPOCH)?.as_nanos();
    Ok((meta.len(), nanos))
}

fn stat(dir: &Path) -> Result<(), Fail> {
    fs::create_dir_all(dir)?;
    let file = dir.join("t.bin");
    let rounds = 100;
    let (mut unchanged, mut length_changed) = (0, 0);
    let mut differences: Vec<u128> = Vec::new();
    let mut seen: Vec<u128> = Vec::new();
    for i in 0..rounds {
        let first = vec![b'a' + (i % 26) as u8; 4096];
        let second = vec![b'A' + (i % 26) as u8; 4096];
        fs::write(&file, &first)?;
        let (len_a, time_a) = stat_of(&file)?;
        fs::write(&file, &second)?;
        let (len_b, time_b) = stat_of(&file)?;
        length_changed += usize::from(len_a != len_b);
        seen.extend([time_a, time_b]);
        if time_a == time_b {
            unchanged += 1;
        } else {
            differences.push(time_b.abs_diff(time_a));
        }
    }
    let resolution = [1_000_000_000u128, 1_000_000, 1_000, 1]
        .into_iter()
        .find(|unit| seen.iter().all(|t| t % unit == 0))
        .unwrap_or(1);
    let unit = match resolution {
        1_000_000_000 => "1 s",
        1_000_000 => "1 ms",
        1_000 => "1 µs",
        _ => "1 ns",
    };
    println!(
        "stat in {} ({}): write {} bytes, stat, write {} other bytes at once, stat; {rounds} times",
        dir.display(),
        std::env::consts::OS,
        4096,
        4096
    );
    println!("  size changed between the two writes       {length_changed} times (expected 0)");
    println!("  changed-time did NOT change               {unchanged} of {rounds}");
    println!(
        "  smallest non-zero difference              {}",
        differences
            .iter()
            .min()
            .map_or("none".to_string(), |d| format!("{d} ns"))
    );
    println!(
        "  largest difference                        {}",
        differences
            .iter()
            .max()
            .map_or("none".to_string(), |d| format!("{d} ns"))
    );
    println!("  every time seen is a multiple of          {unit}");
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    println!("  (the clock now: {now} ns since 1970)");
    let _ = fs::remove_file(&file);
    Ok(())
}

// ------------------------------------------------------------------------------------------ TR5: measure-check

/// A function of 12 lines: its lines 3 to 11 are what the blocks light.
fn generated_function(n: usize) -> String {
    format!(
        "def f_{n}(x):\n    \"\"\"Function {n}.\"\"\"\n    a = x + 1\n    b = a * 2\n    c = b - 1\n    d = c + a\n    e = d * 2\n    f = e - b\n    g = f + c\n    h = g - d\n    i = h + 1\n    return i\n\n\n"
    )
}

/// Times `work` nine times and says its median, smallest and largest in microseconds.
fn nine(label: &str, mut work: impl FnMut()) -> f64 {
    let mut samples: Vec<f64> = (0..9)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed().as_secs_f64() * 1_000_000.0
        })
        .collect();
    let (median, min, max) = median_min_max(&mut samples);
    println!("  {label:<42} median {median:>9.0} µs   (min {min:.0}, max {max:.0})");
    median
}

/// TR5's [measure] (03 §1, §3): a tour of 40 stops and 100 blocks over 12 files, made by the real `plan_stop` and `append`, then
///   - replay: `store::read` (read the file, fold it);
///   - a real check: `check::check` (find every stop, read each of the 12 files once, hash every stop and block);
///   - a recorded one: `check::recall` (the state file, one `stat` for each file and the tour's);
///
/// Each is timed nine times, and the median is the number. The driver builds the index once, which is not what is timed; building the tree (`Index::tree`) is shown apart.
fn measure_check(dir: &Path, driver: Option<&str>) -> Result<(), Fail> {
    use vnoc::cli::project::open;
    use vnoc::tours::add::{AddOptions, plan_stop};
    use vnoc::tours::check::{self, Code};
    use vnoc::tours::record::Detail;
    use vnoc::tours::store::{self, StartParams};

    let driver = driver
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("VNOC_DRIVER").map(PathBuf::from))
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../drivers/python/target/debug")
                .join(format!(
                    "vnoc-driver-python{}",
                    std::env::consts::EXE_SUFFIX
                ))
        });
    if !driver.is_file() {
        return Err(format!(
            "no Python driver at {} (build it, or give its path)",
            driver.display()
        )
        .into());
    }
    let _ = fs::remove_dir_all(dir);
    fs::create_dir_all(dir)?;
    let root = dunce::canonicalize(dir)?;

    // 12 files: four of them with four functions, eight with three
    let mut layout: Vec<(String, Vec<usize>)> = Vec::new();
    let mut next = 0;
    for file in 0..12 {
        let count = if file < 4 { 4 } else { 3 };
        let ids: Vec<usize> = (next..next + count).collect();
        next += count;
        let text: String = ids.iter().map(|n| generated_function(*n)).collect();
        fs::write(root.join(format!("m{file:02}.py")), text)?;
        layout.push((format!("m{file:02}.py"), ids));
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let opened = rt
        .block_on(open(&root, &[driver]))
        .map_err(|e| format!("{e}"))?;
    let tree = opened
        .index
        .read()
        .map_err(|e| e.to_string())?
        .tree(&opened.project.name(), opened.project.prefix());

    let started = store::start(
        &root,
        StartParams {
            title: "measured".to_string(),
            question: None,
            author: None,
            detail: Detail::Normal,
        },
    )?;
    let vnoc_root = store::vnoc_root(&root);
    let mut blocks = 0;
    let mut stops = 0;
    for (file, ids) in &layout {
        for (place, n) in ids.iter().enumerate() {
            // each function is 14 lines with the two blank ones after it
            let at = 1 + 14 * place;
            let l = |from: usize, to: usize| format!("{}-{}", at + from, at + to);
            // 20 stops with three blocks and 20 with two: 100 blocks
            let body = if stops % 2 == 0 {
                format!(
                    "Function {n} adds and doubles.\n\n## {} First\nIt starts.\n\n## {} Second\nIt goes on.\n\n## {} Third\nIt ends.\n",
                    l(2, 4),
                    l(5, 7),
                    l(8, 10)
                )
            } else {
                format!(
                    "Function {n} adds and doubles.\n\n## {} First\nIt starts.\n\n## {} Second\nIt ends.\n",
                    l(2, 5),
                    l(6, 10)
                )
            };
            blocks += if stops % 2 == 0 { 3 } else { 2 };
            stops += 1;
            let loaded = store::read(&vnoc_root, &started.id)?;
            let tour = loaded.1.map_err(|e| e.to_string())?;
            let node = tree
                .find(&format!("{file}::f_{n}"))
                .ok_or("a generated function is not in the index")?;
            let records = {
                let index = opened.index.read().map_err(|e| e.to_string())?;
                plan_stop(
                    &tour,
                    &index,
                    &opened.project,
                    &tree,
                    node,
                    &body,
                    AddOptions::default(),
                )
                .map_err(|r| format!("{r:?}"))?
            };
            store::append(&vnoc_root, &started.id, &records)?;
        }
    }
    let path = started.path.clone();
    let len = fs::metadata(&path)?.len();
    println!(
        "a tour of {stops} stops and {blocks} blocks over {} files; its file is {len} bytes\n",
        layout.len()
    );

    let index = opened.index.read().map_err(|e| e.to_string())?;
    let mut tour = None;
    let replay = nine("replay: read the file and fold it", || {
        tour = store::read(&vnoc_root, &started.id)
            .ok()
            .and_then(|l| l.1.ok());
    });
    let tour = tour.ok_or("the tour does not fold")?;
    assert_eq!(tour.stops.len(), 40);
    let project_name = opened.project.name();
    nine("Index::tree (not part of the check)", || {
        let _ = index.tree(&project_name, opened.project.prefix());
    });
    let mut recorded = None;
    let real = nine("a real check: find, read 12 files, compare", || {
        let code = Code {
            index: &index,
            project: &opened.project,
            tree: &tree,
        };
        let mut read = |p: &Path| source::read_text(p);
        recorded = Some(check::check(&code, &mut read, &tour, len));
    });
    let recorded = recorded.ok_or("no check")?;
    println!(
        "    (fresh: {}, files looked at: {})",
        recorded.fresh(),
        recorded.files.len()
    );
    check::record(&vnoc_root, &started.id, &recorded)?;
    let mut held = false;
    let valid = nine("a valid record: the state file and 13 stats", || {
        held = check::recall(&vnoc_root, &root, &started.id, &path).is_some();
    });
    if !held {
        return Err("the record did not hold".into());
    }
    println!(
        "\nreplay {replay:.0} µs · real check {real:.0} µs · valid record {valid:.0} µs  (medians of 9)"
    );
    Ok(())
}
