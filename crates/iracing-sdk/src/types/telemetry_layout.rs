use std::{collections::HashMap, num::NonZeroUsize};

use crate::irsdk::VariableType;
use crate::{IRacingSDKError, Result, VariableHeaders, VariableRegion, irsdk::VariableHeader};

/// Index of a field in a layout's published header order.
///
/// IDs are stable for the lifetime of that layout. An ID from a different
/// layout is not an identity for the same field, even if its index is in bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldId(usize);

/// Descriptive SDK metadata that does not affect field addressing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMetadata {
    description: Box<str>,
    unit: Box<str>,
    count_as_time: bool,
}

impl FieldMetadata {
    /// Returns the published variable description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the published variable unit.
    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// Returns the SDK's time interpretation marker for the element count.
    pub fn count_as_time(&self) -> bool {
        self.count_as_time
    }
}

/// Validated domain description of one field within a telemetry frame.
///
/// Geometry is stored only in [`VariableRegion`]. Text uses the wire header's
/// existing fixed-string decoding, without trimming or normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLayout {
    name: Box<str>,
    data_type: VariableType,
    region: VariableRegion,
    metadata: FieldMetadata,
}

impl FieldLayout {
    /// Converts an authoritative wire header into a bounded field description.
    ///
    /// # Errors
    ///
    /// Rejects empty names or invalid/out-of-frame geometry. Unsupported storage
    /// discriminants are rejected when decoding [`VariableHeader`] itself.
    pub fn try_from_header(header: &VariableHeader, frame_size: usize) -> Result<Self> {
        let name = header.name().into_owned().into_boxed_str();
        if name.is_empty() {
            return Err(IRacingSDKError::parse_error(
                "FieldLayout",
                "Field name must not be empty",
            ));
        }
        let region = VariableRegion::try_from_header(header, frame_size)?;
        Ok(Self {
            name,
            data_type: header.variable_type,
            region,
            metadata: FieldMetadata {
                description: header.description().into_owned().into_boxed_str(),
                unit: header.unit().into_owned().into_boxed_str(),
                count_as_time: header.count_as_time != 0,
            },
        })
    }

    /// Returns the published field name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the SDK storage type.
    pub fn data_type(&self) -> VariableType {
        self.data_type
    }

    /// Returns the validated frame-relative geometry.
    pub fn region(&self) -> VariableRegion {
        self.region
    }

    /// Returns the number of elements in this field.
    pub fn count(&self) -> usize {
        self.region.count()
    }

    /// Returns descriptive metadata independent of addressing.
    pub fn metadata(&self) -> &FieldMetadata {
        &self.metadata
    }
}

/// Validated runtime description of a complete telemetry frame.
///
/// Canonical field storage preserves the exact variable-header publication
/// order. In-frame overlaps and non-monotonic offsets are accepted; the SDK's
/// metadata is authoritative. Names must be non-empty and unique for lookup.
/// This describes field geometry, independently of physical [`crate::IbtLayout`].
#[derive(Debug, Clone)]
pub struct TelemetryLayout {
    frame_size: NonZeroUsize,
    fields: Box<[FieldLayout]>,
    by_name: HashMap<Box<str>, FieldId>,
}

impl TelemetryLayout {
    /// Builds a layout once from owned header metadata and advertised frame size.
    ///
    /// An empty header snapshot is permitted for a positive frame size. Whether
    /// a source containing telemetry frames may lack metadata belongs to its
    /// provider, rather than to this geometry type.
    ///
    /// # Errors
    ///
    /// Rejects zero frame size, invalid field geometry, and empty/duplicate names.
    pub fn try_from_headers(headers: &VariableHeaders, frame_size: usize) -> Result<Self> {
        let frame_size = NonZeroUsize::new(frame_size).ok_or_else(|| {
            IRacingSDKError::parse_error("TelemetryLayout", "Frame size must be positive")
        })?;
        let mut fields = Vec::with_capacity(headers.len());
        let mut by_name = HashMap::with_capacity(headers.len());
        for header in headers.iter() {
            let field = FieldLayout::try_from_header(header, frame_size.get())?;
            let id = FieldId(fields.len());
            if by_name.insert(field.name.clone(), id).is_some() {
                return Err(IRacingSDKError::parse_error(
                    "TelemetryLayout",
                    format!("Duplicate field name: {}", field.name()),
                ));
            }
            fields.push(field);
        }
        Ok(Self {
            frame_size,
            fields: fields.into_boxed_slice(),
            by_name,
        })
    }

