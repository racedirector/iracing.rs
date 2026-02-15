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

use crate::{Result, TelemetryError, VariableInfo, VariableSchema, VariableType};
use std::collections::HashMap;
use std::io::{Read, Seek};
use tracing::{debug, trace};

// Size constants for IBT format structures
const IRSDK_HEADER_SIZE: usize = 144;
const IRSDK_DISK_SUBHEADER_SIZE: usize = 32;
pub const IRSDK_VAR_HEADER_SIZE: usize = 144;
const IRSDK_VAR_NAME_SIZE: usize = 32;
const IRSDK_VAR_DESC_SIZE: usize = 64;
const IRSDK_VAR_UNIT_SIZE: usize = 32;

/// IBT file header structure (matches iRacing's irsdk_header)
#[derive(Debug, Clone)]
pub struct IbtHeader {
    pub version: i32,
    pub status: i32,
    pub tick_rate: i32,
    pub session_info_update: i32,
    pub session_info_len: i32,
    pub session_info_offset: i32,
    pub num_vars: i32,
    pub var_header_offset: i32,
    pub num_buf: i32,
    pub buf_len: i32,
}

/// IBT disk sub-header (IBT-specific structure)
/// struct irsdk_diskSubHeader {
///   time_t sessionStartDate;   // 8 bytes (i64)
///   double sessionStartTime;   // 8 bytes (f64)
///   double sessionEndTime;     // 8 bytes (f64)
///   int sessionLapCount;       // 4 bytes (i32)
///   int sessionRecordCount;    // 4 bytes (i32)
/// }
#[derive(Debug, Clone)]
pub struct IbtDiskSubHeader {
    pub start_date: i64,   // time_t (unix timestamp)
    pub start_time: f64,   // session start time in seconds
    pub end_time: f64,     // session end time in seconds
    pub lap_count: i32,    // number of laps completed
    pub record_count: i32, // number of telemetry records
}

impl IbtHeader {
    /// Size of the irsdk_header structure in bytes
    pub const HEADER_SIZE: usize = IRSDK_HEADER_SIZE;

    pub fn parse_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        trace!("Reading IBT header ({} bytes)", IRSDK_HEADER_SIZE);
        let mut header_data = [0u8; IRSDK_HEADER_SIZE];
        reader.read_exact(&mut header_data).map_err(|e| TelemetryError::Parse {
            context: "IBT header reading".to_string(),
            details: format!("Failed to read {} header bytes: {}", IRSDK_HEADER_SIZE, e),
        })?;

        // Parse header fields according to irsdk_header structure (little-endian format)
        // struct irsdk_header {
        //   int ver;                    // offset 0
        //   int status;                 // offset 4
        //   int tickRate;               // offset 8
        //   int sessionInfoUpdate;     // offset 12
        //   int sessionInfoLen;        // offset 16
        //   int sessionInfoOffset;     // offset 20
        //   int numVars;               // offset 24
        //   int varHeaderOffset;       // offset 28
        //   int numBuf;                // offset 32
        //   int bufLen;                // offset 36
        //   int pad1[2];               // offset 40, padding for 16-byte alignment
        //   irsdk_varBuf varBuf[IRSDK_MAX_BUFS]; // offset 48, array of buffers
        // }

        let version = parse_i32_le(&header_data, 0)?;
        let status = parse_i32_le(&header_data, 4)?;
        let tick_rate = parse_i32_le(&header_data, 8)?;
        let session_info_update = parse_i32_le(&header_data, 12)?;
        let session_info_len = parse_i32_le(&header_data, 16)?;
        let session_info_offset = parse_i32_le(&header_data, 20)?;
        let num_vars = parse_i32_le(&header_data, 24)?;
        let var_header_offset = parse_i32_le(&header_data, 28)?;
        let num_buf = parse_i32_le(&header_data, 32)?;
        let buf_len = parse_i32_le(&header_data, 36)?;

        debug!(
            "Parsed IBT header: version={}, tick_rate={}, num_vars={}, buf_len={}",
            version, tick_rate, num_vars, buf_len
        );

