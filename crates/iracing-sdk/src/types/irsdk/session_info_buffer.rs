use crate::parse_utils::nul_terminated_bytes;

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
    /// Wraps bytes after a reader has copied an advertised region in full.
    ///
    /// Construction is crate-private so source readers remain responsible for
    /// bounds checking and exact-read semantics.
    #[cfg(test)]
    pub(crate) fn from_snapshot(bytes: Vec<u8>) -> Self {
        // ???: Should we do the nul check here?
        Self { bytes }
    }
}

impl From<SessionInfoBuffer> for String {
    /// Decodes the snapshot up to its first NUL terminator.
    ///
    /// Valid UTF-8 is preserved. Invalid UTF-8 falls back to a byte-for-byte
    /// single-byte character mapping so later iRacing YAML cleanup retains the
    /// original byte values instead of replacing them.
    fn from(buffer: SessionInfoBuffer) -> Self {
        use std::str::from_utf8;

        // Slice the buffer to nul-termination.
        let yaml_candidate = nul_terminated_bytes(&buffer.bytes);

        // Decode the buffer, trying UTF-8 first. If it fails, fallback to ISO-8859-1.
        if let Ok(s) = from_utf8(yaml_candidate) {
            tracing::trace!("Decoded buffer as UTF-8");
            s.to_owned()
        } else {
            tracing::trace!("Decoded buffer as ISO-8859-1");
            yaml_candidate.iter().map(|&b| b as char).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
