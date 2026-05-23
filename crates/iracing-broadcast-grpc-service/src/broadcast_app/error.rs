use iracing_sdk::IRacingSDKError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum BroadcastError {
    #[error("broadcast service observation support is disabled")]
    ObservationDisabled,

    #[error("{0} telemetry is unavailable")]
    CapabilityUnavailable(&'static str),

    #[error("telemetry observation timed out")]
    ObservationTimeout,

    #[error("telemetry source ended before the requested state change")]
    ObservationSourceEnded,

    #[error("failed precondition: {0}")]
    FailedPrecondition(String),

    #[error(transparent)]
    Sdk(#[from] IRacingSDKError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_descriptive() {
        assert_eq!(
            BroadcastError::ObservationDisabled.to_string(),
            "broadcast service observation support is disabled"
        );
        assert_eq!(
            BroadcastError::CapabilityUnavailable("camera selection").to_string(),
            "camera selection telemetry is unavailable"
        );
        assert_eq!(
            BroadcastError::ObservationTimeout.to_string(),
            "telemetry observation timed out"
        );
        assert_eq!(
            BroadcastError::ObservationSourceEnded.to_string(),
            "telemetry source ended before the requested state change"
        );
        assert_eq!(
            BroadcastError::FailedPrecondition("no car found".to_string()).to_string(),
            "failed precondition: no car found"
        );
    }

    #[test]
    fn sdk_error_is_transparent() {
        let sdk_error = IRacingSDKError::connection_failed("sim not running");
        let broadcast_error = BroadcastError::Sdk(sdk_error);
        let message = broadcast_error.to_string();
        assert!(
            message.contains("sim not running"),
            "transparent SDK error should include original message: {message}"
        );
    }

    #[test]
    fn sdk_error_converts_from_iracing_sdk_error() {
        let sdk_error = IRacingSDKError::connection_failed("test");
        let broadcast_error: BroadcastError = sdk_error.into();
        assert!(matches!(broadcast_error, BroadcastError::Sdk(_)));
    }

    #[test]
    fn failed_precondition_preserves_message() {
        let msg = "car index 999 is out of range";
        let error = BroadcastError::FailedPrecondition(msg.to_string());
        assert!(error.to_string().contains(msg));
    }

    #[test]
    fn capability_unavailable_preserves_capability_name() {
        for capability in &[
            "camera selection",
            "replay speed",
            "pit service",
            "force feedback",
        ] {
            let error = BroadcastError::CapabilityUnavailable(capability);
            assert!(
                error.to_string().contains(capability),
                "error message should contain capability name: {}",
                error
            );
        }
    }
}
