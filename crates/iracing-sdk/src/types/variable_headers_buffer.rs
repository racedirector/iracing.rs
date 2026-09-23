use crate::{IRacingSDKError, Result, irsdk::VariableHeader};
use zerocopy::FromBytes;

/// Exact, owned snapshot of decoded SDK variable-header records.
///
/// Construction validates that the source contains exactly the advertised
/// number of complete records. Semantic validation of individual headers
/// remains the responsibility of later wire-to-domain conversion.
#[derive(Debug, Clone)]
pub struct VariableHeadersBuffer {
    headers: Vec<VariableHeader>,
}

impl VariableHeadersBuffer {
    /// Decodes exactly `expected_count` headers from a complete region snapshot.
    pub(crate) fn try_from_region_bytes(bytes: &[u8], expected_count: usize) -> Result<Self> {
        let (chunks, []) = bytes.as_chunks::<{ size_of::<VariableHeader>() }>() else {
            return Err(IRacingSDKError::parse_error(
                "VariableHeadersBuffer",
                format!(
                    "variable-header region length {} is not divisible by record size {}",
                    bytes.len(),
                    size_of::<VariableHeader>(),
                ),
            ));
        };

        if chunks.len() != expected_count {
            return Err(IRacingSDKError::parse_error(
                "VariableHeadersBuffer",
                format!(
                    "expected {expected_count} variable headers, but decoded {}",
                    chunks.len(),
                ),
            ));
        }

        let headers = chunks
            .iter()
            .map(|bytes| {
                VariableHeader::read_from_bytes(bytes).map_err(|error| {
                    IRacingSDKError::parse_error(
                        "VariableHeadersBuffer",
                        format!("failed to decode a complete variable header: {error}"),
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { headers })
    }

    /// Returns the decoded headers as a slice.
    pub fn as_slice(&self) -> &[VariableHeader] {
        &self.headers
    }

    /// Iterates over the decoded headers.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &VariableHeader> {
        self.headers.iter()
    }

    /// Returns the number of decoded headers.
    pub fn len(&self) -> usize {
        self.headers.len()
    }

    /// Returns whether the snapshot contains no headers.
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::irsdk::VariableType;
    use zerocopy::IntoBytes;

    fn header(name: &str, offset: i32) -> VariableHeader {
        VariableHeader::new(
            VariableType::Float,
            offset,
            1,
            false,
            name,
            "Test variable",
            "unit",
        )
        .unwrap()
    }

    #[test]
    fn accepts_zero_headers() {
        let snapshot = VariableHeadersBuffer::try_from_region_bytes(&[], 0).unwrap();

        assert!(snapshot.is_empty());
        assert_eq!(snapshot.iter().len(), 0);
    }

    #[test]
    fn accepts_one_header() {
        let headers = [header("Speed", 4)];
        let snapshot = VariableHeadersBuffer::try_from_region_bytes(headers.as_bytes(), 1).unwrap();

        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot.as_slice()[0].offset, 4);
    }

    #[test]
    fn accepts_multiple_headers() {
        let headers = [header("Speed", 4), header("RPM", 8)];
        let snapshot = VariableHeadersBuffer::try_from_region_bytes(headers.as_bytes(), 2).unwrap();

        assert_eq!(snapshot.len(), 2);
    }

    #[test]
    fn rejects_partial_trailing_record() {
        let mut bytes = header("Speed", 4).as_bytes().to_vec();
        bytes.push(0);

        assert!(VariableHeadersBuffer::try_from_region_bytes(&bytes, 1).is_err());
    }

    #[test]
    fn rejects_extra_complete_record() {
        let headers = [header("Speed", 4), header("RPM", 8)];

        assert!(VariableHeadersBuffer::try_from_region_bytes(headers.as_bytes(), 1).is_err());
    }

    #[test]
    fn rejects_fewer_records_than_advertised() {
        let headers = [header("Speed", 4)];

        assert!(VariableHeadersBuffer::try_from_region_bytes(headers.as_bytes(), 2).is_err());
    }
}
