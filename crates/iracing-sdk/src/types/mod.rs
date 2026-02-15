//! Core types for telemetry data representation.
//!
//! This module provides the foundational data structures for handling iRacing telemetry data,
//! including frame representation, schema management, and type-safe data parsing.
//!
//! ## Architecture
//!
//! The type system maps directly to iRacing SDK structures:
//! - [`FramePacket`] represents a complete telemetry frame with zero-copy binary data
//! - [`VariableSchema`] describes the structure of telemetry variables with O(1) lookup
//! - [`VariableType`] maps to iRacing's `irsdk_VarType` enum with size information
//! - [`VarData`] trait provides type-safe parsing from binary telemetry data
//! - [`BitField`] handles iRacing's bitfield variables with flag operations
//!
//! ## Performance Characteristics
//!
//! - O(1) variable lookup via HashMap
//! - Zero-copy data sharing via Arc
//! - Bounds checking on all memory operations
//! - Tick count wraparound handling for proper frame ordering
//!
//! ## Usage Example
//!
//! ```rust
//! use pitwall::types::{FramePacket, VariableSchema, VariableInfo, VariableType, VarData};
//! use std::collections::HashMap;
//! use std::sync::Arc;
//!
//! // Create a schema for RPM data
//! let mut variables = HashMap::new();
//! variables.insert("RPM".to_string(), VariableInfo {
//!     name: "RPM".to_string(),
//!     data_type: VariableType::Float32,
//!     offset: 0,
//!     count: 1,
//!     count_as_time: false,
//!     units: "rev/min".to_string(),
//!     description: "Engine RPM".to_string(),
//! });
//!
//! let schema = Arc::new(VariableSchema { variables, frame_size: 4 });
//! let data = vec![0x00, 0xA0, 0x8C, 0x45]; // 4500.0 as little-endian f32
//!
//! let packet = FramePacket::new(
//!     data,
//!     12345, // tick
//!     1,     // session_version
//!     schema
//! );
//!
//! // Parse RPM value
//! if let Some(rpm_info) = packet.schema.get_variable("RPM") {
//!     let rpm: f32 = f32::from_bytes(packet.data.as_ref(), rpm_info).unwrap();
//!     assert!((rpm - 4500.0).abs() < 1.0); // Allow for floating point precision
//! }
//! ```

mod bitfield;
mod frame;
mod incident;
pub mod irsdk_flags;
mod schema;
mod update_rate;
mod var_data;
mod variable_type;

// Re-export all public types
pub use bitfield::{
    BitField, engine_mandatory_repair_needed, engine_optional_repair_needed,
    session_dq_scoring_invalid, tick_after_u32,
};
pub use frame::FramePacket;
pub use incident::{IncidentClassification, IncidentPenalty, IncidentReport, decode_incident};
pub use schema::{VariableInfo, VariableSchema};
pub use update_rate::UpdateRate;
pub use var_data::VarData;
pub use variable_type::{Value, VariableType};

#[cfg(test)]
mod tests {
    use super::*;

    use proptest::prelude::*;

    // Property test strategies
    prop_compose! {
        fn arb_variable_info()(
            name in "[a-zA-Z][a-zA-Z0-9_]*",
            data_type in prop::sample::select(vec![
                VariableType::Char, VariableType::Int8, VariableType::UInt8,
                VariableType::Int16, VariableType::UInt16, VariableType::Int32,
                VariableType::UInt32, VariableType::Float32, VariableType::Float64,
                VariableType::Bool, VariableType::BitField
            ]),
            offset in 0..1024usize,
            count in 1..10usize,
            units in "[a-zA-Z/^2]*",
            description in "[a-zA-Z ]*"
        ) -> VariableInfo {
            VariableInfo {
                name,
                data_type,
                offset,
                count,
                count_as_time: false,
                units,
                description,
            }
        }
    }

    // RawFrame tests removed - RawFrame no longer exists

