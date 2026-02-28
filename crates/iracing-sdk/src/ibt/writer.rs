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
    /// Create default `IbtWriteOptions` suitable for creating a new IBT writer.
    ///
    /// The defaults set the header status to `HEADER_STATUS_CONNECTED`, `tick_rate` to 60,
    /// zero session update and lap counters, no session YAML, and no explicit `end_time`.
    ///
    /// # Examples
    ///
    /// ```
    /// let opts = IbtWriteOptions::default();
    /// assert_eq!(opts.tick_rate, 60);
    /// assert!(opts.session_yaml.is_none());
    /// ```
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
    /// Constructs `IbtWriteOptions` from a live `WindowsConnection` by reading its header and optional session YAML.
    ///
    /// The returned options use the connection's `status`, `tick_rate`, and `session_info_update` values and include the session YAML when available. `start_date` is set to the current wall-clock UNIX timestamp; `start_time`, `end_time`, and `lap_count` remain at their defaults because they are not available from shared memory before recording begins.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // `conn` is a `WindowsConnection` obtained from the SDK.
    /// let opts = IbtWriteOptions::from_connection(&conn).unwrap();
    /// ```
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

    /// Create `IbtWriteOptions` populated from an existing `IbtReader`.
    ///
    /// The returned options copy the reader's header and disk-header fields and include
    /// the reader's session YAML (if any).
    ///
    /// # Errors
    ///
    /// Returns an error if extracting the reader's session YAML fails.
    ///
    /// # Examples
    ///
    /// ```
    /// // given `reader: IbtReader`
    /// let opts = IbtWriteOptions::from_reader(&reader).unwrap();
    /// assert_eq!(opts.tick_rate, reader.header().tick_rate);
    /// assert_eq!(opts.start_date, reader.disk_header().start_date);
    /// ```
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

    /// Validates numeric and temporal fields of `IbtWriteOptions`.
    ///
    /// Ensures `tick_rate` is greater than zero, `lap_count` is zero or positive,
    /// and when `end_time` is present it is greater than or equal to `start_time`.
    ///
    /// # Returns
    ///
    /// `Ok(())` if all checks pass; `Err(IRacingSDKError::Parse)` if any constraint is violated.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut opts = IbtWriteOptions::default();
    /// // default options are valid
    /// opts.validate().unwrap();
    ///
    /// // invalid tick_rate
    /// opts.tick_rate = 0;
    /// assert!(opts.validate().is_err());
    /// ```
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
    /// Create a compact projection that selects and repacks the given variables from a source schema into a tightly packed target schema in the same order.
    ///
    /// The returned projection contains a target `VariableSchema` whose variables are laid out consecutively (no padding) in the order provided by `variable_names`, and a list of `ProjectedField` entries that map source offsets to target offsets and widths.
    ///
    /// # Errors
    /// - Returns `IRacingSDKError::Parse` if `variable_names` is empty, contains duplicate names, references a name not present in `source_schema`, or if any size/offset arithmetic overflows while computing target offsets or field sizes.
    /// - Returns `IRacingSDKError::Memory` if a selected variable's source range would read past `source_schema.frame_size`.
    ///
    /// # Examples
    ///
    /// ```
    /// // Select "Speed" and "OnPitRoad" from a source schema and create a compact projection.
    /// let projection = FrameProjection::from_variable_names(&source_schema, vec!["Speed", "OnPitRoad"])?;
    /// assert_eq!(projection.target_schema.frame_size, /* expected packed size */);
    /// ```
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

    /// Creates a compact FrameProjection that selects every variable from `source_schema`, ordered by each variable's source offset and then by name.
    ///
    /// Returns an error if the source schema contains no variables.
    ///
    /// # Examples
    ///
    /// ```
    /// // assume `schema` is a VariableSchema populated with variables
    /// let proj = FrameProjection::from_all(&schema).expect("should build projection");
    /// assert_eq!(proj.source_frame_size, schema.frame_size());
    /// ```
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

    /// Access the compact target schema produced by this projection.
    ///
    /// Returns a reference to the projection's compact `VariableSchema`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use crate::ibt::writer::FrameProjection;
    /// # let proj: FrameProjection = /* obtain projection via `from_all` or `from_variable_names` */ unimplemented!();
    /// let schema = proj.target_schema();
    /// ```
    pub fn target_schema(&self) -> &VariableSchema {
        &self.target_schema
    }

    /// Create a new compact frame by projecting selected fields from a source frame.
    ///
    /// Returns a newly allocated `Vec<u8>` whose length equals the projection's target frame size.
    /// Errors if the source frame is too short or any projected field would read/write out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// // assume `proj` is a prepared `FrameProjection` and `source` is a source frame slice
    /// let source = vec![0u8; proj.source_frame_size];
    /// let target = proj.project_frame(&source).unwrap();
    /// assert_eq!(target.len(), proj.target_schema().frame_size);
    /// ```
    pub fn project_frame(&self, source_frame: &[u8]) -> Result<Vec<u8>> {
        let mut output = vec![0u8; self.target_schema.frame_size];
        self.project_into(source_frame, &mut output)?;
        Ok(output)
    }

    /// Project bytes from a source frame into an existing target frame buffer according to this projection.
    ///
    /// The method copies each projected field's byte range from `source_frame` into the corresponding
    /// offset in `target_frame`. Both slices must be sized to the projection's expectations: `source_frame`
    /// must be at least `source_frame_size` bytes and `target_frame` must equal the projection's target
    /// schema `frame_size`.
    ///
    /// # Errors
    ///
    /// Returns an `IRacingSDKError::Parse` when:
    /// - `source_frame` is smaller than the projection's `source_frame_size`.
    /// - `target_frame` length does not equal the projection's target schema frame size.
    /// - Any individual field copy would access out-of-bounds ranges in either `source_frame` or `target_frame`.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an `IRacingSDKError::Parse` on failure.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Given a `projection` (FrameProjection) and prepared buffers:
    /// let source = vec![0u8; projection.source_frame_size];
    /// let mut target = vec![0u8; projection.target_schema.frame_size];
    ///
    /// // Populate `source` as needed, then project into `target`:
    /// projection.project_into(&source, &mut target).expect("projection failed");
    /// ```
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
    /// Create and initialize an IBT writer file at the given filesystem path.
    ///
    /// The new writer is prepared to accept frames: the schema and options are
    /// validated, the on-disk layout is planned, initial file and disk headers are
    /// written, variable headers and optional session info are emitted, and the
    /// file is positioned at the start of the frame data region.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iracing_sdk::ibt::{IbtWriter, IbtWriteOptions, VariableSchema};
    /// // Construct or obtain a VariableSchema appropriate for your data.
    /// let schema = VariableSchema::default();
    /// let options = IbtWriteOptions::default();
    /// let writer = IbtWriter::create("session.ibt", schema, options).unwrap();
    /// assert_eq!(writer.frame_count(), 0);
    /// ```
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

    /// Returns a reference to the writer's variable schema.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Obtain an `IbtWriter` named `writer` by creating or opening one,
    /// // then read its schema:
    /// let schema = writer.schema();
    /// ```
    pub fn schema(&self) -> &VariableSchema {
        &self.schema
    }

    /// Get the number of frames written so far.
    ///
    /// Returns the current frame count as a `u32`.
    ///
    /// # Examples
    ///
    /// ```
    /// struct Dummy { frame_count: u32 }
    /// impl Dummy { fn frame_count(&self) -> u32 { self.frame_count } }
    /// let d = Dummy { frame_count: 3 };
    /// assert_eq!(d.frame_count(), 3);
    /// ```
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Write a single frame using the writer's target schema.
    ///
    /// On success this increments the writer's internal frame count and persists the frame bytes to disk.
    ///
    /// # Errors
    ///
    /// Returns `Err(IRacingSDKError)` if:
    /// - the writer has been finalized;
    /// - `frame.len()` does not equal the writer schema's `frame_size`;
    /// - the internal record count would overflow `i32::MAX`;
    /// - or an underlying file I/O error occurs while writing.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::fs::File;
    /// # use crates::iracing_sdk::ibt::IbtWriter;
    /// # // assume `writer` is a valid, created `IbtWriter` and `frame` matches `writer.schema().frame_size`
    /// # let mut writer: IbtWriter = unimplemented!();
    /// let frame = vec![0u8; writer.schema().frame_size];
    /// writer.write_frame(&frame).expect("write frame");
    /// ```
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

    /// Projects a source frame using the given projection and writes the resulting frame using the writer's schema.
    ///
    /// Errors if the projection's target frame size does not match the writer's schema or if writing to the file fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // `writer` is an initialized IbtWriter, `projection` is a FrameProjection built from a source schema,
    /// // and `source_frame` is a byte slice containing a frame from the source schema.
    /// writer.write_projected_frame(&source_frame, &projection).unwrap();
    /// ```
    pub fn write_projected_frame(
        &mut self,
        source_frame: &[u8],
        projection: &FrameProjection,
    ) -> Result<()> {
        let mut projected = vec![0u8; self.schema.frame_size];
        self.write_projected_frame_with_buffer(source_frame, projection, &mut projected)
    }

    /// Project a source frame into a provided buffer and write the resulting frame to the IBT file.
    ///
    /// Uses the caller-supplied `target_buffer` to avoid per-frame allocations in high-frequency recording paths.
    /// The projection's target frame size must match the writer's schema frame size.
    ///
    /// # Errors
    ///
    /// Returns an error if the projection target frame size does not match the writer frame size,
    /// if the projection fails (e.g., out-of-bounds copy), or if writing to the file fails.
    ///
    /// # Examples
    ///
    /// ```
    /// // Preallocate source and target buffers appropriate for the schemas:
    /// let source_frame = vec![0u8; projection.source_frame_size];
    /// let mut target_buffer = vec![0u8; writer.schema().frame_size];
    /// writer.write_projected_frame_with_buffer(&source_frame, &projection, &mut target_buffer).unwrap();
    /// ```
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

    /// Finalize the writer by updating headers and flushing all pending output.
    ///
    /// Updates the file and disk headers with final metadata (including final record count and end time),
    /// writes those headers to disk, flushes the file, and marks the writer as finalized so no further
    /// frames may be written.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success; `Err(IRacingSDKError)` if header construction, seeking, writing, or flushing fails.
    ///
    /// # Examples
    ///
    /// ```
    /// // After creating an `IbtWriter` and writing frames:
    /// // let writer = IbtWriter::create(path, schema, options).unwrap();
    /// // writer.write_frame(&frame).unwrap();
    /// // ...
    /// // Consume the writer to finalize the file:
    /// writer.finish().unwrap();
    /// ```
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

    /// Checks that the writer has not been finalized.
    ///
    /// Returns `Ok(())` if the writer is still writable; returns an `IRacingSDKError::Parse` error
    /// with context "IBT writer" and details "Writer is already finalized" if the writer has been finalized.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the writer is not finalized, `Err(IRacingSDKError::Parse { .. })` if it is finalized.
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

