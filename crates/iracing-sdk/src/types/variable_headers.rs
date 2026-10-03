use std::ops::Deref;

use crate::{IRacingSDKError, Result, irsdk::VariableHeader};
use serde::{Serialize, Serializer};
use zerocopy::TryFromBytes;

/// Exact, owned snapshot of decoded SDK variable-header records.
///
/// Construction validates that the source contains exactly the advertised
/// number of complete records. Semantic validation of individual headers
/// remains the responsibility of later wire-to-domain conversion.
/// Serializes as an array of variable headers in their source order.
#[derive(Debug, Clone)]
pub struct VariableHeaders(Box<[VariableHeader]>);

impl VariableHeaders {
    pub(crate) fn new(headers: &[VariableHeader]) -> Self {
        Self(headers.to_vec().into_boxed_slice())
    }

    /// Decodes exactly `expected_count` headers from a complete region snapshot.
    pub(crate) fn try_from_bytes(bytes: &[u8], expected_count: usize) -> Result<Self> {
        if bytes.is_empty() && expected_count == 0 {
            return Ok(Self::default());
        }
        let headers = <[VariableHeader]>::try_ref_from_bytes_with_elems(bytes, expected_count)
            .map_err(IRacingSDKError::from)?;

        Ok(Self::new(headers))
    }

    /// Returns the decoded headers as a slice.
    pub fn as_slice(&self) -> &[VariableHeader] {
        &self.0
    }

    /// Iterates over the decoded headers.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &VariableHeader> {
        self.0.iter()
    }

    /// Returns the number of decoded headers.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether the snapshot contains no headers.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl AsRef<[VariableHeader]> for VariableHeaders {
    fn as_ref(&self) -> &[VariableHeader] {
        &self.0
    }
}

impl Deref for VariableHeaders {
    type Target = [VariableHeader];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for VariableHeaders {
    fn default() -> Self {
        Self::new(&[])
    }
}

impl Serialize for VariableHeaders {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl From<Vec<VariableHeader>> for VariableHeaders {
    fn from(headers: Vec<VariableHeader>) -> Self {
        Self(headers.into_boxed_slice())
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
        let snapshot = VariableHeaders::try_from_bytes(&[], 0).unwrap();

        assert!(snapshot.is_empty());
        assert_eq!(snapshot.iter().len(), 0);
        assert_eq!(
            serde_json::to_value(&snapshot).unwrap(),
            serde_json::json!([])
        );
    }

    #[test]
    fn accepts_one_header() {
        let headers = [header("Speed", 4)];
        let snapshot = VariableHeaders::try_from_bytes(headers.as_bytes(), 1).unwrap();

        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot.as_slice()[0].offset, 4);
    }

    #[test]
    fn accepts_multiple_headers() {
        let headers = [header("Speed", 4), header("RPM", 8)];
        let snapshot = VariableHeaders::try_from_bytes(headers.as_bytes(), 2).unwrap();

        assert_eq!(snapshot.len(), 2);
    }

    #[test]
    fn serializes_headers_as_an_array() {
        let headers = [header("Speed", 4), header("RPM", 8)];
        let snapshot = VariableHeaders::try_from_bytes(headers.as_bytes(), 2).unwrap();

        let value = serde_json::to_value(&snapshot).unwrap();
        let entries = value.as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["name"], "Speed");
        assert_eq!(entries[1]["name"], "RPM");
    }

    #[test]
    fn rejects_partial_trailing_record() {
        let mut bytes = header("Speed", 4).as_bytes().to_vec();
        bytes.push(0);

        assert!(VariableHeaders::try_from_bytes(&bytes, 1).is_err());
    }

    #[test]
    fn rejects_extra_complete_record() {
        let headers = [header("Speed", 4), header("RPM", 8)];

        assert!(VariableHeaders::try_from_bytes(headers.as_bytes(), 1).is_err());
    }

    #[test]
    fn rejects_fewer_records_than_advertised() {
        let headers = [header("Speed", 4)];

        assert!(VariableHeaders::try_from_bytes(headers.as_bytes(), 2).is_err());
    }
}
