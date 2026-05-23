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

#[cfg(test)]
mod tests {
    use tonic::Code;

    use super::*;

    #[test]
    fn broadcast_domain_errors_map_to_transport_statuses() {
        assert_eq!(
            Status::from(BroadcastError::ObservationTimeout).code(),
            Code::DeadlineExceeded
        );
        assert_eq!(
            Status::from(BroadcastError::ObservationSourceEnded).code(),
            Code::Unavailable
        );
        assert_eq!(
            Status::from(BroadcastError::ObservationDisabled).code(),
            Code::FailedPrecondition
        );
        assert_eq!(
            Status::from(BroadcastError::CapabilityUnavailable("camera")).code(),
            Code::FailedPrecondition
        );
        assert_eq!(
            Status::from(BroadcastError::FailedPrecondition(
                "missing car".to_string()
            ))
            .code(),
            Code::FailedPrecondition
        );
    }

    #[test]
    fn sdk_errors_map_to_transport_statuses() {
        assert_eq!(
            broadcast_error_to_status(IRacingSDKError::connection_failed("sim closed")).code(),
            Code::Unavailable
        );
        assert_eq!(
            broadcast_error_to_status(IRacingSDKError::unsupported_platform(
                "live broadcast",
                "Windows",
            ))
            .code(),
            Code::FailedPrecondition
        );
        assert_eq!(
            broadcast_error_to_status(IRacingSDKError::buffer_operation_error(
                "stale buffer",
                Some(0),
            ))
            .code(),
            Code::Unavailable
        );
        assert_eq!(
            broadcast_error_to_status(IRacingSDKError::Parse {
                context: "broadcast response".to_string(),
                details: "bad payload".to_string(),
            })
            .code(),
            Code::Internal
        );
    }
}
