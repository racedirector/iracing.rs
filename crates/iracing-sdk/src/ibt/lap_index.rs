use crate::{IRacingSDKError, IbtReader, Result, SessionInfo, VarData, VariableInfo};

const WRAP_HIGH_WATERMARK: f32 = 0.8;
const WRAP_LOW_WATERMARK: f32 = 0.2;
const PARTIAL_PROGRESS_EPSILON: f32 = 0.001;

/// Player-car lap index built by scanning an [`IbtReader`] once.
///
/// This façade preserves the low-level IBT parsing contract while adding
/// lap-oriented access for offline telemetry analysis.
pub struct IndexedIbt<'a> {
    reader: &'a mut IbtReader,
    index: LapIndex,
    vars: LapIndexVars,
}

impl<'a> IndexedIbt<'a> {
    /// Builds an IndexedIbt by scanning the given IbtReader once to construct an in-memory lap index while preserving the reader's original position.
    ///
    /// The method samples every frame to detect lap boundaries and produces a LapIndex (including per-frame → lap reverse mapping) and resolved LapIndexVars. If the stream contains zero frames, an empty index is returned. The reader's position is restored before returning; if the reader was at end-of-file, that EOF position is preserved.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Open an IbtReader, then build the indexed view for lap navigation.
    /// # use crates::iracing_sdk::ibt::{IbtReader, IndexedIbt};
    /// # fn example(mut reader: IbtReader) -> Result<(), Box<dyn std::error::Error>> {
    /// let indexed = IndexedIbt::build(&mut reader)?;
    /// println!("Found {} laps", indexed.lap_count());
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(reader: &'a mut IbtReader) -> Result<Self> {
        let vars = LapIndexVars::resolve(reader)?;
        let original_frame = reader.current_frame();
        let total_frames = reader.total_frames();

        let index = if total_frames == 0 {
            LapIndex::default()
        } else {
            reader.seek_to_frame(0)?;

            let mut samples = Vec::with_capacity(total_frames);
            while let Some((frame, _tick, _session_version)) = reader.read_next_frame()? {
                let frame_idx = samples.len();
                samples.push(vars.sample(frame_idx, &frame)?);
            }

            build_lap_index(&samples)
        };

        restore_reader_frame(reader, original_frame)?;

        Ok(Self {
            reader,
            index,
            vars,
        })
    }

    /// Number of laps in the built index.
    ///
    /// # Returns
    ///
    /// `usize` count of laps contained in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// // assuming `ib` is an `IndexedIbt` already built
    /// let count = ib.lap_count();
    /// assert_eq!(count, ib.index().laps.len());
    /// ```
    pub fn lap_count(&self) -> usize {
        self.index.laps.len()
    }

    /// Retrieve lap metadata for the given lap ordinal.
    ///
    /// # Returns
    ///
    /// `Some(&LapRef)` with the lap metadata if `idx` is within range, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // `indexed` is an `IndexedIbt` constructed elsewhere
    /// if let Some(lap) = indexed.lap(0) {
    ///     assert!(lap.start_frame <= lap.end_frame);
    /// }
    /// ```
    pub fn lap(&self, idx: usize) -> Option<&LapRef> {
        self.index.laps.get(idx)
    }

    /// Get the lap that contains the given frame index, if any.
    ///
    /// Returns `Some(&LapRef)` when the index has been assigned to a lap in the built index, or `None` when the frame is not part of any indexed lap.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let indexed: IndexedIbt<'_> = /* previously built index */ unimplemented!();
    /// let frame_idx = 123;
    /// if let Some(lap) = indexed.lap_for_frame(frame_idx) {
    ///     println!("Frame {} is in lap {}", frame_idx, lap.ordinal);
    /// } else {
    ///     println!("Frame {} is not in any lap", frame_idx);
    /// }
    /// ```
    pub fn lap_for_frame(&self, frame_idx: usize) -> Option<&LapRef> {
        let lap_idx = self.index.frame_to_lap.get(frame_idx).copied().flatten()?;
        self.index.laps.get(lap_idx)
    }

    /// Seeks the underlying reader to the first frame of the specified indexed lap.
    ///
    /// # Errors
    ///
    /// Returns an `IRacingSDKError::Parse` if `idx` is outside the range of indexed laps.
    /// Propagates any error returned by the underlying reader's seek operation.
    ///
    /// # Examples
    ///
    /// ```
    /// // Assuming `indexed` is a built `IndexedIbt` with at least one lap:
    /// // indexed.seek_to_lap_start(0).unwrap();
    /// ```
    pub fn seek_to_lap_start(&mut self, idx: usize) -> Result<()> {
        let lap = self
            .index
            .laps
            .get(idx)
            .ok_or_else(|| IRacingSDKError::Parse {
                context: "Lap seek".to_string(),
                details: format!("Lap index {idx} out of range"),
            })?;

        self.reader.seek_to_frame(lap.start_frame)
    }

