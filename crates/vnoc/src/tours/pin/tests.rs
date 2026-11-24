//! Every case here makes its own repository in a temp folder (never this one), as `tests/cli.rs` does for `check --changed`.

use std::path::{Path, PathBuf};
use std::process::Command;

use vnoc_domain::RepoPath;

use super::*;
use crate::tours::check::{Placed, Recorded};
use crate::tours::fold::fold;
use crate::tours::record::{Lines, StepId, WhereAt};

/// A new, empty folder, with a git repository in it that commits with no hook, no signing and an identity of its own.
fn repo(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vnoc-pin-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    crate::testdir::remove_at_end(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dunce::canonicalize(dir).unwrap();
    git(&dir, &["init", "-q"]);
    for (key, value) in [
        ("user.name", "t"),
        ("user.email", "t@t"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", ".git/hooks"),
    ] {
        git(&dir, &["config", key, value]);
    }
    dir
}

fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git is installed");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn write(root: &Path, name: &str, text: &str) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Files committed in one commit.
fn commit_all(root: &Path, files: &[(&str, &str)]) {
    for (name, text) in files {
        write(root, name, text);
    }
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "base"]);
}

fn project(root: &Path) -> Project {
    Project::open(root).unwrap()
}

fn path(text: &str) -> RepoPath {
    RepoPath::new(text).unwrap()
}

/// A finished tour with one stop on each of these files (functions at lines 1-2 of each).
fn finished(files: &[&str]) -> Tour {
    let mut lines = vec![
        r#"{"type":"tour","schema":1,"id":"t-0000","title":"T","at":"x","detail":"normal"}"#
            .to_string(),
    ];
    for (i, file) in files.iter().enumerate() {
        lines.push(format!(
            r#"{{"type":"stop","id":"s{}","path":"{file}","qualname":"f","kind":"function","lines":[1,2],"hash":"0123456789abcdef","text":"t"}}"#,
            i + 1
        ));
    }
    lines.push(r#"{"type":"end","at":"x"}"#.to_string());
    fold(&format!("{}\n", lines.join("\n"))).unwrap()
}

fn placed(files: &[&str]) -> Placed {
    Placed {
        stops: files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                (
                    StopId::new(i as u32 + 1).unwrap(),
                    WhereAt {
                        path: path(file),
                        lines: Lines::new(1, 2).unwrap(),
                    },
                )
            })
            .collect(),
        blocks: Vec::new(),
        read: Default::default(),
    }
}

fn fresh() -> Recorded {
    Recorded::empty(0)
}

fn head_of(root: &Path) -> Commit {
    Commit::parse(git(root, &["rev-parse", "HEAD"]).trim()).unwrap()
}

fn answer(root: &Path, files: &[&str]) -> Pinnable {
    let project = project(root);
    let tour = finished(files);
    let head = head(&project, &tour).expect("a repository with a commit");
    can_pin(&project, &tour, head, &fresh(), &placed(files))
}

fn reasons(p: Pinnable) -> Vec<Reason> {
    match p {
        Pinnable::No(reasons) => reasons,
        Pinnable::Yes { head } => panic!("pinnable at {}", head.as_str()),
    }
}

// ------------------------------------------------------------------------------------------------------------------------------ head

