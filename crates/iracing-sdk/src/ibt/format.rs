//! IBT file format structures and parsing
//!
//! Defines the binary structures used in iRacing's IBT file format
//! and provides parsing functions for cross-platform file reading.
//!
//! ## IBT File Structure
//!
//! IBT (iRacing Binary Telemetry) files contain recorded telemetry data from iRacing sessions:
//!
//! 1. **Main Header** (112 bytes) - `irsdk_header` compatible structure
//! 2. **Disk Sub-Header** (32 bytes) - IBT-specific metadata with timing and record counts
//! 3. **Session Info** - YAML session configuration (optional)
//! 4. **Variable Headers** - Array of variable definitions
//! 5. **Frame Data** - Sequential telemetry samples
//!
//! Region offsets are absolute. The telemetry region begins after the latest
//! advertised metadata-region end; its complete-frame count is derived from
//! physical EOF and the frame size. The disk sub-header's record count is a
//! consistency check, not an alternate source of frame bounds. See the
//! repository's `docs/ibt-spec.md` for the evidence and compatibility rules.
//!
//! ## Performance Characteristics
//!
//! - Binary parsing with explicit little-endian byte order handling
//! - Bounds checking for all memory operations
//! - Minimal memory allocations during header parsing
//! - O(1) schema validation after parsing

use crate::{
    IRacingSDKError, Result, VariableHeadersBuffer, VariableSchema, types::VariableHeaderRegion,
};

use std::io::{Read, Seek, SeekFrom};

/// Extract variable schema from IBT file headers
pub(super) fn extract_variable_schema<R: Read + Seek>(
    reader: &mut R,
    region: &VariableHeaderRegion,
    frame_size: usize,
) -> Result<VariableSchema> {
    tracing::debug!(
        "Extracting variable schema for {} variables",
        region.count()
    );

    let offset = region.region.offset;

    // Seek to the variable headers section and parse all variables
    reader.seek(SeekFrom::Start(offset as u64)).map_err(|e| {
        IRacingSDKError::parse_error(
            "Variable headers seek".to_string(),
            format!(
                "Failed to seek to variable headers at offset {}: {}",
                offset, e
            ),
        )
    })?;

    let mut bytes = vec![0; region.region.length];

    reader.read_exact(&mut bytes).map_err(|e| {
        IRacingSDKError::parse_error(
            "Variable Headers read",
            format!("Failed to read headers: {}", e),
        )
    })?;

    let headers = VariableHeadersBuffer::try_from_region_bytes(&bytes, region.count())?;

    VariableSchema::from_headers(&headers, frame_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::irsdk::Header;
    use std::io::Cursor;
    use zerocopy::FromZeros;

    #[test]
    fn empty_variable_region_preserves_frame_size() -> Result<()> {
        let header = Header::new_zeroed();
        let region = VariableHeaderRegion::try_from(&header)?;
        let mut reader = Cursor::new([]);
        let frame_size = 64;

        let schema = extract_variable_schema(&mut reader, &region, frame_size)?;
        assert_eq!(schema.variable_count(), 0);
        assert_eq!(schema.frame_size, frame_size);
        Ok(())
    }
}