    /// Create an iterator over the raw frames that belong to the specified lap.
    ///
    /// The returned `LapFrameIter` yields frames in ascending frame order spanning the lap's
    /// inclusive start and end frame. Returns an error if `idx` is out of range or if
    /// seeking the underlying reader fails.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crates::iracing_sdk::ibt::{IndexedIbt, LapFrameIter};
    /// # fn example(mut indexed: IndexedIbt<'_>) -> anyhow::Result<()> {
    /// let mut iter = indexed.lap_frames(0)?;
    /// while let Some(frame_res) = iter.next() {
    ///     let (data, _, _) = frame_res?;
    ///     // process `data`
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn lap_frames(&mut self, idx: usize) -> Result<LapFrameIter<'_>> {
        let lap = self
            .index
            .laps
            .get(idx)
            .ok_or_else(|| IRacingSDKError::Parse {
                context: "Lap frame iteration".to_string(),
                details: format!("Lap index {idx} out of range"),
            })?;

        self.reader.seek_to_frame(lap.start_frame)?;

        Ok(LapFrameIter {
            reader: self.reader,
            next_frame: lap.start_frame,
            end_frame: lap.end_frame,
        })
    }

    /// Access the telemetry variables that were resolved when the lap index was built.
    ///
    /// A reference to the `LapIndexVars` containing the resolved telemetry fields (e.g., lap counters,
    /// progress, pit-road flag, and optional session time) used during indexing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use iracing_sdk::ibt::IndexedIbt;
    /// fn inspect_vars(indexed: &IndexedIbt<'_>) {
    ///     let vars = indexed.variables();
    ///     let _driver_idx = vars.driver_car_idx;
    /// }
    /// ```
    pub fn variables(&self) -> &LapIndexVars {
        &self.vars
    }

    /// Access the built in-memory lap index.
    ///
    /// The returned `LapIndex` contains the ordered `LapRef` slices and the per-frame
    /// reverse lookup mapping from frame index to lap ordinal.
    ///
    /// # Examples
    ///
    /// ```
    /// // `indexed` is an `IndexedIbt<'_>` previously constructed.
    /// let index_ref = indexed.index();
    /// // Inspect number of laps from the index.
    /// let laps_len = index_ref.laps.len();
    /// assert_eq!(laps_len, indexed.lap_count());
    /// ```
    pub fn index(&self) -> &LapIndex {
        &self.index
    }
}

/// Lap-oriented index for a telemetry file.
#[derive(Debug, Clone, Default)]
pub struct LapIndex {
    /// Ordered lap slices covering the scanned player-car telemetry.
    pub laps: Vec<LapRef>,
    /// Reverse lookup from frame index to lap ordinal.
    pub frame_to_lap: Vec<Option<usize>>,
}

/// Frame-range metadata for a single indexed lap.
#[derive(Debug, Clone, PartialEq)]
pub struct LapRef {
    /// Zero-based ordinal in the built index.
    pub ordinal: usize,
    /// Player lap number observed at the lap start, if available.
    pub lap_number: Option<i32>,
    /// Completed-lap counter observed at the lap start, if available.
    pub completed_laps_at_start: Option<i32>,
    /// Inclusive start frame for this lap slice.
    pub start_frame: usize,
    /// Inclusive end frame for this lap slice.
    pub end_frame: usize,
    /// Session time at the start frame, if available.
    pub start_session_time: Option<f64>,
    /// Session time at the end frame, if available.
    pub end_session_time: Option<f64>,
    /// Phase-1 flags derived while scanning the lap.
    pub flags: LapFlags,
}

/// Phase-1 flags for an indexed lap.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LapFlags {
    /// True when the lap begins after the car has already progressed past start/finish.
    pub partial_first_lap: bool,
    /// True when the file ends before the lap can be observed to complete.
    pub partial_last_lap: bool,
    /// True when at least one frame in the lap reports the player car on pit road.
    pub touches_pit_road: bool,
}

/// Resolved telemetry/session fields used by phase-1 lap indexing.
#[derive(Debug, Clone)]
pub struct LapIndexVars {
    /// Player car index from `DriverInfo.DriverCarIdx`, if present in session YAML.
    pub driver_car_idx: Option<usize>,
    lap_completed: ResolvedVar,
    lap: ResolvedVar,
    lap_dist_pct: ResolvedVar,
    on_pit_road: ResolvedVar,
    /// Session-relative time.
    pub session_time: Option<VariableInfo>,
}

