#![warn(missing_docs)]
//! Rust representations of the native iRacing SDK wire contract.
//!
//! This crate owns types that can be understood from `irsdk_defines.h` and the
//! documented byte layout alone: fixed-layout headers, variable metadata,
//! primitive storage types, constants, enums, flags, and broadcast commands.
//! It deliberately does not parse IBT files, access Windows shared memory,
//! construct telemetry schemas, decode session YAML, or run telemetry streams.

mod bitfield;
mod macros;
mod parse_utils;

#[cfg(feature = "broadcast")]
pub mod broadcast;

pub mod constants;
pub mod flags;
pub mod telemetry;
pub mod variable_type;

mod disk_sub_header;
mod error;
mod header;
mod ibt_header;
mod variable_buffer;
mod variable_header;

pub use bitfield::BitField;
#[cfg(feature = "broadcast")]
pub use broadcast::*;
pub use disk_sub_header::DiskSubHeader;
pub use error::{Error, Result};
pub use flags::*;
pub use header::Header;
pub use ibt_header::IbtHeader;
pub use parse_utils::{decode, encode};
pub use telemetry::*;
pub use variable_buffer::VariableBuffer;
pub use variable_header::VariableHeader;
pub use variable_type::VariableType;

#[cfg(test)]
mod serde_tests {
    use super::*;
    use zerocopy::IntoBytes;

    #[test]
    fn root_wire_structs_round_trip_without_padding() {
        let buffer = VariableBuffer::new(10, 20, 9);
        let header = Header::new(
            constants::IRSDK_VER,
            StatusField::CONNECTED,
            60,
            1,
            100,
            112,
            1,
            212,
            1,
            64,
            10,
            0,
            [buffer; Header::MAX_BUFFERS],
        );
        let disk = DiskSubHeader::new(123, 1.5, 2.5, 3, 4);
        let variable = VariableHeader::new(
            VariableType::Float,
            8,
            1,
            true,
            "Speed",
            "Vehicle speed",
            "m/s",
        )
        .unwrap();

        let header_json = serde_json::to_value(header).unwrap();
        assert!(header_json.get("_pad").is_none());
        assert!(header_json["buffers"][0].get("_pad").is_none());
        let decoded_header: Header = serde_json::from_value(header_json).unwrap();
        assert_eq!(decoded_header.as_bytes(), header.as_bytes());

        let buffer_json = serde_json::to_value(buffer).unwrap();
        assert!(buffer_json.get("_pad").is_none());
        let decoded_buffer: VariableBuffer = serde_json::from_value(buffer_json).unwrap();
        assert_eq!(decoded_buffer.as_bytes(), buffer.as_bytes());

        let decoded_disk: DiskSubHeader =
            serde_json::from_value(serde_json::to_value(disk).unwrap()).unwrap();
        assert_eq!(decoded_disk.as_bytes(), disk.as_bytes());

        let variable_json = serde_json::to_value(variable).unwrap();
        assert_eq!(variable_json["count_as_time"], true);
        let decoded_variable: VariableHeader = serde_json::from_value(variable_json).unwrap();
        assert_eq!(decoded_variable.as_bytes(), variable.as_bytes());
    }

    #[test]
    fn variable_header_deserialization_rejects_invalid_metadata() {
        let invalid = serde_json::json!({
            "variable_type": "Float",
            "offset": -1,
            "count": 1,
            "count_as_time": false,
            "name": "Speed",
            "description": "Vehicle speed",
            "unit": "m/s",
        });
        assert!(serde_json::from_value::<VariableHeader>(invalid).is_err());
    }
}
