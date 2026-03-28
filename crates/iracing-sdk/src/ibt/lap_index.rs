//! Lap-oriented indexing for offline `.ibt` playback.
//!
//! This module provides a phase-1, player-car lap index built on top of [`IbtReader`]
//! without changing the low-level IBT parsing contract.
//!
//! ## Example
//!
//! ```rust,no_run
//! use iracing_sdk::{IbtReader, IndexedIbt};
//!
//! fn summarize(path: &str) -> iracing_sdk::Result<()> {
//!     let mut reader = IbtReader::open(path)?;
//!     let indexed = IndexedIbt::build(&mut reader)?;
//!
//!     for lap_idx in 0..indexed.lap_count() {
//!         let lap = indexed.lap(lap_idx).expect("lap index in range");
//!         println!("lap {} => frames {}..={}", lap_idx, lap.start_frame, lap.end_frame);
//!     }
//!
//!     Ok(())
//! }
//! ```

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
    /// Scan the entire IBT stream once and build a player-car lap index.
    ///
    /// The reader's original position is restored before this method returns,
    /// including the end-of-file position.
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

    /// Return the number of indexed laps.
    pub fn lap_count(&self) -> usize {
        self.index.laps.len()
    }

    /// Return the indexed lap at `idx`, if present.
    pub fn lap(&self, idx: usize) -> Option<&LapRef> {
        self.index.laps.get(idx)
    }

    /// Return the lap that owns `frame_idx`, if any.
    pub fn lap_for_frame(&self, frame_idx: usize) -> Option<&LapRef> {
        let lap_idx = self.index.frame_to_lap.get(frame_idx).copied().flatten()?;
        self.index.laps.get(lap_idx)
    }

    /// Seek the underlying reader to the first frame of the indexed lap.
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

    /// Return an iterator over all frames belonging to the indexed lap.
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

    /// Return the resolved variable set used to build this index.
    pub fn variables(&self) -> &LapIndexVars {
        &self.vars
    }

    /// Return the in-memory lap index.
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

    /// Returns whether lap-completion tracking uses a player-scoped field.
    pub fn uses_player_lap_completed(&self) -> bool {
        matches!(self.lap_completed, ResolvedVar::Player(_))
    }

    /// Returns whether lap-progress tracking uses a player-scoped field.
    pub fn uses_player_lap_dist_pct(&self) -> bool {
        matches!(self.lap_dist_pct, ResolvedVar::Player(_))
    }

    /// Returns whether pit-road tracking uses a player-scoped field.
    pub fn uses_player_on_pit_road(&self) -> bool {
        matches!(self.on_pit_road, ResolvedVar::Player(_))
    }

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

    fn read_i32(&self, frame: &[u8]) -> Result<Option<i32>> {
        match self {
            Self::Player(info) => Ok(Some(i32::from_bytes(frame, info)?)),
            Self::CarIdx { info, car_idx } => {
                Ok(Some(read_array_element::<i32>(frame, info, *car_idx)?))
            }
        }
    }

    fn read_f32(&self, frame: &[u8]) -> Result<Option<f32>> {
        match self {
            Self::Player(info) => Ok(Some(f32::from_bytes(frame, info)?)),
            Self::CarIdx { info, car_idx } => {
                Ok(Some(read_array_element::<f32>(frame, info, *car_idx)?))
            }
        }
    }

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

fn lap_started_mid_lap(sample: &FrameSample) -> bool {
    sample
        .lap_dist_pct
        .map(|progress| progress > PARTIAL_PROGRESS_EPSILON)
        .unwrap_or(false)
}

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
