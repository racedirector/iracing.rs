//! IBT file format structures and parsing
//!
//! Defines the binary structures used in iRacing's IBT file format
//! and provides parsing functions for cross-platform file reading.
//!
//! ## IBT File Structure
//!
//! IBT (iRacing Binary Telemetry) files contain recorded telemetry data from iRacing sessions:
//!
//! 1. **Main Header** (144 bytes) - `irsdk_header` compatible structure
//! 2. **Disk Sub-Header** (32 bytes) - IBT-specific metadata with timing and record counts
//! 3. **Session Info** - YAML session configuration (optional)
//! 4. **Variable Headers** - Array of variable definitions
//! 5. **Frame Data** - Sequential telemetry samples
//!
//! ## Performance Characteristics
//!
//! - Binary parsing with explicit little-endian byte order handling
//! - Bounds checking for all memory operations
//! - Minimal memory allocations during header parsing
//! - O(1) schema validation after parsing

use crate::{
    IRacingSDKError, Result, VariableInfo, VariableSchema, VariableType,
    irsdk::constants::{IRSDK_MAX_DESC, IRSDK_MAX_STRING},
    irsdk::{DiskSubHeader, Header, VariableHeader, WireType},
};
use std::collections::HashMap;
use std::io::{Read, Seek};

fn header_validation_error(details: impl Into<String>) -> IRacingSDKError {
    IRacingSDKError::parse_error("Header validation", details)
}

/// Extract variable schema from IBT file headers
pub fn extract_variable_schema<R: Read + Seek>(
    reader: &mut R,
    header: &Header,
) -> Result<VariableSchema> {
    tracing::debug!(
        "Extracting variable schema for {} variables",
        header.variable_count
    );
    // Handle IBT files with no telemetry data frames (bufLen = 0)
    if header.buffer_length == 0 || header.variable_count <= 0 {
        // File contains only session info, no telemetry data
        return VariableSchema::new(HashMap::new(), 0);
    }

    // Seek to the variable headers section and parse all variables
    reader
        .seek(std::io::SeekFrom::Start(
            header.variable_header_offset as u64,
        ))
        .map_err(|e| IRacingSDKError::Parse {
            context: "Variable headers seek".to_string(),
            details: format!(
                "Failed to seek to variable headers at offset {}: {}",
                header.variable_header_offset, e
            ),
        })?;

    // Convert num_vars to usize upfront to avoid i32-typed ranges
    let num_vars_usize =
        usize::try_from(header.variable_count).map_err(|_| IRacingSDKError::Parse {
            context: "Variable count conversion".to_string(),
            details: format!(
                "Number of variables {} cannot be converted to usize",
                header.variable_count
            ),
        })?;

    // Pre-allocate HashMap to minimize reallocation
    let mut variables = HashMap::with_capacity(num_vars_usize);

    // Parse each variable header
    for i in 0..num_vars_usize {
        let mut var_header_bytes = [0u8; VariableHeader::WIRE_SIZE];
        reader
            .read_exact(&mut var_header_bytes)
            .map_err(|e| IRacingSDKError::Parse {
                context: format!("Variable header {} reading", i),
                details: format!("Failed to read variable header {}: {}", i, e),
            })?;

        // Parse variable header fields
        let var_type = parse_i32_le(&var_header_bytes, 0)?;
        let offset = parse_i32_le(&var_header_bytes, 4)?;
        let count = parse_i32_le(&var_header_bytes, 8)?;

        // Extract null-terminated strings using constants for offsets
        let name = extract_null_terminated_string(&var_header_bytes[16..16 + IRSDK_MAX_STRING]);
        let desc = extract_null_terminated_string(&var_header_bytes[48..48 + IRSDK_MAX_DESC]);
        let unit = extract_null_terminated_string(&var_header_bytes[112..112 + IRSDK_MAX_STRING]);
        let count_as_time = var_header_bytes[12] != 0;

        // Skip empty or invalid variables
        if name.is_empty() || offset < 0 || count <= 0 {
            continue;
        }

        // Convert iRacing var type to our VariableType
        let data_type = match var_type {
            0 => VariableType::Char,     // char
            1 => VariableType::Bool,     // bool
            2 => VariableType::Int32,    // int
            3 => VariableType::BitField, // bitField (treat as int32)
            4 => VariableType::Float32,  // float
            5 => VariableType::Float64,  // double
            _ => {
                // Log unknown types for diagnostics
                tracing::debug!(
                    "Skipping variable '{}' with unknown type {}",
                    name,
                    var_type
                );
                continue;
            }
        };

        variables.insert(
            name.clone(),
            VariableInfo {
                name,
                data_type,
                offset: offset as usize,
                count: count as usize,
                count_as_time,
                units: unit,
                description: desc,
            },
        );
    }

    tracing::debug!(
        "Extracted {} variables with frame size {}",
        variables.len(),
        header.buffer_length
    );
    VariableSchema::new(variables, header.buffer_length as usize)
}