/// Plan on-disk layout for an IBT file given a variable schema and optional session YAML.
///
/// This computes:
/// - ordered variable list (sorted by source offset and name),
/// - session info bytes (null-terminated if provided),
/// - offsets for the variable header block, session info block, and aligned frame data start,
/// - integer-typed lengths/offsets converted to `i32` for on-disk headers,
/// and returns a `WriterLayout` containing those values.
///
/// Errors if any computed length or offset overflows 32-bit ranges, if alignment/size arithmetic overflows,
/// or if any variable has an unsupported IBT type.
///
/// # Examples
///
/// ```rust,no_run
/// # use iracing_sdk::ibt::writer::plan_layout;
/// # use iracing_sdk::schema::VariableSchema;
/// // Given a VariableSchema `schema` (from your application), optionally pass session YAML:
/// let schema = VariableSchema::default(); // placeholder: construct an appropriate schema
/// let layout = plan_layout(&schema, Some("--- session: info ---")).expect("layout planning failed");
/// assert!(layout.frame_data_offset > 0);
/// ```
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

/// Produce a vector of the schema's variables ordered by increasing `offset`, using `name` to break ties.
///
/// # Examples
///
/// ```no_run
/// // Given a VariableSchema `schema`, this returns the variables sorted by offset,
/// // then by name for variables that share the same offset.
/// let ordered = sorted_variables(&schema);
/// for i in 1..ordered.len() {
///     let a = &ordered[i - 1];
///     let b = &ordered[i];
///     assert!(a.offset < b.offset || (a.offset == b.offset && a.name <= b.name));
/// }
/// ```
fn sorted_variables(schema: &VariableSchema) -> Vec<VariableInfo> {
    let mut variables: Vec<VariableInfo> = schema.variables.values().cloned().collect();
    variables.sort_unstable_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.name.cmp(&right.name))
    });
    variables
}

