//! Timer primitives for the embedded JavaScript runtime.

use std::thread;
use std::time::Duration;

use deno_core::futures::channel::oneshot;
use deno_core::{Extension, op2};
use deno_error::JsErrorBox;

deno_core::extension!(hawk_timers, ops = [op_hawk_timer_delay],);

/// Creates the timer extension for one runtime instance.
pub(crate) fn extension() -> Extension {
    hawk_timers::init()
}

#[op2]
async fn op_hawk_timer_delay(#[smi] delay_ms: u32) -> Result<(), JsErrorBox> {
    let duration = Duration::from_millis(u64::from(delay_ms));
    let (sender, receiver) = oneshot::channel();
    thread::Builder::new()
        .name("hawk2ui-js-timer".to_owned())
        .spawn(move || {
            thread::sleep(duration);
            let _ = sender.send(());
        })
        .map_err(|error| {
            JsErrorBox::generic(format!(
                "js-runtime.timer.spawn-failed: native timer thread failed: {error}"
            ))
        })?;

    receiver
        .await
        .map_err(|_| JsErrorBox::generic("js-runtime.timer.cancelled: native timer was cancelled"))
}