        Ok(Self {
            version,
            status,
            tick_rate,
            session_info_update,
            session_info_len,
            session_info_offset,
            num_vars,
            var_header_offset,
            num_buf,
            buf_len,
        })
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 2 {
            return Err(TelemetryError::Version { expected: 2, found: self.version as u32 });
        }

        // Basic sanity checks for negative values
        if self.num_vars < 0 {
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: "Number of variables cannot be negative".to_string(),
            });
        }

        // Note: buf_len can be 0 in IBT files that contain only session info without telemetry data
        if self.buf_len < 0 {
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: "Buffer length cannot be negative".to_string(),
            });
        }

        // Validate offset fields are non-negative (defensive correctness)
        if self.session_info_offset < 0 {
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: "Session info offset cannot be negative".to_string(),
            });
        }

        if self.session_info_len < 0 {
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: "Session info length cannot be negative".to_string(),
            });
        }

        if self.var_header_offset < 0 {
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: "Variable header offset cannot be negative".to_string(),
            });
        }

        // Check for extreme/invalid values that indicate corruption
        if self.buf_len > 100_000_000 {
            // 100MB frame size is unreasonable
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: "Buffer length is unreasonably large".to_string(),
            });
        }

        if self.num_vars > 10_000 {
            // 10k variables is unreasonable
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: "Number of variables is unreasonably large".to_string(),
            });
        }

        Ok(())
    }
}

impl IbtDiskSubHeader {
    /// Size of the disk sub-header structure in bytes
    pub const DISK_HEADER_SIZE: usize = IRSDK_DISK_SUBHEADER_SIZE;

    pub fn parse_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut disk_header_data = [0u8; IRSDK_DISK_SUBHEADER_SIZE];
        reader.read_exact(&mut disk_header_data).map_err(|e| TelemetryError::Parse {
            context: "IBT disk sub-header reading".to_string(),
            details: format!(
                "Failed to read {} disk sub-header bytes: {}",
                IRSDK_DISK_SUBHEADER_SIZE, e
            ),
        })?;

        // Parse disk sub-header fields (little-endian format)
        let start_date = parse_i64_le(&disk_header_data, 0)?;
        let start_time = parse_f64_le(&disk_header_data, 8)?;
        let end_time = parse_f64_le(&disk_header_data, 16)?;
        let lap_count = parse_i32_le(&disk_header_data, 24)?;
        let record_count = parse_i32_le(&disk_header_data, 28)?;

        Ok(Self { start_date, start_time, end_time, lap_count, record_count })
    }
}

