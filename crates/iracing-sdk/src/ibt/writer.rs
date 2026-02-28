//! IBT file writer for telemetry export and recording.
//!
//! This module provides:
//! - [`IbtWriter`] for streaming `.ibt` frame output
//! - [`FrameProjection`] for selecting/repacking variable subsets
//! - [`IbtWriteOptions`] for controlling header/session metadata

use super::{format::IRSDK_VAR_HEADER_SIZE, reader::IbtReader};
use crate::{
    IRacingSDKError, Result, VariableInfo, VariableSchema, VariableType, WindowsConnection,
};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const IBT_VERSION: i32 = 2;
const IBT_NUM_BUF: i32 = 1;
const FILE_HEADER_SIZE: usize = 144;
const DISK_SUBHEADER_SIZE: usize = 32;
const VAR_HEADER_OFFSET: u64 = (FILE_HEADER_SIZE + DISK_SUBHEADER_SIZE) as u64;
const HEADER_STATUS_CONNECTED: i32 = 1;
const HEADER_ALIGNMENT_BYTES: u64 = 16;

/// Configurable metadata for [`IbtWriter`].
#[derive(Debug, Clone)]
pub struct IbtWriteOptions {
    /// Status bits to place in the IBT header.
    pub status: i32,
    /// Telemetry sample rate in Hz.
    pub tick_rate: i32,
    /// Session info update counter.
    pub session_info_update: i32,
    /// Optional session YAML payload.
    pub session_yaml: Option<String>,
    /// Session start date (`time_t`).
    pub start_date: i64,
    /// Session start time in seconds.
    pub start_time: f64,
    /// Optional explicit end time in seconds. If omitted, computed from frames/tick rate.
    pub end_time: Option<f64>,
    /// Lap count metadata.
    pub lap_count: i32,
}

impl Default for IbtWriteOptions {
    fn default() -> Self {
        Self {
            status: HEADER_STATUS_CONNECTED,
            tick_rate: 60,
            session_info_update: 0,
            session_yaml: None,
            start_date: 0,
            start_time: 0.0,
            end_time: None,
            lap_count: 0,
        }
    }
}

impl IbtWriteOptions {
    /// Build write options from a live Windows connection.
    ///
    /// `start_date` is set to the current wall-clock time. `start_time`, `end_time`,
    /// and `lap_count` are left at their defaults (`0.0`, `None`, `0`) because
    /// those values are not available from shared memory before recording begins.
    #[cfg(windows)]
    pub fn from_connection(connection: &WindowsConnection) -> Result<Self> {
        let header = connection.header();
        let raw_session_yaml = connection.session_info();

        let start_date = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        Ok(Self {
            status: header.status,
            tick_rate: header.tick_rate,
            session_info_update: header.session_info_update,
            session_yaml: raw_session_yaml.map(|s| s.to_string()),
            start_date,
            start_time: 0.0,
            end_time: None,
            lap_count: 0,
        })
    }

    /// Build write options from an existing reader's metadata.
    pub fn from_reader(reader: &IbtReader) -> Result<Self> {
        let header = reader.header();
        let disk = reader.disk_header();

        Ok(Self {
            status: header.status,
            tick_rate: header.tick_rate,
            session_info_update: header.session_info_update,
            session_yaml: reader.session_yaml()?,
            start_date: disk.start_date,
            start_time: disk.start_time,
            end_time: Some(disk.end_time),
            lap_count: disk.lap_count,
        })
    }

    fn validate(&self) -> Result<()> {
        if self.tick_rate <= 0 {
            return Err(IRacingSDKError::Parse {
                context: "IBT writer options".to_string(),
                details: format!("tick_rate must be positive, found {}", self.tick_rate),
            });
        }

        if self.lap_count < 0 {
            return Err(IRacingSDKError::Parse {
                context: "IBT writer options".to_string(),
                details: format!("lap_count cannot be negative, found {}", self.lap_count),
            });
        }

        if let Some(end_time) = self.end_time
            && end_time < self.start_time
        {
            return Err(IRacingSDKError::Parse {
                context: "IBT writer options".to_string(),
                details: format!(
                    "end_time ({end_time}) cannot be less than start_time ({})",
                    self.start_time
                ),
            });
        }

        Ok(())
    }
}

/// Frame projection plan for selecting/repacking variables from source frames.
#[derive(Debug, Clone)]
pub struct FrameProjection {
    source_frame_size: usize,
    target_schema: VariableSchema,
    fields: Vec<ProjectedField>,
}

impl FrameProjection {
    /// Build a compact projection from a source schema and selected variable names.
    ///
    /// The resulting target schema is tightly packed in the same order as `variable_names`.
    pub fn from_variable_names<I, S>(
        source_schema: &VariableSchema,
        variable_names: I,
    ) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut selected = Vec::new();
        for variable_name in variable_names {
            selected.push(variable_name.as_ref().to_string());
        }

        if selected.is_empty() {
            return Err(IRacingSDKError::Parse {
                context: "Frame projection".to_string(),
                details: "At least one variable must be selected".to_string(),
            });
        }

        let mut dedupe = HashSet::with_capacity(selected.len());
        for name in &selected {
            if !dedupe.insert(name.clone()) {
                return Err(IRacingSDKError::Parse {
                    context: "Frame projection".to_string(),
                    details: format!("Duplicate variable name in selection: `{name}`"),
                });
            }
        }

        let mut target_offset = 0usize;
        let mut target_variables = HashMap::with_capacity(selected.len());
        let mut fields = Vec::with_capacity(selected.len());

