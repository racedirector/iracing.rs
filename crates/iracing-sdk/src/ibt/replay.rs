//! Owned, deterministic recorded-frame traversal.

use super::{IbtReader, RecordedFrame};
use crate::{IRacingSDKError, Result};
use std::ops::Range;

/// Owns a reader and the cursor for pull-driven sequential traversal.
///
/// The position identifies the next physical record to read, or the exclusive
/// range end at EOF. Reads never advance on failure. No clock, sleep, background
/// task, or synthetic tick is involved. Sampling and wall-clock pacing are
/// separate policies; this adapter visits consecutive recorded indices.
/// The immutable-file requirement of [`IbtReader::open`] applies until the
/// replay and any recovered reader are dropped.
pub struct IbtReplay {
    pub(crate) reader: IbtReader,
    range: Range<usize>,
    position: usize,
}

impl std::fmt::Debug for IbtReplay {
    /// Formats the traversal bounds and position, omitting the reader.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IbtReplay")
            .field("range", &self.range)
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

impl IbtReplay {
    /// Owns the reader with full-recording bounds and a cursor at zero.
    /// An empty recording starts at EOF; no frame bytes are read.
    pub(crate) fn new(reader: IbtReader) -> Self {
        let end = reader.frame_count();
        Self {
            reader,
            range: 0..end,
            position: 0,
        }
    }

    /// Borrows the cursor-free reader for metadata or independent indexed reads.
    pub fn reader(&self) -> &IbtReader {
        &self.reader
    }

    /// Recovers the reader, discarding replay bounds and cursor state.
    pub fn into_reader(self) -> IbtReader {
        self.reader
    }

    /// Returns the next recorded index, or the exclusive end at EOF.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Returns the configured half-open traversal bounds.
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    /// Reports whether the cursor has reached the exclusive traversal end.
    pub fn is_eof(&self) -> bool {
        self.position == self.range.end
    }

    /// Configures bounded traversal and resets the cursor to its start.
    ///
    /// Uses the same validation as [`IbtReader::frames`]: ends clamp to the
    /// recording length, empty ranges at EOF are valid, reversed ranges and
    /// starts beyond EOF fail. An error leaves bounds and position unchanged.
    pub fn set_range(&mut self, range: Range<usize>) -> Result<()> {
        self.reader.frames(range.clone())?;
        self.range = range.start..range.end.min(self.reader.frame_count());
        self.position = self.range.start;
        Ok(())
    }

    /// Seeks within the configured bounds, including the exclusive EOF end.
    ///
    /// `index` is an absolute zero-based record coordinate in the recording.
    /// Returns an error without changing state for positions outside the bounds.
    /// Seeking to an earlier frame permits replay after EOF.
    pub fn seek(&mut self, index: usize) -> Result<()> {
        if index < self.range.start || index > self.range.end {
            return Err(IRacingSDKError::parse_error(
                "IbtReplay::seek",
                format!(
                    "Frame index {index} is outside replay range {:?}",
                    self.range
                ),
            ));
        }
        self.position = index;
        Ok(())
    }

    /// Reads the current record without advancing, returning `None` at EOF.
    /// Returns an error if the complete frame cannot be read, leaving the cursor unchanged.
    pub fn current_frame(&self) -> Result<Option<RecordedFrame>> {
        if self.is_eof() {
            return Ok(None);
        }
        self.reader
            .frames(self.position..self.position + 1)?
            .next()
            .transpose()
    }

    /// Advances one recorded coordinate without reading bytes.
    /// Returns whether a coordinate was consumed; repeated calls at EOF do nothing.
    pub fn advance(&mut self) -> bool {
        if self.is_eof() {
            return false;
        }
        self.position += 1;
        true
    }

    /// Reads one record and advances only after a successful read.
    /// Returns `None` repeatedly at EOF until a seek or range reset.
    /// Propagates [`Self::current_frame`] errors without advancing the cursor.
    pub fn next_frame(&mut self) -> Result<Option<RecordedFrame>> {
        let frame = self.current_frame()?;
        if frame.is_some() {
            self.advance();
        }
        Ok(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::require_smallest_ibt_fixture;

    #[test]
    fn traversal_seek_bounds_and_eof() -> anyhow::Result<()> {
        let path = require_smallest_ibt_fixture()?;
        for reader in [
            IbtReader::open(&path)?,
            IbtReader::from_bytes(std::fs::read(&path)?)?,
        ] {
            let count = reader.frame_count();
            let first = reader.frame(0)?;
            let mut replay = reader.replay();
            assert_eq!(replay.position(), 0);
            assert_eq!(replay.current_frame()?.unwrap().bytes(), first);
            assert_eq!(replay.position(), 0);
            assert_eq!(replay.next_frame()?.unwrap().index(), 0);
            assert_eq!(replay.position(), 1);
            assert_eq!(replay.reader().frame(0)?, first);
            replay.seek(count - 1)?;
            assert_eq!(replay.next_frame()?.unwrap().index(), count - 1);
            assert!(replay.is_eof());
            assert!(replay.next_frame()?.is_none());
            assert!(replay.current_frame()?.is_none());
            assert!(!replay.advance());
            assert!(replay.seek(count + 1).is_err());
            assert_eq!(replay.position(), count);
            replay.seek(0)?;
            assert!(replay.advance());
            assert_eq!(replay.position(), 1);
            replay.set_range(1..3)?;
            assert_eq!(replay.range(), 1..3);
            assert!(replay.seek(0).is_err());
            assert!(replay.seek(4).is_err());
            assert!(replay.set_range(Range { start: 2, end: 1 }).is_err());
            assert!(replay.set_range(count + 1..usize::MAX).is_err());
            assert_eq!(replay.range(), 1..3);
            assert_eq!(replay.position(), 1);
            assert_eq!(replay.next_frame()?.unwrap().index(), 1);
            assert_eq!(replay.next_frame()?.unwrap().index(), 2);
            assert!(replay.next_frame()?.is_none());
            replay.set_range(count - 1..usize::MAX)?;
            assert_eq!(replay.range(), count - 1..count);
            replay.seek(count)?;
            assert!(replay.is_eof());
            replay.set_range(count..count)?;
            assert!(replay.is_eof());
            assert_eq!(replay.into_reader().frame(0)?, first);
        }
        Ok(())
    }

    #[test]
    fn read_failure_retains_cursor_and_empty_recording_is_eof() -> anyhow::Result<()> {
        let bytes = std::fs::read(require_smallest_ibt_fixture()?)?;
        let reader = IbtReader::from_bytes(bytes.clone())?;
        let start = reader.layout().frame_data_start();
        let mut replay = reader.replay();
        replay.reader.owned_bytes_mut().truncate(start);
        assert!(replay.next_frame().is_err());
        assert_eq!(replay.position(), 0);
        *replay.reader.owned_bytes_mut() = bytes.clone();
        assert_eq!(replay.next_frame()?.unwrap().index(), 0);
        let mut empty = IbtReader::from_bytes(bytes[..start].to_vec())?.replay();
        assert!(empty.is_eof());
        assert!(empty.next_frame()?.is_none());
        empty.seek(0)?;
        empty.set_range(0..usize::MAX)?;
        assert!(empty.seek(1).is_err());
        Ok(())
    }
}