/// Verify that the IBT file length is at least large enough to contain headers and all records
/// This is a conservative lower bound based on header values; it does not validate exact layout
pub fn verify_min_length(file_len: u64, header: &Header, disk: &DiskSubHeader) -> Result<()> {
    // Basic lower bound: var headers + frames
    let var_headers_len =
        (header.variable_count as u64).saturating_mul(VariableHeader::WIRE_SIZE as u64);
    let frames_len = (disk.record_count as u64).saturating_mul(header.buffer_length as u64);
    // Start position is var_header_offset; add var headers and frames
    let min_end = (header.variable_header_offset as u64)
        .saturating_add(var_headers_len)
        .saturating_add(frames_len);

    if file_len < min_end {
        return Err(IRacingSDKError::Parse {
            context: "IBT length verification".to_string(),
            details: format!(
                "File too small: len={} < required_min={} (vars={}, records={}, buf_len={})",
                file_len, min_end, header.variable_count, disk.record_count, header.buffer_length
            ),
        });
    }
    Ok(())
}

/// Safe byte parsing helpers with bounds checking
fn parse_i32_le(data: &[u8], offset: usize) -> Result<i32> {
    if offset + 4 > data.len() {
        return Err(IRacingSDKError::Parse {
            context: "Integer parsing".to_string(),
            details: format!(
                "Insufficient data for i32 at offset {} (need 4 bytes, have {})",
                offset,
                data.len() - offset
            ),
        });
    }
    Ok(i32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

/// Extract null-terminated string from byte slice
fn extract_null_terminated_string(bytes: &[u8]) -> String {
    let null_pos = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..null_pos]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_utils::{
        IbtFixture, IbtVariableManifest, load_fixture_manifest, require_smallest_ibt_fixture,
    };
    use anyhow::{Context, Result, ensure};
    use std::fs::File;
    use std::path::Path;

    fn open_buf_reader(path: &Path) -> Result<std::io::BufReader<File>> {
        let file = File::open(path).with_context(|| format!("Opening {}", path.display()))?;
        Ok(std::io::BufReader::new(file))
    }

    fn variable_type(expected: &str) -> VariableType {
        match expected {
            "Char" => VariableType::Char,
            "Bool" => VariableType::Bool,
            "Int32" => VariableType::Int32,
            "BitField" => VariableType::BitField,
            "Float32" => VariableType::Float32,
            "Float64" => VariableType::Float64,
            other => panic!("Unsupported manifest variable type: {}", other),
        }
    }

    fn assert_required_variable(actual: &VariableInfo, expected: &IbtVariableManifest) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.data_type, variable_type(&expected.data_type));
        assert_eq!(actual.offset, expected.offset);
        assert_eq!(actual.count, expected.count);
        assert_eq!(actual.units, expected.units);
    }

    fn parse_fixture(fixture: &IbtFixture) -> Result<(Header, DiskSubHeader, VariableSchema)> {
        let file_path = fixture.fixture_path()?;
        let mut reader = open_buf_reader(&file_path)?;

        let header = Header::try_from_reader(&mut reader)
            .with_context(|| format!("Parsing header from {}", file_path.display()))?;

        let disk_header = DiskSubHeader::try_from_reader(&mut reader)
            .with_context(|| format!("Parsing disk sub-header from {}", file_path.display()))?;

        let schema = extract_variable_schema(&mut reader, &header)
            .with_context(|| format!("Extracting variable schema from {}", file_path.display()))?;
        Ok((header, disk_header, schema))
    }

    #[test]
    fn test_generated_fixture_headers_match_manifest() -> Result<()> {
        use crate::types::StatusField;

        let manifest = load_fixture_manifest()?;
        assert_eq!(manifest.layout.live_header_prefix_size, Header::WIRE_SIZE);
        assert_eq!(
            manifest.layout.ibt_header_size,
            Header::WIRE_SIZE + DiskSubHeader::WIRE_SIZE
        );
        assert_eq!(
            manifest.layout.disk_sub_header_size,
            DiskSubHeader::WIRE_SIZE
        );
        assert_eq!(
            manifest.layout.variable_header_size,
            VariableHeader::WIRE_SIZE
        );

        for fixture in &manifest.fixtures {
            let (header, disk_header, _) = parse_fixture(fixture)?;

            assert_eq!(header.version, 2);
            assert_eq!(header.status, StatusField::CONNECTED);
            assert_eq!(header.tick_rate, fixture.tick_rate);
            assert_eq!(header.variable_count, fixture.num_vars);
            assert_eq!(header.variable_header_offset, fixture.var_header_offset);
            assert_eq!(header.variable_header_offset, 144);
            assert_eq!(header.buffer_length, fixture.frame_size as i32);
            assert_eq!(header.buffer_count, fixture.num_buf);
            assert_eq!(header.session_info_len, fixture.session_info_len);
            assert_eq!(header.session_info_offset, fixture.session_info_offset);
            assert_eq!(header.session_info_update, fixture.session_info_update);

            assert_eq!(
                fixture.disk_sub_header_offset,
                header.variable_header_offset - DiskSubHeader::WIRE_SIZE as i32
            );
            assert_eq!(disk_header.start_date, fixture.disk_header.start_date);
            assert!((disk_header.start_time - fixture.disk_header.start_time).abs() < f64::EPSILON);
            assert!((disk_header.end_time - fixture.disk_header.end_time).abs() < f64::EPSILON);
            assert_eq!(disk_header.lap_count, fixture.disk_header.lap_count);
            assert_eq!(disk_header.record_count, fixture.disk_header.record_count);

            header.validate()?;
        }

        Ok(())
    }

    #[test]
    fn test_generated_fixture_variables_match_manifest() -> Result<()> {
        let manifest = load_fixture_manifest()?;

        for fixture in &manifest.fixtures {
            let (_, _, schema) = parse_fixture(fixture)?;

            assert_eq!(schema.frame_size, fixture.frame_size);
            assert_eq!(schema.variable_count(), fixture.num_vars as usize);

            for expected in &fixture.required_variables {
                let actual = schema.variables.get(&expected.name).with_context(|| {
                    format!(
                        "Fixture {} missing variable {}",
                        fixture.name, expected.name
                    )
                })?;
                assert_required_variable(actual, expected);
            }
        }

        Ok(())
    }

    #[test]
    fn test_generated_fixture_frames_match_manifest() -> Result<()> {
        let manifest = load_fixture_manifest()?;

        for fixture in &manifest.fixtures {
            let file_path = fixture.fixture_path()?;
            let reader = crate::ibt::IbtReader::open(&file_path)
                .with_context(|| format!("Opening {}", file_path.display()))?;
            ensure!(
                reader.total_frames() > 0,
                "Fixture should contain telemetry frames"
            );
            assert_eq!(reader.total_frames(), fixture.num_frames);
            assert_eq!(reader.tick_rate(), fixture.tick_rate as f64);
        }

        Ok(())
    }

    #[test]
    fn test_generated_fixture_profiles_cover_increasing_shapes() -> Result<()> {
        let manifest = load_fixture_manifest()?;
        ensure!(
            manifest.fixtures.len() == 3,
            "Expected exactly three generated IBT fixtures"
        );

        let small = manifest
            .fixtures
            .iter()
            .find(|fixture| fixture.name == "profile_small")
            .context("Missing profile_small fixture")?;
        let medium = manifest
            .fixtures
            .iter()
            .find(|fixture| fixture.name == "profile_medium")
            .context("Missing profile_medium fixture")?;
        let large = manifest
            .fixtures
            .iter()
            .find(|fixture| fixture.name == "profile_large")
            .context("Missing profile_large fixture")?;

        assert!(small.num_vars < medium.num_vars);
        assert!(medium.num_vars < large.num_vars);
        assert!(small.frame_size < medium.frame_size);
        assert!(medium.frame_size < large.frame_size);
        assert!(small.num_frames < medium.num_frames);
        assert!(medium.num_frames < large.num_frames);

        Ok(())
    }

    #[test]
    fn test_truncated_file_handling() {
        let truncated_data = vec![0u8; 10];
        let mut cursor = std::io::Cursor::new(truncated_data);
        let result = Header::try_from_reader(&mut cursor);

        assert!(result.is_err());
        match result.unwrap_err() {
            IRacingSDKError::Parse { .. } => {}
            other => panic!("Expected Parse error, got {:?}", other),
        }
    }

    #[test]
    fn test_invalid_version_handling() -> Result<()> {
        let test_file = require_smallest_ibt_fixture()?;
        let mut data = std::fs::read(&test_file)
            .with_context(|| format!("Reading {}", test_file.display()))?;

        // Corrupt version to 999
        data[0..4].copy_from_slice(&999i32.to_le_bytes());

        let mut cursor = std::io::Cursor::new(data);

        let header_result = Header::try_from_reader(&mut cursor);

        if let Ok(header) = header_result {
            let result = header.validate();
            assert!(matches!(
                result.unwrap_err(),
                IRacingSDKError::Version { .. }
            ));
        }

        Ok(())
    }

    #[test]
    fn test_disk_length_verification_ok() -> Result<()> {
        use std::fs::metadata;
        let file_path = require_smallest_ibt_fixture()?;
        let file_len = metadata(&file_path)?.len();
        let mut reader = open_buf_reader(&file_path)?;
        let header = Header::try_from_reader(&mut reader)
            .with_context(|| format!("Parsing header from {}", file_path.display()))?;

        let disk = DiskSubHeader::try_from_reader(&mut reader)
            .with_context(|| format!("Parsing disk sub-header from {}", file_path.display()))?;

        super::verify_min_length(file_len, &header, &disk)?;
        Ok(())
    }

    #[test]
    fn test_disk_length_verification_truncated() -> Result<()> {
        let file_path = require_smallest_ibt_fixture()?;
        let mut reader = open_buf_reader(&file_path)?;
        let header = Header::try_from_reader(&mut reader)?;
        let disk = DiskSubHeader::try_from_reader(&mut reader)?;
        let result = super::verify_min_length(0, &header, &disk);
        assert!(result.is_err());
        Ok(())
    }
}