        for name in &selected {
            let source_info =
                source_schema
                    .get_variable(name)
                    .ok_or_else(|| IRacingSDKError::Parse {
                        context: "Frame projection".to_string(),
                        details: format!("Variable `{name}` not present in source schema"),
                    })?;

            let field_size = source_info
                .data_type
                .size()
                .checked_mul(source_info.count)
                .ok_or_else(|| IRacingSDKError::Parse {
                    context: "Frame projection".to_string(),
                    details: format!("Field size overflow for variable `{name}`"),
                })?;

            let source_end = source_info.offset.checked_add(field_size).ok_or_else(|| {
                IRacingSDKError::Parse {
                    context: "Frame projection".to_string(),
                    details: format!("Source offset overflow for variable `{name}`"),
                }
            })?;

            if source_end > source_schema.frame_size {
                return Err(IRacingSDKError::Memory {
                    offset: source_info.offset,
                    source: None,
                });
            }

            let projected = VariableInfo {
                name: source_info.name.clone(),
                data_type: source_info.data_type,
                offset: target_offset,
                count: source_info.count,
                count_as_time: source_info.count_as_time,
                units: source_info.units.clone(),
                description: source_info.description.clone(),
            };

            target_variables.insert(projected.name.clone(), projected.clone());
            fields.push(ProjectedField {
                name: projected.name,
                source_offset: source_info.offset,
                target_offset,
                width: field_size,
            });

            target_offset =
                target_offset
                    .checked_add(field_size)
                    .ok_or_else(|| IRacingSDKError::Parse {
                        context: "Frame projection".to_string(),
                        details: format!("Target offset overflow after variable `{name}`"),
                    })?;
        }

        let target_schema = VariableSchema::new(target_variables, target_offset)?;

