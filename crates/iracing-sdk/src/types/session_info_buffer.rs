use crate::parse_utils::nul_terminated_bytes;

/// Borrowed session bytes before the first NUL, without any UTF-8 or YAML validity guarantee.
///
/// Obtain this view through [`SessionInfoBuffer::payload`].
#[derive(Debug, Clone, Copy)]
pub struct SessionInfoPayload<'a> {
    bytes: &'a [u8],
}

/// Encoding declared by the session-information header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionInfoEncoding {
    /// UTF-8 text.
    Utf8,
    /// ISO-8859-1 single-byte text.
    Iso8859_1,
    /// The declaration is absent or its value is not recognized.
    Unknown,
}

impl SessionInfoPayload<'_> {
    /// Inspects `WeekendInfo.Encoding` without decoding or modifying the snapshot.
    ///
    /// Searches the indented lines under `WeekendInfo:` for `Encoding:`. Assumes
    /// the SDK header shape, where this key is not reused in nested objects.
    /// Allows document markers and LF/CRLF endings; ignores bytes after the first NUL.
    pub fn encoding(&self) -> SessionInfoEncoding {
        let bytes = self.bytes;
        let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
        let mut lines = bytes
            .split(|&byte| byte == b'\n')
            .filter(|line| !line.trim_ascii().is_empty() && !line.trim_ascii().starts_with(b"#"));

        if lines
            .find(|line| line.trim_ascii_end() == b"WeekendInfo:")
            .is_none()
        {
            return SessionInfoEncoding::Unknown;
        }

        let encoding = lines
            .take_while(|line| line.starts_with(b" "))
            .find_map(|line| line.trim_ascii().strip_prefix(b"Encoding:"))
            .map(|value| value.trim_ascii());

        match encoding {
            Some(b"UTF8" | b"UTF-8") => SessionInfoEncoding::Utf8,
            Some(b"ISO_8859_1") => SessionInfoEncoding::Iso8859_1,
            _ => SessionInfoEncoding::Unknown,
        }
    }

    /// Decodes the payload according to its declared encoding.
    ///
    /// Declared UTF-8 replaces malformed sequences. An absent or unknown
    /// declaration retains UTF-8-first decoding with an ISO-8859-1 fallback.
    pub fn decode(&self) -> String {
        match self.encoding() {
            SessionInfoEncoding::Utf8 => String::from_utf8_lossy(self.bytes).into_owned(),
            SessionInfoEncoding::Iso8859_1 => self.bytes.iter().map(|&b| char::from(b)).collect(),
            SessionInfoEncoding::Unknown => match std::str::from_utf8(self.bytes) {
                Ok(text) => text.to_owned(),
                Err(_) => self.bytes.iter().map(|&b| char::from(b)).collect(),
            },
        }
    }
}

/// Exact, owned bytes copied from an SDK session-information region.
///
/// The snapshot is source-neutral: live acquisition copies the current mapped
/// region, while IBT acquisition copies the recording's immutable region. The
/// type records ownership of a complete advertised region but does not claim
/// that its contents are valid YAML or correspond atomically to another
/// independently acquired snapshot.
#[derive(Debug, Clone)]
pub struct SessionInfoBuffer {
    /// Complete bytes copied from the advertised session-information region.
    bytes: Vec<u8>,
}