/// Extract variable schema from IBT file headers
pub fn extract_variable_schema<R: Read + Seek>(
    reader: &mut R,
    header: &IbtHeader,
) -> Result<VariableSchema> {
    debug!("Extracting variable schema for {} variables", header.num_vars);
    // Handle IBT files with no telemetry data frames (bufLen = 0)
    if header.buf_len == 0 || header.num_vars <= 0 {
        // File contains only session info, no telemetry data
        return VariableSchema::new(HashMap::new(), 0);
    }

    // Seek to the variable headers section and parse all variables
    reader.seek(std::io::SeekFrom::Start(header.var_header_offset as u64)).map_err(|e| {
        TelemetryError::Parse {
            context: "Variable headers seek".to_string(),
            details: format!(
                "Failed to seek to variable headers at offset {}: {}",
                header.var_header_offset, e
            ),
        }
    })?;

    // Convert num_vars to usize upfront to avoid i32-typed ranges
    let num_vars_usize = usize::try_from(header.num_vars).map_err(|_| TelemetryError::Parse {
        context: "Variable count conversion".to_string(),
        details: format!("Number of variables {} cannot be converted to usize", header.num_vars),
    })?;

    // Pre-allocate HashMap to minimize reallocation
    let mut variables = HashMap::with_capacity(num_vars_usize);

    // Parse each variable header
    for i in 0..num_vars_usize {
        let mut var_header_bytes = [0u8; IRSDK_VAR_HEADER_SIZE];
        reader.read_exact(&mut var_header_bytes).map_err(|e| TelemetryError::Parse {
            context: format!("Variable header {} reading", i),
            details: format!("Failed to read variable header {}: {}", i, e),
        })?;

        // Parse variable header fields
        let var_type = parse_i32_le(&var_header_bytes, 0)?;
        let offset = parse_i32_le(&var_header_bytes, 4)?;
        let count = parse_i32_le(&var_header_bytes, 8)?;

        // Extract null-terminated strings using constants for offsets
        let name = extract_null_terminated_string(&var_header_bytes[16..16 + IRSDK_VAR_NAME_SIZE]);
        let desc = extract_null_terminated_string(&var_header_bytes[48..48 + IRSDK_VAR_DESC_SIZE]);
        let unit =
            extract_null_terminated_string(&var_header_bytes[112..112 + IRSDK_VAR_UNIT_SIZE]);
        let count_as_time = var_header_bytes[12] != 0;

        // Skip empty or invalid variables
        if name.is_empty() || offset < 0 || count <= 0 {
            continue;
        }

        // Convert iRacing var type to our VariableType
        let data_type = match var_type {
            0 => VariableType::Int8,    // char
            1 => VariableType::Bool,    // bool
            2 => VariableType::Int32,   // int
            3 => VariableType::Int32,   // bitField (treat as int32)
            4 => VariableType::Float32, // float
            5 => VariableType::Float64, // double
            _ => {
                // Log unknown types for diagnostics
                debug!("Skipping variable '{}' with unknown type {}", name, var_type);
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

    debug!("Extracted {} variables with frame size {}", variables.len(), header.buf_len);
    VariableSchema::new(variables, header.buf_len as usize)
}

/// Verify that the IBT file length is at least large enough to contain headers and all records
/// This is a conservative lower bound based on header values; it does not validate exact layout
pub fn verify_min_length(file_len: u64, header: &IbtHeader, disk: &IbtDiskSubHeader) -> Result<()> {
    // Basic lower bound: var headers + frames
    let var_headers_len = (header.num_vars as u64).saturating_mul(IRSDK_VAR_HEADER_SIZE as u64);
    let frames_len = (disk.record_count as u64).saturating_mul(header.buf_len as u64);
    // Start position is var_header_offset; add var headers and frames
    let min_end = (header.var_header_offset as u64)
        .saturating_add(var_headers_len)
        .saturating_add(frames_len);

    if file_len < min_end {
        return Err(TelemetryError::Parse {
            context: "IBT length verification".to_string(),
            details: format!(
                "File too small: len={} < required_min={} (vars={}, records={}, buf_len={})",
                file_len, min_end, header.num_vars, disk.record_count, header.buf_len
            ),
        });
    }
    Ok(())
}

/// Safe byte parsing helpers with bounds checking
fn parse_i32_le(data: &[u8], offset: usize) -> Result<i32> {
    if offset + 4 > data.len() {
        return Err(TelemetryError::Parse {
            context: "Integer parsing".to_string(),
            details: format!(
                "Insufficient data for i32 at offset {} (need 4 bytes, have {})",
                offset,
                data.len() - offset
            ),
        });
    }
    Ok(i32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]))
}

fn parse_i64_le(data: &[u8], offset: usize) -> Result<i64> {
    if offset + 8 > data.len() {
        return Err(TelemetryError::Parse {
            context: "Long integer parsing".to_string(),
            details: format!(
                "Insufficient data for i64 at offset {} (need 8 bytes, have {})",
                offset,
                data.len() - offset
            ),
        });
    }
    Ok(i64::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ]))
}