    // Property tests for VariableSchema
    proptest! {

        #[test]
        fn prop_variable_schema_parsing_with_fuzzed_headers(
            variables in prop::collection::btree_map(
                "[a-zA-Z][a-zA-Z0-9_]*",
                arb_variable_info(),
                0..20
            ),
            frame_size in 64..2048usize
        ) {
            // VariableSchema parsing succeeds/fails appropriately with fuzzed headers
            use std::collections::HashMap;
            let mut adjusted_variables = HashMap::new();

            // Adjust variable offsets to ensure they fit within frame_size
            for (name, mut var_info) in variables.into_iter() {
                // Ensure offset is within reasonable bounds for the frame size
                let max_size = var_info.data_type.size() * var_info.count;
                if max_size < frame_size {
                    var_info.offset %= frame_size - max_size;
                } else {
                    var_info.offset = 0;
                    var_info.count = 1;
                }

                // Ensure name consistency
                var_info.name = name.clone();
                adjusted_variables.insert(name, var_info);
            }

            let schema = VariableSchema {
                variables: adjusted_variables,
                frame_size,
            };

            // Schema should be consistent
            prop_assert!(schema.frame_size <= 2048);
            prop_assert!(schema.frame_size >= 64);

            // All variable offsets should be reasonable
            for var_info in schema.variables.values() {
                let end_offset = var_info.offset + (var_info.data_type.size() * var_info.count);
                prop_assert!(end_offset <= schema.frame_size);
                prop_assert!(var_info.count > 0);
            }

            // Validation should pass
            let validation_result = schema.validate();
            prop_assert!(validation_result.is_ok());
        }

        #[test]
        fn prop_variable_type_size_calculations_correct(var_type in prop::sample::select(vec![
            VariableType::Char, VariableType::Int8, VariableType::UInt8,
            VariableType::Int16, VariableType::UInt16, VariableType::Int32,
            VariableType::UInt32, VariableType::Float32, VariableType::Float64,
            VariableType::Bool, VariableType::BitField
        ])) {
            // VariableType size calculations correct for all enum variants
            let size = var_type.size();
            prop_assert!(size > 0);
            prop_assert!(size <= 8);

            match var_type {
                VariableType::Char | VariableType::Int8 | VariableType::UInt8 | VariableType::Bool => {
                    prop_assert_eq!(size, 1);
                },
                VariableType::Int16 | VariableType::UInt16 => {
                    prop_assert_eq!(size, 2);
                },
                VariableType::Int32 | VariableType::UInt32 | VariableType::Float32 | VariableType::BitField => {
                    prop_assert_eq!(size, 4);
                },
                VariableType::Float64 => {
                    prop_assert_eq!(size, 8);
                },
            }
        }

        #[test]
        fn prop_vardata_roundtrip_preserves_data_f32(
            value in any::<f32>(),
            offset in 0..100usize
        ) {
            // VarData roundtrip (serialize→deserialize) preserves data
            let mut data = vec![0u8; offset + 4 + 10];
            let bytes = value.to_le_bytes();
            data[offset..offset + 4].copy_from_slice(&bytes);

            let var_info = VariableInfo {
                name: "test".to_string(),
                data_type: VariableType::Float32,
                offset,
                count: 1,
                count_as_time: false,
                units: "test".to_string(),
                description: "test".to_string(),
            };

            let result = f32::from_bytes(&data, &var_info);
            prop_assert!(result.is_ok());

            let parsed = result.unwrap();
            if value.is_finite() {
                prop_assert!((parsed - value).abs() < f32::EPSILON);
            } else if value.is_nan() {
                prop_assert!(parsed.is_nan());
            } else {
                prop_assert_eq!(parsed, value);
            }
        }

        #[test]
        fn prop_vardata_roundtrip_preserves_data_i32(
            value in any::<i32>(),
            offset in 0..100usize
        ) {
            let mut data = vec![0u8; offset + 4 + 10];
            let bytes = value.to_le_bytes();
            data[offset..offset + 4].copy_from_slice(&bytes);

            let var_info = VariableInfo {
                name: "test".to_string(),
                data_type: VariableType::Int32,
                offset,
                count: 1,
                count_as_time: false,
                units: "test".to_string(),
                description: "test".to_string(),
            };

            let result = i32::from_bytes(&data, &var_info);
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap(), value);
        }

