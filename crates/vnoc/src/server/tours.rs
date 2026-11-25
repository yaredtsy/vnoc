use std::sync::Arc;

use async_lsp::{ErrorCode, ResponseError};

use super::{Core, State, bad_params, blocking, internal, open_project, ready_project, write_lock};
use crate::calls::{self, CallsError};
use crate::identity::{EditSink, Env};
use crate::protocol;
use crate::tours::act::{self, ActError};
use crate::tours::check::{Answer, Mode};
use crate::tours::find;
use crate::tours::on::{self, Query};
use crate::tours::place::Move;
use crate::tours::record::{StopId, TourId};
use crate::tours::route;
use crate::tours::serve::{self, Cannot, GoError};
use crate::tours::settings::{Choice, Key};
use crate::tours::store::{self, Halt, StoreError};
use crate::tours::view;
use crate::watchdog::timed;

// ------------------------------------------------------------------------------------------------------------------------------------ tours

/// A store failure as a response: a tour that is not there, is in both places or cannot be read is the request's; the lock is busy (`REQUEST_FAILED`: the request
/// was fine, the tour was not free); the disk is the core's.
pub(super) fn tour_error(e: StoreError) -> ResponseError {
    match e {
        StoreError::NotFound(_)
        | StoreError::InBoth(_)
        | StoreError::TooBig(_)
        | StoreError::Refused(_)
        | StoreError::Unreadable { .. } => bad_params(e),
        StoreError::Lock(_) => ResponseError::new(ErrorCode::REQUEST_FAILED, e),
        StoreError::Io { .. } | StoreError::Init(_) | StoreError::NoFreeId => internal(e),
    }
}

pub(super) fn parse_tour(id: &str) -> Result<TourId, ResponseError> {
    TourId::parse(id).map_err(bad_params)
}

/// The answer for each of these tours: the core's memory, the recorded check, a check (03 §3). The first two rungs need no index, so they are asked first with none:
/// a tour whose answer is remembered or recorded is answered while the first build or an update is still running. Only a real check waits: for the first build, then
/// for an update that is running (a check made against the index it is about to replace would record a stale answer), then it is made with the index.
pub(super) async fn answers_for(
    state: &Arc<State>,
    rows: Vec<Arc<store::Row>>,
) -> Result<std::collections::BTreeMap<TourId, Answer>, ResponseError> {
    match ask_tours(state, &rows, false).await? {
        Ok(done) => return Ok(done),
        Err(Cannot::Store(e)) => return Err(tour_error(e)),
        Err(Cannot::NeedsIndex) => {}
    }
    // a request that cannot be answered in a moment says to ask again, it does not sit in a slot
    super::wait_for_index(state).await?;
    match ask_tours(state, &rows, true).await? {
        Ok(done) => Ok(done),
        Err(Cannot::Store(e)) => Err(tour_error(e)),
        Err(Cannot::NeedsIndex) => Err(internal("the index is not ready")),
    }
}

/// One ask of the core's tours, with the index or without.
pub(super) async fn ask_tours(
    state: &Arc<State>,
    rows: &[Arc<store::Row>],
    with_index: bool,
) -> Result<Result<std::collections::BTreeMap<TourId, Answer>, Cannot>, ResponseError> {
    let (held, rows) = (state.clone(), rows.to_vec());
    blocking(move || {
        let project = held.project.get().ok_or(Cannot::Store(StoreError::Refused(
            "no folder is open".into(),
        )))?;
        let ask = serve::Ask {
            vnoc_root: &store::vnoc_root(project.git_root()),
            project,
            index: with_index.then_some(&held.index),
        };
        held.tours.answers(&ask, &rows, Mode::default())
    })
    .await
}