fn parse_f64_le(data: &[u8], offset: usize) -> Result<f64> {
    if offset + 8 > data.len() {
        return Err(TelemetryError::Parse {
            context: "Double precision float parsing".to_string(),
            details: format!(
                "Insufficient data for f64 at offset {} (need 8 bytes, have {})",
                offset,
                data.len() - offset
            ),
        });
    }
    Ok(f64::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
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
        FIXTURE_INSTALL_GUIDANCE, require_ibt_fixtures, require_named_ibt_fixture,
        require_smallest_ibt_fixture,
    };
    use anyhow::{Context, Result, ensure};
    use std::fs::File;
    use std::path::{Path, PathBuf};

    fn collect_files() -> Result<Vec<PathBuf>> {
        Ok(require_ibt_fixtures()?)
    }

    fn find_fixture(expected: &str) -> Result<PathBuf> {
        Ok(require_named_ibt_fixture(expected)?)
    }

    fn open_buf_reader(path: &Path) -> Result<std::io::BufReader<File>> {
        let file = File::open(path).with_context(|| format!("Opening {}", path.display()))?;
        Ok(std::io::BufReader::new(file))
    }

    #[test]
    fn test_ford_mustang_gt4_road_atlanta_header() -> Result<()> {
        let file_path = find_fixture("fordmustanggt4_roadatlanta club 2025-09-13 11-30-23.ibt")?;

        let mut buf_reader = open_buf_reader(&file_path)?;
        let header = IbtHeader::parse_from_reader(&mut buf_reader)
            .with_context(|| format!("Parsing header from {}", file_path.display()))?;

        // Ford Mustang GT4 Road Atlanta specific header assertions
        assert_eq!(header.version, 2);
        assert_eq!(header.tick_rate, 60);
        assert_eq!(header.num_vars, 287);
        assert_eq!(header.var_header_offset, 144);
        assert_eq!(header.buf_len, 1107);
        assert_eq!(header.num_buf, 1);
        assert_eq!(header.session_info_len, 11340);
        assert_eq!(header.session_info_offset, 41472);

        header.validate()?;
        Ok(())
    }

    #[test]
    fn test_ford_mustang_gt4_road_atlanta_variables() -> Result<()> {
        let file_path = find_fixture("fordmustanggt4_roadatlanta club 2025-09-13 11-30-23.ibt")?;

        let mut buf_reader = open_buf_reader(&file_path)?;
        let header = IbtHeader::parse_from_reader(&mut buf_reader)
            .with_context(|| format!("Parsing header from {}", file_path.display()))?;
        let _disk_header = IbtDiskSubHeader::parse_from_reader(&mut buf_reader)
            .with_context(|| format!("Parsing sub-header from {}", file_path.display()))?;
        let schema = extract_variable_schema(&mut buf_reader, &header)
            .with_context(|| format!("Extracting variable schema from {}", file_path.display()))?;

        // Ford Mustang GT4 specific variable assertions
        assert_eq!(schema.frame_size, 1107);
        assert_eq!(schema.variable_count(), 287);

        // Check for key variables and their exact offsets in GT4
        assert!(schema.variables.contains_key("Speed"));
        assert!(schema.variables.contains_key("SessionTime"));
        assert!(schema.variables.contains_key("LapDist"));
        assert!(schema.variables.contains_key("LapCompleted"));
        assert!(schema.variables.contains_key("Brake"));
        assert!(schema.variables.contains_key("Throttle"));

        // Verify specific variable details for Ford Mustang GT4
        let speed_var = &schema.variables["Speed"];
        assert_eq!(speed_var.offset, 310);
        assert_eq!(speed_var.data_type, VariableType::Float32);
        assert_eq!(speed_var.units, "m/s");

        let session_time_var = &schema.variables["SessionTime"];
        assert_eq!(session_time_var.offset, 0);
        assert_eq!(session_time_var.data_type, VariableType::Float64);
        assert_eq!(session_time_var.units, "s");

        let lap_dist_var = &schema.variables["LapDist"];
        assert_eq!(lap_dist_var.offset, 217);
        assert_eq!(lap_dist_var.data_type, VariableType::Float32);
        assert_eq!(lap_dist_var.units, "m");
        Ok(())
    }

    #[test]
    fn test_ford_mustang_gt4_road_atlanta_frames() -> Result<()> {
        let file_path = find_fixture("fordmustanggt4_roadatlanta club 2025-09-13 11-30-23.ibt")?;
        let reader = crate::ibt::IbtReader::open(&file_path)
            .with_context(|| format!("Opening {}", file_path.display()))?;
        ensure!(reader.total_frames() > 0, "Fixture should contain telemetry frames");
        assert_eq!(reader.total_frames(), 19873);
        Ok(())
    }

    #[test]
    fn test_supercars_camaro_jerez_header() -> Result<()> {
        let file_path =
            find_fixture("supercars chevycamarogen3_jerez moto 2025-08-07 20-35-12.ibt")?;

        let mut buf_reader = open_buf_reader(&file_path)?;
        let header = IbtHeader::parse_from_reader(&mut buf_reader)
            .with_context(|| format!("Parsing header from {}", file_path.display()))?;

        // Supercars Camaro Jerez specific header assertions
        assert_eq!(header.version, 2);
        assert_eq!(header.tick_rate, 60);
        assert_eq!(header.num_vars, 283);
        assert_eq!(header.var_header_offset, 144);
        assert_eq!(header.buf_len, 1094);
        assert_eq!(header.num_buf, 1);
        assert_eq!(header.session_info_len, 70555);
        assert_eq!(header.session_info_offset, 40896);

        header.validate()?;
        Ok(())
    }

    #[test]
    fn test_supercars_camaro_jerez_variables() -> Result<()> {
        let file_path =
            find_fixture("supercars chevycamarogen3_jerez moto 2025-08-07 20-35-12.ibt")?;

        let mut buf_reader = open_buf_reader(&file_path)?;
        let header = IbtHeader::parse_from_reader(&mut buf_reader)
            .with_context(|| format!("Parsing header from {}", file_path.display()))?;
        let _disk_header = IbtDiskSubHeader::parse_from_reader(&mut buf_reader)
            .with_context(|| format!("Parsing sub-header from {}", file_path.display()))?;
        let schema = extract_variable_schema(&mut buf_reader, &header)
            .with_context(|| format!("Extracting variable schema from {}", file_path.display()))?;

        // Supercars specific variable assertions
        assert_eq!(schema.frame_size, 1094);
        assert_eq!(schema.variable_count(), 283);

        // Verify Supercars-specific variables exist
        assert!(schema.variables.contains_key("Speed"));
        assert!(schema.variables.contains_key("SessionTime"));
        assert!(schema.variables.contains_key("LapDist"));

        // Supercars has different frame layout than GT4
        let speed_var = &schema.variables["Speed"];
        assert_eq!(speed_var.data_type, VariableType::Float32);
        assert_eq!(speed_var.units, "m/s");
        assert_eq!(speed_var.offset, 310); // Same offset as GT4

        let session_time_var = &schema.variables["SessionTime"];
        assert_eq!(session_time_var.offset, 0);
        assert_eq!(session_time_var.data_type, VariableType::Float64);
        Ok(())
    }

    #[test]
    fn test_supercars_camaro_jerez_frames() -> Result<()> {
        let file_path =
            find_fixture("supercars chevycamarogen3_jerez moto 2025-08-07 20-35-12.ibt")?;
        let reader = crate::ibt::IbtReader::open(&file_path)
            .with_context(|| format!("Opening {}", file_path.display()))?;
        ensure!(reader.total_frames() > 0, "Fixture should contain telemetry frames");
        assert_eq!(reader.total_frames(), 31221);
        Ok(())
    }

    #[test]
    fn test_supercars_camaro_okayama_header() -> Result<()> {
        let file_path =
            find_fixture("supercars chevycamarogen3_okayama full 2025-08-28 19-49-16.ibt")?;

        let mut buf_reader = open_buf_reader(&file_path)?;
        let header = IbtHeader::parse_from_reader(&mut buf_reader)
            .with_context(|| format!("Parsing header from {}", file_path.display()))?;

        // Supercars Camaro Okayama specific header assertions
        assert_eq!(header.version, 2);
        assert_eq!(header.tick_rate, 60);
        assert_eq!(header.num_vars, 283);
        assert_eq!(header.var_header_offset, 144);
        assert_eq!(header.buf_len, 1094);
        assert_eq!(header.num_buf, 1);
        assert_eq!(header.session_info_len, 60847);
        assert_eq!(header.session_info_offset, 40896);

        header.validate()?;
        Ok(())
    }

    #[test]
    fn test_supercars_camaro_okayama_variables() -> Result<()> {
        let file_path =
            find_fixture("supercars chevycamarogen3_okayama full 2025-08-28 19-49-16.ibt")?;

        let mut buf_reader = open_buf_reader(&file_path)?;
        let header = IbtHeader::parse_from_reader(&mut buf_reader)
            .with_context(|| format!("Parsing header from {}", file_path.display()))?;
        let _disk_header = IbtDiskSubHeader::parse_from_reader(&mut buf_reader)
            .with_context(|| format!("Parsing sub-header from {}", file_path.display()))?;
        let schema = extract_variable_schema(&mut buf_reader, &header)
            .with_context(|| format!("Extracting variable schema from {}", file_path.display()))?;

        // Okayama session variable assertions
        assert_eq!(schema.frame_size, 1094);
        assert_eq!(schema.variable_count(), 283);

        // Essential variables present
        assert!(schema.variables.contains_key("Speed"));
        assert!(schema.variables.contains_key("RPM"));
        assert!(schema.variables.contains_key("Gear"));
        assert!(schema.variables.contains_key("SessionTime"));

        // Check variable types and units
        let rpm_var = &schema.variables["RPM"];
        assert_eq!(rpm_var.data_type, VariableType::Float32);
        assert_eq!(rpm_var.units, "revs/min");

        let gear_var = &schema.variables["Gear"];
        assert_eq!(gear_var.data_type, VariableType::Int32);
        assert_eq!(gear_var.units, "");
        Ok(())
    }

    #[test]
    fn test_supercars_camaro_okayama_frames() -> Result<()> {
        let file_path =
            find_fixture("supercars chevycamarogen3_okayama full 2025-08-28 19-49-16.ibt")?;
        let reader = crate::ibt::IbtReader::open(&file_path)
            .with_context(|| format!("Opening {}", file_path.display()))?;
        ensure!(reader.total_frames() > 0, "Fixture should contain telemetry frames");
        assert_eq!(reader.total_frames(), 51183);
        Ok(())
    }

    #[test]
    fn test_essential_variables_across_all_files() -> Result<()> {
        let files = collect_files()?;
        ensure!(
            files.len() == 3,
            "Expected 3 IBT fixtures, found {}. {}",
            files.len(),
            FIXTURE_INSTALL_GUIDANCE
        );

        for file_path in &files {
            let mut buf_reader = open_buf_reader(file_path)?;
            let header = IbtHeader::parse_from_reader(&mut buf_reader)
                .with_context(|| format!("Parsing header from {}", file_path.display()))?;
            let _disk_header = IbtDiskSubHeader::parse_from_reader(&mut buf_reader)
                .with_context(|| format!("Parsing sub-header from {}", file_path.display()))?;
            let schema = extract_variable_schema(&mut buf_reader, &header)
                .with_context(|| format!("Extracting schema from {}", file_path.display()))?;

            for key in ["Speed", "SessionTime", "LapDist", "LapCompleted", "Brake", "Throttle"] {
                assert!(
                    schema.variables.contains_key(key),
                    "File {} missing {} variable",
                    file_path.display(),
                    key
                );
            }

            assert!(
                schema.variable_count() >= 280,
                "File {} has too few variables: {}",
                file_path.display(),
                schema.variable_count()
            );

            assert!(
                schema.frame_size >= 1000,
                "File {} has unexpectedly small frame size: {}",
                file_path.display(),
                schema.frame_size
            );
        }
        Ok(())
    }

    #[test]
    fn test_ford_vs_supercars_variable_differences() -> Result<()> {
        let ford_file = find_fixture("fordmustanggt4_roadatlanta club 2025-09-13 11-30-23.ibt")?;
        let supercars_file =
            find_fixture("supercars chevycamarogen3_jerez moto 2025-08-07 20-35-12.ibt")?;

        let mut ford_reader = open_buf_reader(&ford_file)?;
        let ford_header = IbtHeader::parse_from_reader(&mut ford_reader)
            .with_context(|| format!("Parsing Ford header from {}", ford_file.display()))?;
        let _ford_disk = IbtDiskSubHeader::parse_from_reader(&mut ford_reader)
            .with_context(|| format!("Parsing Ford sub-header from {}", ford_file.display()))?;
        let ford_schema = extract_variable_schema(&mut ford_reader, &ford_header)
            .with_context(|| format!("Extracting Ford schema from {}", ford_file.display()))?;

        let mut supercars_reader = open_buf_reader(&supercars_file)?;
        let supercars_header =
            IbtHeader::parse_from_reader(&mut supercars_reader).with_context(|| {
                format!("Parsing Supercars header from {}", supercars_file.display())
            })?;
        let _supercars_disk = IbtDiskSubHeader::parse_from_reader(&mut supercars_reader)
            .with_context(|| {
                format!("Parsing Supercars sub-header from {}", supercars_file.display())
            })?;
        let supercars_schema = extract_variable_schema(&mut supercars_reader, &supercars_header)
            .with_context(|| {
                format!("Extracting Supercars schema from {}", supercars_file.display())
            })?;

        // Ford GT4 has more variables than Supercars
        assert_eq!(ford_schema.variable_count(), 287);
        assert_eq!(supercars_schema.variable_count(), 283);
        assert!(ford_schema.variable_count() > supercars_schema.variable_count());

        // Ford GT4 has larger frame size
        assert_eq!(ford_schema.frame_size, 1107);
        assert_eq!(supercars_schema.frame_size, 1094);
        assert!(ford_schema.frame_size > supercars_schema.frame_size);

        // Both should have Speed at same offset

        assert_eq!(ford_schema.variables["Speed"].offset, 310);
        assert_eq!(supercars_schema.variables["Speed"].offset, 310);
        Ok(())
    }

    #[test]
    fn test_truncated_file_handling() {
        let truncated_data = vec![0u8; 10];
        let mut cursor = std::io::Cursor::new(truncated_data);
        let result = IbtHeader::parse_from_reader(&mut cursor);

        assert!(result.is_err());
        match result.unwrap_err() {
            TelemetryError::Parse { .. } => {}
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

        let header_result = IbtHeader::parse_from_reader(&mut cursor);

        if let Ok(header) = header_result {
            let result = header.validate();
            assert!(matches!(result.unwrap_err(), TelemetryError::Version { .. }));
        }

        Ok(())
    }

    #[test]
    fn test_disk_length_verification_ok() -> Result<()> {
        use std::fs::metadata;
        let file_path = require_smallest_ibt_fixture()?;
        let file_len = metadata(&file_path)?.len();
        let mut reader = open_buf_reader(&file_path)?;
        let header = IbtHeader::parse_from_reader(&mut reader)
            .with_context(|| format!("Parsing header from {}", file_path.display()))?;
        let disk = IbtDiskSubHeader::parse_from_reader(&mut reader)
            .with_context(|| format!("Parsing disk sub-header from {}", file_path.display()))?;
        super::verify_min_length(file_len, &header, &disk)?;
        Ok(())
    }

    #[test]
    fn test_disk_length_verification_truncated() -> Result<()> {
        let file_path = require_smallest_ibt_fixture()?;
        let mut reader = open_buf_reader(&file_path)?;
        let header = IbtHeader::parse_from_reader(&mut reader)?;
        let disk = IbtDiskSubHeader::parse_from_reader(&mut reader)?;
        let result = super::verify_min_length(0, &header, &disk);
        assert!(result.is_err());
        Ok(())
    }
}
