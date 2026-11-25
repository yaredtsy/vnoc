use async_lsp::{ErrorCode, ResponseError};

use super::{Core, internal, ready_project};
use crate::calls::{self, CallsError};
use crate::protocol::{CallTreeParams, CallTreeResult, HasCallsParams, HasCallsResult};
use crate::watchdog::timed;

/// `vnoc/callTree`: the driver's tree for the node `key`, merged, with each target given the key the index knows it by.
/// A request the editor cancels is dropped by the lifecycle layer, which drops the driver request under it, and that cancels the driver's work.
/// Every full tree is asked for with a depth cap (`calls::DEPTH_CAP`): a node at it is `cut`, and opening it asks for its own tree.
pub(super) fn call_tree(
    core: &mut Core,
    params: CallTreeParams,
) -> impl std::future::Future<Output = Result<CallTreeResult, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("callTree", async move {
        let project = ready_project(&core.state).await?;
        calls::tree_for(
            project,
            &core.state.index,
            &core.host,
            &params.key,
            Some(calls::DEPTH_CAP),
        )
        .await
        .map_err(calls_error)
    })
}

/// `vnoc/hasCalls`: how many nodes opening `key` would show. The driver is asked for one level, and the answer is merged exactly as `vnoc/callTree`
/// merges its own; only the count leaves the core.
pub(super) fn has_calls(
    core: &mut Core,
    params: HasCallsParams,
) -> impl std::future::Future<Output = Result<HasCallsResult, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("hasCalls", async move {
        let project = ready_project(&core.state).await?;
        let answer = calls::ask(project, &core.state.index, &core.host, &params.key, Some(1))
            .await
            .map_err(calls_error)?;
        let revision = core.state.index.read().unwrap().revision();
        Ok(HasCallsResult {
            revision,
            count: calls::count_calls(&answer.root),
        })
    })
}

/// A call-tree failure as the response code it has always had.
pub(super) fn calls_error(e: CallsError) -> ResponseError {
    match e {
        CallsError::Invalid(message) => ResponseError::new(ErrorCode::INVALID_PARAMS, message),
        CallsError::MissingCapability(message) => {
            ResponseError::new(ErrorCode::REQUEST_FAILED, message)
        }
        CallsError::Internal(message) => internal(message),
    }
}
