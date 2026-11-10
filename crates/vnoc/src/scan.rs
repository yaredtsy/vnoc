//! Which files are in scope. Skeleton rule: every `.py` file under the opened folder, honouring `.gitignore`, skipping hidden
//! folders (so `.git` and `.vnoc` are out). The real scope rule (`vnoc.toml` `include` / `exclude`) arrives with M1.

use std::path::Path;

pub struct Scan {
    /// repo-relative, `/` separators, sorted
    pub files: Vec<String>,
    /// there were more than `max` files and the rest were dropped
    pub truncated: bool,
}

/// Blocking: walks the disk. Call it through `spawn_blocking`.
pub fn python_files(root: &Path, max: usize) -> std::io::Result<Scan> {
    if !root.is_dir() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("{} is not a folder", root.display())));
    }
    let mut files = Vec::new();
    // `require_git(false)`: honour .gitignore even when the opened folder is not (the top of) a git repository
    for entry in ignore::WalkBuilder::new(root).hidden(true).git_ignore(true).git_exclude(true).git_global(false).require_git(false).build().flatten() {
        let path = entry.path();
        if !entry.file_type().is_some_and(|t| t.is_file()) || path.extension().and_then(|e| e.to_str()) != Some("py") {
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else { continue };
        // a path that is not UTF-8 cannot be a repo path in V-NOC: skipped
        if let Some(rel) = rel.to_str() {
            files.push(rel.replace('\\', "/"));
        }
    }
    files.sort();
    let truncated = files.len() > max;
    files.truncate(max);
    Ok(Scan { files, truncated })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(label: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vnoc-scan-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn put(root: &Path, rel: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "x = 1\n").unwrap();
    }

    #[test]
    fn finds_python_files_sorted_and_relative() {
        let d = fresh("basic");
        for f in ["b/z.py", "a.py", "b/a.py", "notes.txt", ".hidden/x.py", ".vnoc/y.py"] {
            put(&d, f);
        }
        let s = python_files(&d, 100).unwrap();
        assert_eq!(s.files, vec!["a.py", "b/a.py", "b/z.py"]);
        assert!(!s.truncated);
    }

    #[test]
    fn honours_gitignore_and_says_when_it_truncated() {
        let d = fresh("ignore");
        std::fs::write(d.join(".gitignore"), "build/\n").unwrap();
        for f in ["build/gen.py", "a.py", "b.py", "c.py"] {
            put(&d, f);
        }
        let s = python_files(&d, 2).unwrap();
        assert_eq!(s.files, vec!["a.py", "b.py"]);
        assert!(s.truncated, "c.py was dropped, and build/ was ignored");
    }

    #[test]
    fn a_missing_folder_is_an_error() {
        assert!(python_files(Path::new("/definitely/not/here"), 10).is_err());
    }
}
