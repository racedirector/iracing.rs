use super::session_info_buffer::SessionInfoBuffer;
use crate::{IRacingSDKError, Result};
use std::ops::Deref;

/// Represents a sanitized session string from the iRacing SDK.
pub(crate) struct IRacingSessionString(String);

impl AsRef<str> for IRacingSessionString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for IRacingSessionString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<&str> for IRacingSessionString {
    type Error = IRacingSDKError;

    fn try_from(value: &str) -> Result<Self> {
        Self::try_from(value.to_owned())
    }
}

impl TryFrom<String> for IRacingSessionString {
    type Error = IRacingSDKError;

    fn try_from(value: String) -> Result<Self> {
        // Filter out invalid control characters (\n, \r, \t are valid)
        let yaml: String = value
            .chars()
            .filter(|ch| !matches!(ch, '\x00'..='\x08' | '\x0B'..='\x0C' | '\x0E'..='\x1F'))
            .collect();

        // Reject empty after cleaning
        if yaml.trim().is_empty() {
            return Err(IRacingSDKError::Parse {
                context: "YAML preprocessing".into(),
                details: "YAML is empty after preprocessing".into(),
            });
        }

        Ok(IRacingSessionString(yaml))
    }
}

impl TryFrom<SessionInfoBuffer> for IRacingSessionString {
    type Error = IRacingSDKError;

    fn try_from(value: SessionInfoBuffer) -> Result<Self> {
        let decoded: String = value.into();
        decoded.try_into()
    }
}

impl From<IRacingSessionString> for String {
    fn from(value: IRacingSessionString) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iracing_session_string_control_characters_removed() {
        let result: IRacingSessionString = "WeekendInfo:\n\x00\x01\x02  TrackName: test\x03"
            .to_string()
            .try_into()
            .unwrap();

        // Assert that invalid characters were removed and that the correct values remain.
        assert!(!result.contains('\x00'));
        assert!(!result.contains('\x01'));
        assert!(!result.contains('\x02'));
        assert!(!result.contains('\x03'));
        assert!(result.contains("WeekendInfo"));
        assert!(result.contains("TrackName"));
    }

    #[test]
    fn test_iracing_session_string_keeps_valid_whitespace() {
        let result: IRacingSessionString = "Key:\n\r\t  Value".to_string().try_into().unwrap();

        assert!(result.contains('\n'));
        assert!(result.contains('\r'));
        assert!(result.contains('\t'));
    }
}
