use crate::ProtocolError;

pub(super) fn header_validation_error(details: impl Into<String>) -> ProtocolError {
    ProtocolError::parse_error("Header validation", details)
}

pub(super) fn variable_header_validation_error(details: impl Into<String>) -> ProtocolError {
    ProtocolError::parse_error("Variable header validation", details)
}