    /// Returns the advertised size of one complete frame in bytes.
    pub fn frame_size(&self) -> usize {
        self.frame_size.get()
    }

    /// Returns the number of published fields.
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns whether the layout has no published fields.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Returns a field by its layout-local ID, or `None` for an out-of-range ID.
    pub fn field(&self, id: FieldId) -> Option<&FieldLayout> {
        self.fields.get(id.0)
    }

    /// Resolves an exact published name to its stable ID and field description.
    pub fn field_by_name(&self, name: &str) -> Option<(FieldId, &FieldLayout)> {
        let id = *self.by_name.get(name)?;
        Some((id, &self.fields[id.0]))
    }

    /// Iterates over fields and their IDs in exact header publication order.
    pub fn fields(&self) -> impl ExactSizeIterator<Item = (FieldId, &FieldLayout)> {
        self.fields
            .iter()
            .enumerate()
            .map(|(index, field)| (FieldId(index), field))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(name: &str, data_type: VariableType, offset: i32, count: i32) -> VariableHeader {
        VariableHeader::new(data_type, offset, count, false, name, "", "").unwrap()
    }

    #[test]
    fn all_storage_types_have_bounded_scalar_and_array_extents() {
        // Synthetic geometry isolates storage widths from capture-specific offsets.
        for data_type in [
            VariableType::Character,
            VariableType::Boolean,
            VariableType::Integer,
            VariableType::BitField,
            VariableType::Float,
            VariableType::Double,
        ] {
            for count in [1, 3] {
                let end = 7 + data_type.byte_size() * count as usize;
                let wire = header("Storage", data_type, 7, count);
                let field = FieldLayout::try_from_header(&wire, end).unwrap();
                assert_eq!(field.data_type(), data_type);
                assert_eq!(field.region().as_range(), 7..end);
                assert_eq!(field.count(), count as usize);
                assert!(FieldLayout::try_from_header(&wire, end - 1).is_err());
            }
        }
    }

    #[test]
    fn preserves_publication_order_even_with_overlaps_and_nonmonotonic_offsets() {
        let headers = VariableHeaders::new(&[
            header("Speed", VariableType::Float, 8, 1),
            header("RPM", VariableType::Float, 0, 3),
            header("Brake", VariableType::Float, 4, 1),
        ]);
        let layout = TelemetryLayout::try_from_headers(&headers, 12).unwrap();
        assert_eq!(layout.frame_size(), 12);
        assert_eq!(layout.len(), 3);
        assert!(!layout.is_empty());
        assert_eq!(layout.fields().len(), 3);
        let names: Vec<_> = layout.fields().map(|(_, field)| field.name()).collect();
        assert_eq!(names, ["Speed", "RPM", "Brake"]);
        for (index, (id, field)) in layout.fields().enumerate() {
            assert_eq!(id, FieldId(index));
            assert_eq!(layout.field(id), Some(field));
            assert_eq!(layout.field_by_name(field.name()), Some((id, field)));
            assert_eq!(layout.clone().field(id), Some(field));
        }
        assert!(layout.field(FieldId(3)).is_none());
        assert!(layout.field_by_name("Missing").is_none());
        assert!(layout.field_by_name("speed").is_none());
    }

    #[test]
    fn rejects_ambiguous_names_without_normalizing_them() {
        let speed = header("Speed", VariableType::Float, 0, 1);
        assert!(
            TelemetryLayout::try_from_headers(&VariableHeaders::new(&[speed, speed]), 4).is_err()
        );
        let empty = header("", VariableType::Float, 0, 1);
        assert!(TelemetryLayout::try_from_headers(&VariableHeaders::new(&[empty]), 4).is_err());
        let spaced = header(" Speed ", VariableType::Float, 0, 1);
        let layout =
            TelemetryLayout::try_from_headers(&VariableHeaders::new(&[speed, spaced]), 4).unwrap();
        assert!(layout.field_by_name(" Speed ").is_some());
    }

    #[test]
    fn rejects_invalid_signed_geometry() {
        let valid = header("Speed", VariableType::Float, 0, 1);
        for (offset, count) in [(-1, 1), (0, -1), (0, 0)] {
            let mut wire = valid;
            wire.offset = offset;
            wire.count = count;
            assert!(TelemetryLayout::try_from_headers(&VariableHeaders::new(&[wire]), 4).is_err());
        }
        assert!(TelemetryLayout::try_from_headers(&VariableHeaders::new(&[valid]), 3).is_err());
    }

    #[test]
    fn permits_empty_metadata_only_with_positive_frame_size() {
        let headers = VariableHeaders::default();
        let layout = TelemetryLayout::try_from_headers(&headers, 4).unwrap();
        assert!(layout.is_empty());
        assert_eq!(layout.len(), 0);
        assert_eq!(layout.fields().len(), 0);
        assert!(layout.field_by_name("Speed").is_none());
        assert!(TelemetryLayout::try_from_headers(&headers, 0).is_err());
    }

    #[test]
    fn metadata_is_owned_and_preserved() {
        let layout = {
            let wire = VariableHeader::new(
                VariableType::Float,
                0,
                1,
                true,
                "Speed",
                "Vehicle speed",
                "m/s",
            )
            .unwrap();
            TelemetryLayout::try_from_headers(&VariableHeaders::new(&[wire]), 4).unwrap()
        };
        let (_, field) = layout.field_by_name("Speed").unwrap();
        assert_eq!(field.metadata().description(), "Vehicle speed");
        assert_eq!(field.metadata().unit(), "m/s");
        assert!(field.metadata().count_as_time());
    }

    #[test]
    fn invalid_storage_discriminants_are_rejected_at_wire_boundary() {
        use zerocopy::IntoBytes;
        for raw in [-1i32, 6, 99] {
            let wire = header("Speed", VariableType::Float, 0, 1);
            let mut bytes = wire.as_bytes().to_vec();
            bytes[..4].copy_from_slice(&raw.to_le_bytes());
            assert!(VariableHeader::try_from_bytes(&bytes).is_err());
            assert!(VariableHeaders::try_from_bytes(&bytes, 1).is_err());
        }
    }

    #[test]
    fn captured_disk_metadata_fits_its_advertised_frame() {
        // Use one coherent reference snapshot, including its original frame size.
        // Its name-keyed YAML cannot establish SDK publication order; the test
        // above separately verifies preservation of the input header order.
        #[derive(serde::Deserialize)]
        struct Reference {
            examples: Vec<Capture>,
        }
        #[derive(serde::Deserialize)]
        struct Capture {
            frame_size: usize,
            variables: std::collections::BTreeMap<String, CapturedField>,
        }
        #[derive(serde::Deserialize)]
        struct CapturedField {
            name: String,
            data_type: VariableType,
            offset: i32,
            count: i32,
            count_as_time: bool,
            description: String,
            units: String,
        }
        let reference: Reference = serde_yaml_ng::from_str(include_str!(
            "../../../../docs/reference/disk-variable-schema.yml"
        ))
        .unwrap();
        let capture = &reference.examples[0];
        let headers: Vec<_> = capture
            .variables
            .values()
            .map(|field| {
                VariableHeader::new(
                    field.data_type,
                    field.offset,
                    field.count,
                    field.count_as_time,
                    &field.name,
                    &field.description,
                    &field.units,
                )
                .unwrap()
            })
            .collect();
        let layout =
            TelemetryLayout::try_from_headers(&VariableHeaders::new(&headers), capture.frame_size)
                .unwrap();
        assert_eq!(layout.len(), capture.variables.len());
        for field in capture.variables.values() {
            let (_, actual) = layout.field_by_name(&field.name).unwrap();
            assert_eq!(actual.data_type(), field.data_type);
            assert_eq!(actual.region().offset(), field.offset as usize);
            assert_eq!(actual.count(), field.count as usize);
            assert_eq!(actual.metadata().description(), field.description);
            assert_eq!(actual.metadata().unit(), field.units);
            assert_eq!(actual.metadata().count_as_time(), field.count_as_time);
        }
    }
}