/// Serialize IBT file header fields into a FILE_HEADER_SIZE-byte array.
///
/// The returned array contains the packed IBT file header constructed from the
/// provided schema, writer options, and layout, with `record_count` and
/// `frame_data_offset` filled into the appropriate header fields.
///
/// # Errors
///
/// Returns `IRacingSDKError::Parse` if `schema.frame_size` or the number of
/// variables in `layout.ordered_variables` does not fit into an `i32`.
///
/// # Examples
///
/// ```
/// // Construct minimal inputs (types are from this crate).
/// let schema = VariableSchema { frame_size: 128, ..Default::default() };
/// let options = IbtWriteOptions::default();
/// let layout = WriterLayout {
///     ordered_variables: vec![],
///     session_bytes: vec![],
///     var_header_offset: 0,
///     session_info_offset: 0,
///     session_info_len: 0,
///     frame_data_offset: 0,
/// };
/// let header = build_header_bytes(&schema, &options, &layout, 0, 0).unwrap();
/// assert_eq!(header.len(), FILE_HEADER_SIZE);
/// ```
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

/// Build the 32-byte disk subheader used in the IBT file.
///
/// The returned buffer encodes, in little-endian order and fixed offsets:
/// - bytes 0..8: `start_date` as `i64`
/// - bytes 8..16: `start_time` as `f64`
/// - bytes 16..24: `end_time` as `f64`
/// - bytes 24..28: `lap_count` as `i32`
/// - bytes 28..32: `record_count` as `i32`
///
/// # Examples
///
/// ```
/// let start_date: i64 = 1_700_000_000;
/// let start_time: f64 = 123.5;
/// let end_time: f64 = 456.75;
/// let lap_count: i32 = 3;
/// let record_count: i32 = 42;
///
/// let bytes = build_disk_header_bytes(start_date, start_time, end_time, lap_count, record_count);
/// assert_eq!(bytes.len(), 32);
///
/// let sd = i64::from_le_bytes(bytes[0..8].try_into().unwrap());
/// let st = f64::from_le_bytes(bytes[8..16].try_into().unwrap());
/// let et = f64::from_le_bytes(bytes[16..24].try_into().unwrap());
/// let lc = i32::from_le_bytes(bytes[24..28].try_into().unwrap());
/// let rc = i32::from_le_bytes(bytes[28..32].try_into().unwrap());
///
/// assert_eq!(sd, start_date);
/// assert_eq!(st, start_time);
/// assert_eq!(et, end_time);
/// assert_eq!(lc, lap_count);
/// assert_eq!(rc, record_count);
/// ```
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

