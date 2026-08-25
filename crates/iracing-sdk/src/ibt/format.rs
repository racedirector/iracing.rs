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
    IRacingSDKError, Result, VariableHeadersBuffer, VariableSchema, VariablesHashMap,
    types::{DiskSubHeader, Header},
};
use crate::{VariableHeader, WireType};
use std::io::{Read, Seek, SeekFrom};

/// Size in bytes of a single variable header entry (`irsdk_varHeader`).
pub const IRSDK_VAR_HEADER_SIZE: usize = 144;

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
        return Ok(VariableSchema::default());
    }

    let offset = u64::try_from(header.variable_header_offset).map_err(|_| {
        IRacingSDKError::parse_error(
            "Schema parse",
            format!(
                "Variable header offset {} cannot be converted to u64",
                header.variable_header_offset
            ),
        )
    })?;
    let count = usize::try_from(header.variable_count).map_err(|_| IRacingSDKError::Parse {
        context: "Variable count conversion".to_string(),
        details: format!(
            "Number of variables {} cannot be converted to usize",
            header.variable_count
        ),
    })?;

    let length =
        count
            .checked_mul(VariableHeader::WIRE_SIZE)
            .ok_or(IRacingSDKError::parse_error(
                "Schema parse",
                "Invalid length for variables buffer",
            ))?;

    reader.seek(SeekFrom::Start(offset)).map_err(|error| {
        IRacingSDKError::parse_error(
            "Schema parse",
            format!("Could not seek to variable headers at {offset}: {error}"),
        )
    })?;

    let mut bytes = vec![0u8; length];
    reader.read_exact(&mut bytes).map_err(|error| {
        IRacingSDKError::parse_error(
            "Schema parse",
            format!("Could not read variable-header snapshot: {error}"),
        )
    })?;

    let variable_headers = VariableHeadersBuffer::from_snapshot(bytes);
    let variables: VariablesHashMap = variable_headers.try_into()?;
    let frame_size = usize::try_from(header.buffer_length).map_err(|_| {
        IRacingSDKError::parse_error(
            "Schema parse",
            format!(
                "Frame size {} cannot be converted to usize",
                header.buffer_length
            ),
        )
    })?;

    VariableSchema::new(variables, frame_size)
}

/// Verify that the IBT file length is at least large enough to contain headers and all records
/// This is a conservative lower bound based on header values; it does not validate exact layout
pub fn verify_min_length(file_len: u64, header: &Header, disk: &DiskSubHeader) -> Result<()> {
    // Basic lower bound: var headers + frames
    let var_headers_len =
        (header.variable_count as u64).saturating_mul(IRSDK_VAR_HEADER_SIZE as u64);
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_utils::{
        IbtFixture, IbtVariableManifest, load_fixture_manifest, require_smallest_ibt_fixture,
    };
    use crate::types::{IbtHeader, WireType, irsdk::StatusField};
    use crate::{VariableHeader, VariableInfo, VariableType};
    use anyhow::{Context, Result, ensure};
    use std::fs::File;
    use std::path::Path;

    fn open_buf_reader(path: &Path) -> Result<std::io::BufReader<File>> {
        let file = File::open(path).with_context(|| format!("Opening {}", path.display()))?;
        Ok(std::io::BufReader::new(file))
    }

    fn variable_type(expected: &str) -> VariableType {
        match expected {
            "Char" => VariableType::Character,
            "Bool" => VariableType::Boolean,
            "Int32" => VariableType::Integer,
            "BitField" => VariableType::BitField,
            "Float32" => VariableType::Float,
            "Float64" => VariableType::Double,
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

    fn parse_fixture(fixture: &IbtFixture) -> Result<(IbtHeader, VariableSchema)> {
        let file_path = fixture.fixture_path()?;
        let mut reader = open_buf_reader(&file_path)?;

        let ibt_header = IbtHeader::try_from_reader(&mut reader)
            .with_context(|| format!("Parsing header from {}", file_path.display()))?;

        let schema = extract_variable_schema(&mut reader, &ibt_header.header)
            .with_context(|| format!("Extracting variable schema from {}", file_path.display()))?;
        Ok((ibt_header, schema))
    }

    #[test]
    fn test_generated_fixture_headers_match_manifest() -> Result<()> {
        let manifest = load_fixture_manifest()?;
        assert_eq!(manifest.layout.live_header_prefix_size, Header::WIRE_SIZE);
        assert_eq!(manifest.layout.ibt_header_size, IbtHeader::SIZE);
        assert_eq!(
            manifest.layout.disk_sub_header_size,
            DiskSubHeader::WIRE_SIZE
        );
        assert_eq!(
            manifest.layout.variable_header_size,
            VariableHeader::WIRE_SIZE
        );

        for fixture in &manifest.fixtures {
            let (IbtHeader { header, sub_header }, _) = parse_fixture(fixture)?;

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
            assert_eq!(sub_header.start_date, fixture.disk_header.start_date);
            assert!((sub_header.start_time - fixture.disk_header.start_time).abs() < f64::EPSILON);
            assert!((sub_header.end_time - fixture.disk_header.end_time).abs() < f64::EPSILON);
            assert_eq!(sub_header.lap_count, fixture.disk_header.lap_count);
            assert_eq!(sub_header.record_count, fixture.disk_header.record_count);

            header.validate()?;
        }

        Ok(())
    }

    #[test]
    fn test_generated_fixture_variables_match_manifest() -> Result<()> {
        let manifest = load_fixture_manifest()?;

        for fixture in &manifest.fixtures {
            let (_, schema) = parse_fixture(fixture)?;

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
