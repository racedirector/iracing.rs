use tonic::Status;

use crate::broadcast_app::BroadcastError;

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

impl From<BroadcastError> for Status {
    fn from(error: BroadcastError) -> Self {
        let message = error.to_string();

        match error {
            BroadcastError::ObservationTimeout => Status::deadline_exceeded(message),
            BroadcastError::ObservationSourceEnded => Status::unavailable(message),
            BroadcastError::FailedPrecondition(_)
            | BroadcastError::ObservationDisabled
            | BroadcastError::CapabilityUnavailable(_) => Status::failed_precondition(message),
            BroadcastError::Sdk(error) => broadcast_error_to_status(error),
        }
    }
}