/// The node `text` names, as the question `list --on` asks of the map. Waits for the first build. The root of the tree is every tour's: refused in the
/// command's words.
pub(super) async fn node_query(state: &Arc<State>, text: &str) -> Result<Query, ResponseError> {
    let project = ready_project(state).await?;
    let index = state.index.read().unwrap_or_else(|e| e.into_inner());
    let tree = index.tree(&project.name(), project.prefix());
    let node = find::node_named(&index, project, &tree, text)
        .ok_or_else(|| bad_params(format!("no node is named `{text}`")))?;
    Query::of(node).ok_or_else(|| bad_params(on::ROOT_IS_EVERY_TOURS))
}

/// `vnoc/tours`: the rows of the list, newest first. The freshness of the rows named in `check` is asked for (and so, if need be, checked); the others say what the
/// core remembers.
pub(super) fn tours(
    core: &mut Core,
    params: Option<protocol::ToursParams>,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tours", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let params = params.unwrap_or_default();
        let asked: Vec<TourId> = params
            .check
            .iter()
            .map(|id| parse_tour(id))
            .collect::<Result<_, _>>()?;
        let rows = {
            let (state, vnoc_root) = (state.clone(), vnoc_root.clone());
            blocking(move || state.tours.sync(&vnoc_root).0).await?
        };
        let to_check: Vec<_> = rows
            .iter()
            .filter(|r| asked.contains(&r.id))
            .cloned()
            .collect();
        let mut known = {
            let (state, ids) = (
                state.clone(),
                rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
            );
            blocking(move || state.tours.remembered(&ids)).await?
        };
        if !to_check.is_empty() {
            for (id, answer) in answers_for(&state, to_check).await? {
                known.insert(id, answer.recorded);
            }
        }
        // the node, if one was asked for: the one thing here that needs the index; found by what the tree says of it
        let query = match params.on {
            Some(text) => Some(node_query(&state, &text).await?),
            None => None,
        };
        let shown = {
            let (state, rows) = (state.clone(), rows.clone());
            blocking(move || {
                let project = state.project.get()?;
                let root = store::vnoc_root(project.git_root());
                let shown = serve::list_rows(project, &rows, &known, true, &|id| {
                    store::read_place(&root, id)
                });
                Some(match &query {
                    Some(query) => serve::rows_on(shown, &rows, &state.tours.on(query)),
                    None => shown,
                })
            })
            .await?
            .ok_or_else(|| internal("no folder is open"))?
        };
        Ok(serde_json::json!({ "tours": shown }))
    })
}

/// `vnoc/tour`: the tour as the stage plays it. Its freshness is asked for; the place is the one written, as the tour is now.
pub(super) fn tour(
    core: &mut Core,
    params: protocol::TourParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tour", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let id = parse_tour(&params.id)?;
        let row = {
            let (state, vnoc_root, id) = (state.clone(), vnoc_root.clone(), id.clone());
            blocking(move || state.tours.row(&vnoc_root, &id)).await?
        }
        .ok_or_else(|| bad_params(StoreError::NotFound(id.clone())))?;
        let Ok(found) = &row.tour else {
            let why = row.tour.as_ref().err().map(|e| e.why.clone());
            return Ok(serde_json::json!({ "unreadable": why }));
        };
        let answer = answers_for(&state, vec![row.clone()]).await?.remove(&id);
        let shown = {
            let (state, row, found) = (state.clone(), row.clone(), found.clone());
            blocking(move || {
                let project = state.project.get()?;
                let root = store::vnoc_root(project.git_root());
                let mut shown = serve::show_view(
                    project,
                    &found,
                    row.place,
                    &row.path,
                    store::read_place(&root, &row.id).as_ref(),
                );
                if let Some(answer) = &answer {
                    view::mark(&mut shown, answer);
                }
                Some(shown)
            })
            .await?
            .ok_or_else(|| internal("no folder is open"))?
        };
        serde_json::to_value(shown).map_err(internal)
    })
}