impl LapIndexVars {
    /// Resolves and returns the telemetry variables required to build a lap index from an IBT reader.
    ///
    /// Looks up the session variable schema and selects player-scoped variables when available;
    /// otherwise falls back to car-indexed variants using the session's `DriverInfo.DriverCarIdx`.
    /// The returned `LapIndexVars` contains resolved descriptors for lap completion, lap number,
    /// lap distance percent, on-pit-road, an optional resolved driver car index, and an optional
    /// `SessionTime` variable if present in the schema.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given a mutable `reader: IbtReader` already opened for a file:
    /// let vars = LapIndexVars::resolve(&reader)?;
    /// // `vars` can now be used to sample per-frame lap-related values used by the indexer.
    /// ```
    fn resolve(reader: &IbtReader) -> Result<Self> {
        let schema = reader.variables();
        let driver_car_idx = parse_driver_car_idx(reader)?;

        let lap_completed = ResolvedVar::player_or_car_idx(
            schema.get_variable("LapCompleted").cloned(),
            schema.get_variable("CarIdxLapCompleted").cloned(),
            driver_car_idx,
            "LapCompleted",
            "CarIdxLapCompleted",
        )?;

        let lap = ResolvedVar::player_or_car_idx(
            schema.get_variable("Lap").cloned(),
            schema.get_variable("CarIdxLap").cloned(),
            driver_car_idx,
            "Lap",
            "CarIdxLap",
        )?;

        let lap_dist_pct = ResolvedVar::player_or_car_idx(
            schema.get_variable("LapDistPct").cloned(),
            schema.get_variable("CarIdxLapDistPct").cloned(),
            driver_car_idx,
            "LapDistPct",
            "CarIdxLapDistPct",
        )?;

        let on_pit_road = ResolvedVar::player_or_car_idx(
            schema.get_variable("OnPitRoad").cloned(),
            schema.get_variable("CarIdxOnPitRoad").cloned(),
            driver_car_idx,
            "OnPitRoad",
            "CarIdxOnPitRoad",
        )?;

        Ok(Self {
            driver_car_idx,
            lap_completed,
            lap,
            lap_dist_pct,
            on_pit_road,
            session_time: schema.get_variable("SessionTime").cloned(),
        })
    }

    /// Determines whether lap-completion tracking uses the player-scoped variable.
    ///
    /// # Returns
    /// `true` if lap completion is sourced from the player-scoped variable, `false` otherwise.
    pub fn uses_player_lap_completed(&self) -> bool {
        matches!(self.lap_completed, ResolvedVar::Player(_))
    }

    /// Indicates whether lap progress is read from a player-scoped variable.
    ///
    /// # Returns
    ///
    /// `true` if lap progress is available as a player-scoped variable, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// // assuming `vars: LapIndexVars`
    /// let uses_player = vars.uses_player_lap_dist_pct();
    /// ```
    pub fn uses_player_lap_dist_pct(&self) -> bool {
        matches!(self.lap_dist_pct, ResolvedVar::Player(_))
    }

    /// Indicates whether pit-road tracking is sourced from a player-scoped variable.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crates::iracing_sdk::ibt::lap_index::{LapIndexVars, ResolvedVar, VariableInfo};
    /// // Construct a LapIndexVars where `on_pit_road` is player-scoped.
    /// let vars = LapIndexVars {
    ///     on_pit_road: ResolvedVar::Player(VariableInfo::default()),
    ///     ..Default::default()
    /// };
    /// assert!(vars.uses_player_on_pit_road());
    /// ```
    ///
    /// # Returns
    ///
    /// `true` if `on_pit_road` is resolved as a player-scoped variable, `false` otherwise.
    pub fn uses_player_on_pit_road(&self) -> bool {
        matches!(self.on_pit_road, ResolvedVar::Player(_))
    }

    /// Samples lap-related telemetry values from a raw IBT frame into a FrameSample.
    ///
    /// Reads the resolved variables (lap completion counter, lap number, lap distance percentage,
    /// on-pit-road flag, and optional session time) for `frame` and returns a `FrameSample` with
    /// those values and the provided `frame_idx`.
    ///
    /// # Returns
    ///
    /// A `FrameSample` containing the extracted telemetry fields for the given frame.
    ///
    /// # Errors
    ///
    /// Propagates any read or conversion errors produced while extracting variable values.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // `vars` is a `LapIndexVars` previously resolved from an `IbtReader`,
    /// // and `frame_bytes` is a raw frame buffer obtained from the reader.
    /// let sample = vars.sample(0, &frame_bytes)?;
    /// ```
    fn sample(&self, frame_idx: usize, frame: &[u8]) -> Result<FrameSample> {
        Ok(FrameSample {
            frame_idx,
            lap_completed: self.lap_completed.read_i32(frame)?,
            lap_number: self.lap.read_i32(frame)?,
            lap_dist_pct: self.lap_dist_pct.read_f32(frame)?,
            on_pit_road: self.on_pit_road.read_bool(frame)?,
            session_time: self
                .session_time
                .as_ref()
                .map(|info| f64::from_bytes(frame, info))
                .transpose()?,
        })
    }
}

