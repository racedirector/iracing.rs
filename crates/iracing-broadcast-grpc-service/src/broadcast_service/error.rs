use tonic::Status;

use iracing_sdk::IRacingSDKError;

pub(crate) fn broadcast_error_to_status(error: IRacingSDKError) -> Status {
    let message = error.to_string();
    let retryable = error.is_retryable();

    tracing::warn!(
        error = %error,
        retryable,
        "iRacing broadcast operation failed"
    );

    match &error {
        IRacingSDKError::Connection { .. } => Status::unavailable(message),
        IRacingSDKError::UnsupportedPlatform { .. } => Status::failed_precondition(message),
        #[cfg(windows)]
        IRacingSDKError::WindowsApi { .. } => Status::unavailable(message),
        IRacingSDKError::Buffer { .. } if retryable => Status::unavailable(message),
        _ if retryable => Status::unavailable(message),
        _ => Status::internal(message),
    }
}
