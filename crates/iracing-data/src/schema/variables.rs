//! iRacing Variable Schema Parsing
//!
//! This module provides parsing and validation for iRacing's `irsdk_varHeader` structures
//! from Windows shared memory. Variable headers define the schema for all telemetry data
//! fields available in each frame.
//!
//! # iRacing Variable Header Layout
//!
//! Each variable header follows the C structure layout from the iRacing SDK:
//! ```c
//! typedef struct irsdk_varHeader
//! {
//!     int type;                           // irsdk_VarType enum value
//!     int offset;                         // offset in bytes from buffer start
//!     int count;                          // number of elements (1 for scalar)
//!     int pad;                            // padding for alignment
//!     char name[IRSDK_MAX_STRING];        // variable name (32 bytes)
//!     char desc[IRSDK_MAX_DESC];          // description (64 bytes)
//!     char unit[IRSDK_MAX_STRING];        // units (32 bytes)
//! } irsdk_varHeader;
//! ```
//!
//! # Memory Layout and Alignment
//!
//! - Each variable header: 144 bytes total
//! - String fields: null-terminated C strings with fixed-size buffers
//! - Alignment: 4-byte boundaries to match iRacing's C implementation
//! - Array location: `numVars` headers starting at `varHeaderOffset` in shared memory
//!
//! # Type Mapping
//!
//! iRacing SDK types map to our `VariableType` enum:
//! - `irsdk_char` (0) → `VariableType::Char`
//! - `irsdk_bool` (1) → `VariableType::Bool`
//! - `irsdk_int` (2) → `VariableType::Int32`
//! - `irsdk_bitField` (3) → `VariableType::BitField`
//! - `irsdk_float` (4) → `VariableType::Float32`
//! - `irsdk_double` (5) → `VariableType::Float64`
//!
//! # Schema Building Process
//!
//! 1. Use `SchemaInfo` from parsed header to locate variable definitions
//! 2. Parse `numVars` count of 144-byte variable headers from `varHeaderOffset`
//! 3. Convert C strings to Rust `String` with proper encoding handling
//! 4. Map iRacing types to `VariableType` enum with validation
//! 5. Build `HashMap<String, VariableInfo>` for O(1) variable lookup
//! 6. Validate schema consistency: uniqueness, bounds, overlaps
//!
//! # Performance Characteristics
//!
//! This implementation is optimized for <1ms latency requirement:
//! - Zero-copy string parsing where possible
//! - Pre-computed HashMap for O(1) variable lookup
//! - Comprehensive validation with early error detection
//! - Efficient memory layout matching iRacing's C structures

use crate::{Result, TelemetryError, VariableInfo, VariableSchema, VariableType};
use std::collections::HashMap;
use tracing::{debug, trace, warn};

/// Size constants matching iRacing SDK
const IRSDK_MAX_STRING: usize = 32; // For name and unit fields
const IRSDK_MAX_DESC: usize = 64; // For description field
const VAR_HEADER_SIZE: usize = std::mem::size_of::<IRSDKVarHeader>();

/// iRacing variable header structure matching the C SDK layout
#[repr(C)]
#[derive(Debug, Clone)]
struct IRSDKVarHeader {
    /// Variable type (irsdk_VarType enum)
    var_type: i32,
    /// Offset in bytes from buffer start
    offset: i32,
    /// Number of elements (1 for scalar, >1 for arrays)
    count: i32,
    /// Whether the count field should be interpreted as time
    count_as_time: u8,
    /// Padding for alignment (matches 3-byte C padding)
    pad: [u8; 3],
    /// Variable name (32 bytes, null-terminated)
    name: [u8; IRSDK_MAX_STRING],
    /// Variable description (64 bytes, null-terminated)
    desc: [u8; IRSDK_MAX_DESC],
    /// Variable units (32 bytes, null-terminated)
    unit: [u8; IRSDK_MAX_STRING],
}

