//! Where a command works: the root (05 §2, C9), the driver (C8), and the project opened and built (`open`).
//!
//! `find_root` and `driver_paths` get what they look at as arguments, not from the process, so a test can try each rule on a temp folder.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::{Failure, Global};
use crate::driver_host::{DriverHost, LanguageState};
use crate::index::{Index, Touched, refresh};
use crate::project::Project;

/// Every driver to run (docs/23 §5), by the first rule that gives any:
/// 1. `explicit`: paths named by the caller (`initializationOptions.drivers` or `driverPath`);
/// 2. else `env_list` (`VNOC_DRIVERS`: paths separated as the OS's `PATH` is: `:`, or `;` on Windows);
/// 3. else `env_one` (`VNOC_DRIVER`);
/// 4. else every driver file beside the binary's **real** path (`current_exe` with symlinks followed: a link in `~/.local/bin` must find the
///    drivers in the extension's `bin/`), sorted by name.
///
/// An explicit list replaces discovery: nothing is added to it. An empty value is not set. Whether a path exists is not asked here: a driver
/// that is not there says so when it fails to start.
pub fn discover(
    explicit: &[PathBuf],
    env_list: Option<&OsStr>,
    env_one: Option<&OsStr>,
    current_exe: Option<&Path>,
) -> Vec<PathBuf> {
    if !explicit.is_empty() {
        return explicit.to_vec();
    }
    let listed: Vec<PathBuf> = env_list
        .map(|value| std::env::split_paths(value).collect())
        .unwrap_or_default();
    let listed: Vec<PathBuf> = listed
        .into_iter()
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    if !listed.is_empty() {
        return listed;
    }
    if let Some(one) = env_one.filter(|value| !value.is_empty()) {
        return vec![PathBuf::from(one)];
    }
    current_exe
        .and_then(|exe| dunce::canonicalize(exe).ok())
        .and_then(|real| real.parent().map(drivers_in))
        .unwrap_or_default()
}

/// The driver files in `folder`: a file (or a link to one; not a folder) named `vnoc-driver-<name>` plus the OS's executable suffix, where `<name>` is lower case
/// letters, digits, `-` or `_` (so `vnoc-driver-python.d` and `vnoc-driver-python.pdb`, which a build leaves beside it, are not drivers), and,
/// where files have a mode, one that can be run. Sorted by file name.
pub fn drivers_in(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        // a link to a driver is a driver: `metadata` follows it (a folder named like one is not)
        .filter(|entry| std::fs::metadata(entry.path()).is_ok_and(|m| m.is_file()))
        .filter(|entry| entry.file_name().to_str().is_some_and(is_driver_file_name))
        .filter(|entry| can_run(&entry.path()))
        .map(|entry| entry.path())
        .collect();
    found.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    found
}

fn is_driver_file_name(name: &str) -> bool {
    name.strip_prefix("vnoc-driver-")
        .and_then(|rest| rest.strip_suffix(std::env::consts::EXE_SUFFIX))
        .is_some_and(|language| {
            !language.is_empty()
                && language
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        })
}

