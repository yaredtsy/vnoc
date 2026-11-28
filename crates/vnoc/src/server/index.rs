use std::sync::atomic::Ordering;

use async_lsp::ResponseError;

use super::{Core, State, Status, Work, internal};
use crate::driver_host::LanguageState;
use crate::index::{self, BuildError, Touched, refresh};
use crate::protocol;
use crate::protocol::{
    IndexChanged, IndexChangedParams, LanguageInfo, ProjectInfoResult, SearchCardsParams,
    SearchCardsResult, SearchParams, SearchResult, StructureResult,
};
use crate::search::{Prepared, answer, prepare};
use crate::userdata::UserData;
use crate::watchdog::timed;
use vnoc_search::{Limits, matches, run};

impl Core {
    /// Starts an update in the background.
    pub(super) fn spawn_update(&self, work: Work) {
        if matches!(work, Work::Everything)
            && self.state.everything_pending.swap(true, Ordering::AcqRel)
        {
            tracing::debug!("a full read is already waiting: dropped");
            return;
        }
        let core = self.clone();
        tokio::spawn(async move { core.update(work).await });
    }

    pub(super) async fn update(&self, work: Work) {
        let Some(project) = self.state.project.get() else {
            return;
        };
        let _only_one = self.state.one_at_a_time.lock().await;
        if matches!(work, Work::Everything) {
            self.state
                .everything_pending
                .store(false, Ordering::Release);
        }
        // until one build has worked, every update is a full one (this also retries a build that failed); so is the one after an update that did not finish
        let built = matches!(*self.state.status.borrow(), Status::Ready);
        let first = !built || self.state.needs_everything.load(Ordering::Acquire);
        let limit = self.state.update_limit;
        if !built && let Some(hold) = super::test_millis("VNOC_HOLD_FIRST_BUILD_MS") {
            tokio::time::sleep(hold).await; // for the window tests: a first build that takes longer than a request may wait
        }
        // the whole update is bounded: past the limit it is dropped where it stands (it holds no lock across a driver call), `one_at_a_time` goes with this scope, and the
        // driver is told to cancel the call that was running
        let result = tokio::time::timeout(limit, async {
            match &work {
                Work::Files(paths) if !first => {
                    refresh(
                        project,
                        &self.host,
                        &self.state.index,
                        Touched::Files(paths),
                    )
                    .await
                }
                _ => refresh(project, &self.host, &self.state.index, Touched::All).await,
            }
        })
        .await
        .unwrap_or(Err(BuildError::TooLong(limit)));
        match result {
            Ok(changed) => {
                self.state.needs_everything.store(false, Ordering::Release);
                self.state.status.send_replace(Status::Ready);
                if changed || !built {
                    self.announce();
                }
            }
            // after a first success the old index stays; the driver was dropped, so the next update starts a new one
            Err(e) if !built => {
                tracing::error!("the first build failed: {e}");
                self.state
                    .status
                    .send_replace(Status::Failed(e.to_string()));
            }
            Err(e) => {
                tracing::warn!("update failed, the index is as it was: {e}");
                self.state.needs_everything.store(true, Ordering::Release);
            }
        }
    }

    pub(super) fn announce(&self) {
        let revision = self.state.index.read().unwrap().revision();
        if let Err(e) = self
            .client
            .notify::<IndexChanged>(IndexChangedParams { revision })
        {
            tracing::warn!("could not tell the editor about revision {revision}: {e}");
        }
    }
}

/// `vnoc/structure`: the tree, from the index. Waits for the first build.
pub(super) fn structure(
    core: &mut Core,
    _ignored: serde_json::Value,
) -> impl std::future::Future<Output = Result<StructureResult, ResponseError>> + Send + use<> {
    let (state, host) = (core.state.clone(), core.host.clone());
    timed("structure", async move {
        let status = state
            .status
            .subscribe()
            .wait_for(|s| !matches!(s, Status::Building))
            .await
            .map_err(internal)?
            .clone();
        if let Status::Failed(message) = status {
            return Err(internal(message));
        }
        let project = state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        let capabilities = host.capabilities().await;
        let index = state.index.read().unwrap();
        Ok(index::structure(&index, project, capabilities))
    })
}

/// `vnoc/languages`: the drivers and what each said. Waits for the first build (it starts every driver), and answers whether or not it worked: a
/// driver that failed is in the list with its reason, which is what the editor shows.
pub(super) fn languages(
    core: &mut Core,
    _ignored: serde_json::Value,
) -> impl std::future::Future<Output = Result<Vec<LanguageInfo>, ResponseError>> + Send + use<> {
    let (state, host) = (core.state.clone(), core.host.clone());
    timed("languages", async move {
        state
            .status
            .subscribe()
            .wait_for(|s| !matches!(s, Status::Building))
            .await
            .map_err(internal)?;
        Ok(host
            .languages()
            .await
            .into_iter()
            .map(|l| {
                let (state, error) = match l.state {
                    LanguageState::Running => (protocol::LanguageState::Running, None),
                    LanguageState::Failed(why) => (protocol::LanguageState::Failed, Some(why)),
                };
                LanguageInfo {
                    name: l.name,
                    version: l.version,
                    path: l.path.to_string_lossy().into_owned(),
                    extensions: l.extensions,
                    capabilities: l.capabilities,
                    state,
                    error,
                    warnings: l.warnings,
                }
            })
            .collect())
    })
}