/// iRacing SDK variable type constants (for reference)
///
/// These constants map to the irsdk_VarType enum values used in IBT files
/// and live telemetry. They document the numeric values found in the type
/// field of IRSDKVarHeader structs.
#[allow(dead_code)]
mod irsdk_var_type {
    pub const IRSDK_CHAR: i32 = 0;
    pub const IRSDK_BOOL: i32 = 1;
    pub const IRSDK_INT: i32 = 2;
    pub const IRSDK_BITFIELD: i32 = 3;
    pub const IRSDK_FLOAT: i32 = 4;
    pub const IRSDK_DOUBLE: i32 = 5;
}

impl IRSDKVarHeader {
    /// Parse variable header from raw memory bytes with validation
    pub fn parse_from_memory(memory: &[u8], offset: usize) -> Result<Self> {
        trace!(offset, "Parsing variable header from memory");

        // Validate we have enough bytes for a complete header
        if offset + VAR_HEADER_SIZE > memory.len() {
            return Err(TelemetryError::Memory { offset, source: None });
        }

        // Zero-copy parsing: directly read from memory
        // Safety: We've validated the memory length above and use read_unaligned for robustness
        let header = unsafe {
            std::ptr::read_unaligned(memory.as_ptr().add(offset) as *const IRSDKVarHeader)
        };

        // Validate basic header fields
        header.validate()?;

        Ok(header)
    }

    /// Validate header fields for reasonable values
    fn validate(&self) -> Result<()> {
        // iRacing reserves count >= 0 and count_as_time <= 1
        if self.count < 0 {
            return Err(TelemetryError::Parse {
                context: "Variable header validation".to_string(),
                details: format!("Negative element count: {}", self.count),
            });
        }

        if self.count_as_time > 1 {
            return Err(TelemetryError::Parse {
                context: "Variable header validation".to_string(),
                details: format!("Invalid count_as_time flag: {}", self.count_as_time),
            });
        }

        Ok(())
    }

    /// Convert C string bytes to Rust String
    fn c_string_to_string(bytes: &[u8]) -> String {
        // Find null terminator or use full length
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());

        // Convert to UTF-8, replacing invalid sequences
        String::from_utf8_lossy(&bytes[..end]).to_string()
    }

    /// Map iRacing variable type to our VariableType enum
    fn map_variable_type(irsdk_type: i32) -> VariableType {
        match irsdk_type {
            irsdk_var_type::IRSDK_CHAR => VariableType::Char,
            irsdk_var_type::IRSDK_BOOL => VariableType::Bool,
            irsdk_var_type::IRSDK_INT => VariableType::Int32,
            irsdk_var_type::IRSDK_BITFIELD => VariableType::BitField,
            irsdk_var_type::IRSDK_FLOAT => VariableType::Float32,
            irsdk_var_type::IRSDK_DOUBLE => VariableType::Float64,
            _ => {
                warn!(irsdk_type, "Unknown iRacing variable type, defaulting to Int32");
                VariableType::Int32 // Safe default for unknown types
            }
        }
    }

    /// Convert to VariableInfo for schema building
    pub fn to_variable_info(&self) -> VariableInfo {
        VariableInfo {
            name: Self::c_string_to_string(&self.name),
            data_type: Self::map_variable_type(self.var_type),
            offset: self.offset as usize,
            count: self.count as usize,
            count_as_time: self.count_as_time(),
            units: Self::c_string_to_string(&self.unit),
            description: Self::c_string_to_string(&self.desc),
        }
    }

    /// Indicates whether the variable count should be treated as elapsed time
    fn count_as_time(&self) -> bool {
        self.count_as_time != 0
    }
}

