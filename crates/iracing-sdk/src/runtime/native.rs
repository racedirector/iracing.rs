//! Native runtime implementations backed by Tokio.

use std::time::Duration;

/// Timer implementation backed by `tokio::time::sleep`.
pub struct TokioTimer;

impl super::Timer for TokioTimer {
    async fn sleep(duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

#[cfg(windows)]
/// Windows wait runtime backed by `tokio::task::spawn_blocking`.
pub struct TokioWaitRuntime;

#[cfg(windows)]
impl super::WaitRuntime for TokioWaitRuntime {
    async fn wait_for_update(
        connection: &crate::windows::Connection,
        timeout: Duration,
    ) -> crate::Result<crate::WaitResult> {
        let event_handle = connection.event_handle_raw();

        tokio::task::spawn_blocking(move || {
            crate::windows::Connection::wait_for_event_handle(event_handle, timeout)
        })
        .await
        .map_err(|error| {
            crate::IRacingSDKError::buffer_operation_error(
                format!("Event wait task panicked: {error}"),
                None,
            )
        })?
    }
}