impl SessionInfoBuffer {
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Borrows the session payload, excluding the first NUL and any padding.
    pub fn payload(&self) -> SessionInfoPayload<'_> {
        SessionInfoPayload {
            bytes: nul_terminated_bytes(self.as_bytes()),
        }
    }

    pub(crate) fn from_checked_region(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.to_vec(),
        }
    }

    /// Wraps bytes after a reader has copied an advertised region in full.
    ///
    /// Construction is crate-private so source readers remain responsible for
    /// bounds checking and exact-read semantics.
    #[cfg(test)]
    pub(crate) fn from_snapshot(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl From<SessionInfoBuffer> for String {
    /// Decodes the NUL-bounded payload using [`SessionInfoPayload::decode`].
    fn from(buffer: SessionInfoBuffer) -> Self {
        buffer.payload().decode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_borrows_only_text_and_preserves_region() {
        let bytes = b"WeekendInfo:\n Encoding: UTF8\n\0padding";
        let buffer = SessionInfoBuffer::from_checked_region(bytes);
        let payload = buffer.payload();
        assert_eq!(payload.bytes, &bytes[..bytes.len() - 8]);
        assert_eq!(payload.bytes.as_ptr(), buffer.as_bytes().as_ptr());
        assert_eq!(buffer.as_bytes(), bytes);
        assert_eq!(payload.encoding(), SessionInfoEncoding::Utf8);
    }

    #[test]
    fn declared_encoding_controls_decoding() {
        let utf8 = SessionInfoBuffer::from_checked_region(
            b"WeekendInfo:\n Encoding: UTF8\n Name: \xc3\xa9\xff",
        );
        assert!(utf8.payload().decode().ends_with("\u{e9}\u{fffd}"));
        let latin1 = SessionInfoBuffer::from_checked_region(
            b"WeekendInfo:\n Encoding: ISO_8859_1\n Name: \xc3\xa9",
        );
        assert!(String::from(latin1).ends_with("\u{c3}\u{a9}"));
    }

    #[test]
    fn decoding_stops_before_invalid_bytes_after_nul() {
        let buffer = SessionInfoBuffer::from_checked_region(b"\xc3\xa9\0\xff");
        assert_eq!(String::from(buffer), "\u{e9}");
    }

    #[test]
    fn iso_8859_1_fallback_preserves_single_byte_codepoints() {
        let buffer = SessionInfoBuffer::from_checked_region(&[0x80, 0x93, 0x96, 0xe9]);
        assert_eq!(String::from(buffer), "\u{80}\u{93}\u{96}\u{e9}");
    }

    #[test]
    fn decoding_does_not_sanitize_or_reject_empty_text() {
        for bytes in [b"".as_slice(), b"\0padding", b"\x01 \n"] {
            let buffer = SessionInfoBuffer::from_checked_region(bytes);
            let expected = if bytes.starts_with(b"\0") {
                ""
            } else {
                std::str::from_utf8(bytes).unwrap()
            };
            assert_eq!(String::from(buffer), expected);
        }
    }

    #[test]
    fn test_session_info_buffer_with_null_terminator() {
        let bytes = b"SessionInfo:\n  TrackName: test\0padding".to_vec();
        let buffer = SessionInfoBuffer::from_snapshot(bytes);

        let result: String = buffer.into();
        assert_eq!(result, "SessionInfo:\n  TrackName: test");
    }

    #[test]
    fn test_session_info_buffer_without_null_terminator() {
        let bytes = b"SessionInfo:\n  TrackName: test".to_vec();
        let buffer = SessionInfoBuffer::from_snapshot(bytes);

        let result: String = buffer.into();
        assert_eq!(result, "SessionInfo:\n  TrackName: test");
    }

    #[test]
    fn test_decode_yaml_from_utf8_with_special_characters() {
        let input = "DriverInfo:\n  UserName: \"José 🚗\"\n  CarScreenName: \"Mazda MX-5 – Cup\"";
        let bytes = input.as_bytes().to_vec();
        let buffer = SessionInfoBuffer::from_snapshot(bytes);
        let result: String = buffer.into();

        assert_eq!(result, input);
    }

    #[test]
    fn test_decode_yaml_from_iso_8859_1() {
        let bytes = [
            b'D', b'r', b'i', b'v', b'e', b'r', b'I', b'n', b'f', b'o', b':', b'\n', b' ', b' ',
            b'U', b's', b'e', b'r', b'N', b'a', b'm', b'e', b':', b' ', b'"', b'J', b'o', b's',
            0xE9, b'"', b'\n', b' ', b' ', b'C', b'a', b'r', b'S', b'c', b'r', b'e', b'e', b'n',
            b'N', b'a', b'm', b'e', b':', b' ', b'M', b'a', b'z', b'd', b'a', b' ', b'M', b'X',
            b'-', b'5', b' ', b'-', b' ', b'C', b'u', b'p',
        ]
        .to_vec();

        let buffer = SessionInfoBuffer::from_snapshot(bytes);
        let result: String = buffer.into();

        assert_eq!(
            result,
            "DriverInfo:\n  UserName: \"José\"\n  CarScreenName: Mazda MX-5 - Cup"
        )
    }
}

#[cfg(test)]
mod encoding_tests {
    use crate::{SessionInfoBuffer, SessionInfoEncoding};

    #[test]
    fn detects_captured_headers() {
        for (bytes, expected) in [
            (
                include_bytes!("../../../../test-data/session-yaml/utf-8-snapshot.yml").as_slice(),
                SessionInfoEncoding::Utf8,
            ),
            (
                include_bytes!("../../../../test-data/session-yaml/iso-8859-1-snapshot.yml")
                    .as_slice(),
                SessionInfoEncoding::Iso8859_1,
            ),
        ] {
            assert_eq!(
                SessionInfoBuffer::from_checked_region(bytes)
                    .payload()
                    .encoding(),
                expected
            );
        }
    }

    #[test]
    fn detects_header_without_decoding_payload() {
        for bytes in [
            b"---\nWeekendInfo:\n Encoding: UTF8\n Name: \xff".as_slice(),
            b"\xef\xbb\xbf---\r\nWeekendInfo:\r\n  Encoding: UTF-8 \r\n",
            b"---\nWeekendInfo:\n  TrackName: test\n\n# comment\n  Encoding:   UTF8\n",
        ] {
            assert_eq!(
                SessionInfoBuffer::from_checked_region(bytes)
                    .payload()
                    .encoding(),
                SessionInfoEncoding::Utf8
            );
        }
    }

    #[test]
    fn ignores_missing_unknown_and_out_of_scope_declarations() {
        for bytes in [
            b"".as_slice(),
            b"WeekendInfo:\n Encoding: OTHER\n",
            b"WeekendInfo:\n TrackName: test\nDriverInfo:\n Encoding: UTF8\n",
            b"DriverInfo:\n Encoding: UTF8\n",
            b"WeekendInfo:\n\0 Encoding: UTF8\n",
        ] {
            assert_eq!(
                SessionInfoBuffer::from_checked_region(bytes)
                    .payload()
                    .encoding(),
                SessionInfoEncoding::Unknown
            );
        }
    }
}
