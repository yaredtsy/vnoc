//! Where a command works: the root (05 §2, C9), the driver (C8), and the project opened and built (`open`).
//!
//! `find_root` and `driver_path` get what they look at as arguments, not from the process, so a test can try each rule on a temp folder.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::{Failure, Global};
use crate::driver_host::DriverHost;
use crate::index::{Index, Touched, refresh};
use crate::project::Project;

/// The driver's file name, next to the binary (`.exe` on Windows).
pub fn driver_file_name() -> String {
    format!("vnoc-driver-python{}", std::env::consts::EXE_SUFFIX)
}

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

/// The driver files in `folder`: a file (not a folder) named `vnoc-driver-<name>` plus the OS's executable suffix, where `<name>` is lower case
/// letters, digits, `-` or `_` (so `vnoc-driver-python.d` and `vnoc-driver-python.pdb`, which a build leaves beside it, are not drivers), and,
/// where files have a mode, one that can be run. Sorted by file name.
pub fn drivers_in(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file()))
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

/// The driver to run (C8): `--driver`, else `VNOC_DRIVER`, else the driver file next to the binary, where the binary is its **real** path
/// (`current_exe` with symlinks followed: a link in `~/.local/bin` must find the driver in the extension's `bin/`).
///
/// A `--driver` that does not exist is an error of its own. When nothing is found the message says where it looked, and why each place was
/// skipped.
pub fn driver_path(
    flag: Option<&Path>,
    env: Option<&OsStr>,
    current_exe: Option<&Path>,
) -> Result<PathBuf, Failure> {
    if let Some(flag) = flag {
        return if flag.exists() {
            Ok(flag.to_path_buf())
        } else {
            Err(Failure::Environment(format!(
                "the driver given with --driver does not exist: {}",
                flag.display()
            )))
        };
    }
    let from_env = match env.filter(|value| !value.is_empty()) {
        None => "not set".to_string(),
        Some(value) => {
            let path = Path::new(value);
            if path.exists() {
                return Ok(path.to_path_buf());
            }
            format!("{} does not exist", path.display())
        }
    };
    let beside = match current_exe.and_then(|exe| dunce::canonicalize(exe).ok()) {
        None => "the binary's own path is not known".to_string(),
        Some(real) => {
            let path = real.with_file_name(driver_file_name());
            if path.exists() {
                return Ok(path);
            }
            format!("{} does not exist", path.display())
        }
    };
    Err(Failure::Environment(format!(
        "no driver found. Looked in:\n  --driver: not given\n  VNOC_DRIVER: {from_env}\n  next to the binary: {beside}\n\
         Give one with --driver <path> or VNOC_DRIVER."
    )))
}

/// A project opened and its index built from nothing: what every command that reads code starts from.
pub struct Opened {
    pub project: Project,
    pub host: DriverHost,
    pub index: RwLock<Index>,
}

/// The root and the driver as this process sees them (the current folder, `VNOC_DRIVER`, `current_exe`), then [`open`].
pub async fn open_from(global: &Global) -> Result<Opened, Failure> {
    let cwd = std::env::current_dir()
        .map_err(|e| Failure::Environment(format!("the current folder is not known: {e}")))?;
    let root = find_root(&cwd, global.root.as_deref());
    let driver = driver_path(
        global.driver.as_deref(),
        std::env::var_os("VNOC_DRIVER").as_deref(),
        std::env::current_exe().ok().as_deref(),
    )?;
    open(&root, &driver).await
}

/// The root as this process sees it, and nothing built: for a command that needs no index and so no driver (`doc read <ID>`). The driver is not
/// looked for, so a missing one is no error here.
pub fn open_without_driver(global: &Global) -> Result<Project, Failure> {
    let cwd = std::env::current_dir()
        .map_err(|e| Failure::Environment(format!("the current folder is not known: {e}")))?;
    Project::open(&find_root(&cwd, global.root.as_deref()))
        .map_err(|e| Failure::Usage(e.to_string()))
}

/// Opens `root`, points a driver host at `driver`, and builds the whole index (`refresh(…, Touched::All)`). The driver starts with the
/// first file it has to read, and exits when `Opened` is dropped.
pub async fn open(root: &Path, driver: &Path) -> Result<Opened, Failure> {
    let project = Project::open(root).map_err(|e| Failure::Usage(e.to_string()))?;
    let host = DriverHost::default();
    host.set_path(Some(driver.to_path_buf()));
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
    fn the_driver_comes_from_the_flag_then_the_environment_then_beside_the_binary() {
        let dir = fresh("driver-order");
        let (flagged, from_env) = (dir.join("flagged"), dir.join("from-env"));
        std::fs::write(&flagged, "").unwrap();
        std::fs::write(&from_env, "").unwrap();
        let exe = dir.join("bin/vnoc");
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(&exe, "").unwrap();
        let beside = dir.join("bin").join(driver_file_name());
        std::fs::write(&beside, "").unwrap();

        let env = Some(from_env.as_os_str());
        assert_eq!(
            driver_path(Some(&flagged), env, Some(&exe)).unwrap(),
            flagged
        );
        assert_eq!(driver_path(None, env, Some(&exe)).unwrap(), from_env);
        assert_eq!(driver_path(None, None, Some(&exe)).unwrap(), beside);
        // an empty VNOC_DRIVER is not set
        assert_eq!(
            driver_path(None, Some(OsStr::new("")), Some(&exe)).unwrap(),
            beside
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
            std::fs::write(dir.join("real").join(driver_file_name()), "").unwrap();
            std::os::unix::fs::symlink(dir.join("real/vnoc"), dir.join("links/vnoc")).unwrap();
            assert_eq!(
                driver_path(None, None, Some(&dir.join("links/vnoc"))).unwrap(),
                dir.join("real").join(driver_file_name())
            );
        }
    }

    #[test]
    fn a_driver_that_is_not_found_says_where_it_looked_and_why_each_place_was_skipped() {
        let dir = fresh("driver-none");
        let exe = dir.join("vnoc");
        std::fs::write(&exe, "").unwrap();
        let Err(Failure::Environment(text)) = driver_path(None, None, Some(&exe)) else {
            panic!("an environment failure");
        };
        assert!(text.contains("--driver: not given"), "{text}");
        assert!(text.contains("VNOC_DRIVER: not set"), "{text}");
        assert!(
            text.contains(&format!(
                "{} does not exist",
                dir.join(driver_file_name()).display()
            )),
            "{text}"
        );

        let missing = dir.join("nope");
        let Err(Failure::Environment(text)) =
            driver_path(None, Some(missing.as_os_str()), Some(&exe))
        else {
            panic!("an environment failure");
        };
        assert!(
            text.contains(&format!(
                "VNOC_DRIVER: {} does not exist",
                missing.display()
            )),
            "{text}"
        );

        let Err(Failure::Environment(text)) = driver_path(None, None, None) else {
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
        let missing = dir.join("nope");
        let Err(Failure::Environment(text)) = driver_path(Some(&missing), None, None) else {
            panic!("an environment failure");
        };
        assert!(
            text.starts_with("the driver given with --driver does not exist"),
            "{text}"
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