#[test]
fn what_needs_no_code_is_asked_first_and_each_answer_has_its_reason() {
    let tour = finished(&["a.py"]);
    // not a repository: a folder with no `.git` anywhere above it
    let plain = std::env::temp_dir().join(format!("vnoc-pin-plain-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&plain);
    crate::testdir::remove_at_end(&plain);
    std::fs::create_dir_all(&plain).unwrap();
    let plain = dunce::canonicalize(plain).unwrap();
    assert_eq!(
        head(&project(&plain), &tour),
        Err(vec![Reason::NotARepository])
    );
    // a repository with no commit yet: `rev-parse --verify -q HEAD` exits 1 and says nothing
    let root = repo("head");
    assert_eq!(head(&project(&root), &tour), Err(vec![Reason::NoCommit]));
    commit_all(&root, &[("a.py", "def f():\n    pass\n")]);
    assert_eq!(head(&project(&root), &tour), Ok(head_of(&root)));
    // a tour that is not finished, and one with nothing written
    let mut writing = finished(&["a.py"]);
    writing.state = State::Writing;
    assert_eq!(
        head(&project(&root), &writing),
        Err(vec![Reason::Unfinished])
    );
    let empty = fold(
        "{\"type\":\"tour\",\"schema\":1,\"id\":\"t-0000\",\"title\":\"T\",\"at\":\"x\",\"detail\":\"normal\"}\n{\"type\":\"end\",\"at\":\"x\"}\n",
    )
    .unwrap();
    assert_eq!(head(&project(&root), &empty), Err(vec![Reason::NoStops]));
}

#[test]
fn a_merge_or_a_rebase_in_the_middle_is_a_reason() {
    let root = repo("progress");
    commit_all(&root, &[("a.py", "def f():\n    pass\n")]);
    let tour = finished(&["a.py"]);
    let merge = root.join(".git/MERGE_HEAD");
    std::fs::write(&merge, head_of(&root).as_str()).unwrap();
    assert_eq!(
        head(&project(&root), &tour),
        Err(vec![Reason::InProgress { what: "a merge" }])
    );
    std::fs::remove_file(&merge).unwrap();
    for folder in ["rebase-merge", "rebase-apply"] {
        std::fs::create_dir(root.join(".git").join(folder)).unwrap();
        assert_eq!(
            head(&project(&root), &tour),
            Err(vec![Reason::InProgress { what: "a rebase" }]),
            "{folder}"
        );
        std::fs::remove_dir(root.join(".git").join(folder)).unwrap();
    }
    assert!(head(&project(&root), &tour).is_ok());
}

// -------------------------------------------------------------------------------------------------------------------------- can_pin

#[test]
fn files_all_committed_are_pinnable_at_head() {
    let root = repo("clean");
    commit_all(
        &root,
        &[
            ("a.py", "def f():\n    pass\n"),
            ("b.py", "def f():\n    pass\n"),
        ],
    );
    assert_eq!(
        answer(&root, &["a.py", "b.py"]),
        Pinnable::Yes {
            head: head_of(&root)
        }
    );
}

#[test]
fn one_modified_and_one_untracked_file_are_both_named_with_their_status() {
    let root = repo("dirty");
    commit_all(
        &root,
        &[
            ("a.py", "def f():\n    pass\n"),
            ("c.py", "def f():\n    pass\n"),
        ],
    );
    write(&root, "a.py", "def f():\n    return 1\n");
    write(&root, "b.py", "def f():\n    pass\n");
    // a staged change counts too, and the files come in the order of the tour
    write(&root, "c.py", "def f():\n    return 3\n");
    git(&root, &["add", "c.py"]);
    let said = reasons(answer(&root, &["b.py", "a.py", "c.py"]));
    let Reason::Uncommitted { files } = &said[0] else {
        panic!("{said:?}")
    };
    assert_eq!(
        files.iter().map(Changed::line).collect::<Vec<_>>(),
        ["?? b.py", " M a.py", "M  c.py"]
    );
    assert_eq!(said.len(), 1, "an untracked file is said once: {said:?}");
    assert_eq!(
        said[0].words(),
        "3 files have changes that are not committed"
    );
    let shown = said[0].lines(false);
    assert_eq!(shown[1], "  ?? b.py");
    assert!(said[0].lines(true)[0].contains("of this tour"));
    // and only that is in the way, so `--commit` has something to do
    assert!(
        answer(&root, &["b.py", "a.py", "c.py"])
            .only_uncommitted()
            .is_some()
    );
}

#[test]
fn the_files_a_cover_offers_to_commit_are_the_tours_own_by_git_alone_in_the_order_of_the_route() {
    let root = repo("offers");
    commit_all(
        &root,
        &[
            ("a.py", "def f():\n    pass\n"),
            ("b.py", "def f():\n    pass\n"),
            ("other.py", "x = 1\n"),
        ],
    );
    write(&root, "b.py", "def f():\n    return 1\n");
    write(&root, "a.py", "def f():\n    return 2\n");
    write(&root, "other.py", "x = 2\n");
    write(&root, "new.py", "def f():\n    pass\n");
    let tour = finished(&["b.py", "a.py", "b.py", "new.py"]);
    let files: Vec<String> = uncommitted_files(&project(&root), &tour)
        .iter()
        .map(Changed::line)
        .collect();
    assert_eq!(
        files,
        [" M b.py", " M a.py", "?? new.py"],
        "other.py is not the tour's"
    );
    // everything committed: nothing to offer
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "all"]);
    assert!(uncommitted_files(&project(&root), &tour).is_empty());
}