        Ok(Self {
            source_frame_size: source_schema.frame_size,
            target_schema,
            fields,
        })
    }

    /// Build a compact projection containing every variable from source schema.
    pub fn from_all(source_schema: &VariableSchema) -> Result<Self> {
        let mut variables: Vec<&VariableInfo> = source_schema.variables.values().collect();
        variables.sort_unstable_by(|left, right| {
            left.offset
                .cmp(&right.offset)
                .then_with(|| left.name.cmp(&right.name))
        });

        let names = variables.into_iter().map(|var| var.name.as_str());
        Self::from_variable_names(source_schema, names)
    }

    /// Get the compact target schema produced by this projection.
    pub fn target_schema(&self) -> &VariableSchema {
        &self.target_schema
    }

    /// Project source frame bytes into a freshly allocated target frame.
    pub fn project_frame(&self, source_frame: &[u8]) -> Result<Vec<u8>> {
        let mut output = vec![0u8; self.target_schema.frame_size];
        self.project_into(source_frame, &mut output)?;
        Ok(output)
    }

    /// Project source frame bytes into an existing target frame buffer.
    pub fn project_into(&self, source_frame: &[u8], target_frame: &mut [u8]) -> Result<()> {
        if source_frame.len() < self.source_frame_size {
            return Err(IRacingSDKError::Parse {
                context: "Frame projection".to_string(),
                details: format!(
                    "Source frame too small: expected at least {} bytes, got {}",
                    self.source_frame_size,
                    source_frame.len()
                ),
            });
        }

        if target_frame.len() != self.target_schema.frame_size {
            return Err(IRacingSDKError::Parse {
                context: "Frame projection".to_string(),
                details: format!(
                    "Target frame size mismatch: expected {} bytes, got {}",
                    self.target_schema.frame_size,
                    target_frame.len()
                ),
            });
        }

        for field in &self.fields {
            let src_end = field.source_offset + field.width;
            let dst_end = field.target_offset + field.width;

            let source_slice = source_frame
                .get(field.source_offset..src_end)
                .ok_or_else(|| IRacingSDKError::Parse {
                    context: "Frame projection".to_string(),
                    details: format!("Source frame out of bounds while copying `{}`", field.name),
                })?;

            let target_slice = target_frame
                .get_mut(field.target_offset..dst_end)
                .ok_or_else(|| IRacingSDKError::Parse {
                    context: "Frame projection".to_string(),
                    details: format!("Target frame out of bounds while copying `{}`", field.name),
                })?;

            target_slice.copy_from_slice(source_slice);
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ProjectedField {
    name: String,
    source_offset: usize,
    target_offset: usize,
    width: usize,
}

#[derive(Debug)]
struct WriterLayout {
    ordered_variables: Vec<VariableInfo>,
    session_bytes: Vec<u8>,
    var_header_offset: i32,
    session_info_offset: i32,
    session_info_len: i32,
    frame_data_offset: i32,
}

/// Streaming writer for creating `.ibt` files.
pub struct IbtWriter {
    path: PathBuf,
    file: File,
    schema: VariableSchema,
    options: IbtWriteOptions,
    layout: WriterLayout,
    frame_count: u32,
    finalized: bool,
}

impl IbtWriter {
    /// Create a new writer that writes to `path`.
    pub fn create<P: AsRef<Path>>(
        path: P,
        schema: VariableSchema,
        options: IbtWriteOptions,
    ) -> Result<Self> {
        schema.validate()?;
        options.validate()?;

        let layout = plan_layout(&schema, options.session_yaml.as_deref())?;
        let path_buf = path.as_ref().to_path_buf();

        let mut file = File::create(&path_buf).map_err(|source| IRacingSDKError::File {
            path: path_buf.clone(),
            source,
        })?;

        let initial_header =
            build_header_bytes(&schema, &options, &layout, 0, layout.frame_data_offset)?;
        file.write_all(&initial_header)
            .map_err(|source| IRacingSDKError::File {
                path: path_buf.clone(),
                source,
            })?;

        let initial_disk = build_disk_header_bytes(
            options.start_date,
            options.start_time,
            options.end_time.unwrap_or(options.start_time),
            options.lap_count,
            0,
        );
        file.write_all(&initial_disk)
            .map_err(|source| IRacingSDKError::File {
                path: path_buf.clone(),
                source,
            })?;

        file.seek(SeekFrom::Start(layout.var_header_offset as u64))
            .map_err(|source| IRacingSDKError::File {
                path: path_buf.clone(),
                source,
            })?;

        for variable in &layout.ordered_variables {
            let bytes = encode_var_header(variable)?;
            file.write_all(&bytes)
                .map_err(|source| IRacingSDKError::File {
                    path: path_buf.clone(),
                    source,
                })?;
        }

        if layout.session_info_len > 0 {
            pad_to_offset(&mut file, layout.session_info_offset as u64, &path_buf)?;
            file.write_all(&layout.session_bytes)
                .map_err(|source| IRacingSDKError::File {
                    path: path_buf.clone(),
                    source,
                })?;
        }

        pad_to_offset(&mut file, layout.frame_data_offset as u64, &path_buf)?;

        Ok(Self {
            path: path_buf,
            file,
            schema,
            options,
            layout,
            frame_count: 0,
            finalized: false,
        })
    }

    /// Access the writer schema.
    pub fn schema(&self) -> &VariableSchema {
        &self.schema
    }

    /// Number of frames written so far.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Write one frame in target schema layout.
    pub fn write_frame(&mut self, frame: &[u8]) -> Result<()> {
        self.ensure_not_finalized()?;

        if frame.len() != self.schema.frame_size {
            return Err(IRacingSDKError::Parse {
                context: "IBT frame write".to_string(),
                details: format!(
                    "Frame size mismatch: expected {} bytes, got {}",
                    self.schema.frame_size,
                    frame.len()
                ),
            });
        }

        if self.frame_count == i32::MAX as u32 {
            return Err(IRacingSDKError::Parse {
                context: "IBT frame write".to_string(),
                details: "record_count overflow: maximum i32::MAX frames reached".to_string(),
            });
        }

        self.file
            .write_all(frame)
            .map_err(|source| IRacingSDKError::File {
                path: self.path.clone(),
                source,
            })?;

        self.frame_count += 1;
        Ok(())
    }

    /// Project a source frame and write it using this writer's schema.
    pub fn write_projected_frame(
        &mut self,
        source_frame: &[u8],
        projection: &FrameProjection,
    ) -> Result<()> {
        let mut projected = vec![0u8; self.schema.frame_size];
        self.write_projected_frame_with_buffer(source_frame, projection, &mut projected)
    }

    /// Project a source frame into `target_buffer` and write it.
    ///
    /// This avoids per-frame allocations in high-frequency live recording paths.
    pub fn write_projected_frame_with_buffer(
        &mut self,
        source_frame: &[u8],
        projection: &FrameProjection,
        target_buffer: &mut [u8],
    ) -> Result<()> {
        if projection.target_schema().frame_size != self.schema.frame_size {
            return Err(IRacingSDKError::Parse {
                context: "IBT frame write".to_string(),
                details: format!(
                    "Projection target frame size {} does not match writer frame size {}",
                    projection.target_schema().frame_size,
                    self.schema.frame_size
                ),
            });
        }

        projection.project_into(source_frame, target_buffer)?;
        self.write_frame(target_buffer)
    }

    /// Finalize headers and flush all output.
    ///
    /// This must be called after writing frames so `record_count` metadata is correct.
    pub fn finish(mut self) -> Result<()> {
        self.ensure_not_finalized()?;

        let record_count_i32 =
            i32::try_from(self.frame_count).map_err(|_| IRacingSDKError::Parse {
                context: "IBT finalize".to_string(),
                details: format!("record_count {} exceeds i32::MAX", self.frame_count),
            })?;

        let computed_end_time =
            self.options.start_time + (self.frame_count as f64 / self.options.tick_rate as f64);
        let end_time = self.options.end_time.unwrap_or(computed_end_time);

        let header_bytes = build_header_bytes(
            &self.schema,
            &self.options,
            &self.layout,
            record_count_i32,
            self.layout.frame_data_offset,
        )?;

        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|source| IRacingSDKError::File {
                path: self.path.clone(),
                source,
            })?;

        self.file
            .write_all(&header_bytes)
            .map_err(|source| IRacingSDKError::File {
                path: self.path.clone(),
                source,
            })?;

        let disk_bytes = build_disk_header_bytes(
            self.options.start_date,
            self.options.start_time,
            end_time,
            self.options.lap_count,
            record_count_i32,
        );

        self.file
            .seek(SeekFrom::Start(FILE_HEADER_SIZE as u64))
            .map_err(|source| IRacingSDKError::File {
                path: self.path.clone(),
                source,
            })?;

        self.file
            .write_all(&disk_bytes)
            .map_err(|source| IRacingSDKError::File {
                path: self.path.clone(),
                source,
            })?;

        self.file.flush().map_err(|source| IRacingSDKError::File {
            path: self.path.clone(),
            source,
        })?;

        self.finalized = true;
        Ok(())
    }

    fn ensure_not_finalized(&self) -> Result<()> {
        if self.finalized {
            return Err(IRacingSDKError::Parse {
                context: "IBT writer".to_string(),
                details: "Writer is already finalized".to_string(),
            });
        }

        Ok(())
    }
}

fn plan_layout(schema: &VariableSchema, session_yaml: Option<&str>) -> Result<WriterLayout> {
    let ordered_variables = sorted_variables(schema);

    let mut session_bytes = session_yaml
        .map(|yaml| yaml.as_bytes().to_vec())
        .unwrap_or_default();

    if !session_bytes.is_empty() && session_bytes.last() != Some(&0u8) {
        session_bytes.push(0);
    }

    let var_headers_len = (ordered_variables.len() as u64)
        .checked_mul(IRSDK_VAR_HEADER_SIZE as u64)
        .ok_or_else(|| IRacingSDKError::Parse {
            context: "IBT layout planning".to_string(),
            details: "Variable header length overflow".to_string(),
        })?;

    let var_section_end = VAR_HEADER_OFFSET
        .checked_add(var_headers_len)
        .ok_or_else(|| IRacingSDKError::Parse {
            context: "IBT layout planning".to_string(),
            details: "Variable section end overflow".to_string(),
        })?;

    let (session_info_offset_u64, frame_start_base) = if session_bytes.is_empty() {
        (0u64, var_section_end)
    } else {
        let session_offset = align_up(var_section_end, HEADER_ALIGNMENT_BYTES)?;
        let frame_start = session_offset
            .checked_add(session_bytes.len() as u64)
            .ok_or_else(|| IRacingSDKError::Parse {
                context: "IBT layout planning".to_string(),
                details: "Session section end overflow".to_string(),
            })?;
        (session_offset, frame_start)
    };

    let frame_data_offset_u64 = align_up(frame_start_base, HEADER_ALIGNMENT_BYTES)?;

    let session_info_offset =
        i32::try_from(session_info_offset_u64).map_err(|_| IRacingSDKError::Parse {
            context: "IBT layout planning".to_string(),
            details: format!(
                "session_info_offset {} does not fit in i32",
                session_info_offset_u64
            ),
        })?;

    let frame_data_offset =
        i32::try_from(frame_data_offset_u64).map_err(|_| IRacingSDKError::Parse {
            context: "IBT layout planning".to_string(),
            details: format!(
                "frame_data_offset {} does not fit in i32",
                frame_data_offset_u64
            ),
        })?;

    let session_info_len =
        i32::try_from(session_bytes.len()).map_err(|_| IRacingSDKError::Parse {
            context: "IBT layout planning".to_string(),
            details: format!(
                "session_info_len {} does not fit in i32",
                session_bytes.len()
            ),
        })?;

    let var_header_offset =
        i32::try_from(VAR_HEADER_OFFSET).map_err(|_| IRacingSDKError::Parse {
            context: "IBT layout planning".to_string(),
            details: "var_header_offset does not fit in i32".to_string(),
        })?;

    for variable in &ordered_variables {
        ensure_supported_variable_type(variable)?;

        let _offset = i32::try_from(variable.offset).map_err(|_| IRacingSDKError::Parse {
            context: "IBT layout planning".to_string(),
            details: format!(
                "Variable `{}` offset {} does not fit in i32",
                variable.name, variable.offset
            ),
        })?;

        let _count = i32::try_from(variable.count).map_err(|_| IRacingSDKError::Parse {
            context: "IBT layout planning".to_string(),
            details: format!(
                "Variable `{}` count {} does not fit in i32",
                variable.name, variable.count
            ),
        })?;
    }

    Ok(WriterLayout {
        ordered_variables,
        session_bytes,
        var_header_offset,
        session_info_offset,
        session_info_len,
        frame_data_offset,
    })
}

fn sorted_variables(schema: &VariableSchema) -> Vec<VariableInfo> {
    let mut variables: Vec<VariableInfo> = schema.variables.values().cloned().collect();
    variables.sort_unstable_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.name.cmp(&right.name))
    });
    variables
}

