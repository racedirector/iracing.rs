use std::io::Read;
use zerocopy::FromBytes;

use crate::{Error, Result};

pub(crate) fn read_wire_bytes<T: FromBytes>(bytes: &[u8]) -> Result<T> {
    T::read_from_bytes(bytes).map_err(|_| Error::WireSize {
        expected: size_of::<T>(),
        actual: bytes.len(),
    })
}

pub(crate) fn read_wire_bytes_from_io<T: FromBytes, R: Read>(reader: &mut R) -> Result<T> {
    T::read_from_io(reader).map_err(|_| Error::WireSize {
        expected: 1,
        actual: 1,
    })
}

pub(crate) fn nul_terminated_bytes(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    &bytes[..end]
}
