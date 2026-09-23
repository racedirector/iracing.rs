use crate::{IRacingSDKError, Result, irsdk::VariableHeader};

/// Exact, owned snapshot of decoded SDK variable-header records.
pub struct VariableHeadersSnapshot {
    headers: Vec<VariableHeader>,
}

impl VariableHeadersSnapshot {
    /// Decodes exactly `expected_count` headers from a complete region snapshot.
    ///
    /// # Errors
    ///
    /// Returns a parse error when the byte length contains a partial header or
    /// the number of complete headers differs from `expected_count`.
    pub fn try_from_region_bytes(bytes: Vec<u8>, expected_count: usize) -> Result<Self> {
        let (chunks, []) = bytes.as_chunks::<{ size_of::<VariableHeader>() }>() else {
            return Err(IRacingSDKError::parse_error(
                "VariableHeadersSnapshot",
                format!(
                    "`bytes` length {} is not divisible by VariableHeader size {}",
                    bytes.len(),
                    size_of::<VariableHeader>(),
                ),
            ));
        };

        if chunks.len() != expected_count {
            return Err(IRacingSDKError::parse_error(
                "VariableHeadersSnapshot",
                format!(
                    "expected {expected_count} headers, but found {}",
                    chunks.len(),
                ),
            ));
        }

        let headers = chunks
            .iter()
            .map(|bytes| {
                VariableHeader::read_from_bytes(bytes)
                    .expect("Chunk is exactly size_of::<VariableHeader>()")
            })
            .collect();

        Ok(Self { headers })
    }

    /// Returns the decoded headers as a slice.
    pub fn as_slice(&self) -> &[VariableHeader] {
        &self.headers
    }

    /// Returns the number of decoded headers.
    pub fn len(&self) -> usize {
        self.headers.len()
    }

    /// Iterates over the decoded headers.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &VariableHeader> {
        self.headers.iter()
    }

    /// Returns whether the snapshot contains no headers.
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::irsdk::VariableType as IRSDKVariableType;
    use zerocopy::IntoBytes;

    #[test]
    fn zero_variable_header_snapshot_from_bytes() {
        let headers: Vec<VariableHeader> = vec![];

        let bytes = headers.as_bytes();

        assert!(VariableHeadersSnapshot::try_from_region_bytes(bytes.into(), 0).is_ok());
    }

    #[test]
    fn single_variable_header_snapshot_from_bytes() {
        let headers = [VariableHeader::new(
            IRSDKVariableType::Float,
            4,
            1,
            false,
            "Speed",
            "Vehicle speed",
            "m/s",
        )
        .unwrap()];

        let bytes = headers.as_bytes();

        let snapshot_result = VariableHeadersSnapshot::try_from_region_bytes(bytes.into(), 1);
        assert!(snapshot_result.is_ok());
        let snapshot = snapshot_result.unwrap();
        assert_eq!(snapshot.headers.len(), 1);
    }

    #[test]
    fn many_variable_header_snapshot_from_bytes() {
        let headers = [
            VariableHeader::new(
                IRSDKVariableType::Float,
                4,
                1,
                false,
                "Speed",
                "Vehicle speed",
                "m/s",
            )
            .unwrap(),
            VariableHeader::new(
                IRSDKVariableType::Boolean,
                8,
                1,
                false,
                "PlayerIsInCar",
                "Current player is in car",
                "",
            )
            .unwrap(),
            VariableHeader::new(
                IRSDKVariableType::Float,
                9,
                1,
                false,
                "Speed",
                "Vehicle speed",
                "m/s",
            )
            .unwrap(),
        ];

        let bytes = headers.as_bytes();
        let snapshot_result = VariableHeadersSnapshot::try_from_region_bytes(bytes.into(), 3);
        assert!(snapshot_result.is_ok());
        let snapshot = snapshot_result.unwrap();
        assert_eq!(snapshot.headers.len(), 3);
    }

    #[test]
    fn too_many_variable_header_snapshot_from_bytes() {
        let headers = [
            VariableHeader::new(
                IRSDKVariableType::Float,
                4,
                1,
                false,
                "Speed",
                "Vehicle speed",
                "m/s",
            )
            .unwrap(),
            VariableHeader::new(
                IRSDKVariableType::Boolean,
                8,
                1,
                false,
                "PlayerIsInCar",
                "Current player is in car",
                "",
            )
            .unwrap(),
            VariableHeader::new(
                IRSDKVariableType::Float,
                9,
                1,
                false,
                "Speed",
                "Vehicle speed",
                "m/s",
            )
            .unwrap(),
        ];

        let bytes = headers.as_bytes();
        let snapshot_result = VariableHeadersSnapshot::try_from_region_bytes(bytes.into(), 2);
        assert!(snapshot_result.is_err());
    }

    #[test]
    fn not_enough_variable_header_snapshot_from_bytes() {
        let headers = [
            VariableHeader::new(
                IRSDKVariableType::Float,
                4,
                1,
                false,
                "Speed",
                "Vehicle speed",
                "m/s",
            )
            .unwrap(),
            VariableHeader::new(
                IRSDKVariableType::Boolean,
                8,
                1,
                false,
                "PlayerIsInCar",
                "Current player is in car",
                "",
            )
            .unwrap(),
        ];

        let bytes = headers.as_bytes();
        let snapshot_result = VariableHeadersSnapshot::try_from_region_bytes(bytes.into(), 3);
        assert!(snapshot_result.is_err());
    }

    #[test]
    fn trailing_bytes_snapshot_from_bytes() {
        let mut bytes = vec![];

        let headers = VariableHeader::new(
            IRSDKVariableType::Float,
            4,
            1,
            false,
            "Speed",
            "Vehicle speed",
            "m/s",
        )
        .unwrap();

        bytes.extend_from_slice(headers.as_bytes());
        bytes.extend_from_slice(&[1, 2, 3]);

        let snapshot_result = VariableHeadersSnapshot::try_from_region_bytes(bytes, 1);
        assert!(snapshot_result.is_err());
    }
}
