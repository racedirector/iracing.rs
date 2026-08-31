use super::IRSDK_VERSION;
use crate::IRacingSDKError;

pub(super) fn header_validation_error(details: impl Into<String>) -> IRacingSDKError {
    IRacingSDKError::parse_error("Header validation", details)
}

pub(super) fn mismatched_version_error(actual: u32) -> IRacingSDKError {
    IRacingSDKError::Version {
        expected: IRSDK_VERSION as u32,
        found: actual,
    }
}

pub(super) fn variable_header_validation_error(details: impl Into<String>) -> IRacingSDKError {
    IRacingSDKError::parse_error("Variable header validation", details)
}