/// Parse variable schema from shared memory using header information
pub fn parse_variable_schema(
    memory: &[u8],
    num_vars: i32,
    var_header_offset: i32,
    buffer_length: i32,
) -> Result<VariableSchema> {
    debug!(num_vars, var_header_offset, buffer_length, "Parsing variable schema from memory");

    // Validate input parameters
    if num_vars <= 0 {
        return Err(TelemetryError::Parse {
            context: "Schema parsing".to_string(),
            details: format!("Invalid variable count: {}", num_vars),
        });
    }

    if var_header_offset < 0 {
        return Err(TelemetryError::Parse {
            context: "Schema parsing".to_string(),
            details: format!("Invalid variable header offset: {}", var_header_offset),
        });
    }

    // Calculate total size needed for all variable headers
    let total_headers_size = (num_vars as usize) * VAR_HEADER_SIZE;
    let headers_start = var_header_offset as usize;
    let headers_end = headers_start + total_headers_size;

    // Validate memory bounds
    if headers_end > memory.len() {
        return Err(TelemetryError::Memory { offset: headers_end, source: None });
    }

    // Parse all variable headers
    let mut variables = HashMap::with_capacity(num_vars as usize);
    let mut failed_count = 0;

    for i in 0..num_vars {
        let header_offset = headers_start + (i as usize * VAR_HEADER_SIZE);

        match IRSDKVarHeader::parse_from_memory(memory, header_offset) {
            Ok(var_header) => {
                let var_info = var_header.to_variable_info();

                // Skip variables with empty names or invalid properties (common with padding/unused slots)
                if var_info.name.is_empty() || var_info.count == 0 {
                    continue;
                }

                // Check for duplicate names
                if variables.contains_key(&var_info.name) {
                    warn!(name = %var_info.name, "Duplicate variable name found");
                }

                variables.insert(var_info.name.clone(), var_info);
            }
            Err(e) => {
                failed_count += 1;
                warn!(
                    error = %e,
                    header_index = i,
                    "Failed to parse variable header, skipping"
                );
                continue;
            }
        }
    }

    if failed_count > 0 {
        warn!(failed_count, total = num_vars, "Some variable headers failed to parse");
    }

    debug!(parsed_count = variables.len(), expected_count = num_vars, "Variable parsing completed");

    // Build schema with validation
    let schema = VariableSchema::new(variables, buffer_length as usize)?;

    Ok(schema)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::mem;

    // Compile-time size verification
    const _: () = assert!(mem::size_of::<IRSDKVarHeader>() == VAR_HEADER_SIZE);

    #[test]
    fn variable_header_size_matches_expected_layout() {
        assert_eq!(mem::size_of::<IRSDKVarHeader>(), 144);

        // Verify field alignment matches C struct
        let header = IRSDKVarHeader {
            var_type: 0,
            offset: 0,
            count: 0,
            count_as_time: 0,
            pad: [0; 3],
            name: [0; IRSDK_MAX_STRING],
            desc: [0; IRSDK_MAX_DESC],
            unit: [0; IRSDK_MAX_STRING],
        };

        // Check field offsets using pointer arithmetic
        let base_ptr = &header as *const _ as usize;
        let type_offset = (&header.var_type as *const _ as usize) - base_ptr;
        let offset_offset = (&header.offset as *const _ as usize) - base_ptr;
        let count_offset = (&header.count as *const _ as usize) - base_ptr;
        let name_offset = (&header.name as *const _ as usize) - base_ptr;
        let desc_offset = (&header.desc as *const _ as usize) - base_ptr;
        let unit_offset = (&header.unit as *const _ as usize) - base_ptr;

        assert_eq!(type_offset, 0);
        assert_eq!(offset_offset, 4);
        assert_eq!(count_offset, 8);
        assert_eq!(name_offset, 16);
        assert_eq!(desc_offset, 48);
        assert_eq!(unit_offset, 112);
    }

    #[test]
    fn c_string_conversion_works() {
        // Test normal string
        let test_bytes = b"RPM\0\0\0\0";
        let result = IRSDKVarHeader::c_string_to_string(test_bytes);
        assert_eq!(result, "RPM");

        // Test string without null terminator
        let test_bytes = b"Speed";
        let result = IRSDKVarHeader::c_string_to_string(test_bytes);
        assert_eq!(result, "Speed");

        // Test empty string
        let test_bytes = b"\0\0\0\0";
        let result = IRSDKVarHeader::c_string_to_string(test_bytes);
        assert_eq!(result, "");
    }

    #[test]
    fn variable_type_mapping_works() {
        assert_eq!(IRSDKVarHeader::map_variable_type(0), VariableType::Char);
        assert_eq!(IRSDKVarHeader::map_variable_type(1), VariableType::Bool);
        assert_eq!(IRSDKVarHeader::map_variable_type(2), VariableType::Int32);
        assert_eq!(IRSDKVarHeader::map_variable_type(3), VariableType::BitField);
        assert_eq!(IRSDKVarHeader::map_variable_type(4), VariableType::Float32);
        assert_eq!(IRSDKVarHeader::map_variable_type(5), VariableType::Float64);

        // Unknown types default to Int32
        assert_eq!(IRSDKVarHeader::map_variable_type(99), VariableType::Int32);
    }

    #[test]
    fn insufficient_memory_returns_error() {
        let small_memory = vec![0u8; 100]; // Less than 144 bytes needed
        let result = IRSDKVarHeader::parse_from_memory(&small_memory, 0);
        assert!(result.is_err());
    }

    // Property test strategies for generating valid variable headers
    prop_compose! {
        fn arb_valid_var_header()(
            name in "[a-zA-Z][a-zA-Z0-9_]*",
            desc in "[a-zA-Z0-9 _-]*",
            unit in "[a-zA-Z0-9/*^-]*",
            var_type in 0..6i32,
            offset in 0..100000i32,
            count in 1..64i32,
            count_as_time in prop::bool::ANY
        ) -> IRSDKVarHeader {
            let mut header = IRSDKVarHeader {
                var_type,
                offset,
                count,
                count_as_time: count_as_time as u8,
                pad: [0; 3],
                name: [0; IRSDK_MAX_STRING],
                desc: [0; IRSDK_MAX_DESC],
                unit: [0; IRSDK_MAX_STRING],
            };

            // Copy strings with null termination
            let name_bytes = name.as_bytes();
            let len = name_bytes.len().min(IRSDK_MAX_STRING - 1);
            header.name[..len].copy_from_slice(&name_bytes[..len]);

            let desc_bytes = desc.as_bytes();
            let len = desc_bytes.len().min(IRSDK_MAX_DESC - 1);
            header.desc[..len].copy_from_slice(&desc_bytes[..len]);

            let unit_bytes = unit.as_bytes();
            let len = unit_bytes.len().min(IRSDK_MAX_STRING - 1);
            header.unit[..len].copy_from_slice(&unit_bytes[..len]);

            header
        }
    }

    prop_compose! {
        fn arb_corrupted_var_header()(
            var_type in (6..100i32).prop_union(-100..0i32),
            offset in i32::MIN..0,
            count in i32::MIN..0,
            count_as_time in 2..=u8::MAX as i32
        ) -> IRSDKVarHeader {
            IRSDKVarHeader {
                var_type,
                offset,
                count,
                count_as_time: count_as_time as u8,
                pad: [0; 3],
                name: [0; IRSDK_MAX_STRING],
                desc: [0; IRSDK_MAX_DESC],
                unit: [0; IRSDK_MAX_STRING],
            }
        }
    }

    // Property tests for comprehensive validation
    proptest! {
        #[test]
        fn prop_variable_parsing_from_generated_headers(
            header in arb_valid_var_header()
        ) {
            // Convert header to bytes
            let header_bytes = unsafe {
                std::slice::from_raw_parts(
                    &header as *const _ as *const u8,
                    VAR_HEADER_SIZE
                )
            };

            // Parsing should succeed for valid headers
            let parsed = IRSDKVarHeader::parse_from_memory(header_bytes, 0);
            prop_assert!(parsed.is_ok());

            // Convert to VariableInfo and validate
            let var_info = header.to_variable_info();
            prop_assert!(!var_info.name.is_empty());
            prop_assert!(var_info.count > 0);
        }

        #[test]
        fn prop_corrupted_headers_handled_gracefully(
            header in arb_corrupted_var_header()
        ) {
            // Convert header to bytes
            let header_bytes = unsafe {
                std::slice::from_raw_parts(
                    &header as *const _ as *const u8,
                    VAR_HEADER_SIZE
                )
            };

            // Parsing corrupted headers should either fail validation or handle gracefully
            let parsed = IRSDKVarHeader::parse_from_memory(header_bytes, 0);
            if let Ok(parsed_header) = parsed {
                // If parsing succeeded, conversion to VariableInfo should work
                let var_info = parsed_header.to_variable_info();
                // Unknown types should default to Int32
                let is_known_type = matches!(header.var_type, 0..=5);
                if !is_known_type {
                    prop_assert_eq!(var_info.data_type, VariableType::Int32);
                }
            }
            // Otherwise, validation should have caught the error
        }

        #[test]
        fn prop_all_irsdk_types_map_correctly(
            irsdk_type in 0..6i32
        ) {
            let mapped_type = IRSDKVarHeader::map_variable_type(irsdk_type);

            // All valid iRacing types should map to known VariableType variants
            match irsdk_type {
                0 => prop_assert_eq!(mapped_type, VariableType::Char),
                1 => prop_assert_eq!(mapped_type, VariableType::Bool),
                2 => prop_assert_eq!(mapped_type, VariableType::Int32),
                3 => prop_assert_eq!(mapped_type, VariableType::BitField),
                4 => prop_assert_eq!(mapped_type, VariableType::Float32),
                5 => prop_assert_eq!(mapped_type, VariableType::Float64),
                _ => panic!("Invalid irsdk_type {} outside valid range 0-5", irsdk_type),
            }
        }

        #[test]
        fn prop_schema_building_with_valid_variables(
            var_count in 1..100usize,
            buffer_len in 1000..50000i32
        ) {
            // Create valid variable headers
            let mut memory = Vec::new();
            let header_offset = 1000; // Start headers after some offset

            // Add padding before headers
            memory.resize(header_offset, 0);

            for i in 0..var_count {
                let header = IRSDKVarHeader {
                    var_type: 2, // Int32
                    offset: (i * 4) as i32, // Non-overlapping offsets
                    count: 1,
                    count_as_time: 0,
                    pad: [0; 3],
                    name: {
                        let mut name = [0; IRSDK_MAX_STRING];
                        let name_str = format!("Var{}", i);
                        let name_bytes = name_str.as_bytes();
                        let len = name_bytes.len().min(IRSDK_MAX_STRING - 1);
                        name[..len].copy_from_slice(&name_bytes[..len]);
                        name
                    },
                    desc: [0; IRSDK_MAX_DESC],
                    unit: [0; IRSDK_MAX_STRING],
                };

                let header_bytes = unsafe {
                    std::slice::from_raw_parts(
                        &header as *const _ as *const u8,
                        VAR_HEADER_SIZE
                    )
                };
                memory.extend_from_slice(header_bytes);
            }

            // Parse schema
            let result = parse_variable_schema(
                &memory,
                var_count as i32,
                header_offset as i32,
                buffer_len
            );

            prop_assert!(result.is_ok());
            let schema = result.unwrap();
            prop_assert_eq!(schema.variable_count(), var_count);
            prop_assert_eq!(schema.frame_size, buffer_len as usize);
        }
    }

    #[test]
    #[cfg(feature = "benchmark")]
    fn benchmark_variable_schema_parsing_performance() {
        use std::time::Instant;

        // Create realistic test data based on live iRacing structure
        let num_vars = 331;
        let var_header_offset = 524400;
        let buffer_length = 7817;

        // Build memory with realistic variable headers
        let mut memory = vec![0u8; 2_000_000]; // Large enough buffer

        // Create test variable headers with realistic data
        for i in 0..num_vars {
            let header_offset = var_header_offset + (i * VAR_HEADER_SIZE);

            let var_header = IRSDKVarHeader {
                var_type: match i % 5 {
                    0 => 4, // Float32 (most common)
                    1 => 2, // Int32
                    2 => 1, // Bool
                    3 => 3, // BitField
                    4 => 5, // Float64
                    _ => 4, // Float32 default
                },
                offset: (i * 4) as i32, // 4-byte spacing
                count: if i < 45 {
                    match i % 3 {
                        0 => 64, // Car arrays
                        1 => 6,  // Suspension arrays
                        _ => 1,  // Scalar
                    }
                } else {
                    1
                }, // Most are scalar
                count_as_time: if i % 11 == 0 { 1 } else { 0 },
                pad: [0; 3],
                name: {
                    let mut name = [0; IRSDK_MAX_STRING];
                    let name_str = match i % 10 {
                        0 => "SessionTime",
                        1 => "RPM",
                        2 => "Speed",
                        3 => "Gear",
                        4 => "CarIdxF2Time",
                        5 => "LFshockDefl",
                        6 => "SessionFlags",
                        7 => "PitsOpen",
                        8 => "LapDeltaToBestLap",
                        _ => "TestVariable",
                    };
                    let full_name = format!("{}{}", name_str, i);
                    let name_bytes = full_name.as_bytes();
                    let len = name_bytes.len().min(IRSDK_MAX_STRING - 1);
                    name[..len].copy_from_slice(&name_bytes[..len]);
                    name
                },
                desc: {
                    let mut desc = [0; IRSDK_MAX_DESC];
                    let desc_str = "Test variable description";
                    let desc_bytes = desc_str.as_bytes();
                    let len = desc_bytes.len().min(IRSDK_MAX_DESC - 1);
                    desc[..len].copy_from_slice(&desc_bytes[..len]);
                    desc
                },
                unit: {
                    let mut unit = [0; IRSDK_MAX_STRING];
                    let unit_str = match i % 4 {
                        0 => "s",   // seconds
                        1 => "rpm", // revolutions per minute
                        2 => "m/s", // meters per second
                        _ => "n/a", // no unit
                    };
                    let unit_bytes = unit_str.as_bytes();
                    let len = unit_bytes.len().min(IRSDK_MAX_STRING - 1);
                    unit[..len].copy_from_slice(&unit_bytes[..len]);
                    unit
                },
            };

            // Write header to memory
            unsafe {
                let header_ptr = memory.as_mut_ptr().add(header_offset) as *mut IRSDKVarHeader;
                std::ptr::write_unaligned(header_ptr, var_header);
            }
        }

        // Warm up the parsing code
        for _ in 0..10 {
            let _ = parse_variable_schema(
                &memory,
                num_vars as i32,
                var_header_offset as i32,
                buffer_length,
            );
        }

        // Benchmark schema parsing performance
        const NUM_ITERATIONS: usize = 1000;
        let start = Instant::now();

        for _ in 0..NUM_ITERATIONS {
            let _ = parse_variable_schema(
                &memory,
                num_vars as i32,
                var_header_offset as i32,
                buffer_length,
            )
            .expect("Schema parsing should succeed");
        }

        let elapsed = start.elapsed();
        let avg_duration_nanos = elapsed.as_nanos() as f64 / NUM_ITERATIONS as f64;
        let avg_duration_micros = avg_duration_nanos / 1000.0;

        println!(
            "Variable schema parsing performance: avg {:.2}ns ({:.3}μs) per parse, {} iterations",
            avg_duration_nanos, avg_duration_micros, NUM_ITERATIONS
        );

        // Verify <1ms latency requirement (1ms = 1,000,000ns)
        assert!(
            avg_duration_nanos < 1_000_000.0,
            "Variable schema parsing should be <1ms, got {:.2}ns",
            avg_duration_nanos
        );

        // For 60Hz updates, we need much faster than 16.67ms per frame
        // Schema parsing should be fast enough to not impact telemetry loop
        // Allow up to 750μs to account for system variance while staying well under 1ms
        assert!(
            avg_duration_nanos < 750_000.0, // <750μs for good headroom with system variance
            "Variable schema parsing should be <750μs for 60Hz updates, got {:.2}ns",
            avg_duration_nanos
        );

        // Performance target: schema parsing should be orders of magnitude faster than frame rate
        if avg_duration_nanos < 100_000.0 {
            println!(
                "✅ Excellent performance: {}x faster than 100μs target",
                100_000.0 / avg_duration_nanos
            );
        } else {
            println!("⚠️  Performance acceptable but could be improved");
        }
    }
}