fn build_header_bytes(
    schema: &VariableSchema,
    options: &IbtWriteOptions,
    layout: &WriterLayout,
    record_count: i32,
    frame_data_offset: i32,
) -> Result<[u8; FILE_HEADER_SIZE]> {
    let mut bytes = [0u8; FILE_HEADER_SIZE];

    let buf_len = i32::try_from(schema.frame_size).map_err(|_| IRacingSDKError::Parse {
        context: "IBT header serialization".to_string(),
        details: format!("frame_size {} does not fit in i32", schema.frame_size),
    })?;

    let num_vars =
        i32::try_from(layout.ordered_variables.len()).map_err(|_| IRacingSDKError::Parse {
            context: "IBT header serialization".to_string(),
            details: format!(
                "num_vars {} does not fit in i32",
                layout.ordered_variables.len()
            ),
        })?;

    write_i32(&mut bytes, 0, IBT_VERSION);
    write_i32(&mut bytes, 4, options.status);
    write_i32(&mut bytes, 8, options.tick_rate);
    write_i32(&mut bytes, 12, options.session_info_update);
    write_i32(&mut bytes, 16, layout.session_info_len);
    write_i32(&mut bytes, 20, layout.session_info_offset);
    write_i32(&mut bytes, 24, num_vars);
    write_i32(&mut bytes, 28, layout.var_header_offset);
    write_i32(&mut bytes, 32, IBT_NUM_BUF);
    write_i32(&mut bytes, 36, buf_len);

    // varBuf[0]: tick_count + buf_offset + pad[2]
    write_i32(&mut bytes, 48, record_count);
    write_i32(&mut bytes, 52, frame_data_offset);

    Ok(bytes)
}

