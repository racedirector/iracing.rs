use std::time::Duration;

use gloo_timers::future::TimeoutFuture;

/// Timer implementation for `wasm32` targets.
pub struct WasmTimer;

impl super::Timer for WasmTimer {
    async fn sleep(duration: Duration) {
        TimeoutFuture::new(super::duration_to_timeout_ms(duration)).await;
    }
}
