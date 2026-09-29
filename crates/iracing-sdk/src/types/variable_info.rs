#[cfg(feature = "codegen")]
use schemars::JsonSchema;

use crate::{
    IRacingSDKError, Result, VariableRegion,
    irsdk::{VariableHeader, VariableType as IRSDKVariableType},
    parse_utils,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeStruct};

#[cfg(feature = "codegen")]
fn storage_type_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "enum": ["Character", "Boolean", "Integer", "BitField", "Float", "Double"]
    })
}

/// # Variable info
/// Information about a specific telemetry variable.
#[derive(Debug, Clone)]
pub struct VariableInfo {
    /// # Name
    /// Variable name as defined by iRacing
    pub name: String,
    /// # Data type
    /// Data type of the variable
    data_type: IRSDKVariableType,
    /// Region for parsing
    region: VariableRegion,
    /// # Count as time
    /// Whether the simulator treats the sample count as elapsed time
    pub count_as_time: bool,
    /// # Units
    /// Units of measurement (e.g., "m/s", "C", "N*m")
    pub units: String,
    /// # Description
    /// Human-readable description
    pub description: String,
}

#[cfg(feature = "codegen")]
#[allow(dead_code)]
#[derive(JsonSchema)]
struct VariableInfoSchema {
    name: String,
    #[schemars(schema_with = "storage_type_schema")]
    data_type: IRSDKVariableType,
    offset: usize,
    count: usize,
    count_as_time: bool,
    units: String,
    description: String,
}

#[cfg(feature = "codegen")]
impl JsonSchema for VariableInfo {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "VariableInfo".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        VariableInfoSchema::json_schema(generator)
    }
}

impl VariableInfo {
    /// Constructs metadata with frame-relative geometry checked immediately.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        name: String,
        data_type: IRSDKVariableType,
        offset: usize,
        count: usize,
        frame_size: usize,
        count_as_time: bool,
        units: String,
        description: String,
    ) -> Result<Self> {
        let region = VariableRegion::try_new(offset, data_type.byte_size(), count, frame_size)?;
        Ok(Self {
            name,
            data_type,
            region,
            count_as_time,
            units,
            description,
        })
    }

    /// Converts an SDK header into metadata validated against `frame_size`.
    pub fn try_from_header(header: &VariableHeader, frame_size: usize) -> Result<Self> {
        let offset = usize::try_from(header.offset).map_err(|_| {
            IRacingSDKError::parse_error(
                "VariableInfo::try_from",
                format!("Could not convert {} to usize", header.offset),
            )
        })?;

        let count = usize::try_from(header.count).map_err(|_| {
            IRacingSDKError::parse_error(
                "VariableInfo::try_from",
                format!("Could not convert {} to usize", header.count,),
            )
        })?;

        Self::try_new(
            parse_utils::c_string_to_string(&header.name),
            header.variable_type,
            offset,
            count,
            frame_size,
            header.count_as_time != 0,
            parse_utils::c_string_to_string(&header.unit),
            parse_utils::c_string_to_string(&header.description),
        )
    }

    /// Returns the validated frame-relative region.
    pub fn region(&self) -> VariableRegion {
        self.region
    }

    /// Returns the variable's storage type.
    pub fn data_type(&self) -> IRSDKVariableType {
        self.data_type
    }

    /// Returns the starting byte offset within a frame.
    pub fn offset(&self) -> usize {
        self.region.offset()
    }

    /// Returns the number of elements.
    pub fn count(&self) -> usize {
        self.region.count()
    }

    /// Retain the existing frame-relative scalar decoder until the region-based
    /// decoding work is handled separately.
    pub(crate) fn set_scalar_offset(&mut self, offset: usize, region_end: usize) -> Result<()> {
        self.region = VariableRegion::try_new(offset, self.data_type.byte_size(), 1, region_end)?;
        Ok(())
    }
}

impl Serialize for VariableInfo {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("VariableInfo", 7)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("data_type", &self.data_type)?;
        state.serialize_field("offset", &self.offset())?;
        state.serialize_field("count", &self.count())?;
        state.serialize_field("count_as_time", &self.count_as_time)?;
        state.serialize_field("units", &self.units)?;
        state.serialize_field("description", &self.description)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for VariableInfo {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Fields {
            name: String,
            data_type: IRSDKVariableType,
            offset: usize,
            count: usize,
            count_as_time: bool,
            units: String,
            description: String,
        }

        let fields = Fields::deserialize(deserializer)?;
        Self::try_new(
            fields.name,
            fields.data_type,
            fields.offset,
            fields.count,
            usize::MAX,
            fields.count_as_time,
            fields.units,
            fields.description,
        )
        .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_irsdk::VariableType;

    #[test]
    fn variable_info_from_header_builds_region() {
        let header = VariableHeader::new(
            VariableType::Float,
            4,
            2,
            false,
            "WheelSpeed",
            "Wheel speed samples",
            "m/s",
        )
        .unwrap();

        let info = VariableInfo::try_from_header(&header, 12).unwrap();

        assert_eq!(info.offset(), 4);
        assert_eq!(info.count(), 2);
        assert_eq!(info.region().as_region().as_range(), 4..12);
    }

    #[test]
    fn variable_info_rejects_extent_past_frame_during_construction() {
        let header =
            VariableHeader::new(VariableType::Double, 8, 1, false, "SessionTime", "", "s").unwrap();

        assert!(VariableInfo::try_from_header(&header, 15).is_err());
        assert!(VariableInfo::try_from_header(&header, 16).is_ok());
        assert!(
            VariableInfo::try_new(
                "Overflow".into(),
                VariableType::Double,
                usize::MAX - 3,
                1,
                usize::MAX,
                false,
                String::new(),
                String::new()
            )
            .is_err()
        );
        assert!(
            VariableInfo::try_new(
                "Overflow".into(),
                VariableType::Double,
                0,
                usize::MAX,
                usize::MAX,
                false,
                String::new(),
                String::new()
            )
            .is_err()
        );
    }

    #[test]
    fn serialization_preserves_legacy_geometry_fields() {
        let info = VariableInfo::try_new(
            "WheelSpeed".into(),
            VariableType::Float,
            4,
            2,
            12,
            false,
            "m/s".into(),
            String::new(),
        )
        .unwrap();
        let value = serde_json::to_value(&info).unwrap();
        assert_eq!(value["offset"], 4);
        assert_eq!(value["count"], 2);
        assert!(value.get("region").is_none());

        let decoded: VariableInfo = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.region().as_range(), 4..12);
        assert_eq!(decoded.data_type(), VariableType::Float);
    }

    #[test]
    fn deserialization_rejects_invalid_geometry_and_type() {
        let info = VariableInfo::try_new(
            "Speed".into(),
            VariableType::Float,
            0,
            1,
            4,
            false,
            String::new(),
            String::new(),
        )
        .unwrap();
        let mut value = serde_json::to_value(info).unwrap();
        value["count"] = serde_json::json!(0);
        assert!(serde_json::from_value::<VariableInfo>(value.clone()).is_err());
        value["count"] = serde_json::json!(1);
        value["offset"] = serde_json::json!(usize::MAX);
        assert!(serde_json::from_value::<VariableInfo>(value.clone()).is_err());
        value["offset"] = serde_json::json!(0);
        value["data_type"] = serde_json::json!("ElementTypeCount");
        assert!(serde_json::from_value::<VariableInfo>(value).is_err());
    }
}