fn build_disk_header_bytes(
    start_date: i64,
    start_time: f64,
    end_time: f64,
    lap_count: i32,
    record_count: i32,
) -> [u8; DISK_SUBHEADER_SIZE] {
    let mut bytes = [0u8; DISK_SUBHEADER_SIZE];

    write_i64(&mut bytes, 0, start_date);
    write_f64(&mut bytes, 8, start_time);
    write_f64(&mut bytes, 16, end_time);
    write_i32(&mut bytes, 24, lap_count);
    write_i32(&mut bytes, 28, record_count);

    bytes
}

fn encode_var_header(variable: &VariableInfo) -> Result<[u8; IRSDK_VAR_HEADER_SIZE]> {
    ensure_supported_variable_type(variable)?;

    let mut bytes = [0u8; IRSDK_VAR_HEADER_SIZE];

    let var_type =
        variable_type_to_ibt(variable.data_type).ok_or_else(|| IRacingSDKError::Parse {
            context: "IBT variable serialization".to_string(),
            details: format!(
                "Unsupported variable type {:?} for `{}`",
                variable.data_type, variable.name
            ),
        })?;

    let offset = i32::try_from(variable.offset).map_err(|_| IRacingSDKError::Parse {
        context: "IBT variable serialization".to_string(),
        details: format!(
            "Variable `{}` offset {} does not fit in i32",
            variable.name, variable.offset
        ),
    })?;

    let count = i32::try_from(variable.count).map_err(|_| IRacingSDKError::Parse {
        context: "IBT variable serialization".to_string(),
        details: format!(
            "Variable `{}` count {} does not fit in i32",
            variable.name, variable.count
        ),
    })?;

    write_i32(&mut bytes, 0, var_type);
    write_i32(&mut bytes, 4, offset);
    write_i32(&mut bytes, 8, count);
    bytes[12] = u8::from(variable.count_as_time);

    write_c_string(&mut bytes[16..48], &variable.name);
    write_c_string(&mut bytes[48..112], &variable.description);
    write_c_string(&mut bytes[112..144], &variable.units);

    Ok(bytes)
}

fn ensure_supported_variable_type(variable: &VariableInfo) -> Result<()> {
    if variable_type_to_ibt(variable.data_type).is_none() {
        return Err(IRacingSDKError::Parse {
            context: "IBT variable type validation".to_string(),
            details: format!(
                "Variable `{}` uses unsupported type {:?} for IBT serialization",
                variable.name, variable.data_type
            ),
        });
    }

    Ok(())
}

fn variable_type_to_ibt(data_type: VariableType) -> Option<i32> {
    match data_type {
        VariableType::Char | VariableType::Int8 => Some(0),
        VariableType::Bool => Some(1),
        VariableType::Int32 => Some(2),
        VariableType::BitField => Some(3),
        VariableType::Float32 => Some(4),
        VariableType::Float64 => Some(5),
        _ => None,
    }
}

fn pad_to_offset(file: &mut File, target_offset: u64, path: &Path) -> Result<()> {
    let current = file
        .stream_position()
        .map_err(|source| IRacingSDKError::File {
            path: path.to_path_buf(),
            source,
        })?;

    if current > target_offset {
        return Err(IRacingSDKError::Parse {
            context: "IBT writer padding".to_string(),
            details: format!(
                "Current position {} exceeds target offset {}",
                current, target_offset
            ),
        });
    }

    let pad_len = (target_offset - current) as usize;
    if pad_len > 0 {
        let zeros = vec![0u8; pad_len];
        file.write_all(&zeros)
            .map_err(|source| IRacingSDKError::File {
                path: path.to_path_buf(),
                source,
            })?;
    }

    Ok(())
}

fn align_up(value: u64, alignment: u64) -> Result<u64> {
    if alignment == 0 {
        return Ok(value);
    }

    let add = alignment - 1;
    let adjusted = value
        .checked_add(add)
        .ok_or_else(|| IRacingSDKError::Parse {
            context: "IBT layout alignment".to_string(),
            details: "Alignment overflow".to_string(),
        })?;

    Ok((adjusted / alignment) * alignment)
}