#[cfg(unix)]
fn can_run(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn can_run(_: &Path) -> bool {
    true
}

/// The folder a command works on, by the first rule that applies (05 §2):
/// 1. `root_flag`, if given (a relative one is taken from `cwd`);
/// 2. else the nearest folder at or above `cwd` that holds `.git` (a folder, or the file a worktree or a submodule has);
/// 3. else the nearest that holds a `.vnoc/` folder;
/// 4. else `cwd`.
///
/// `.git` comes before `.vnoc/` because the core reads `.vnoc/` at the **git root**: root and that folder are then the same one.
pub fn find_root(cwd: &Path, root_flag: Option<&Path>) -> PathBuf {
    if let Some(flag) = root_flag {
        // an absolute `flag` replaces `cwd` in `join`; never a `\\?\` path
        return dunce::simplified(&cwd.join(flag)).to_path_buf();
    }
    let nearest = |holds: &dyn Fn(&Path) -> bool| cwd.ancestors().find(|dir| holds(dir));
    nearest(&|dir| dir.join(".git").exists())
        .or_else(|| nearest(&|dir| dir.join(".vnoc").is_dir()))
        .unwrap_or(cwd)
        .to_path_buf()
}

/// The drivers to run (C8, docs/23 §5): [`discover`]'s list, or a failure that says where it looked.
///
/// A path given with `--driver` that does not exist is an error of its own (a typo is not a driver that failed). A path from `VNOC_DRIVERS` or
/// `VNOC_DRIVER` is not asked about: a driver that is not there is listed by `status` as failed, with the reason. When nothing is found the
/// message names each place and what it held.
pub fn driver_paths(
    flags: &[PathBuf],
    env_list: Option<&OsStr>,
    env_one: Option<&OsStr>,
    current_exe: Option<&Path>,
) -> Result<Vec<PathBuf>, Failure> {
    if let Some(missing) = flags.iter().find(|flag| !flag.exists()) {
        return Err(Failure::Environment(format!(
            "the driver given with --driver does not exist: {}",
            missing.display()
        )));
    }
    let found = discover(flags, env_list, env_one, current_exe);
    if !found.is_empty() {
        return Ok(found);
    }
    let value = |env: Option<&OsStr>| match env {
        None => "not set".to_string(),
        Some(value) if value.is_empty() => "empty".to_string(),
        Some(value) => format!("{} holds no path", value.to_string_lossy()),
    };
    let beside = match current_exe.and_then(|exe| dunce::canonicalize(exe).ok()) {
        None => "the binary's own path is not known".to_string(),
        Some(real) => {
            let folder = real.parent().unwrap_or(&real);
            format!("no vnoc-driver-* file in {}", folder.display())
        }
    };
    Err(Failure::Environment(format!(
        "no driver found. Looked in:\n  --driver: not given\n  VNOC_DRIVERS: {}\n  VNOC_DRIVER: {}\n  next to the binary: {beside}\n\
         Give one with --driver <path> (repeat it for more), VNOC_DRIVERS or VNOC_DRIVER.",
        value(env_list),
        value(env_one)
    )))
}

/// A project opened and its index built from nothing: what every command that reads code starts from.
pub struct Opened {
    pub project: Project,
    pub host: DriverHost,
    pub index: RwLock<Index>,
}

/// The root and the drivers as this process sees them (the current folder, `VNOC_DRIVERS`, `VNOC_DRIVER`, `current_exe`), then [`open`].
pub async fn open_from(global: &Global) -> Result<Opened, Failure> {
    let cwd = std::env::current_dir()
        .map_err(|e| Failure::Environment(format!("the current folder is not known: {e}")))?;
    let root = find_root(&cwd, global.root.as_deref());
    let drivers = driver_paths(
        &global.driver,
        std::env::var_os("VNOC_DRIVERS").as_deref(),
        std::env::var_os("VNOC_DRIVER").as_deref(),
        std::env::current_exe().ok().as_deref(),
    )?;
    open(&root, &drivers).await
}

/// The root as this process sees it, and nothing built: for a command that needs no index and so no driver (`doc read <ID>`). The driver is not
/// looked for, so a missing one is no error here.
pub fn open_without_driver(global: &Global) -> Result<Project, Failure> {
    let cwd = std::env::current_dir()
        .map_err(|e| Failure::Environment(format!("the current folder is not known: {e}")))?;
    Project::open(&find_root(&cwd, global.root.as_deref()))
        .map_err(|e| Failure::Usage(e.to_string()))
}

/// Opens `root`, points a driver host at `drivers`, starts them all (the handshakes say which extensions the scan collects), and builds the whole
/// index (`refresh(…, Touched::All)`). The drivers exit when `Opened` is dropped.
///
/// One driver that does not start is not a failure here: its files are shown without definitions and `status` says why. When **none** started,
/// there is nothing to read the code with: a failure that names each driver and its reason.
pub async fn open(root: &Path, drivers: &[PathBuf]) -> Result<Opened, Failure> {
    let project = Project::open(root).map_err(|e| Failure::Usage(e.to_string()))?;
    let host = DriverHost::default();
    host.set_paths(drivers.to_vec());
    host.start_all().await;
    let languages = host.languages().await;
    if !languages.is_empty()
        && languages
            .iter()
            .all(|l| matches!(l.state, LanguageState::Failed(_)))
    {
        let each: Vec<String> = languages
            .iter()
            .map(|l| match &l.state {
                LanguageState::Failed(reason) => {
                    format!("  {} ({}): {reason}", l.name, l.path.display())
                }
                LanguageState::Running => unreachable!("all failed"),
            })
            .collect();
        return Err(Failure::Environment(format!(
            "no driver started:\n{}",
            each.join("\n")
        )));
    }
    let index = RwLock::new(Index::default());
    refresh(&project, &host, &index, Touched::All)
        .await
        .map_err(|e| Failure::Environment(e.to_string()))?;
    Ok(Opened {
        project,
        host,
        index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vnoc-cli-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dunce::canonicalize(dir).unwrap()
    }

    #[test]
    fn the_root_flag_wins_and_a_relative_one_is_taken_from_the_current_folder() {
        let top = fresh("root-flag");
        std::fs::create_dir_all(top.join(".git")).unwrap();
        std::fs::create_dir_all(top.join("a/b")).unwrap();
        assert_eq!(
            find_root(&top.join("a"), Some(&top.join("a/b"))),
            top.join("a/b")
        );
        assert_eq!(
            find_root(&top.join("a"), Some(Path::new("b"))),
            top.join("a/b")
        );
    }

    #[test]
    fn the_nearest_git_folder_is_the_root() {
        let top = fresh("root-git");
        std::fs::create_dir_all(top.join(".git")).unwrap();
        std::fs::create_dir_all(top.join("billing/deep")).unwrap();
        assert_eq!(find_root(&top.join("billing/deep"), None), top);
    }

    #[test]
    fn a_git_file_is_a_git_root_too() {
        // a worktree or a submodule has a `.git` file
        let top = fresh("root-gitfile");
        std::fs::write(top.join(".git"), "gitdir: /elsewhere\n").unwrap();
        std::fs::create_dir_all(top.join("src")).unwrap();
        assert_eq!(find_root(&top.join("src"), None), top);
    }

    #[test]
    fn git_comes_before_vnoc_even_when_the_vnoc_folder_is_nearer() {
        let top = fresh("root-order");
        std::fs::create_dir_all(top.join(".git")).unwrap();
        std::fs::create_dir_all(top.join("sample/.vnoc")).unwrap();
        std::fs::create_dir_all(top.join("sample/billing")).unwrap();
        assert_eq!(find_root(&top.join("sample/billing"), None), top);
    }

    #[test]
    fn without_git_the_nearest_vnoc_folder_is_the_root() {
        let top = fresh("root-vnoc");
        std::fs::create_dir_all(top.join(".vnoc")).unwrap();
        std::fs::create_dir_all(top.join("x/y")).unwrap();
        assert_eq!(find_root(&top.join("x/y"), None), top);
    }

    #[test]
    fn the_drivers_come_from_the_flags_then_the_environment_lists_then_beside_the_binary() {
        let dir = fresh("driver-order");
        let (one, two) = (dir.join("one"), dir.join("two"));
        let (from_list, from_one) = (dir.join("from-list"), dir.join("from-one"));
        for file in [&one, &two, &from_list, &from_one] {
            std::fs::write(file, "").unwrap();
        }
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        let exe = dir.join("bin/vnoc");
        std::fs::write(&exe, "").unwrap();
        let python = put_driver(&dir.join("bin"), "python");
        let bend = put_driver(&dir.join("bin"), "bend");

        let list = std::env::join_paths([&from_list, &dir.join("second")]).unwrap();
        let single = from_one.as_os_str();
        let flags = [one.clone(), two.clone()];
        assert_eq!(
            driver_paths(&flags, Some(&list), Some(single), Some(&exe)).unwrap(),
            flags
        );
        assert_eq!(
            driver_paths(&[], Some(&list), Some(single), Some(&exe)).unwrap(),
            [from_list.clone(), dir.join("second")]
        );
        assert_eq!(
            driver_paths(&[], None, Some(single), Some(&exe)).unwrap(),
            [from_one]
        );
        assert_eq!(
            driver_paths(&[], None, None, Some(&exe)).unwrap(),
            [bend.clone(), python.clone()]
        );
        // an empty value is not set
        assert_eq!(
            driver_paths(&[], Some(OsStr::new("")), Some(OsStr::new("")), Some(&exe)).unwrap(),
            [bend, python]
        );
    }

    #[test]
    fn beside_the_binary_means_beside_its_real_path() {
        #[cfg(unix)]
        {
            let dir = fresh("driver-link");
            std::fs::create_dir_all(dir.join("real")).unwrap();
            std::fs::create_dir_all(dir.join("links")).unwrap();
            std::fs::write(dir.join("real/vnoc"), "").unwrap();
            let python = put_driver(&dir.join("real"), "python");
            let bend = put_driver(&dir.join("real"), "bend");
            std::os::unix::fs::symlink(dir.join("real/vnoc"), dir.join("links/vnoc")).unwrap();
            assert_eq!(
                driver_paths(&[], None, None, Some(&dir.join("links/vnoc"))).unwrap(),
                [bend, python]
            );
        }
    }

    #[test]
    fn a_driver_that_is_not_found_says_where_it_looked_and_what_each_place_held() {
        let dir = fresh("driver-none");
        let exe = dir.join("vnoc");
        std::fs::write(&exe, "").unwrap();
        let Err(Failure::Environment(text)) = driver_paths(&[], None, None, Some(&exe)) else {
            panic!("an environment failure");
        };
        assert!(text.contains("--driver: not given"), "{text}");
        assert!(text.contains("VNOC_DRIVERS: not set"), "{text}");
        assert!(text.contains("VNOC_DRIVER: not set"), "{text}");
        assert!(
            text.contains(&format!(
                "next to the binary: no vnoc-driver-* file in {}",
                dir.display()
            )),
            "{text}"
        );

        // set, but with nothing in it
        let Err(Failure::Environment(text)) =
            driver_paths(&[], Some(OsStr::new("")), Some(OsStr::new("")), Some(&exe))
        else {
            panic!("an environment failure");
        };
        assert!(text.contains("VNOC_DRIVERS: empty"), "{text}");
        assert!(text.contains("VNOC_DRIVER: empty"), "{text}");

        let Err(Failure::Environment(text)) = driver_paths(&[], None, None, None) else {
            panic!("an environment failure");
        };
        assert!(
            text.contains("the binary's own path is not known"),
            "{text}"
        );
    }

    #[test]
    fn a_driver_given_with_the_flag_that_does_not_exist_is_an_error_of_its_own() {
        let dir = fresh("driver-flag");
        let present = dir.join("present");
        std::fs::write(&present, "").unwrap();
        let missing = dir.join("nope");
        // the second of two is the one named
        let Err(Failure::Environment(text)) =
            driver_paths(&[present, missing.clone()], None, None, None)
        else {
            panic!("an environment failure");
        };
        assert_eq!(
            text,
            format!(
                "the driver given with --driver does not exist: {}",
                missing.display()
            )
        );
    }

    #[test]
    fn a_path_from_the_environment_that_is_not_there_is_left_to_say_so_when_it_fails_to_start() {
        let dir = fresh("driver-env-missing");
        let missing = dir.join("nope");
        assert_eq!(
            driver_paths(&[], Some(missing.as_os_str()), None, None).unwrap(),
            [missing]
        );
    }

    // ── discover (docs/23 §5) ──────────────────────────────────────────────────────────────────────────────────────────

    /// A file a driver could be: named `vnoc-driver-<name>` + the OS's suffix, and runnable.
    fn put_driver(folder: &Path, name: &str) -> PathBuf {
        let path = folder.join(format!(
            "vnoc-driver-{name}{}",
            std::env::consts::EXE_SUFFIX
        ));
        std::fs::write(&path, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn an_explicit_list_replaces_everything_else() {
        let dir = fresh("discover-explicit");
        let exe = dir.join("vnoc");
        std::fs::write(&exe, "").unwrap();
        put_driver(&dir, "python");
        let given = vec![dir.join("one"), dir.join("two")];
        let env = std::env::join_paths([dir.join("env")]).unwrap();
        assert_eq!(
            discover(&given, Some(&env), Some(OsStr::new("x")), Some(&exe)),
            given,
            "nothing is added to a list that was given"
        );
    }

    #[test]
    fn the_environment_lists_come_in_the_order_the_os_splits_them() {
        let dir = fresh("discover-env");
        let paths = [dir.join("a driver"), dir.join("b"), dir.join("c")];
        // `:` on Unix, `;` on Windows: join with what this OS splits by
        let list = std::env::join_paths(&paths).unwrap();
        assert_eq!(discover(&[], Some(&list), None, None), paths);
        // the list wins over the single path, and an empty value is not set
        assert_eq!(
            discover(&[], Some(&list), Some(OsStr::new("single")), None),
            paths
        );
        assert_eq!(
            discover(&[], Some(OsStr::new("")), Some(OsStr::new("single")), None),
            [PathBuf::from("single")]
        );
        assert_eq!(
            discover(&[], None, Some(OsStr::new("single")), None),
            [PathBuf::from("single")]
        );
        assert!(discover(&[], Some(OsStr::new("")), Some(OsStr::new("")), None).is_empty());
    }

    #[test]
    fn every_driver_beside_the_binary_is_found_sorted_by_name() {
        let dir = fresh("discover-beside");
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        let bin = dir.join("bin");
        let exe = bin.join(format!("vnoc{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&exe, "").unwrap();
        let python = put_driver(&bin, "python");
        let bend = put_driver(&bin, "bend");
        let typescript = put_driver(&bin, "type-script_2");
        // things that are not drivers
        std::fs::write(bin.join("vnoc-driver-python.d"), "").unwrap(); // what a build leaves beside it
        std::fs::write(bin.join("vnoc-driver-bend.pdb"), "").unwrap();
        std::fs::write(bin.join("vnoc-driver-"), "").unwrap(); // no name
        std::fs::write(
            bin.join(format!("vnoc-driver-{}", std::env::consts::EXE_SUFFIX)),
            "",
        )
        .unwrap();
        std::fs::write(bin.join("vnoc-driverpython"), "").unwrap(); // no dash
        std::fs::write(bin.join("my-vnoc-driver-python"), "").unwrap();
        std::fs::write(bin.join("vnoc-driver-Upper"), "").unwrap();
        std::fs::create_dir_all(bin.join(format!(
            "vnoc-driver-folder{}",
            std::env::consts::EXE_SUFFIX
        )))
        .unwrap(); // a folder named like a driver
        assert_eq!(
            discover(&[], None, None, Some(&exe)),
            [bend, python, typescript]
        );
        // with no binary path, nothing is beside it
        assert!(discover(&[], None, None, None).is_empty());
    }

    #[test]
    fn the_suffix_decides_on_windows_and_is_empty_elsewhere() {
        // `EXE_SUFFIX` is `.exe` on Windows and empty elsewhere: the same rule reads both
        let suffix = std::env::consts::EXE_SUFFIX;
        assert!(is_driver_file_name(&format!("vnoc-driver-python{suffix}")));
        assert!(!is_driver_file_name(&format!("vnoc-driver-{suffix}")));
        if suffix.is_empty() {
            assert!(!is_driver_file_name("vnoc-driver-python.exe"));
        } else {
            assert!(!is_driver_file_name("vnoc-driver-python"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_cannot_be_run_is_not_a_driver() {
        use std::os::unix::fs::PermissionsExt;
        let dir = fresh("discover-mode");
        let path = put_driver(&dir, "text");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(drivers_in(&dir).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_driver_is_a_driver_and_a_dangling_one_is_not() {
        let dir = fresh("discover-driver-link");
        let target = dir.join("elsewhere");
        std::fs::write(&target, "").unwrap();
        let linked = dir.join("vnoc-driver-linked");
        std::os::unix::fs::symlink(&target, &linked).unwrap();
        std::os::unix::fs::symlink(dir.join("gone"), dir.join("vnoc-driver-dangling")).unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(drivers_in(&dir), [linked]);
    }

    #[cfg(unix)]
    #[test]
    fn the_drivers_are_looked_for_beside_the_real_path_of_a_link() {
        let dir = fresh("discover-link");
        std::fs::create_dir_all(dir.join("real")).unwrap();
        std::fs::create_dir_all(dir.join("links")).unwrap();
        std::fs::write(dir.join("real/vnoc"), "").unwrap();
        let python = put_driver(&dir.join("real"), "python");
        std::os::unix::fs::symlink(dir.join("real/vnoc"), dir.join("links/vnoc")).unwrap();
        assert_eq!(
            discover(&[], None, None, Some(&dir.join("links/vnoc"))),
            [python]
        );
    }
}