/// Serialize a `VariableInfo` into its fixed-size IBT variable header representation.
///
/// Produces a IRSDK_VAR_HEADER_SIZE-byte array containing the variable type, offset,
/// count, the count-as-time flag, and the name/description/units as C-style fixed fields.
///
/// # Errors
///
/// Returns an `IRacingSDKError::Parse` if the variable's type is unsupported for IBT
/// or if the variable's offset or count do not fit in a 32-bit signed integer.
///
/// # Examples
///
/// ```
/// use crates::iracing_sdk::ibt::writer::{encode_var_header};
/// use crates::iracing_sdk::variable::{VariableInfo, VariableType};
///
/// let var = VariableInfo {
///     name: "Speed".to_string(),
///     description: "Vehicle speed".to_string(),
///     units: "kph".to_string(),
///     data_type: VariableType::Float32,
///     offset: 0,
///     count: 1,
///     count_as_time: false,
/// };
///
/// let header = encode_var_header(&var).expect("encode variable header");
/// assert_eq!(header.len(), super::IRSDK_VAR_HEADER_SIZE);
/// ```
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

/// Validates that a variable's data type is supported for IBT serialization.
///
/// Returns `Ok(())` if the variable's `data_type` can be mapped to an IBT type, or returns an
/// `IRacingSDKError::Parse` describing the unsupported type otherwise.
///
/// # Examples
///
/// ```
/// // Construct a VariableInfo with a supported type (example fields omitted for brevity)
/// let var = VariableInfo {
///     name: "speed".into(),
///     data_type: VariableType::Float32,
///     offset: 0,
///     count: 1,
///     description: "".into(),
///     units: "".into(),
///     count_as_time: false,
/// };
/// ensure_supported_variable_type(&var).unwrap();
/// ```
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

/// Maps a `VariableType` to its corresponding IBT numeric type code.
///
/// Returns `Some(code)` for supported variable types, or `None` when the type has no IBT mapping.
///
/// # Examples
///
/// ```
/// let code = variable_type_to_ibt(VariableType::Float32);
/// assert_eq!(code, Some(4));
/// ```
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

/// Pads `file` with zero bytes until its stream position equals `target_offset`.
///
/// If the file is already beyond `target_offset`, an `IRacingSDKError::Parse` error is returned.
/// IO errors are returned as `IRacingSDKError::File` containing the provided `path`.
///
/// # Examples
///
/// ```
/// use std::fs::File;
/// use std::path::Path;
/// // create a temporary file path in the system temp directory
/// let tmp = std::env::temp_dir().join("pad_to_offset_example.bin");
/// let mut file = File::create(&tmp).unwrap();
/// // ensure we pad to offset 128
/// super::pad_to_offset(&mut file, 128, Path::new(&tmp)).unwrap();
/// assert_eq!(file.stream_position().unwrap(), 128);
/// ```
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