/// Iterator over the raw frames belonging to an indexed lap.
pub struct LapFrameIter<'a> {
    reader: &'a mut IbtReader,
    next_frame: usize,
    end_frame: usize,
}

impl<'a> Iterator for LapFrameIter<'a> {
    type Item = Result<(Vec<u8>, u32, u32)>;

    /// Advance the lap-frame iterator and yield the next raw frame belonging to the configured lap slice.
    ///
    /// The iterator returns frames in ascending frame index order until the configured end frame is passed,
    /// yielding an error if the underlying reader fails or if the reader reaches end-of-stream before the
    /// expected frame range is exhausted.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given a `lap_iter: LapFrameIter<'_>`, iterate its frames:
    /// # use crates::iracing_sdk::ibt::LapFrameIter;
    /// # fn use_iter(mut lap_iter: LapFrameIter<'_>) {
    /// while let Some(result) = lap_iter.next() {
    ///     match result {
    ///         Ok((bytes, a, b)) => {
    ///             // process frame bytes and metadata
    ///             let _ = bytes;
    ///             let _ = a;
    ///             let _ = b;
    ///         }
    ///         Err(e) => {
    ///             // handle read/parse error
    ///             let _ = e;
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_frame > self.end_frame {
            return None;
        }

        self.next_frame += 1;
        match self.reader.read_next_frame() {
            Ok(Some(frame)) => Some(Ok(frame)),
            Ok(None) => Some(Err(IRacingSDKError::Parse {
                context: "Lap frame iteration".to_string(),
                details: "indexed lap ended before reader produced a frame".to_string(),
            })),
            Err(err) => Some(Err(err)),
        }
    }
}

#[derive(Debug, Clone)]
enum ResolvedVar {
    Player(VariableInfo),
    CarIdx { info: VariableInfo, car_idx: usize },
}

impl ResolvedVar {
    /// Selects a resolution strategy for a lap-related variable: prefer the player-scoped variable when present,
    /// otherwise select the car-indexed variable using `driver_car_idx` and validate bounds.
    ///
    /// On success returns `ResolvedVar::Player(info)` if `player` is `Some`, or
    /// `ResolvedVar::CarIdx { info, car_idx }` when falling back to the car-indexed variant and `driver_car_idx` is in-range.
    /// On failure returns a `IRacingSDKError::Parse` describing why neither resolution was possible or why the car index was out of bounds.
    ///
    /// # Parameters
    ///
    /// - `player`: optional player-scoped `VariableInfo`.
    /// - `car_idx_info`: optional car-indexed `VariableInfo` (array) whose `count` is validated against `driver_car_idx`.
    /// - `driver_car_idx`: resolved driver car index to use when falling back to `car_idx_info`.
    /// - `player_name`: name for the player-scoped variable used in error messages.
    /// - `car_idx_name`: name for the car-indexed variable used in error messages.
    ///
    /// # Returns
    ///
    /// `ResolvedVar::Player(info)` or `ResolvedVar::CarIdx { info, car_idx }`; a `IRacingSDKError::Parse` on resolution failure.
    ///
    /// # Examples
    ///
    /// ```
    /// // Illustrative (non-exhaustive) usage:
    /// // let resolved = ResolvedVar::player_or_car_idx(player_info, car_idx_info, driver_car_idx, "Lap", "LapCarIdx")?;
    /// // match resolved {
    /// //     ResolvedVar::Player(info) => { /* use player-scoped value */ }
    /// //     ResolvedVar::CarIdx { info, car_idx } => { /* read array element at car_idx */ }
    /// // }
    /// ```
    fn player_or_car_idx(
        player: Option<VariableInfo>,
        car_idx_info: Option<VariableInfo>,
        driver_car_idx: Option<usize>,
        player_name: &str,
        car_idx_name: &str,
    ) -> Result<Self> {
        if let Some(info) = player {
            return Ok(Self::Player(info));
        }

        match (car_idx_info, driver_car_idx) {
            (Some(info), Some(car_idx)) if car_idx < info.count => {
                Ok(Self::CarIdx { info, car_idx })
            }
            (Some(info), Some(car_idx)) => Err(IRacingSDKError::Parse {
                context: "Lap index variable resolution".to_string(),
                details: format!(
                    "{car_idx_name} has count {} but DriverCarIdx resolved to {car_idx}",
                    info.count
                ),
            }),
            (Some(_), None) => Err(IRacingSDKError::Parse {
                context: "Lap index variable resolution".to_string(),
                details: format!(
                    "Missing DriverInfo.DriverCarIdx required to use {car_idx_name} fallback"
                ),
            }),
            (None, _) => Err(IRacingSDKError::Parse {
                context: "Lap index variable resolution".to_string(),
                details: format!(
                    "Neither {player_name} nor {car_idx_name} is available in this IBT schema"
                ),
            }),
        }
    }