/// `vnoc/tourGo`: the same function as `vnoc tour go`, under the tours lock; the place written is remembered, so the watcher's word of it is not an echo.
pub(super) fn tour_go(
    core: &mut Core,
    params: protocol::TourGoParams,
) -> impl std::future::Future<Output = Result<crate::tours::view::PlaceView, ResponseError>> + Send + use<>
{
    let state = core.state.clone();
    timed("tourGo", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let id = parse_tour(&params.id)?;
        let wanted = Move::parse(&params.r#move).map_err(bad_params)?;
        let gone = blocking(move || state.tours.go(&vnoc_root, &id, wanted))
            .await?
            .map_err(|e| match e {
                GoError::Store(e) => tour_error(e),
                GoError::NoSuch { .. } => bad_params(e),
            })?;
        Ok(view::place_view(&gone.tour, gone.place))
    })
}

/// `vnoc/tourOpen`: sets `place.open`, the same way.
pub(super) fn tour_open(
    core: &mut Core,
    params: protocol::TourOpenParams,
) -> impl std::future::Future<Output = Result<crate::tours::view::PlaceView, ResponseError>> + Send + use<>
{
    let state = core.state.clone();
    timed("tourOpen", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let id = parse_tour(&params.id)?;
        blocking(move || {
            let place = state.tours.open(&vnoc_root, &id, params.open)?;
            Ok(state.tours.place_view(&vnoc_root, &id, place))
        })
        .await?
        .map_err(tour_error)
    })
}

/// `vnoc/tourHalt`: writes `halt` through `store::halt`, the function of `vnoc tour halt`, and answers the tour's state.
pub(super) fn tour_halt(
    core: &mut Core,
    params: protocol::TourHaltParams,
) -> impl std::future::Future<Output = Result<protocol::TourHaltResult, ResponseError>> + Send + use<>
{
    let state = core.state.clone();
    timed("tourHalt", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let id = parse_tour(&params.id)?;
        let (did, tour) = blocking(move || state.tours.halt(&vnoc_root, &id))
            .await?
            .map_err(tour_error)?;
        Ok(protocol::TourHaltResult {
            halt: match did {
                Halt::Written => "written",
                Halt::Already => "already",
                Halt::Finished => "finished",
            }
            .to_string(),
            state: view::state_word(tour.state).to_string(),
        })
    })
}

/// `vnoc/tourCode`: one stop of a pinned tour as it was written; `null` when there is none to give (03 §5).
pub(super) fn tour_code(
    core: &mut Core,
    params: protocol::TourCodeParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tourCode", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let id = parse_tour(&params.id)?;
        let stop = StopId::parse(&params.stop).map_err(bad_params)?;
        let row = {
            let (state, vnoc_root, id) = (state.clone(), vnoc_root.clone(), id.clone());
            blocking(move || state.tours.row(&vnoc_root, &id)).await?
        }
        .ok_or_else(|| bad_params(StoreError::NotFound(id.clone())))?;
        let tour = row.tour.as_ref().map_err(|e| {
            bad_params(StoreError::Unreadable {
                id: id.clone(),
                why: e.why.clone(),
                too_new: e.too_new,
            })
        })?;
        let tour = tour.clone();
        let code = blocking(move || {
            let project = state.project.get()?;
            Some(serve::stop_code(project, &tour, stop))
        })
        .await?
        .ok_or_else(|| internal("no folder is open"))?
        .map_err(bad_params)?;
        serde_json::to_value(code).map_err(internal)
    })
}

/// `vnoc/tourCounts`: how many tours pass through each node, by the tree's key. Waits for the first build; the map of nodes is the one `vnoc/tours { on }`
/// asks, so nothing is replayed that was not.
pub(super) fn tour_counts(
    core: &mut Core,
    _ignored: serde_json::Value,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tourCounts", async move {
        let project = ready_project(&state).await?;
        let vnoc_root = store::vnoc_root(project.git_root());
        // the tours are looked at (`stat`s) before they are counted: one that was written by another process and not announced is counted too
        {
            let (state, vnoc_root) = (state.clone(), vnoc_root.clone());
            blocking(move || state.tours.sync(&vnoc_root)).await?;
        }
        let counts = {
            let state = state.clone();
            blocking(move || {
                let project = state.project.get()?;
                let tree = {
                    let index = state.index.read().unwrap_or_else(|e| e.into_inner());
                    index.tree(&project.name(), project.prefix())
                };
                Some(state.tours.counts(&tree))
            })
            .await?
            .ok_or_else(|| internal("no folder is open"))?
        };
        Ok(serde_json::json!({ "counts": counts }))
    })
}

