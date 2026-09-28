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
