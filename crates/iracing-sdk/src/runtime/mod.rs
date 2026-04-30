//! Runtime abstractions for provider scheduling and blocking waits.

use std::{future::Future, time::Duration};

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use native::TokioTimer;
#[cfg(all(windows, not(target_arch = "wasm32")))]
pub use native::TokioWaitRuntime;
#[cfg(target_arch = "wasm32")]
pub use wasm::WasmTimer;

#[cfg(not(target_arch = "wasm32"))]
/// Default timer implementation for the current target.
pub type DefaultTimer = TokioTimer;
#[cfg(target_arch = "wasm32")]
/// Default timer implementation for the current target.
pub type DefaultTimer = WasmTimer;

#[cfg(all(windows, not(target_arch = "wasm32")))]
/// Default wait runtime for Windows live telemetry.
pub type DefaultWaitRuntime = TokioWaitRuntime;

/// Portable async sleep primitive for provider pacing and polling.
pub trait Timer {
    /// Sleep for the provided duration.
    fn sleep(duration: Duration) -> impl Future<Output = ()>;
}

#[cfg(all(windows, not(target_arch = "wasm32")))]
/// Runtime hook for offloading Windows event waits from async providers.
pub trait WaitRuntime {
    /// Wait for the next live telemetry signal without coupling providers to a runtime.
    fn wait_for_update(
        connection: &crate::windows::Connection,
        timeout: Duration,
    ) -> impl Future<Output = crate::Result<crate::WaitResult>>;
}

pub(crate) fn duration_to_timeout_ms(duration: Duration) -> u32 {
    duration.as_millis().min(u32::MAX as u128) as u32
}

#[cfg(test)]
mod tests {
    use super::duration_to_timeout_ms;
    use std::time::Duration;

    #[test]
    fn duration_to_timeout_ms_clamps_to_u32_max() {
        assert_eq!(duration_to_timeout_ms(Duration::MAX), u32::MAX);
    }

    #[test]
    fn duration_to_timeout_ms_preserves_small_values() {
        assert_eq!(duration_to_timeout_ms(Duration::from_millis(16)), 16);
        assert_eq!(duration_to_timeout_ms(Duration::from_micros(500)), 0);
    }
}