/// How long a `vnoc/tourCalls` request waits for the count: the core answers a request at a time per slot (`ConcurrencyLayer`: as many as there are cores), and a count asks the
/// driver once for each stop. A count that takes longer goes on in the background and the request is answered with an error that says to ask again; the answer is kept when it is done.
const CALLS_WAIT: std::time::Duration = std::time::Duration::from_millis(1500);
/// How long the driver is given for one stop's calls.
const CALLS_ASK: std::time::Duration = std::time::Duration::from_secs(4);

/// `vnoc/tourCalls`: for each written function or class stop, how many of the functions it calls, one level down, are not a stop of this tour (design 04 §3: the
/// card's `+N`). The callees are the ones `vnoc tour route` finds ([`route::from_calls`], one level, the same `known`), each counted once, by tree key. Kept in the
/// core until the tour's file or a stop's file changes; waits for the first build. The count is made by a task of its own (one at a time for a tour), so a driver that is slow
/// holds no request: this one answers within [`CALLS_WAIT`], or says to ask again.
pub(super) fn tour_calls(
    core: &mut Core,
    params: protocol::TourParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("tourCalls", async move {
        let state = core.state.clone();
        let project = ready_project(&state).await?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let id = parse_tour(&params.id)?;
        let row = {
            let (state, vnoc_root, id) = (state.clone(), vnoc_root.clone(), id.clone());
            blocking(move || state.tours.row(&vnoc_root, &id)).await?
        }
        .ok_or_else(|| bad_params(StoreError::NotFound(id.clone())))?;
        if let Err(e) = &row.tour {
            return Err(bad_params(StoreError::Unreadable {
                id: id.clone(),
                why: e.why.clone(),
                too_new: e.too_new,
            }));
        }
        if let Some(off) = state.tours.calls_kept(&id) {
            return Ok(serde_json::json!({ "off": off }));
        }
        if state.calls_running.lock().unwrap().insert(id.clone()) {
            let (core, row, id) = (core.clone(), row.clone(), id.clone());
            tokio::spawn(async move {
                if let Err(e) = count_calls(&core, &row).await {
                    tracing::debug!("the calls of tour {id} were not counted: {}", e.message);
                }
                core.state.calls_running.lock().unwrap().remove(&id);
            });
        }
        let deadline = tokio::time::Instant::now() + CALLS_WAIT;
        loop {
            if let Some(off) = state.tours.calls_kept(&id) {
                return Ok(serde_json::json!({ "off": off }));
            }
            let running = state.calls_running.lock().unwrap().contains(&id);
            if !running || tokio::time::Instant::now() >= deadline {
                return Err(being_counted());
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
}

/// The refusal of `vnoc/tourCalls` while the count goes on: in words, and as `data.askAgain`.
fn being_counted() -> ResponseError {
    super::ask_again_with("the calls of this tour are being counted: ask again")
}

/// What a stop's driver ask came to, for the count of `vnoc/tourCalls`.
enum Asked<T> {
    Calls(T),
    /// the driver has no call trees: no stop has calls to count
    NoDriver,
    /// a node the driver cannot start a tree at, a driver that is slow (past [`CALLS_ASK`]) or broke: the stop is left out of the count, which goes on and is kept all the
    /// same (the tour's or a stop's file changing drops it): a count made again at every ask would never end
    Left,
}

fn of_the_ask<T>(asked: Result<Result<T, CallsError>, tokio::time::error::Elapsed>) -> Asked<T> {
    match asked {
        Ok(Ok(answered)) => Asked::Calls(answered),
        Ok(Err(CallsError::MissingCapability(_))) => Asked::NoDriver,
        Ok(Err(CallsError::Invalid(_) | CallsError::Internal(_))) | Err(_) => Asked::Left,
    }
}

/// The count of `vnoc/tourCalls` for one tour, kept when the tour is as it was when it started (a stop whose driver ask failed is left out).
async fn count_calls(core: &Core, row: &Arc<store::Row>) -> Result<(), ResponseError> {
    let state = core.state.clone();
    let project = ready_project(&state).await?;
    let found = row
        .tour
        .as_ref()
        .map_err(|e| bad_params(e.why.clone()))?
        .clone();
    // an update that is running goes first: it needs the driver too, and the calls are of the index it is about to give
    drop(state.one_at_a_time.lock().await);
    // the stops as the view has them: where each is now (a check moved its key), which are written, which are gone
    let answer = answers_for(&state, vec![row.clone()])
        .await?
        .remove(&row.id);
    let shown = {
        let (state, row) = (state.clone(), row.clone());
        blocking(move || {
            let project = state.project.get()?;
            let root = store::vnoc_root(project.git_root());
            let mut shown = serve::show_view(
                project,
                &found,
                row.place,
                &row.path,
                store::read_place(&root, &row.id).as_ref(),
            );
            if let Some(answer) = &answer {
                view::mark(&mut shown, answer);
            }
            Some(shown)
        })
        .await?
        .ok_or_else(|| internal("no folder is open"))?
    };
    let tree = {
        let index = state.index.read().unwrap_or_else(|e| e.into_inner());
        index.tree(&project.name(), project.prefix())
    };
    let stops_of_the_tour: std::collections::BTreeSet<&str> = shown
        .stops
        .iter()
        .filter_map(|s| s.key.as_deref())
        .collect();
    let known = |target: &vnoc_driver_protocol::CallTarget| {
        let location = vnoc_domain::LocationKey::new(
            project.repo_path(target.file.as_deref()?).ok()?,
            target.qualname.clone(),
            target.occurrence,
        )
        .ok()?;
        tree.find(&location.to_string()).and_then(route::known_of)
    };
    let mut off = std::collections::BTreeMap::new();
    for stop in &shown.stops {
        let (Some(key), Some("function" | "class")) = (stop.key.as_deref(), stop.kind.as_deref())
        else {
            continue;
        };
        if stop.planned || stop.check == Some("gone") {
            continue;
        }
        let Some(root) = tree.find(key).and_then(route::known_of) else {
            continue;
        };
        let asked = tokio::time::timeout(
            CALLS_ASK,
            calls::ask(project, &state.index, &core.host, key, Some(1)),
        )
        .await;
        let answered = match of_the_ask(asked) {
            Asked::Calls(answered) => answered,
            Asked::NoDriver => break,
            Asked::Left => continue,
        };
        let item = route::from_calls(&calls::merge(&answered.root), root, 1, &known);
        let outside: std::collections::BTreeSet<&str> = item
            .children
            .iter()
            .map(|c| c.known.id.as_str())
            .filter(|callee| !stops_of_the_tour.contains(callee))
            .collect();
        if !outside.is_empty() {
            off.insert(stop.id.clone(), outside.len());
        }
    }
    let files: std::collections::BTreeSet<String> = shown
        .stops
        .iter()
        .flat_map(|s| std::iter::once(s.path.clone()).chain(s.now.as_ref().map(|n| n.path.clone())))
        .collect();
    // kept also when the index moved while the calls were asked: a file that changed drops the count through `vnoc/filesChanged`, as it does an answer
    state.tours.keep_calls(row, off, files);
    Ok(())
}

// ------------------------------------------------------------------------------------------------------------------------------------ the user's actions

/// An action's failure as a response: the request's mistakes are `INVALID_PARAMS` with the command's sentence, the store's are its own, the rest is the core's.
pub(super) fn act_error(e: ActError) -> ResponseError {
    use crate::identity::AttachError as A;
    match e {
        ActError::Store(e) => tour_error(e),
        ActError::Usage(said) | ActError::NoStop { said, .. } => bad_params(said),
        ActError::Attach(
            e @ (A::TitleTaken { .. }
            | A::BadTitle
            | A::BodyTooLarge
            | A::BodyHasFrontmatter
            | A::BadTag(_)
            | A::BadAuthor
            | A::BadKey(_)
            | A::NotInIndex(_)),
        ) => bad_params(e),
        ActError::Attach(e) => internal(e),
        ActError::Environment(said) => internal(said),
    }
}

/// `vnoc/tourPin`: [`act::pin_begin`] and [`act::pin_finish`], the functions of `vnoc tour pin`. With `commit` the project's write lock is taken first (and the
/// attach mutex before it), then the tours lock inside the pin: the order of every writer.
pub(super) fn tour_pin(
    core: &mut Core,
    params: protocol::TourPinParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("tourPin", async move {
        let state = core.state.clone();
        let project = ready_project(&state).await?;
        let id = parse_tour(&params.id)?;
        if params.dry && params.commit.is_some() {
            return Err(bad_params(
                "a dry pin commits nothing: ask for it with no `commit`",
            ));
        }
        let dry = params.dry;
        let _one_at_a_time;
        let _one_writer;
        if params.commit.is_some() {
            _one_at_a_time = Some(state.attaching.lock().await);
            _one_writer = Some(write_lock(project).await?);
        } else {
            _one_at_a_time = None;
            _one_writer = None;
        }
        let vnoc_root = store::vnoc_root(project.git_root());
        let message = params.commit.as_ref().and_then(|c| c.message.clone());
        let commit = params.commit.is_some();
        let held = state.clone();
        let done = blocking(move || {
            let state = held;
            let project = state
                .project
                .get()
                .ok_or(ActError::Environment("no folder is open".into()))?;
            let ask = if dry {
                act::Ask::Look
            } else {
                act::Ask::Pin {
                    commit,
                    message: message.as_deref(),
                }
            };
            act::not_in_both(&vnoc_root, &id)?;
            let tour = act::read_tour(&vnoc_root, &id)?;
            if let Some(commit) = act::pinned_already(project, &tour) {
                return Ok(serde_json::json!({
                    "tour": id.to_string(),
                    "already": true,
                    "state": "pinned",
                    "commit": commit.as_str(),
                }));
            }
            let out = match act::pin_begin(project, &vnoc_root, &id, &ask)? {
                act::Begun::Done(pinned) => pinned.out,
                act::Begun::Check { measured, head } => {
                    act::pin_finish(
                        project,
                        &state.index,
                        &vnoc_root,
                        &id,
                        &ask,
                        &measured,
                        head,
                    )?
                    .out
                }
            };
            let mut shown =
                serde_json::to_value(&out).map_err(|e| ActError::Environment(e.to_string()))?;
            // each reason says itself, in the core's words: a page makes no sentence from a `code`
            if let Some(reasons) = shown.get_mut("reasons").and_then(|r| r.as_array_mut()) {
                for (said, reason) in reasons.iter_mut().zip(&out.reasons) {
                    said["words"] = serde_json::json!(reason.words());
                }
            }
            shown["tour"] = serde_json::json!(id.to_string());
            Ok(shown)
        })
        .await?
        .map_err(act_error)?;
        Ok(done)
    })
}

/// `vnoc/tourIds`: [`act::write_ids`], the function of `vnoc tour ids`, with the server's drivers. The edits go to the files on disk, as the command's do: a
/// tour's stops are not open documents the editor could give (`vnoc/writeId` is for the node under the cursor); the watcher tells the core of the files, as of
/// an `add`. The project's write lock is first; each marker's edit takes the tours lock to rebase the tours.
pub(super) fn tour_ids(
    core: &mut Core,
    params: protocol::TourIdsParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("tourIds", async move {
        let project = ready_project(&core.state).await?;
        let id = parse_tour(&params.id)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        // the same lock as attach: both write an ID and a node.toml
        let _one_at_a_time = core.state.attaching.lock().await;
        let _one_writer = write_lock(project).await?;
        let mut env = Env {
            project,
            index: &core.state.index,
            host: &core.host,
            edits: EditSink::Disk,
        };
        let written = act::write_ids(&mut env, &vnoc_root, &id, &[])
            .await
            .map_err(act_error)?;
        let mut shown = serde_json::to_value(&written).map_err(internal)?;
        shown["words"] = serde_json::json!(written.sentence());
        Ok(shown)
    })
}

/// `vnoc/tourEdit`: [`act::edit`].
pub(super) fn tour_edit(
    core: &mut Core,
    params: protocol::TourEditParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tourEdit", async move {
        let project = open_project(&state)?;
        let id = parse_tour(&params.id)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let (title, question) = (params.title.clone(), params.question.clone());
        {
            let id = id.clone();
            blocking(move || {
                act::edit(&vnoc_root, &id, title.as_deref(), question.as_deref(), None)
            })
            .await?
            .map_err(act_error)?;
        }
        Ok(serde_json::json!({
            "tour": id.to_string(),
            "title": params.title.as_deref().map(str::trim),
            "question": params.question.as_deref().map(str::trim),
        }))
    })
}

/// `vnoc/tourShare`: [`store::share`], the function of `vnoc tour share`.
pub(super) fn tour_share(
    core: &mut Core,
    params: protocol::TourShareParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tourShare", async move {
        let project = open_project(&state)?;
        let id = parse_tour(&params.id)?;
        let root = project.git_root().to_path_buf();
        let vnoc_root = store::vnoc_root(&root);
        let to = if params.share {
            store::Where::Project
        } else {
            store::Where::Private
        };
        let moved = {
            let id = id.clone();
            blocking(move || store::share(&vnoc_root, &id, to))
                .await?
                .map_err(tour_error)?
        };
        let path = moved
            .strip_prefix(&root)
            .unwrap_or(&moved)
            .to_string_lossy()
            .replace('\\', "/");
        Ok(serde_json::json!({
            "tour": id.to_string(),
            "folder": view::place_word(to),
            "path": path,
        }))
    })
}