/// Waits for the first build, then the query read and its corpus (`search::prepare`): what `vnoc/search` and `vnoc/searchCards` both start from.
/// No `.await` while a lock is held: the waiting is done before `prepare` takes any.
async fn prepared(state: &State, text: &str) -> Result<Prepared, ResponseError> {
    let status = state
        .status
        .subscribe()
        .wait_for(|s| !matches!(s, Status::Building))
        .await
        .map_err(internal)?
        .clone();
    if let Status::Failed(message) = status {
        return Err(internal(message));
    }
    let project = state
        .project
        .get()
        .ok_or_else(|| internal("no folder is open"))?;
    Ok(prepare(
        project,
        &state.index,
        &state.corpus,
        &state.doc_index,
        text,
    ))
}

/// `vnoc/search`: the nodes a query finds. The corpus is built from the tree on the first search after a change and kept (`search::kept`); the search
/// itself runs on it with no lock held.
pub(super) fn search(
    core: &mut Core,
    params: SearchParams,
) -> impl std::future::Future<Output = Result<SearchResult, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("search", async move {
        let found = prepared(&state, &params.query).await?;
        let defaults = Limits::default();
        let limits = Limits {
            nodes: params.nodes.unwrap_or(defaults.nodes),
            docs: params.docs.unwrap_or(defaults.docs),
        };
        let results = run(&found.corpus, &found.query, found.exact, limits);
        Ok(answer(found.revision, &found.corpus, &found.query, results))
    })
}

/// `vnoc/searchCards`: which of the asked node ids the query finds (`vnoc_search::matches`).
pub(super) fn search_cards(
    core: &mut Core,
    params: SearchCardsParams,
) -> impl std::future::Future<Output = Result<SearchCardsResult, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("searchCards", async move {
        let found = prepared(&state, &params.query).await?;
        let asked: Vec<&str> = params.ids.iter().map(String::as_str).collect();
        let hit = matches(&found.corpus, &found.query, found.exact, &asked);
        Ok(SearchCardsResult {
            revision: found.revision,
            hits: params
                .ids
                .iter()
                .zip(hit)
                .filter(|&(_, hit)| hit)
                .map(|(id, _)| id.clone())
                .collect(),
        })
    })
}

/// `vnoc/projectInfo`: whether the first-attach question still has to be asked.
pub(super) fn project_info(
    core: &mut Core,
    _ignored: serde_json::Value,
) -> impl std::future::Future<Output = Result<ProjectInfoResult, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("projectInfo", async move {
        let project = state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        let settings = UserData::new(project.git_root())
            .settings()
            .map_err(internal)?;
        Ok(ProjectInfoResult {
            initialized: settings.initialized,
            markers: settings.markers,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;
    use std::sync::Arc;
    use std::time::Duration;

    /// An update that can never end (a driver that starts and says nothing) is stopped at the limit: the status says so in words, the lock is free, and the next
    /// update is a full one that tries again. Needs a process that hangs: a script, so not on Windows.
    #[cfg(unix)]
    #[tokio::test]
    async fn an_update_that_runs_past_its_limit_is_stopped_and_frees_the_lock() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("vnoc-update-limit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::testdir::remove_at_end(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let driver = dir.join("hangs");
        std::fs::write(&driver, "#!/bin/sh\nexec sleep 30\n").unwrap();
        std::fs::set_permissions(&driver, std::fs::Permissions::from_mode(0o755)).unwrap();

        let state = Arc::new(State {
            update_limit: Duration::from_millis(400),
            ..State::new()
        });
        assert!(state.project.set(Project::open(&dir).unwrap()).is_ok());
        let core = Core {
            client: async_lsp::ClientSocket::new_closed(),
            host: Arc::default(),
            state,
        };
        core.host.set_paths(vec![driver]);

        let started = std::time::Instant::now();
        core.update(Work::Everything).await;
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the update was not stopped: {:?}",
            started.elapsed()
        );
        match &*core.state.status.borrow() {
            Status::Failed(why) => assert!(why.contains("took longer"), "{why}"),
            other => panic!(
                "the status is not Failed: {}",
                matches!(other, Status::Ready)
            ),
        }
        assert!(
            core.state.one_at_a_time.try_lock().is_ok(),
            "the update still holds the lock"
        );
    }

    /// After a good first build, an update that fails keeps the old index and Ready, and the next update reads everything again.
    #[tokio::test]
    async fn a_failed_update_after_a_first_build_keeps_the_index_and_the_next_one_reads_everything()
    {
        let dir = std::env::temp_dir().join(format!("vnoc-update-retry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::testdir::remove_at_end(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let state = Arc::new(State::new());
        assert!(state.project.set(Project::open(&dir).unwrap()).is_ok());
        state.status.send_replace(Status::Ready);
        let core = Core {
            client: async_lsp::ClientSocket::new_closed(),
            host: Arc::default(), // no driver: the update fails at once
            state,
        };
        core.update(Work::Files(vec![dir.join("a.py")])).await;
        assert!(matches!(*core.state.status.borrow(), Status::Ready));
        assert!(
            core.state
                .needs_everything
                .load(std::sync::atomic::Ordering::Acquire)
        );
    }
}