/// A finished tour with one stop on each of these files, pinned at `commit` when one is given.
fn finished_pinned(files: &[&str], commit: Option<&str>) -> Tour {
    let mut lines = vec![
        r#"{"type":"tour","schema":1,"id":"t-0000","title":"T","at":"x","detail":"normal"}"#
            .to_string(),
    ];
    for (i, file) in files.iter().enumerate() {
        lines.push(format!(
            r#"{{"type":"stop","id":"s{}","path":"{file}","qualname":"f","kind":"function","lines":[1,2],"hash":"0123456789abcdef","text":"t"}}"#,
            i + 1
        ));
    }
    lines.push(r#"{"type":"end","at":"x"}"#.to_string());
    if let Some(commit) = commit {
        lines.push(format!(
            r#"{{"type":"pin","commit":"{commit}","at":"x","where":{{"s1":["{}",1,2]}}}}"#,
            files[0]
        ));
    }
    fold(&format!("{}\n", lines.join("\n"))).unwrap()
}

fn calls_of(run: impl FnOnce()) -> usize {
    let before = GIT_CALLS.with(std::cell::Cell::get);
    run();
    GIT_CALLS.with(std::cell::Cell::get) - before
}

#[test]
fn a_list_of_three_finished_unpinned_tours_asks_git_status_once_with_the_same_answers() {
    use crate::tours::serve::Offering;
    let root = repo("offers-list");
    commit_all(
        &root,
        &[
            ("a.py", "def f():\n    pass\n"),
            ("b.py", "def f():\n    pass\n"),
            ("c.py", "def f():\n    pass\n"),
        ],
    );
    write(&root, "a.py", "def f():\n    return 1\n");
    write(&root, "c.py", "def f():\n    return 3\n");
    let project = project(&root);
    let tours = [
        finished(&["a.py", "b.py"]),
        finished(&["b.py"]),
        finished(&["c.py", "a.py"]),
    ];
    let offering = Offering::of(&project);
    let refs: Vec<&Tour> = tours.iter().collect();
    let mut together = Vec::new();
    let calls = calls_of(|| together = offering.offers_of(&project, &refs));
    assert_eq!(calls, 1, "one git status for the whole list");
    let alone: Vec<_> = tours.iter().map(|t| offering.offers(&project, t)).collect();
    assert_eq!(together, alone, "the same answers as one tour at a time");
    assert_eq!(together[0].as_ref().unwrap().commit, ["a.py"]);
    // (the offer to write IDs is there for each, as the project's settings ask)
    assert!(together[1].as_ref().unwrap().commit.is_empty());
    assert_eq!(together[2].as_ref().unwrap().commit, ["c.py", "a.py"]);
}

#[test]
fn a_commit_the_list_pins_at_is_asked_about_once_however_many_tours_are_pinned_there() {
    use crate::tours::serve::Offering;
    let root = repo("offers-pinned");
    commit_all(&root, &[("a.py", "def f():\n    pass\n")]);
    let head = git(&root, &["rev-parse", "HEAD"]).trim().to_string();
    write(&root, "a.py", "def f():\n    return 1\n");
    let project = project(&root);
    let tours = [
        finished_pinned(&["a.py"], Some(&head)),
        finished_pinned(&["a.py"], Some(&head)),
        finished_pinned(&["a.py"], None),
    ];
    let offering = Offering::of(&project);
    let refs: Vec<&Tour> = tours.iter().collect();
    let mut together = Vec::new();
    let calls = calls_of(|| together = offering.offers_of(&project, &refs));
    assert_eq!(
        calls, 2,
        "one merge-base for the commit, one status for the unpinned tour"
    );
    assert!(together[0].as_ref().unwrap().commit.is_empty());
    assert!(together[1].as_ref().unwrap().commit.is_empty());
    assert_eq!(together[2].as_ref().unwrap().commit, ["a.py"]);
}

#[test]
fn an_ignored_file_and_a_file_git_never_had_are_worded_by_why() {
    let root = repo("ignored");
    commit_all(
        &root,
        &[
            (".gitignore", "generated.py\n"),
            ("a.py", "def f():\n    pass\n"),
        ],
    );
    write(&root, "generated.py", "def f():\n    pass\n");
    // ignored: `status` says nothing for it, `ls-tree` does not name it, `check-ignore` says why (TR0, check 6)
    assert_eq!(
        reasons(answer(&root, &["a.py", "generated.py"])),
        [Reason::NotHeld {
            path: path("generated.py"),
            ignored: true
        }]
    );
    assert_eq!(
        reasons(answer(&root, &["generated.py"]))[0].words(),
        "generated.py is ignored by git: this tour cannot be pinned"
    );
    assert!(
        answer(&root, &["a.py", "generated.py"])
            .only_uncommitted()
            .is_none(),
        "committing cannot fix an ignored file"
    );
    // a path that is not there at all reads as clean to `status`, and is not in HEAD
    assert_eq!(
        reasons(answer(&root, &["nowhere.py"])),
        [Reason::NotHeld {
            path: path("nowhere.py"),
            ignored: false
        }]
    );
    assert_eq!(
        reasons(answer(&root, &["nowhere.py"]))[0].words(),
        "nowhere.py is not in the last commit"
    );
}

#[test]
fn a_stop_that_changed_or_is_gone_is_refused_by_id_before_git_is_asked_about_files() {
    let root = repo("stale");
    commit_all(&root, &[("a.py", "def f():\n    pass\n")]);
    let project = project(&root);
    let tour = finished(&["a.py", "b.py", "c.py"]);
    let mut recorded = fresh();
    recorded
        .stops
        .insert(StopId::new(2).unwrap(), StopState::Changed);
    recorded
        .stops
        .insert(StopId::new(3).unwrap(), StopState::Gone);
    let got = can_pin(
        &project,
        &tour,
        head(&project, &tour).unwrap(),
        &recorded,
        &placed(&["a.py", "b.py", "c.py"]),
    );
    let said = reasons(got);
    assert_eq!(
        said,
        [Reason::Stale {
            changed: vec![StopId::new(2).unwrap()],
            gone: vec![StopId::new(3).unwrap()]
        }]
    );
    assert_eq!(
        said[0].words(),
        "s2 changed since it was written · s3 is gone: replace what changed, then pin"
    );
}

#[test]
#[allow(clippy::print_stderr)] // a test that cannot run here says so, loudly
fn a_file_in_a_submodule_is_refused_with_its_reason() {
    let outer = repo("sub-outer");
    let inner = repo("sub-inner");
    commit_all(&inner, &[("m.py", "def f():\n    pass\n")]);
    commit_all(&outer, &[("a.py", "def f():\n    pass\n")]);
    let added = Command::new("git")
        .arg("-C")
        .arg(&outer)
        .args(["-c", "protocol.file.allow=always", "submodule", "add", "-q"])
        .arg(&inner)
        .arg("vendor")
        .output()
        .unwrap();
    if !added.status.success() {
        eprintln!(
            "SKIPPED: a submodule could not be made here: {}",
            String::from_utf8_lossy(&added.stderr)
        );
        return;
    }
    git(&outer, &["commit", "-qm", "with a submodule"]);
    let said = reasons(answer(&outer, &["a.py", "vendor/m.py"]));
    assert_eq!(
        said,
        [Reason::Submodule {
            path: path("vendor/m.py"),
            module: "vendor".to_string()
        }]
    );
    assert!(said[0].words().contains("in the submodule vendor"));
}

// ------------------------------------------------------------------------------------------------------------------------------ make

#[test]
fn a_pin_record_has_head_and_where_every_stop_and_block_is_now() {
    let mut p = placed(&["a.py", "b.py"]);
    p.blocks = vec![
        (
            StepId::parse("s1.b1").unwrap(),
            Lines::new(1, 2).unwrap(),
            StopId::new(1).unwrap(),
        ),
        (
            StepId::parse("s2.b1").unwrap(),
            Lines::new(2, 2).unwrap(),
            StopId::new(2).unwrap(),
        ),
    ];
    let head = Commit::parse("f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a").unwrap();
    let record = make(head.clone(), &p);
    assert_eq!(record.commit, head);
    let line = crate::tours::record::print_line(&crate::tours::Record::Pin(record));
    let value: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
    assert_eq!(
        value["where"],
        serde_json::json!({
            "s1": ["a.py", 1, 2], "s1.b1": ["a.py", 1, 2],
            "s2": ["b.py", 1, 2], "s2.b1": ["b.py", 2, 2]
        })
    );
    assert_eq!(
        p.files(),
        [path("a.py"), path("b.py")],
        "the files, each once, in route order"
    );
}

// -------------------------------------------------------------------------------------------------------------------- commit_files

fn changed(status: &str, file: &str) -> Changed {
    Changed {
        status: status.to_string(),
        path: path(file),
    }
}

#[test]
fn commit_files_commits_exactly_those_files_and_other_staged_files_stay_staged() {
    let root = repo("commit");
    commit_all(&root, &[("a.py", "x = 1\n"), ("other.py", "y = 1\n")]);
    write(&root, "a.py", "x = 2\n");
    write(&root, "b.py", "z = 1\n");
    // another file, staged, that the tour has nothing to do with
    write(&root, "other.py", "y = 2\n");
    git(&root, &["add", "other.py"]);
    write(&root, "staged-new.py", "n = 1\n");
    git(&root, &["add", "staged-new.py"]);
    let done = commit_files(
        &project(&root),
        &[changed(" M", "a.py"), changed("??", "b.py")],
        "Code for the tour \"T\"",
    )
    .unwrap();
    assert_eq!(done.commit, head_of(&root));
    let in_commit = git(&root, &["show", "--name-only", "--format=", "HEAD"]);
    assert_eq!(in_commit.lines().collect::<Vec<_>>(), ["a.py", "b.py"]);
    assert_eq!(
        git(&root, &["log", "-1", "--format=%s"]).trim(),
        "Code for the tour \"T\""
    );
    // the other two are still staged, and still not in the commit
    let status = git(&root, &["status", "--porcelain"]);
    assert_eq!(
        status.lines().collect::<Vec<_>>(),
        ["M  other.py", "A  staged-new.py"]
    );
}

#[test]
fn a_file_named_like_a_pattern_is_that_file_and_a_message_that_starts_with_a_dash_is_a_message() {
    let root = repo("literal");
    commit_all(&root, &[("keep.py", "k = 1\n")]);
    write(&root, "we*rd.py", "w = 1\n");
    write(&root, "weird.py", "w = 2\n");
    commit_files(
        &project(&root),
        &[changed("??", "we*rd.py")],
        "--amend everything",
    )
    .unwrap();
    assert_eq!(
        git(&root, &["show", "--name-only", "--format=%s", "HEAD"])
            .lines()
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>(),
        ["--amend everything", "we*rd.py"]
    );
    assert_eq!(
        git(&root, &["status", "--porcelain"]).trim(),
        "?? weird.py",
        "the other file the pattern would have matched is not committed"
    );
}

#[cfg(unix)]
#[test]
fn a_hook_that_fails_fails_the_commit_with_its_words_and_nothing_stays_staged_or_moved() {
    use std::os::unix::fs::PermissionsExt;
    let root = repo("hook");
    commit_all(&root, &[("a.py", "x = 1\n")]);
    let before = head_of(&root);
    write(&root, "a.py", "x = 2\n");
    write(&root, "b.py", "z = 1\n");
    let hook = root.join(".git/hooks/pre-commit");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(&hook, "#!/bin/sh\necho 'hook says no' >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let failed = commit_files(
        &project(&root),
        &[changed(" M", "a.py"), changed("??", "b.py")],
        "m",
    )
    .unwrap_err();
    let CommitError::Refused(said) = &failed else {
        panic!("{failed:?}")
    };
    assert!(said.contains("hook says no"), "{said}");
    assert_eq!(head_of(&root), before, "HEAD did not move");
    assert_eq!(
        git(&root, &["status", "--porcelain"])
            .lines()
            .collect::<Vec<_>>(),
        [" M a.py", "?? b.py"],
        "b.py was taken out of the index again"
    );
}

// ---------------------------------------------------------------------------------------------------------------------- as_written

#[test]
fn as_written_reads_the_lines_of_the_commit_not_of_the_disk() {
    let root = repo("written");
    commit_all(&root, &[("a.py", "one\ntwo\nthree\nfour\n")]);
    let commit = head_of(&root);
    write(&root, "a.py", "changed\n");
    let p = project(&root);
    assert_eq!(
        as_written(&p, commit.as_str(), "a.py", 2, 3),
        Some(vec!["two".to_string(), "three".to_string()])
    );
    // the same again comes from memory: the answer is the commit's, not the disk's
    assert_eq!(
        as_written(&p, commit.as_str(), "a.py", 1, 1),
        Some(vec!["one".to_string()])
    );
    // lines past the end are cut at it; a first line past the end is nothing
    assert_eq!(
        as_written(&p, commit.as_str(), "a.py", 4, 9),
        Some(vec!["four".to_string()])
    );
    assert_eq!(as_written(&p, commit.as_str(), "a.py", 5, 6), None);
}

#[test]
fn as_written_splits_as_source_does_crlf_and_a_byte_order_mark() {
    let root = repo("crlf");
    commit_all(&root, &[("a.py", "\u{feff}one\r\ntwo\r\n")]);
    let commit = head_of(&root);
    assert_eq!(
        as_written(&project(&root), commit.as_str(), "a.py", 1, 2),
        Some(vec!["one".to_string(), "two".to_string()])
    );
}

#[test]
fn as_written_is_none_for_a_commit_this_repository_lacks_a_path_it_lacks_and_what_a_pin_may_not_name()
 {
    let root = repo("none");
    commit_all(&root, &[("a.py", "one\n")]);
    let p = project(&root);
    let good = head_of(&root);
    let lacking = "0123456789abcdef0123456789abcdef01234567";
    for (what, commit, file) in [
        ("a commit that is not in this repository", lacking, "a.py"),
        ("a path the commit does not have", good.as_str(), "nope.py"),
        ("an option as the commit", "--help", "a.py"),
        (
            "an option as long as a commit",
            "--output=/tmp/x-0000000000000000000",
            "a.py",
        ),
        ("a ref name", "HEAD", "a.py"),
        ("a path with ..", good.as_str(), "../a.py"),
        ("an absolute path", good.as_str(), "/etc/passwd"),
        ("an empty path", good.as_str(), ""),
    ] {
        assert_eq!(as_written(&p, commit, file, 1, 1), None, "{what}");
    }
}

// ------------------------------------------------------------------------------------------------------------------------ reachable

#[test]
fn a_commit_is_reachable_when_head_has_it_behind_it() {
    let root = repo("reach");
    commit_all(&root, &[("a.py", "one\n")]);
    let first = head_of(&root);
    let p = project(&root);
    assert!(has_commit(&p, &first));
    assert!(reachable(&p, &first), "HEAD itself");
    git(&root, &["checkout", "-q", "-b", "side"]);
    commit_all(&root, &[("a.py", "two\n")]);
    let side = head_of(&root);
    assert!(reachable(&p, &first), "behind HEAD");
    git(&root, &["checkout", "-q", "-"]);
    // on the other branch the side commit is in the repository and not behind HEAD: a pin to it can be made again
    assert!(has_commit(&p, &side));
    assert!(!reachable(&p, &side));
    // a commit the repository has never had
    let lacking = Commit::parse("0123456789abcdef0123456789abcdef01234567").unwrap();
    assert!(!has_commit(&p, &lacking));
    assert!(!reachable(&p, &lacking));
}

#[test]
fn the_reasons_are_worded_and_serialise_with_a_code() {
    let table: [(Reason, &str, &str); 6] = [
        (
            Reason::NotARepository,
            "this is not a git repository",
            "not_a_repository",
        ),
        (
            Reason::GitNotFound,
            "git was not found: pinning needs it",
            "git_not_found",
        ),
        (
            Reason::NoCommit,
            "this repository has no commit yet",
            "no_commit",
        ),
        (
            Reason::InProgress { what: "a rebase" },
            "a rebase is in progress: finish it first",
            "in_progress",
        ),
        (
            Reason::Unfinished,
            "the tour is still being written: finish it first",
            "unfinished",
        ),
        (
            Reason::Uncommitted {
                files: vec![changed(" M", "a.py")],
            },
            "1 file has changes that are not committed",
            "uncommitted",
        ),
    ];
    for (reason, words, code) in table {
        assert_eq!(reason.words(), words);
        assert_eq!(serde_json::to_value(&reason).unwrap()["code"], code);
    }
}