/// Round `value` up to the next multiple of `alignment`.
///
/// If `alignment` is 0, the original `value` is returned unchanged.
///
/// # Errors
///
/// Returns `IRacingSDKError::Parse` if computing the aligned value overflows u64.
///
/// # Examples
///
/// ```
/// // 5 aligned up to 4 -> 8
/// assert_eq!(align_up(5, 4).unwrap(), 8);
/// // alignment 0 returns the input
/// assert_eq!(align_up(7, 0).unwrap(), 7);
/// ```
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

/// Writes a 32-bit signed integer into `buf` at `offset` in little-endian byte order.
///
/// This will panic if `offset + 4` is greater than `buf.len()`.
///
/// # Examples
///
/// ```
/// let mut buf = [0u8; 4];
/// write_i32(&mut buf, 0, -42);
/// assert_eq!(i32::from_le_bytes(buf), -42);
/// ```
fn write_i32(buf: &mut [u8], offset: usize, value: i32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// Writes a 64-bit signed integer to the given byte slice in little-endian order at the specified offset.
///
/// The function overwrites 8 bytes starting at `offset` with `value` encoded as little-endian.
///
/// # Panics
///
/// Panics if `offset + 8` is greater than `buf.len()`.
///
/// # Examples
///
/// ```
/// let mut buf = [0u8; 16];
/// write_i64(&mut buf, 4, -1i64);
/// assert_eq!(&buf[4..12], &(-1i64).to_le_bytes());
/// ```
fn write_i64(buf: &mut [u8], offset: usize, value: i64) {
    buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

/// Writes an `f64` value into a byte buffer in little-endian order at the specified offset.
///
/// Panics if `offset + 8` is greater than `buf.len()`.
///
/// # Examples
///
/// ```
/// let mut buf = [0u8; 16];
/// write_f64(&mut buf, 4, 1.5);
/// assert_eq!(&buf[4..12], &1.5f64.to_le_bytes());
/// ```
fn write_f64(buf: &mut [u8], offset: usize, value: f64) {
    buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

/// Writes `value` into `dst` as a C-style, null-terminated string, truncating if necessary and zero-padding the remainder.
///
/// The written string occupies at most `dst.len() - 1` bytes of actual text; the final byte is always `0` when `dst` is non-empty.
///
/// # Examples
///
/// ```
/// let mut buf = [0u8; 8];
/// write_c_string(&mut buf, "hi");
/// assert_eq!(&buf, b"hi\0\0\0\0\0");
///
/// let mut buf = [0u8; 4];
/// write_c_string(&mut buf, "longer-than-3");
/// // only 3 bytes of text fit, last byte remains null
/// assert_eq!(&buf, b"lon\0");
/// ```
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

    /// Builds a sample source `VariableSchema` containing `SessionTime`, `Speed`, `OnPitRoad`, and `Gear`
    /// with their expected types, offsets, counts, and metadata.
    ///
    /// # Examples
    ///
    /// ```
    /// let schema = build_source_schema();
    /// assert_eq!(schema.frame_size(), 41);
    /// assert!(schema.variable("Speed").is_some());
    /// ```
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

    /// Builds a telemetry source frame byte buffer for the given schema with the specified fields populated.
    ///
    /// The returned buffer has length `schema.frame_size` and contains little-endian encoded values for
    /// the `SessionTime` (f64), `Speed` (f32), `Gear` (i32), and `OnPitRoad` (u8 as boolean) variables
    /// at the offsets defined by `schema`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Construct or obtain a VariableSchema that defines variables named
    /// // "SessionTime", "Speed", "Gear", and "OnPitRoad", then:
    /// // let schema: VariableSchema = ...;
    /// // let frame = build_source_frame(&schema, 12.34, 55.0, 3, false);
    /// // assert_eq!(frame.len(), schema.frame_size);
    /// ```
    fn build_source_frame(
    schema: &VariableSchema,
    session_time: f64,
    speed: f32,
    gear: i32,
    on_pit_road: bool,
    ) -> Vec<u8> {
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

    /// Create a unique temporary `.ibt` file path in the system temporary directory incorporating `name`.
    ///
    /// The returned path is formed as `iracing-sdk-{name}-{pid}-{nanos}.ibt` and is suitable for use when
    /// creating a temporary IBT file for the given logical `name`.
    ///
    /// # Examples
    ///
    /// ```
    /// let path = temp_ibt_path("session");
    /// assert!(path.extension().and_then(|s| s.to_str()) == Some("ibt"));
    /// assert!(path.to_string_lossy().contains("iracing-sdk-session-"));
    /// ```
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
}