    /// Read a 32-bit signed integer from the provided raw frame according to this resolved variable.
    ///
    /// Returns the extracted `i32` wrapped in `Ok(Some(...))` when present, or propagates any read/parse error.
    ///
    /// # Examples
    ///
    /// ```
    /// // Conceptual example:
    /// // let val = ResolvedVar::Player(var_info).read_i32(frame)?;
    /// // assert_eq!(val, Some(expected_i32));
    /// ```
    fn read_i32(&self, frame: &[u8]) -> Result<Option<i32>> {
        match self {
            Self::Player(info) => Ok(Some(i32::from_bytes(frame, info)?)),
            Self::CarIdx { info, car_idx } => {
                Ok(Some(read_array_element::<i32>(frame, info, *car_idx)?))
            }
        }
    }

    /// Reads an `f32` value for this resolved variable from a raw frame.
    ///
    /// Returns `Ok(Some(value))` when the variable is present and successfully converted; conversion
    /// errors are propagated as `Err`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use crates::iracing_sdk::ibt::ResolvedVar;
    /// # let frame: Vec<u8> = Vec::new();
    /// # let info = /* VariableInfo for an f32 variable */ unimplemented!();
    /// let resolved = ResolvedVar::Player(info);
    /// let value = resolved.read_f32(&frame).unwrap();
    /// // `value` is `Some(f32)` when present.
    /// ```
    fn read_f32(&self, frame: &[u8]) -> Result<Option<f32>> {
        match self {
            Self::Player(info) => Ok(Some(f32::from_bytes(frame, info)?)),
            Self::CarIdx { info, car_idx } => {
                Ok(Some(read_array_element::<f32>(frame, info, *car_idx)?))
            }
        }
    }

