use std::io::Read;

use crate::{DiskSubHeader, Header};

use crate::{Error, Result};

/// The fixed wire preamble of an iRacing `.ibt` recording.
///
/// Contains the SDK [`Header`] followed by the recording's [`DiskSubHeader`].
/// Decoding copies the native-layout bytes into an owned value; it does not
/// validate field values, offsets, record counts, or the rest of the file.
#[repr(C)]
#[derive(
    Debug,
    Clone,
    Copy,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IbtHeader {
    header: Header,
    disk_header: DiskSubHeader,
}

impl IbtHeader {
    /// Decodes one complete IBT preamble from its exact wire representation.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WireSize`] when `bytes` is not exactly
    /// `size_of::<IbtHeader>()` bytes. Pass only the preamble, not the whole file.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self> {
        <Self as zerocopy::FromBytes>::read_from_bytes(bytes).map_err(Error::from)
    }

    /// Reads one complete IBT preamble from the current position in `reader`.
    ///
    /// Consumes `size_of::<IbtHeader>()` bytes on success, leaving subsequent
    /// file contents unread. Field values are not validated.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if reading fails, including when the reader cannot
    /// supply a complete preamble. A failed read may consume part of the input.
    pub fn try_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        <Self as zerocopy::FromBytes>::read_from_io(reader).map_err(Error::from)
    }

    /// Returns the SDK header containing telemetry and session metadata.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Returns the recording's timing and advisory record-count metadata.
    pub fn disk_header(&self) -> &DiskSubHeader {
        &self.disk_header
    }
}
