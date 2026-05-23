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