    /// Reads a boolean value for this resolved variable from the given raw frame bytes.
    ///
    /// Returns `Some(value)` when the variable is present in the frame, `None` when the variable is absent,
    /// and propagates conversion or read errors via the returned `Result`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Demonstrates calling `read_bool` with a raw frame slice.
    /// let resolved: ResolvedVar = /* ResolvedVar::Player(info) or ResolvedVar::CarIdx { .. } */;
    /// let frame: Vec<u8> = vec![];
    /// let result = resolved.read_bool(&frame);
    /// ```
    fn read_bool(&self, frame: &[u8]) -> Result<Option<bool>> {
        match self {
            Self::Player(info) => Ok(Some(bool::from_bytes(frame, info)?)),
            Self::CarIdx { info, car_idx } => {
                Ok(Some(read_array_element::<bool>(frame, info, *car_idx)?))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct FrameSample {
    frame_idx: usize,
    lap_completed: Option<i32>,
    lap_number: Option<i32>,
    lap_dist_pct: Option<f32>,
    on_pit_road: Option<bool>,
    session_time: Option<f64>,
}

/// Build an in-memory lap index from a sequence of per-frame samples.
///
/// Constructs a `LapIndex` that contains ordered `LapRef` slices and a per-frame
/// reverse lookup mapping which assigns each input sample frame to the lap
/// ordinal that contains it.
///
— Returns a `LapIndex` where:
/// - `laps` contains one `LapRef` per detected lap slice (at least one if
///   `samples` is non-empty).
/// - `frame_to_lap` is sized to `samples.len()` and maps each frame index to the
///   containing lap ordinal (`None` if a frame is not assigned).
///
/// When `samples` is empty, returns `LapIndex::default()`.
///
/// # Examples
///
/// ```
/// // Indexing an empty sample set yields an empty index.
/// let idx = build_lap_index(&[]);
/// assert!(idx.laps.is_empty());
/// assert!(idx.frame_to_lap.is_empty());
/// ```
fn build_lap_index(samples: &[FrameSample]) -> LapIndex {
    if samples.is_empty() {
        return LapIndex::default();
    }

    let mut laps = Vec::new();
    let mut frame_to_lap = vec![None; samples.len()];
    let mut current_start = 0usize;

    for idx in 1..samples.len() {
        if is_lap_boundary(&samples[idx - 1], &samples[idx]) {
            let lap_idx = laps.len();
            let partial_first_lap = lap_idx == 0 && lap_started_mid_lap(&samples[current_start]);
            let partial_last_lap = false;
            let lap = finish_lap(
                &samples[current_start..idx],
                lap_idx,
                partial_first_lap,
                partial_last_lap,
            );
            mark_frames(&mut frame_to_lap, lap_idx, lap.start_frame, lap.end_frame);
            laps.push(lap);
            current_start = idx;
        }
    }

    let lap_idx = laps.len();
    let partial_first_lap = lap_idx == 0 && lap_started_mid_lap(&samples[current_start]);
    let lap = finish_lap(&samples[current_start..], lap_idx, partial_first_lap, true);
    mark_frames(&mut frame_to_lap, lap_idx, lap.start_frame, lap.end_frame);
    laps.push(lap);

    LapIndex { laps, frame_to_lap }
}

/// Constructs a `LapRef` summarizing a non-empty slice of frame samples for a single lap.
///
/// The returned `LapRef` uses the first sample for start metadata (lap number, completed laps,
/// start frame, start session time) and the last sample for end frame/time; `flags.touches_pit_road`
/// is true if any sample in the slice reports `on_pit_road == Some(true)`.
///
/// # Examples
///
/// ```
/// let samples = [
///     FrameSample {
///         frame_idx: 10,
///         lap_completed: Some(0),
///         lap_number: Some(1),
///         lap_dist_pct: Some(0.0),
///         on_pit_road: Some(false),
///         session_time: Some(12.0),
///     },
///     FrameSample {
///         frame_idx: 50,
///         lap_completed: Some(0),
///         lap_number: Some(1),
///         lap_dist_pct: Some(0.9),
///         on_pit_road: Some(true),
///         session_time: Some(52.0),
///     },
/// ];
///
/// let lap = finish_lap(&samples, 0, false, false);
/// assert_eq!(lap.ordinal, 0);
/// assert_eq!(lap.start_frame, 10);
/// assert_eq!(lap.end_frame, 50);
/// assert!(lap.flags.touches_pit_road);
/// ```
fn finish_lap(
    lap_samples: &[FrameSample],
    ordinal: usize,
    partial_first_lap: bool,
    partial_last_lap: bool,
) -> LapRef {
    let start = &lap_samples[0];
    let end = lap_samples.last().expect("lap_samples is never empty");

    LapRef {
        ordinal,
        lap_number: start.lap_number,
        completed_laps_at_start: start.lap_completed,
        start_frame: start.frame_idx,
        end_frame: end.frame_idx,
        start_session_time: start.session_time,
        end_session_time: end.session_time,
        flags: LapFlags {
            partial_first_lap,
            partial_last_lap,
            touches_pit_road: lap_samples
                .iter()
                .any(|sample| sample.on_pit_road == Some(true)),
        },
    }
}

/// Assigns the given lap ordinal to every frame slot in the inclusive frame range.
///
/// The `frame_to_lap` slice is indexed by frame number; this function sets each entry
/// in `frame_to_lap[start_frame..=end_frame]` to `Some(lap_idx)`.
///
/// # Panics
///
/// Panics if the inclusive range `start_frame..=end_frame` is out of bounds for `frame_to_lap`.
///
/// # Examples
///
/// ```
/// let mut frame_to_lap = vec![None; 6];
/// mark_frames(&mut frame_to_lap, 2, 1, 3);
/// assert_eq!(frame_to_lap, vec![None, Some(2), Some(2), Some(2), None, None]);
/// ```
fn mark_frames(
    frame_to_lap: &mut [Option<usize>],
    lap_idx: usize,
    start_frame: usize,
    end_frame: usize,
) {
    for slot in &mut frame_to_lap[start_frame..=end_frame] {
        *slot = Some(lap_idx);
    }
}

/// Detects whether a lap boundary occurs between two adjacent frame samples.
///
/// Returns `true` if the completed-laps counter increases from `previous` to `current`, or if the lap distance percent wraps from greater than or equal to `WRAP_HIGH_WATERMARK` to less than or equal to `WRAP_LOW_WATERMARK`; returns `false` otherwise.
///
/// # Examples
///
/// ```
/// let prev = FrameSample {
///     frame_idx: 0,
///     lap_completed: Some(1),
///     lap_number: None,
///     lap_dist_pct: Some(0.85),
///     on_pit_road: None,
///     session_time: None,
/// };
/// let curr = FrameSample {
///     frame_idx: 1,
///     lap_completed: Some(2),
///     lap_number: None,
///     lap_dist_pct: Some(0.05),
///     on_pit_road: None,
///     session_time: None,
/// };
/// assert!(is_lap_boundary(&prev, &curr));
/// ```
fn is_lap_boundary(previous: &FrameSample, current: &FrameSample) -> bool {
    if let (Some(prev), Some(curr)) = (previous.lap_completed, current.lap_completed)
        && curr > prev
    {
        return true;
    }

    matches!(
        (previous.lap_dist_pct, current.lap_dist_pct),
        (Some(prev), Some(curr)) if prev >= WRAP_HIGH_WATERMARK && curr <= WRAP_LOW_WATERMARK
    )
}

/// Reports whether the sample's lap distance indicates the lap had already started before this frame.
///
/// # Returns
///
/// `true` if `lap_dist_pct` is `Some(progress)` and `progress > PARTIAL_PROGRESS_EPSILON`, `false` otherwise.
///
/// # Examples
///
/// ```
/// let s = FrameSample {
///     frame_idx: 0,
///     lap_completed: None,
///     lap_number: None,
///     lap_dist_pct: Some(0.5),
///     on_pit_road: None,
///     session_time: None,
/// };
/// assert!(lap_started_mid_lap(&s));
///
/// let s2 = FrameSample {
///     lap_dist_pct: None,
///     ..s
/// };
/// assert!(!lap_started_mid_lap(&s2));
/// ```
fn lap_started_mid_lap(sample: &FrameSample) -> bool {
    sample
        .lap_dist_pct
        .map(|progress| progress > PARTIAL_PROGRESS_EPSILON)
        .unwrap_or(false)
}

/// Parses the reader's session YAML and returns the session's `DriverInfo.DriverCarIdx` as a `usize` when available.
///
/// Returns `Ok(Some(usize))` when `DriverCarIdx` is present and non-negative, `Ok(None)` when the session YAML,
/// `driver_info`, or `driver_car_idx` are missing, and an `IRacingSDKError::Parse` if the value is negative or
/// if parsing the session YAML fails.
///
/// # Examples
///
/// ```
/// # use crates::iracing_sdk::ibt::lap_index::parse_driver_car_idx;
/// # use crates::iracing_sdk::ibt::IbtReader;
/// # fn example(reader: &IbtReader) -> Result<Option<usize>, _> {
/// let car_idx = parse_driver_car_idx(reader)?;
/// match car_idx {
///     Some(idx) => println!("Driver car index: {}", idx),
///     None => println!("Driver car index not present"),
/// }
/// # Ok(car_idx)
/// # }
/// ```
fn parse_driver_car_idx(reader: &IbtReader) -> Result<Option<usize>> {
    let Some(session_yaml) = reader.session_yaml()? else {
        return Ok(None);
    };

    let session = SessionInfo::parse(&session_yaml)?;
    let Some(driver_info) = session.driver_info else {
        return Ok(None);
    };
    let Some(driver_car_idx) = driver_info.driver_car_idx else {
        return Ok(None);
    };

    usize::try_from(driver_car_idx)
        .map(Some)
        .map_err(|_| IRacingSDKError::Parse {
            context: "Lap index session parsing".to_string(),
            details: format!("DriverCarIdx cannot be negative: {driver_car_idx}"),
        })
}

/// Reads a single element from an array-typed variable at the given index and returns it parsed as `T`.
///
/// The function interprets `info` as describing an array layout and extracts the element at `index`,
/// returning the parsed value or an error if parsing fails or bounds/format issues occur.
///
/// # Examples
///
/// ```
/// // Given a byte slice `frame` and a `VariableInfo` describing an array-typed variable,
/// // read the element at index 2 as an f32.
/// let value: f32 = read_array_element::<f32>(&frame, &array_info, 2).unwrap();
/// ```
fn read_array_element<T: VarData>(frame: &[u8], info: &VariableInfo, index: usize) -> Result<T> {
    let element_size = info.data_type.size();
    let offset = info.offset + (index * element_size);
    let element_info = VariableInfo {
        name: info.name.clone(),
        data_type: info.data_type,
        offset,
        count: 1,
        count_as_time: info.count_as_time,
        units: info.units.clone(),
        description: info.description.clone(),
    };

    T::from_bytes(frame, &element_info)
}

/// Restores the reader's cursor to the originally observed frame, preserving EOF state when necessary.
///
/// If the IBT has zero frames, no action is taken. If `original_frame` is less than the reader's
/// total frames, the reader is seeked to that frame. If `original_frame` is greater than or equal
/// to the total frame count, the reader is positioned at end-of-file (the reader is seeked to the
/// last frame and then advanced once to produce an EOF state).
///
/// # Errors
///
/// Returns an error if seeking or reading from the provided `IbtReader` fails.
///
/// # Examples
///
/// ```
/// // restore_reader_frame will seek back to `original_frame` when possible,
/// // or restore EOF when the original position was past the last frame.
/// let mut reader = IbtReader::open_fixture("small.ibt").unwrap();
/// let original = reader.current_frame();
/// // ... perform indexing that iterates to EOF ...
/// restore_reader_frame(&mut reader, original).unwrap();
/// assert_eq!(reader.current_frame(), original);
/// ```
fn restore_reader_frame(reader: &mut IbtReader, original_frame: usize) -> Result<()> {
    let total_frames = reader.total_frames();
    if total_frames == 0 {
        return Ok(());
    }

    if original_frame < total_frames {
        return reader.seek_to_frame(original_frame);
    }

    reader.seek_to_frame(total_frames - 1)?;
    let _ = reader.read_next_frame()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use test_utils::require_smallest_ibt_fixture;

    /// Creates a FrameSample using the provided lap telemetry values and deterministic test defaults.
    ///
    /// The constructed sample sets `on_pit_road` to `Some(false)` and `session_time` to the `frame_idx` converted to `f64`.
    ///
    /// # Examples
    ///
    /// ```
    /// let s = sample(42, 1, 3, 0.75);
    /// assert_eq!(s.frame_idx, 42);
    /// assert_eq!(s.lap_completed, Some(1));
    /// assert_eq!(s.lap_number, Some(3));
    /// assert_eq!(s.lap_dist_pct, Some(0.75));
    /// assert_eq!(s.on_pit_road, Some(false));
    /// assert_eq!(s.session_time, Some(42.0));
    /// ```
    fn sample(
        frame_idx: usize,
        lap_completed: i32,
        lap_number: i32,
        lap_dist_pct: f32,
    ) -> FrameSample {
        FrameSample {
            frame_idx,
            lap_completed: Some(lap_completed),
            lap_number: Some(lap_number),
            lap_dist_pct: Some(lap_dist_pct),
            on_pit_road: Some(false),
            session_time: Some(frame_idx as f64),
        }
    }

    #[test]
    fn detects_laps_from_lap_completed_increments() {
        let mut samples = vec![
            sample(0, 10, 11, 0.10),
            sample(1, 10, 11, 0.55),
            sample(2, 11, 12, 0.01),
            sample(3, 11, 12, 0.40),
            sample(4, 12, 13, 0.02),
        ];
        samples[3].on_pit_road = Some(true);

        let index = build_lap_index(&samples);

        assert_eq!(index.laps.len(), 3);
        assert_eq!(index.laps[0].start_frame, 0);
        assert_eq!(index.laps[0].end_frame, 1);
        assert!(index.laps[0].flags.partial_first_lap);
        assert!(!index.laps[0].flags.partial_last_lap);
        assert_eq!(index.laps[1].start_frame, 2);
        assert_eq!(index.laps[1].end_frame, 3);
        assert!(index.laps[1].flags.touches_pit_road);
        assert_eq!(index.laps[2].start_frame, 4);
        assert_eq!(index.laps[2].end_frame, 4);
        assert!(index.laps[2].flags.partial_last_lap);
        assert_eq!(
            index.frame_to_lap,
            vec![Some(0), Some(0), Some(1), Some(1), Some(2)]
        );
    }

    #[test]
    fn falls_back_to_progress_wrap_when_completion_counter_is_flat() {
        let samples = vec![
            sample(0, 0, 1, 0.76),
            sample(1, 0, 1, 0.95),
            sample(2, 0, 2, 0.03),
            sample(3, 0, 2, 0.22),
        ];

        let index = build_lap_index(&samples);

        assert_eq!(index.laps.len(), 2);
        assert_eq!(index.laps[0].start_frame, 0);
        assert_eq!(index.laps[0].end_frame, 1);
        assert_eq!(index.laps[1].start_frame, 2);
        assert_eq!(index.laps[1].end_frame, 3);
    }

    #[test]
    fn preserves_reader_position_after_build() -> Result<()> {
        let fixture = match require_smallest_ibt_fixture() {
            Ok(path) => path,
            Err(err) => {
                eprintln!("{err}");
                return Ok(());
            }
        };
        let mut reader = IbtReader::open(&fixture)?;

        if reader.total_frames() == 0 {
            let indexed = IndexedIbt::build(&mut reader)?;
            assert_eq!(indexed.lap_count(), 0);
            assert_eq!(reader.current_frame(), 0);
            return Ok(());
        }

        let target = (reader.total_frames() / 2).min(reader.total_frames() - 1);
        reader.seek_to_frame(target)?;

        let frame_count = {
            let indexed = IndexedIbt::build(&mut reader)?;
            indexed.index.frame_to_lap.len()
        };
        assert_eq!(reader.current_frame(), target);
        assert_eq!(frame_count, reader.total_frames());
        Ok(())
    }

    #[test]
    fn restores_eof_position_after_build() -> Result<()> {
        let fixture = match require_smallest_ibt_fixture() {
            Ok(path) => path,
            Err(err) => {
                eprintln!("{err}");
                return Ok(());
            }
        };
        let mut reader = IbtReader::open(&fixture)?;

        while reader.read_next_frame()?.is_some() {}

        let total_frames = reader.total_frames();
        let _indexed = IndexedIbt::build(&mut reader)?;
        assert_eq!(reader.current_frame(), total_frames);
        Ok(())
    }
}
