//! Keeping the index up to date with the code on disk.
//!
//! One function does both jobs. It walks the scope (cheap), compares the file list with the index, and asks the driver to
//! outline only what needs it: the files the editor said were touched, and files the index has not seen. The driver is the
//! slow part, so nothing else is ever re-outlined.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::Instant;

use vnoc_domain::RepoPath;

use super::Index;
use crate::driver_host::{DriverHost, HostError};
use crate::project::Project;
use crate::scan;
use crate::userdata::UserData;

/// More files than this and the rest are not read (the index says so). A guard, not a design.
const MAX_FILES: usize = 20_000;

/// A batch this big (a branch switch) is not worth sorting through file by file: it is treated as `Touched::All`.
const TOO_MANY_TOUCHED: usize = 500;

/// Which files the caller knows have changed content (files that are new, or gone, are found by the walk).
pub enum Touched<'a> {
    /// these absolute paths
    Files(&'a [PathBuf]),
    /// read everything again (a rebuild)
    All,
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error(transparent)]
    Driver(#[from] HostError),
    #[error("reading the folder failed: {0}")]
    Scan(#[from] tokio::task::JoinError),
}

/// Brings `index` in line with the disk. Returns whether anything changed (the revision then went up by one).
///
/// The index lock is only held to read the current state and, at the end, to apply the result; never across the driver calls.
/// Callers run one refresh at a time (the server holds a mutex), so the state read at the start is still the state at the end.
pub async fn refresh(project: &Project, host: &DriverHost, index: &RwLock<Index>, touched: Touched<'_>) -> Result<bool, BuildError> {
    let started = Instant::now();
    let walk_project = project.clone();
    let scan = tokio::task::spawn_blocking(move || scan::python_files(&walk_project, MAX_FILES)).await?;
    let walked = started.elapsed();
    let in_scope: BTreeSet<RepoPath> = scan.files.iter().cloned().collect();

    let to_read: Vec<RepoPath> = {
        let known = index.read().unwrap();
        let everything = match &touched {
            Touched::All => true,
            Touched::Files(paths) => paths.len() > TOO_MANY_TOUCHED,
        };
        let touched_paths: BTreeSet<RepoPath> = match &touched {
            Touched::Files(paths) if !everything => paths.iter().filter_map(|p| project.repo_path_of(p)).collect(),
            _ => BTreeSet::new(),
        };
        scan.files.iter().filter(|p| everything || touched_paths.contains(*p) || !known.knows(p)).cloned().collect()
    };

    let mut outlined = Vec::with_capacity(to_read.len());
    for path in to_read {
        let outcome = match host.outline(project.git_root(), path.as_str()).await {
            Ok(result) => Ok((result.defs, result.file_id)),
            // the driver answered with an error for THIS file: note it, carry on with the others
            Err(HostError::Remote { message, .. }) => Err(message),
            Err(e) => return Err(e.into()),
        };
        outlined.push((path, outcome));
    }

    // `.vnoc/` is small and read from disk every time: a doc added by a teammate's `git pull` is as good as one added here
    let data = UserData::new(project.git_root());
    let report = tokio::task::spawn_blocking(move || data.load_all()).await?;
    for problem in &report.problems {
        tracing::warn!("{}", problem.text);
    }

    let read = outlined.len();
    let changed = {
        let mut index = index.write().unwrap();
        let mut changed = index.retain_only(&in_scope);
        changed |= index.set_records(report);
        changed |= index.set_folders(scan.folders);
        changed |= index.set_truncated(scan.truncated);
        for (path, outcome) in outlined {
            changed |= index.set_outline(path, outcome);
        }
        if changed {
            index.resolve_all();
            index.next_revision();
        }
        changed
    };
    tracing::info!("index: walked {} files in {} ms, read {read} with the driver, {} ms in all{}", in_scope.len(), walked.as_millis(), started.elapsed().as_millis(), if changed { "" } else { " (nothing changed)" });
    Ok(changed)
}
