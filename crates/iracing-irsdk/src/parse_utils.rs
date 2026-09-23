use std::{any::type_name, io::Read};
use zerocopy::{ConvertError, FromBytes, TryFromBytes};

use crate::{Error, Result};

pub(crate) fn read_wire_bytes<T: FromBytes>(bytes: &[u8]) -> Result<T> {
    T::read_from_bytes(bytes).map_err(|_| Error::WireSize {
        expected: size_of::<T>(),
        actual: bytes.len(),
    })
}

pub(crate) fn try_from_wire_bytes<T: TryFromBytes>(bytes: &[u8]) -> Result<T> {
    T::try_read_from_bytes(bytes).map_err(|error| match error {
        ConvertError::Size(_) => Error::WireSize {
            expected: size_of::<T>(),
            actual: bytes.len(),
        },
        ConvertError::Validity(_) => Error::InvalidWireValue {
            target: type_name::<T>(),
        },
        ConvertError::Alignment(never) => match never {},
    })
}

pub(crate) fn read_wire_bytes_from_io<T: FromBytes, R: Read>(reader: &mut R) -> Result<T> {
    T::read_from_io(reader).map_err(|_| Error::WireSize {
        expected: 1,
        actual: 1,
    })
}
