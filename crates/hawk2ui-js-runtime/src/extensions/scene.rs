use std::cell::RefCell;
use std::rc::Rc;

use deno_core::{Extension, OpState, op2};
use deno_error::JsErrorBox;

use crate::SceneOpBatch;

/// Shared log of validated scene batches committed by JavaScript.
#[derive(Clone, Debug, Default)]
pub(crate) struct SceneCommitLog {
    batches: Rc<RefCell<Vec<SceneOpBatch>>>,
}

impl SceneCommitLog {
    /// Records a validated scene batch.
    pub(crate) fn push(&self, batch: SceneOpBatch) {
        self.batches.borrow_mut().push(batch);
    }

    /// Returns all committed batches without clearing them.
    pub(crate) fn snapshot(&self) -> Vec<SceneOpBatch> {
        self.batches.borrow().clone()
    }
}

deno_core::extension!(
    hawk_scene,
    ops = [op_hawk_scene_commit],
    options = {
        commit_log: SceneCommitLog,
    },
    state = |state, options| state.put(options.commit_log)
);

/// Creates the scene bridge extension for one runtime instance.
pub(crate) fn extension(commit_log: SceneCommitLog) -> Extension {
    hawk_scene::init(commit_log)
}

#[op2]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_scene_commit(
    state: Rc<RefCell<OpState>>,
    #[serde] batch: SceneOpBatch,
) -> Result<(), JsErrorBox> {
    batch
        .validate()
        .map_err(|error| JsErrorBox::generic(error.to_string()))?;

    let state = state.borrow();
    state.borrow::<SceneCommitLog>().push(batch);
    Ok(())
}
