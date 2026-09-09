use super::{VariableHeader, irsdk::WireType};

/// Exact, owned snapshot of a variable-header region advertised by an SDK header.
///
/// Construction is restricted to crate-internal source adapters after they have
/// checked the advertised region and copied or read it in full. Semantic
/// validation of individual headers belongs to later wire-to-domain conversion.
#[derive(Debug, Clone)]
pub struct VariableHeadersBuffer {
    bytes: Vec<u8>,
}

impl VariableHeadersBuffer {
    pub(crate) fn from_checked_region(bytes: &[u8]) -> Self {
        Self::from_owned_checked_region(bytes.to_vec())
    }

    /// Takes ownership of a region checked and read in full by a source adapter.
    /// The bytes must contain exactly the advertised number of complete headers.
    pub(crate) fn from_owned_checked_region(bytes: Vec<u8>) -> Self {
        debug_assert_eq!(bytes.len() % VariableHeader::WIRE_SIZE, 0);
        Self { bytes }
    }

    /// Iterates over the wire headers represented by this exact snapshot.
    pub fn iter_headers(&self) -> impl ExactSizeIterator<Item = VariableHeader> + '_ {
        let (chunks, _) = self.bytes.as_chunks::<{ VariableHeader::WIRE_SIZE }>();

        chunks
            .iter()
            .map(|bytes| unsafe { VariableHeader::read_from_bytes_unchecked(bytes) })
    }
}
