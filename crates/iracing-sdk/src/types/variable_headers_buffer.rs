use crate::irsdk::VariableHeader;
use zerocopy::FromBytes;

/// Exact, owned snapshot of a variable-header region advertised by an SDK header.
///
/// Construction is restricted to crate-internal source adapters after they have
/// checked the advertised region and copied or read it in full. Semantic
/// validation of individual headers belongs to later wire-to-domain conversion.
#[derive(Debug, Clone)]
pub struct VariableHeadersBuffer {
    bytes: Vec<VariableHeader>,
}

impl VariableHeadersBuffer {}
