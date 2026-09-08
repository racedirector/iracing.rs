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
    IRacingSDKError, Result, VariableHeadersBuffer, VariableSchema, irsdk::Header,
    types::VariableHeaderRegion,
};

use std::io::{Read, Seek, SeekFrom};

/// Extract variable schema from IBT file headers
pub fn extract_variable_schema<R: Read + Seek>(
    reader: &mut R,
    header: &Header,
) -> Result<VariableSchema> {
    tracing::debug!(
        "Extracting variable schema for {} variables",
        header.variable_count
    );

    let region = VariableHeaderRegion::try_from(header)?;

    let frame_size = usize::try_from(header.buffer_length).map_err(|_| {
        IRacingSDKError::parse_error(
            "Variable headers parse",
            "Could not parse buffer_length to usize",
        )
    })?;

    // Seek to the variable headers section and parse all variables
    reader
        .seek(SeekFrom::Start(region.offset() as u64))
        .map_err(|e| {
            IRacingSDKError::parse_error(
                "Variable headers seek".to_string(),
                format!(
                    "Failed to seek to variable headers at offset {}: {}",
                    region.offset(),
                    e
                ),
            )
        })?;

    let mut bytes = vec![0; region.length()];

    reader.read_exact(&mut bytes).map_err(|e| {
        IRacingSDKError::parse_error(
            "Variable Headers read",
            format!("Failed to read headers: {}", e),
        )
    })?;

    let headers = VariableHeadersBuffer::from_owned_checked_region(bytes);

    VariableSchema::from_headers(&headers, frame_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{VariableInfo, VariableType};

    use crate::test_utils::{
        IbtVariableManifest, load_fixture_manifest, require_smallest_ibt_fixture,
    };
    use anyhow::{Context, Result};
    use std::fs::File;
    use std::path::Path;

    fn open_buf_reader(path: &Path) -> Result<std::io::BufReader<File>> {
        let file = File::open(path).with_context(|| format!("Opening {}", path.display()))?;
        Ok(std::io::BufReader::new(file))
    }

    #[test]
    fn schema_rejects_invalid_region_metadata() -> Result<()> {
        let path = require_smallest_ibt_fixture()?;
        let mut reader = open_buf_reader(&path)?;
        let mut header = Header::try_from_reader(&mut reader)?;
        let original_count = header.variable_count;

        header.variable_count = -1;
        assert!(extract_variable_schema(&mut reader, &header).is_err());

        header.variable_count = original_count;
        header.variable_header_offset = -1;
        assert!(extract_variable_schema(&mut reader, &header).is_err());
        Ok(())
    }

    #[test]
    fn empty_variable_region_preserves_frame_size() -> Result<()> {
        let path = require_smallest_ibt_fixture()?;
        let mut reader = open_buf_reader(&path)?;
        let mut header = Header::try_from_reader(&mut reader)?;
        header.variable_count = 0;

        let schema = extract_variable_schema(&mut reader, &header)?;
        assert_eq!(schema.variable_count(), 0);
        assert_eq!(schema.frame_size, header.buffer_length as usize);
        Ok(())
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

    #[test]
    fn test_generated_fixture_variables_match_manifest() -> Result<()> {
        let manifest = load_fixture_manifest()?;

        for fixture in &manifest.fixtures {
            let path = fixture.fixture_path()?;
            let mut reader = open_buf_reader(&path)?;
            let header = Header::try_from_reader(&mut reader)?;
            let schema = extract_variable_schema(&mut reader, &header)?;

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
}
