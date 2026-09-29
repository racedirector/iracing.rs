use std::io::Read;
use zerocopy::{FromBytes, TryFromBytes};

use crate::Result;

pub(crate) fn read_wire_bytes<T: FromBytes>(bytes: &[u8]) -> Result<T> {
    T::read_from_bytes(bytes).map_err(|error| error.into())
}

pub(crate) fn try_from_wire_bytes<T: TryFromBytes>(bytes: &[u8]) -> Result<T> {
    T::try_read_from_bytes(bytes).map_err(|error| error.into())
}

pub(crate) fn read_wire_bytes_from_io<T: FromBytes, R: Read>(reader: &mut R) -> Result<T> {
    T::read_from_io(reader).map_err(|error| error.into())
}

/// Helpers for decoding fixed-width string fields from SDK wire bytes.
pub mod decode {
    use std::borrow::Cow;

    /// Decodes bytes up to the first NUL, replacing invalid UTF-8 if needed.
    pub fn fixed_string(bytes: &[u8]) -> Cow<'_, str> {
        let end = bytes
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end])
    }
}

/// Helpers for encoding NUL-terminated ASCII strings into fixed-width SDK fields.
pub mod encode {
    use crate::{Error, Result};

    /// Encodes an ASCII string into a zero-padded field of `N` bytes.
    ///
    /// Returns an error if the value is not ASCII, contains a NUL, or cannot fit
    /// with a NUL terminator. `field` identifies the invalid configuration value.
    pub fn fixed_string<const N: usize>(field: &'static str, value: &str) -> Result<[u8; N]> {
        if !value.is_ascii() {
            return Err(Error::invalid_configuration(
                field,
                "must contain ASCII only",
            ));
        }
        if value.as_bytes().contains(&0) {
            return Err(Error::invalid_configuration(
                field,
                "must not contain an interior NUL byte",
            ));
        }
        if value.len() >= N {
            return Err(Error::invalid_configuration(
                field,
                format!("must be shorter than {N} bytes to remain NUL-terminated"),
            ));
        }

        let mut bytes = [0; N];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Ok(bytes)
    }
}
