//! Session YAML representations used while extracting, decoding, and sanitizing
//! iRacing session information.

use std::{borrow::Cow, ops::Deref};

use crate::{IRacingSDKError, Result};

fn trim_ascii(mut bytes: &[u8]) -> &[u8] {
    while bytes.first().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[1..];
    }

    while bytes.last().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[..bytes.len() - 1];
    }

    bytes
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionYamlEncoding {
    Utf8,
    Iso8859_1,
}

impl SessionYamlEncoding {
    fn from_buffer(buffer: &[u8]) -> Self {
        let mut in_weekend_info = false;

        for line in buffer.split(|&byte| byte == b'\n') {
            let trimmed = trim_ascii(line);

            if !in_weekend_info {
                in_weekend_info = trimmed == b"WeekendInfo:";
                continue;
            }

            // A new top-level key ends WeekendInfo.
            if !line.first().is_some_and(u8::is_ascii_whitespace) {
                break;
            }

            if let Some(value) = trimmed.strip_prefix(b"Encoding:") {
                return if trim_ascii(value) == b"UTF8" {
                    Self::Utf8
                } else {
                    Self::Iso8859_1
                };
            }
        }

        Self::Iso8859_1
    }

    fn decode(&self, buffer: Cow<'_, [u8]>) -> Result<String> {
        match self {
            Self::Utf8 => match buffer {
                Cow::Borrowed(bytes) => std::str::from_utf8(bytes)
                    .map(str::to_owned)
                    .map_err(IRacingSDKError::invalid_session_yaml_utf8),
                Cow::Owned(bytes) => String::from_utf8(bytes).map_err(|error| {
                    IRacingSDKError::invalid_session_yaml_utf8(error.utf8_error())
                }),
            },

            Self::Iso8859_1 => Ok(buffer.iter().map(|&byte| char::from(byte)).collect()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Decoded session YAML with unsupported control characters removed.
///
/// This representation is suitable for passing to
/// [`SessionInfo::parse_sanitized`](super::SessionInfo::parse_sanitized).
pub struct SanitizedSessionYaml(String);

impl SanitizedSessionYaml {
    pub(crate) fn new(yaml: String) -> Self {
        Self(yaml)
    }

    /// Returns the sanitized YAML as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes this value and returns the sanitized YAML string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for SanitizedSessionYaml {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for SanitizedSessionYaml {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) struct DecodedSessionYaml(String);

impl DecodedSessionYaml {
    pub(crate) fn new(yaml: String) -> Self {
        Self(yaml)
    }

    /// Filters control characters from the decoded session yaml.
    pub(crate) fn sanitize(self) -> SanitizedSessionYaml {
        SanitizedSessionYaml(
            self.0
                .chars()
                .filter(|&ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t'))
                .collect(),
        )
    }
}

impl Deref for DecodedSessionYaml {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub(crate) struct SessionYamlBytes<'a> {
    bytes: Cow<'a, [u8]>,
    encoding: SessionYamlEncoding,
}

impl<'a> SessionYamlBytes<'a> {
    pub(crate) fn from_region(buffer: &'a [u8], offset: i32, length: i32) -> Result<Option<Self>> {
        if length == 0 {
            return Ok(None);
        }

        let start = usize::try_from(offset).map_err(|_| {
            IRacingSDKError::invalid_session_yaml_region(offset, length, buffer.len())
        })?;

        let slice_length = usize::try_from(length).map_err(|_| {
            IRacingSDKError::invalid_session_yaml_region(offset, length, buffer.len())
        })?;

        let end =
            start
                .checked_add(slice_length)
                .ok_or(IRacingSDKError::invalid_session_yaml_region(
                    offset,
                    length,
                    buffer.len(),
                ))?;

        let region = buffer
            .get(start..end)
            .ok_or(IRacingSDKError::invalid_session_yaml_region(
                offset,
                length,
                buffer.len(),
            ))?;

        Self::from_cow(Cow::Borrowed(region))
    }

    pub(crate) fn from_slice(buffer: &'a [u8]) -> Result<Option<Self>> {
        Self::from_cow(Cow::Borrowed(buffer))
    }

    pub(crate) fn from_owned(buffer: Vec<u8>) -> Result<Option<Self>> {
        Self::from_cow(Cow::Owned(buffer))
    }

    fn from_cow(mut buffer: Cow<'a, [u8]>) -> Result<Option<Self>> {
        // Nul termination check
        let end = buffer
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(buffer.len());

        let bytes = &buffer[..end];
        if bytes.is_empty() {
            return Ok(None);
        }

        match &mut buffer {
            Cow::Borrowed(bytes) => {
                *bytes = &bytes[..end];
            }
            Cow::Owned(bytes) => {
                bytes.truncate(end);
            }
        }

        let encoding = SessionYamlEncoding::from_buffer(buffer.as_ref());

        Ok(Some(Self {
            bytes: buffer,
            encoding,
        }))
    }

    pub(crate) fn decode(self) -> Result<DecodedSessionYaml> {
        let decoded = self.encoding.decode(self.bytes)?;
        Ok(DecodedSessionYaml(decoded))
    }
}

pub(crate) trait SessionYamlSource {
    fn session_yaml_bytes(&self) -> Result<Option<SessionYamlBytes<'_>>>;
}

#[cfg(test)]
mod tests {
    use super::SessionYamlEncoding;

    #[test]
    fn detects_utf8_in_weekend_info() {
        let yaml = b"WeekendInfo:\r\n  TrackName: test\r\n  Encoding: UTF8\r\nSessionInfo:\r\n";

        assert_eq!(
            SessionYamlEncoding::from_buffer(yaml),
            SessionYamlEncoding::Utf8
        );
    }

    #[test]
    fn ignores_encoding_outside_weekend_info() {
        let yaml =
            b"Encoding: UTF8\nWeekendInfo:\n  TrackName: test\nSessionInfo:\n  Encoding: UTF8\n";

        assert_eq!(
            SessionYamlEncoding::from_buffer(yaml),
            SessionYamlEncoding::Iso8859_1
        );
    }

    #[test]
    fn ignores_nested_encoding_in_weekend_info() {
        let yaml =
            b"WeekendInfo:\n  Nested:\n    Encoding: UTF8\n  TrackName: test\nSessionInfo:\n";

        assert_eq!(
            SessionYamlEncoding::from_buffer(yaml),
            SessionYamlEncoding::Iso8859_1
        );
    }

    #[test]
    fn ignores_nested_weekend_info_section() {
        let yaml = b"Metadata:\n  WeekendInfo:\n    Encoding: UTF8\n";

        assert_eq!(
            SessionYamlEncoding::from_buffer(yaml),
            SessionYamlEncoding::Iso8859_1
        );
    }

    #[test]
    fn defaults_unknown_weekend_encoding_to_iso_8859_1() {
        let yaml = b"WeekendInfo:\n  Encoding: WINDOWS_1252\nSessionInfo:\n";

        assert_eq!(
            SessionYamlEncoding::from_buffer(yaml),
            SessionYamlEncoding::Iso8859_1
        );
    }

    #[test]
    fn recognizes_weekend_info_after_utf8_bom() {
        let yaml = b"\xEF\xBB\xBFWeekendInfo:\n  Encoding: UTF8\n";

        assert_eq!(
            SessionYamlEncoding::from_buffer(yaml),
            SessionYamlEncoding::Utf8
        );
    }
}