/// `vnoc/tourDelete`: [`store::delete`], the function of `vnoc tour delete`; the token is the trash entry.
pub(super) fn tour_delete(
    core: &mut Core,
    params: protocol::TourDeleteParams,
) -> impl std::future::Future<Output = Result<protocol::TourDeleteResult, ResponseError>> + Send + use<>
{
    let state = core.state.clone();
    timed("tourDelete", async move {
        let project = open_project(&state)?;
        let id = parse_tour(&params.id)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let gone = blocking(move || store::delete(&vnoc_root, &id))
            .await?
            .map_err(tour_error)?;
        Ok(protocol::TourDeleteResult { token: gone.entry })
    })
}

/// `vnoc/tourRestore`: [`store::restore`]: the file and the state come back where they were.
pub(super) fn tour_restore(
    core: &mut Core,
    params: protocol::TourRestoreParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tourRestore", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let back = blocking(move || store::restore(&vnoc_root, &params.token))
            .await?
            .map_err(tour_error)?;
        Ok(serde_json::json!({
            "tour": back.id.to_string(),
            "folder": view::place_word(back.was),
        }))
    })
}

/// `vnoc/tourSettings`: [`act::settings`], the function of `vnoc tour settings`. A key that is given is changed first, under the project's write lock (it is
/// the project's file).
pub(super) fn tour_settings(
    core: &mut Core,
    params: Option<protocol::TourSettingsParams>,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tourSettings", async move {
        let project = open_project(&state)?;
        let params = params.unwrap_or_default();
        let choice = |text: &str| {
            Choice::parse(text)
                .ok_or_else(|| bad_params(format!("`{text}` is not one of always, ask, never")))
        };
        let mut set = Vec::new();
        for (key, text) in [(Key::Ids, &params.ids), (Key::Commit, &params.commit)] {
            if let Some(text) = text {
                set.push((key, choice(text)?));
            }
        }
        let _one_writer = if set.is_empty() {
            None
        } else {
            Some(write_lock(project).await?)
        };
        let shown = {
            let state = state.clone();
            blocking(move || {
                let project = state
                    .project
                    .get()
                    .ok_or(ActError::Environment("no folder is open".into()))?;
                act::settings(project, &set)
            })
            .await?
            .map_err(act_error)?
        };
        serde_json::to_value(shown).map_err(internal)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;
    use crate::tours::check::Source;
    use crate::tours::record::Detail;
    use std::path::PathBuf;
    use std::time::Duration;

    #[tokio::test]
    async fn a_stop_whose_driver_ask_fails_or_runs_out_of_time_is_left_out_and_the_count_goes_on() {
        let late = tokio::time::timeout(Duration::from_millis(1), std::future::pending::<()>())
            .await
            .unwrap_err();
        assert!(matches!(of_the_ask::<u8>(Err(late)), Asked::Left));
        for failed in [
            CallsError::Internal("the driver went away".into()),
            CallsError::Invalid("not a function".into()),
        ] {
            assert!(matches!(of_the_ask::<u8>(Ok(Err(failed))), Asked::Left));
        }
        assert!(matches!(
            of_the_ask::<u8>(Ok(Err(CallsError::MissingCapability("no callTree".into())))),
            Asked::NoDriver
        ));
        assert!(matches!(of_the_ask(Ok(Ok(7u8))), Asked::Calls(7)));
    }

    #[test]
    fn the_refusal_while_the_calls_are_counted_says_ask_again_in_words_and_in_data() {
        let e = being_counted();
        assert_eq!(e.code, ErrorCode::REQUEST_FAILED);
        assert!(e.message.ends_with("ask again"), "{e:?}");
        assert_eq!(e.data, Some(serde_json::json!({ "askAgain": true })));
    }

    /// A state with a folder open and no index: the first build has not happened (`Status::Building`), and a tour with no stop in it.
    fn state_with_a_tour(label: &str) -> (Arc<State>, PathBuf) {
        let dir = std::env::temp_dir().join(format!("vnoc-server-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::testdir::remove_at_end(&dir);
        std::fs::create_dir_all(dir.join(".vnoc")).unwrap();
        store::start(
            &dir,
            store::StartParams {
                title: "T".into(),
                question: None,
                author: None,
                detail: Detail::Normal,
            },
        )
        .unwrap();
        (opened(&dir), dir)
    }

    fn opened(dir: &std::path::Path) -> Arc<State> {
        let state = Arc::new(State::new());
        assert!(state.project.set(Project::open(dir).unwrap()).is_ok());
        state
    }

    fn rows_of(state: &State, dir: &std::path::Path) -> Vec<Arc<store::Row>> {
        let rows = state.tours.sync(&dir.join(".vnoc")).0;
        assert_eq!(rows.len(), 1);
        rows
    }

    #[tokio::test]
    async fn a_tour_whose_record_is_valid_is_answered_while_an_update_holds_the_lock_and_the_first_build_is_not_done()
     {
        let (state, dir) = state_with_a_tour("no-wait");
        // the first ask records the tour (it has no written stop: nothing to check)
        let rows = rows_of(&state, &dir);
        let first = answers_for(&state, rows).await.unwrap();
        assert_eq!(first.values().next().unwrap().from, Source::Check);

        // an update is running (it holds `one_at_a_time`) and there is no index: the memory answers
        let running = state.one_at_a_time.lock().await;
        let rows = rows_of(&state, &dir);
        let asked = tokio::time::timeout(Duration::from_secs(5), answers_for(&state, rows))
            .await
            .expect("the answer did not wait for the update");
        assert_eq!(asked.unwrap().values().next().unwrap().from, Source::Memory);

        // a core that has just started has no memory, but the record on disk is valid: the record answers
        let fresh = opened(&dir);
        let rows = rows_of(&fresh, &dir);
        let asked = tokio::time::timeout(Duration::from_secs(5), answers_for(&fresh, rows))
            .await
            .expect("the answer did not wait for the update");
        assert_eq!(asked.unwrap().values().next().unwrap().from, Source::Record);
        drop(running);
    }
}