fn write_i32(buf: &mut [u8], offset: usize, value: i32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_i64(buf: &mut [u8], offset: usize, value: i64) {
    buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_f64(buf: &mut [u8], offset: usize, value: f64) {
    buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_c_string(dst: &mut [u8], value: &str) {
    dst.fill(0);
    if dst.is_empty() {
        return;
    }

    let max_len = dst.len() - 1;
    let src = value.as_bytes();
    let copy_len = src.len().min(max_len);
    dst[..copy_len].copy_from_slice(&src[..copy_len]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VarData;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn build_source_schema() -> VariableSchema {
        let mut vars = HashMap::new();

        vars.insert(
            "SessionTime".to_string(),
            VariableInfo {
                name: "SessionTime".to_string(),
                data_type: VariableType::Float64,
                offset: 0,
                count: 1,
                count_as_time: false,
                units: "s".to_string(),
                description: "Session time".to_string(),
            },
        );

        vars.insert(
            "Speed".to_string(),
            VariableInfo {
                name: "Speed".to_string(),
                data_type: VariableType::Float32,
                offset: 24,
                count: 1,
                count_as_time: false,
                units: "m/s".to_string(),
                description: "Speed".to_string(),
            },
        );

        vars.insert(
            "OnPitRoad".to_string(),
            VariableInfo {
                name: "OnPitRoad".to_string(),
                data_type: VariableType::Bool,
                offset: 40,
                count: 1,
                count_as_time: false,
                units: "".to_string(),
                description: "Pit road status".to_string(),
            },
        );

        vars.insert(
            "Gear".to_string(),
            VariableInfo {
                name: "Gear".to_string(),
                data_type: VariableType::Int32,
                offset: 16,
                count: 1,
                count_as_time: false,
                units: "".to_string(),
                description: "Current gear".to_string(),
            },
        );

        VariableSchema::new(vars, 41).expect("valid source schema")
    }

    fn build_source_frame(
        schema: &VariableSchema,
        session_time: f64,
        speed: f32,
        gear: i32,
        on_pit_road: bool,
    ) -> Vec<u8> {
        let mut frame = vec![0u8; schema.frame_size];

        let session_time_info = schema.get_variable("SessionTime").unwrap();
        frame[session_time_info.offset..session_time_info.offset + 8]
            .copy_from_slice(&session_time.to_le_bytes());

        let speed_info = schema.get_variable("Speed").unwrap();
        frame[speed_info.offset..speed_info.offset + 4].copy_from_slice(&speed.to_le_bytes());

        let gear_info = schema.get_variable("Gear").unwrap();
        frame[gear_info.offset..gear_info.offset + 4].copy_from_slice(&gear.to_le_bytes());

        let on_pit_road_info = schema.get_variable("OnPitRoad").unwrap();
        frame[on_pit_road_info.offset] = u8::from(on_pit_road);

        frame
    }

    fn temp_ibt_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time moved backwards")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "iracing-sdk-{name}-{}-{nanos}.ibt",
            std::process::id()
        ))
    }

    #[test]
    fn projection_repacks_variables_into_compact_schema() -> Result<()> {
        let source_schema = build_source_schema();

        let projection =
            FrameProjection::from_variable_names(&source_schema, ["Speed", "OnPitRoad"])?;
        let target = projection.target_schema();

        assert_eq!(target.frame_size, 5);

        let speed = target.get_variable("Speed").unwrap();
        assert_eq!(speed.offset, 0);

        let on_pit_road = target.get_variable("OnPitRoad").unwrap();
        assert_eq!(on_pit_road.offset, 4);

        Ok(())
    }

    #[test]
    fn writer_roundtrip_with_projection() -> Result<()> {
        let source_schema = build_source_schema();
        let projection =
            FrameProjection::from_variable_names(&source_schema, ["Speed", "OnPitRoad"])?;

        let options = IbtWriteOptions {
            tick_rate: 60,
            session_info_update: 7,
            session_yaml: Some("WeekendInfo:\n  TrackName: test\n".to_string()),
            start_time: 12.5,
            ..IbtWriteOptions::default()
        };

        let file_path = temp_ibt_path("writer-roundtrip");

        {
            let mut writer =
                IbtWriter::create(&file_path, projection.target_schema().clone(), options)?;

            let frame_a = build_source_frame(&source_schema, 1.0, 100.25, 3, false);
            let frame_b = build_source_frame(&source_schema, 2.0, 98.5, 2, true);

            writer.write_projected_frame(&frame_a, &projection)?;
            writer.write_projected_frame(&frame_b, &projection)?;
            writer.finish()?;
        }

        let mut reader = IbtReader::open(&file_path)?;
        assert_eq!(reader.total_frames(), 2);
        assert_eq!(reader.variables().frame_size, 5);
        assert_eq!(reader.disk_header().record_count, 2);

        let speed_info = reader
            .variables()
            .get_variable("Speed")
            .expect("speed variable");
        let speed_info = speed_info.clone();

        let on_pit_road_info = reader
            .variables()
            .get_variable("OnPitRoad")
            .expect("on pit road variable");
        let on_pit_road_info = on_pit_road_info.clone();

        let (frame0, _, _) = reader.read_next_frame()?.expect("frame 0");
        let (frame1, _, _) = reader.read_next_frame()?.expect("frame 1");

        let speed0: f32 = f32::from_bytes(&frame0, &speed_info)?;
        let speed1: f32 = f32::from_bytes(&frame1, &speed_info)?;

        let on_pit0: bool = bool::from_bytes(&frame0, &on_pit_road_info)?;
        let on_pit1: bool = bool::from_bytes(&frame1, &on_pit_road_info)?;

        assert!((speed0 - 100.25).abs() < f32::EPSILON);
        assert!((speed1 - 98.5).abs() < f32::EPSILON);
        assert!(!on_pit0);
        assert!(on_pit1);

        std::fs::remove_file(&file_path).ok();
        Ok(())
    }

    #[test]
    fn writer_rejects_unsupported_types() {
        let mut vars = HashMap::new();
        vars.insert(
            "Unsupported".to_string(),
            VariableInfo {
                name: "Unsupported".to_string(),
                data_type: VariableType::UInt16,
                offset: 0,
                count: 1,
                count_as_time: false,
                units: "".to_string(),
                description: "unsupported".to_string(),
            },
        );

        let schema = VariableSchema::new(vars, 2).expect("schema");
        let file_path = temp_ibt_path("writer-unsupported");

        let result = IbtWriter::create(&file_path, schema, IbtWriteOptions::default());
        assert!(result.is_err());

        // Best effort cleanup in case a file was created before validation failure.
        std::fs::remove_file(&file_path).ok();
    }

    #[test]
    fn projection_rejects_empty_variable_list() {
        let schema = build_source_schema();
        let result = FrameProjection::from_variable_names(&schema, Vec::<String>::new());
        assert!(result.is_err());
    }

    #[test]
    fn projection_rejects_duplicate_variables() {
        let schema = build_source_schema();
        let result = FrameProjection::from_variable_names(&schema, ["Speed", "Speed"]);
        assert!(result.is_err());
    }

    #[test]
    fn projection_rejects_missing_variables() {
        let schema = build_source_schema();
        let result = FrameProjection::from_variable_names(&schema, ["Speed", "NonExistent"]);
        assert!(result.is_err());
    }

    #[test]
    fn projection_validates_source_frame_size() -> Result<()> {
        let schema = build_source_schema();
        let projection = FrameProjection::from_variable_names(&schema, ["Speed"])?;

        let small_frame = vec![0u8; 2]; // Too small
        let result = projection.project_frame(&small_frame);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn projection_validates_target_frame_size() -> Result<()> {
        let schema = build_source_schema();
        let projection = FrameProjection::from_variable_names(&schema, ["Speed"])?;

        let source_frame = build_source_frame(&schema, 1.0, 100.0, 3, false);
        let mut wrong_size_target = vec![0u8; 2]; // Should be 4 for Float32

        let result = projection.project_into(&source_frame, &mut wrong_size_target);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn writer_validates_frame_size_on_write() -> Result<()> {
        let schema = build_source_schema();
        let projection = FrameProjection::from_variable_names(&schema, ["Speed"])?;
        let file_path = temp_ibt_path("writer-frame-size");

        let mut writer = IbtWriter::create(
            &file_path,
            projection.target_schema().clone(),
            IbtWriteOptions::default(),
        )?;

        let wrong_size_frame = vec![0u8; 2]; // Should be 4
        let result = writer.write_frame(&wrong_size_frame);
        assert!(result.is_err());

        std::fs::remove_file(&file_path).ok();
        Ok(())
    }

    #[test]
    fn writer_rejects_writes_after_finalize() -> Result<()> {
        let schema = build_source_schema();
        let projection = FrameProjection::from_variable_names(&schema, ["Speed"])?;
        let file_path = temp_ibt_path("writer-after-finalize");

        let mut writer = IbtWriter::create(
            &file_path,
            projection.target_schema().clone(),
            IbtWriteOptions::default(),
        )?;

        let frame = vec![0u8; projection.target_schema().frame_size];
        writer.write_frame(&frame)?;
        writer.finish()?;

        // Writer is moved by finish(), so we can't test this directly
        // The important part is that finish() consumes the writer

        std::fs::remove_file(&file_path).ok();
        Ok(())
    }

    #[test]
    fn ibt_write_options_validates_tick_rate() {
        let mut options = IbtWriteOptions::default();
        options.tick_rate = 0;
        assert!(options.validate().is_err());

        options.tick_rate = -1;
        assert!(options.validate().is_err());

        options.tick_rate = 60;
        assert!(options.validate().is_ok());
    }

    #[test]
    fn ibt_write_options_validates_lap_count() {
        let mut options = IbtWriteOptions::default();
        options.lap_count = -1;
        assert!(options.validate().is_err());

        options.lap_count = 0;
        assert!(options.validate().is_ok());

        options.lap_count = 100;
        assert!(options.validate().is_ok());
    }

    #[test]
    fn ibt_write_options_validates_end_time() {
        let mut options = IbtWriteOptions::default();
        options.start_time = 10.0;
        options.end_time = Some(5.0); // Before start time
        assert!(options.validate().is_err());

        options.end_time = Some(15.0); // After start time
        assert!(options.validate().is_ok());

        options.end_time = None;
        assert!(options.validate().is_ok());
    }

    #[test]
    fn writer_handles_session_yaml() -> Result<()> {
        let schema = build_source_schema();
        let file_path = temp_ibt_path("writer-yaml");

        let yaml_content = "WeekendInfo:\n  TrackName: TestTrack\n  TrackCity: TestCity\n";

        let options = IbtWriteOptions {
            session_yaml: Some(yaml_content.to_string()),
            ..IbtWriteOptions::default()
        };

        {
            let mut writer = IbtWriter::create(&file_path, schema.clone(), options)?;
            let frame = build_source_frame(&schema, 1.0, 50.0, 2, false);
            writer.write_frame(&frame)?;
            writer.finish()?;
        }

        let reader = IbtReader::open(&file_path)?;
        let yaml = reader.session_yaml()?;
        assert!(yaml.is_some());
        assert!(yaml.unwrap().contains("TestTrack"));

        std::fs::remove_file(&file_path).ok();
        Ok(())
    }

    #[test]
    fn writer_handles_empty_session_yaml() -> Result<()> {
        let schema = build_source_schema();
        let file_path = temp_ibt_path("writer-no-yaml");

        let options = IbtWriteOptions {
            session_yaml: None,
            ..IbtWriteOptions::default()
        };

        {
            let mut writer = IbtWriter::create(&file_path, schema.clone(), options)?;
            let frame = build_source_frame(&schema, 1.0, 50.0, 2, false);
            writer.write_frame(&frame)?;
            writer.finish()?;
        }

        let reader = IbtReader::open(&file_path)?;
        let yaml = reader.session_yaml()?;
        assert!(yaml.is_none() || yaml.as_ref().map(|s| s.is_empty()).unwrap_or(true));

        std::fs::remove_file(&file_path).ok();
        Ok(())
    }

    #[test]
    fn writer_computes_end_time_from_frames() -> Result<()> {
        let schema = build_source_schema();
        let file_path = temp_ibt_path("writer-end-time");

        let options = IbtWriteOptions {
            tick_rate: 60,
            start_time: 0.0,
            end_time: None,
            ..IbtWriteOptions::default()
        };

        {
            let mut writer = IbtWriter::create(&file_path, schema.clone(), options)?;

            for _ in 0..60 {
                let frame = build_source_frame(&schema, 1.0, 50.0, 2, false);
                writer.write_frame(&frame)?;
            }

            writer.finish()?;
        }

        let reader = IbtReader::open(&file_path)?;
        let disk_header = reader.disk_header();

        // 60 frames at 60Hz = 1 second
        assert!((disk_header.end_time - 1.0).abs() < 0.01);

        std::fs::remove_file(&file_path).ok();
        Ok(())
    }

    #[test]
    fn writer_preserves_explicit_end_time() -> Result<()> {
        let schema = build_source_schema();
        let file_path = temp_ibt_path("writer-explicit-end-time");

        let options = IbtWriteOptions {
            tick_rate: 60,
            start_time: 0.0,
            end_time: Some(100.0),
            ..IbtWriteOptions::default()
        };

        {
            let mut writer = IbtWriter::create(&file_path, schema.clone(), options)?;
            let frame = build_source_frame(&schema, 1.0, 50.0, 2, false);
            writer.write_frame(&frame)?;
            writer.finish()?;
        }

        let reader = IbtReader::open(&file_path)?;
        let disk_header = reader.disk_header();

        assert!((disk_header.end_time - 100.0).abs() < 0.01);

        std::fs::remove_file(&file_path).ok();
        Ok(())
    }

    #[test]
    fn projection_preserves_variable_metadata() -> Result<()> {
        let schema = build_source_schema();
        let projection = FrameProjection::from_variable_names(&schema, ["Speed", "OnPitRoad"])?;

        let target = projection.target_schema();
        let speed = target.get_variable("Speed").unwrap();

        assert_eq!(speed.name, "Speed");
        assert_eq!(speed.data_type, VariableType::Float32);
        assert_eq!(speed.units, "m/s");
        assert_eq!(speed.description, "Speed");

        Ok(())
    }

    #[test]
    fn writer_handles_large_frame_counts() -> Result<()> {
        let schema = build_source_schema();
        let file_path = temp_ibt_path("writer-large-count");

        {
            let mut writer = IbtWriter::create(&file_path, schema.clone(), IbtWriteOptions::default())?;

            for _ in 0..10000 {
                let frame = build_source_frame(&schema, 1.0, 50.0, 2, false);
                writer.write_frame(&frame)?;
            }

            writer.finish()?;
        }

        let reader = IbtReader::open(&file_path)?;
        assert_eq!(reader.total_frames(), 10000);

        std::fs::remove_file(&file_path).ok();
        Ok(())
    }

    #[test]
    fn projection_handles_count_as_time_flag() -> Result<()> {
        let mut vars = HashMap::new();
        vars.insert(
            "SessionTime".to_string(),
            VariableInfo {
                name: "SessionTime".to_string(),
                data_type: VariableType::Float64,
                offset: 0,
                count: 1,
                count_as_time: true, // This flag should be preserved
                units: "s".to_string(),
                description: "Session time".to_string(),
            },
        );

        let schema = VariableSchema::new(vars, 8)?;
        let projection = FrameProjection::from_variable_names(&schema, ["SessionTime"])?;

        let projected_var = projection.target_schema().get_variable("SessionTime").unwrap();
        assert!(projected_var.count_as_time);

        Ok(())
    }

    #[test]
    fn writer_frame_count_updates_correctly() -> Result<()> {
        let schema = build_source_schema();
        let file_path = temp_ibt_path("writer-frame-count");

        let mut writer = IbtWriter::create(&file_path, schema.clone(), IbtWriteOptions::default())?;
        assert_eq!(writer.frame_count(), 0);

        let frame = build_source_frame(&schema, 1.0, 50.0, 2, false);
        writer.write_frame(&frame)?;
        assert_eq!(writer.frame_count(), 1);

        writer.write_frame(&frame)?;
        assert_eq!(writer.frame_count(), 2);

        writer.write_frame(&frame)?;
        assert_eq!(writer.frame_count(), 3);

        writer.finish()?;

        std::fs::remove_file(&file_path).ok();
        Ok(())
    }

    #[test]
    fn projection_target_schema_matches_variable_order() -> Result<()> {
        let schema = build_source_schema();
        let projection = FrameProjection::from_variable_names(
            &schema,
            ["OnPitRoad", "Speed", "SessionTime"],
        )?;

        let target = projection.target_schema();

        // Variables should be repacked in order: OnPitRoad (1 byte), Speed (4 bytes), SessionTime (8 bytes)
        let on_pit_road = target.get_variable("OnPitRoad").unwrap();
        let speed = target.get_variable("Speed").unwrap();
        let session_time = target.get_variable("SessionTime").unwrap();

        assert_eq!(on_pit_road.offset, 0);
        assert_eq!(speed.offset, 1);
        assert_eq!(session_time.offset, 5);
        assert_eq!(target.frame_size, 13); // 1 + 4 + 8

        Ok(())
    }
}