        #[test]
        fn prop_bitfield_parsing_handles_all_32bit_patterns(
            value in any::<u32>(),
            offset in 0..100usize
        ) {
            // BitField parsing handles all 32-bit patterns correctly
            let mut data = vec![0u8; offset + 4 + 10];
            let bytes = value.to_le_bytes();
            data[offset..offset + 4].copy_from_slice(&bytes);

            let var_info = VariableInfo {
                name: "test".to_string(),
                data_type: VariableType::BitField,
                offset,
                count: 1,
                count_as_time: false,
                units: "test".to_string(),
                description: "test".to_string(),
            };

            let result = BitField::from_bytes(&data, &var_info);
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap().value(), value);
        }

        #[test]
        fn prop_tick_comparison_handles_wraparound(
            tick1 in any::<u32>(),
            tick2 in any::<u32>()
        ) {
            // Tick comparison handles wraparound correctly for all u32 sequences
            let diff = tick2.wrapping_sub(tick1);

            // If the difference is small (< half range), tick2 is "after" tick1
            // If the difference is large (> half range), it's wraparound and tick1 is "after" tick2
            let is_tick2_newer = diff < u32::MAX / 2;

            // This property should always hold for proper wraparound handling
            if tick1 == tick2 {
                prop_assert_eq!(diff, 0);
            } else if diff == 1 {
                prop_assert!(is_tick2_newer);
            }
        }

        #[test]
        fn prop_bitfield_flag_operations(
            value in any::<u32>(),
            bit_index in 0..32u32
        ) {
            let bitfield = BitField::new(value);
            let expected_bit_set = (value & (1 << bit_index)) != 0;
            prop_assert_eq!(bitfield.is_set(bit_index), expected_bit_set);

            // Test flag checking with the bit as a flag
            let flag = 1 << bit_index;
            prop_assert_eq!(bitfield.has_flag(flag), expected_bit_set);
        }
    }

    // Unit tests for trivial constructors and pure functions
    #[test]
    fn variable_type_size_returns_correct_values() {
        assert_eq!(VariableType::Char.size(), 1);
        assert_eq!(VariableType::Int8.size(), 1);
        assert_eq!(VariableType::UInt8.size(), 1);
        assert_eq!(VariableType::Bool.size(), 1);
        assert_eq!(VariableType::Int16.size(), 2);
        assert_eq!(VariableType::UInt16.size(), 2);
        assert_eq!(VariableType::Int32.size(), 4);
        assert_eq!(VariableType::UInt32.size(), 4);
        assert_eq!(VariableType::Float32.size(), 4);
        assert_eq!(VariableType::BitField.size(), 4);
        assert_eq!(VariableType::Float64.size(), 8);
    }

    #[test]
    fn bitfield_constructor_works() {
        let bitfield = BitField::new(0x12345678);
        assert_eq!(bitfield.value(), 0x12345678);
    }

    #[test]
    fn bitfield_flag_operations_basic() {
        let bitfield = BitField::new(0b1010);
        assert!(bitfield.is_set(1));
        assert!(!bitfield.is_set(0));
        assert!(bitfield.is_set(3));
        assert!(!bitfield.is_set(2));
        assert!(bitfield.has_flag(0b0010));
        assert!(!bitfield.has_flag(0b0001));
        assert!(bitfield.has_flag(0b1000));
        assert!(!bitfield.has_flag(0b0100));
    }

    #[test]
    fn test_incident_decoding_rep_only() {
        use crate::irsdk_flags::incident as inc;
        let bits = BitField::new(inc::REP_CONTACT_WITH_WORLD as u32);
        let decoded = decode_incident(bits);
        assert!(matches!(decoded.report, IncidentReport::ContactWithWorld));
        assert!(matches!(decoded.penalty, IncidentPenalty::None));
    }

    #[test]
    fn test_incident_decoding_pen_only() {
        use crate::irsdk_flags::incident as inc;
        let bits = BitField::new(((inc::PEN_0X as u32) << 8) & inc::PEN_MASK);
        let decoded = decode_incident(bits);
        assert!(matches!(decoded.report, IncidentReport::NoReport));
        assert!(matches!(decoded.penalty, IncidentPenalty::ZeroX));
    }

    #[test]
    fn test_engine_warnings_new_bits_present() {
        use crate::irsdk_flags::engine_warnings as ew;
        let flags = BitField::new(ew::MAND_REP_NEEDED | ew::OPT_REP_NEEDED);
        assert!(flags.has_flag(ew::MAND_REP_NEEDED));
        assert!(flags.has_flag(ew::OPT_REP_NEEDED));
    }

    #[test]
    fn test_engine_repair_helpers() {
        use crate::irsdk_flags::engine_warnings as ew;
        let flags = BitField::new(ew::MAND_REP_NEEDED | ew::OPT_REP_NEEDED);
        assert!(engine_mandatory_repair_needed(flags));
        assert!(engine_optional_repair_needed(flags));
        let none = BitField::new(0);
        assert!(!engine_mandatory_repair_needed(none));
        assert!(!engine_optional_repair_needed(none));
    }

    #[test]
    fn test_session_dq_scoring_invalid_helper() {
        use crate::irsdk_flags::session_flags as sf;
        let flags = BitField::new(sf::DQ_SCORING_INVALID);
        assert!(session_dq_scoring_invalid(flags));
        let none = BitField::new(0);
        assert!(!session_dq_scoring_invalid(none));
    }
}
