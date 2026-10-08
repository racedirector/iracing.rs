//! iRacing shared memory connection aligned with C++ SDK
//!
//! This module provides direct memory mapping to iRacing's shared memory
//! following the same patterns as the official C++ SDK implementation.

use iracing_irsdk::{StatusField, VariableBuffer};

use crate::{
    ByteRegion, FrameRegion, IRacingSDKError, Result, SessionInfoBytes, SessionInfoRegion,
    VariableHeaders, VariableHeadersRegion,
    provider::{SessionInformationBytesProvider, VariableHeadersProvider},
    source::live::{Source, WaitResult},
};
use std::mem::offset_of;
use std::time::Duration;

use iracing_irsdk::Header;

/// Direct connection to iRacing shared memory
#[derive(Debug)]
pub struct Connection {
    source: Source,

    last_tick_count: i32,
}

/// Owned telemetry bytes and metadata from one accepted live acquisition.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LiveFrameSnapshot {
    /// Stable telemetry bytes copied inside the consistency window.
    pub data: Vec<u8>,
    /// Accepted native SDK tick count.
    pub tick: i32,
    /// Session update counter observed throughout the accepted attempt.
    pub session_info_update: i32,
}

impl Connection {
    /// Attempt to connect to iRacing shared memory
    ///
    /// # Errors
    /// Returns an error when opening the mapping/event fails or the mapped
    /// extent cannot hold the complete fixed SDK header.
    pub fn try_connect() -> Result<Self> {
        Self::from_source(Source::try_connect()?)
    }

    fn from_source(source: Source) -> Result<Self> {
        if source.len() < size_of::<Header>() {
            return Err(IRacingSDKError::parse_error(
                "Header",
                "Mapped header is truncated",
            ));
        }
        Ok(Self {
            source,
            last_tick_count: i32::MAX,
        })
    }

    /// Copies the live header into an owned value.
    ///
    /// The result remains stable after the copy, but fields may reflect different
    /// publication instants if the simulator updates them during the copy.
    /// Frame acquisition still requires separate synchronization-word checks.
    ///
    /// # Errors
    /// Returns an error if the mapping is too short or the source copy fails.
    pub fn header_snapshot(&self) -> Result<Header> {
        if self.source.len() < size_of::<Header>() {
            return Err(IRacingSDKError::parse_error(
                "Header",
                "Mapped header is truncated",
            ));
        }
        // SAFETY: The full header fits in the retained mapping. Header implements
        // FromBytes, and the source copies into disjoint, owned storage.
        unsafe { self.source.copy_value_unchecked::<Header>(0) }
    }

    /// The connection status advertised from the header
    pub fn status(&self) -> StatusField {
        let raw = self
            .source
            .read_i32(offset_of!(Header, status))
            .unwrap_or_default();

        StatusField::from_bits_retain(raw)
    }

    /// Check if iRacing is connected
    pub fn is_connected(&self) -> bool {
        self.status().is_connected()
    }

    /// Get session info update counter
    pub fn session_info_update(&self) -> i32 {
        // SAFETY: Activation validated the fixed header. This aligned word
        // remains within our retained mapping for the reader's lifetime.
        unsafe {
            self.source
                .read_i32_unchecked(offset_of!(Header, session_info_update))
                .unwrap_or(-1)
        }
    }

    /// The tick rate advertised by the header
    pub fn tick_rate(&self) -> i32 {
        // SAFETY: Activation validated the fixed header. This aligned word
        // remains within our retained mapping for the reader's lifetime.
        unsafe {
            self.source
                .read_i32_unchecked(offset_of!(Header, tick_rate))
                .unwrap_or(-1)
        }
    }

    /// The last processed tick count
    pub fn last_tick_count(&self) -> i32 {
        self.last_tick_count
    }

    /// Wait for new telemetry data (synchronous - blocks thread)
    pub fn wait_for_update(&self, timeout: Duration) -> Result<WaitResult> {
        self.source.wait_for_update(timeout)
    }

    /// Wait for new telemetry data (async - cooperative, non-blocking)
    pub async fn wait_for_update_async(&self, timeout: Duration) -> Result<WaitResult> {
        self.source.wait_for_update_async(timeout).await
    }

    /// Acquires owned bytes and coherent metadata for the published current frame.
    ///
    /// `Ok(None)` means disconnected, equal/reset ticks, or two unsuccessful
    /// consistency attempts. The initial tick establishes a baseline without
    /// returning a frame. Each attempt checks ticks, session version and selected
    /// slot geometry around the actual copy; accepted bytes remain stable even
    /// when the simulator changes its mapping afterward.
    ///
    /// # Errors
    /// Returns malformed advertised geometry or bounded acquisition failures.
    /// This replaces the former borrowed-byte `Option` result: callers consume
    /// the snapshot directly rather than re-reading metadata after acceptance.
    pub fn get_new_data(&mut self) -> Result<Option<LiveFrameSnapshot>> {
        self.acquire_frame(|_, _| {})
    }

    fn acquire_frame(
        &mut self,
        mut after_copy: impl FnMut(&Source, usize),
    ) -> Result<Option<LiveFrameSnapshot>> {
        let invalid = || {
            IRacingSDKError::parse_error(
                "Connection::get_new_data",
                "Invalid live frame geometry or scalar access",
            )
        };
        if !self.is_connected() {
            self.last_tick_count = i32::MAX;
            return Ok(None);
        }
        let header = self.header_snapshot()?;
        if header.buffer_count <= 0 || header.buffer_count > Header::MAX_BUFFERS as i32 {
            return Err(invalid());
        }
        let current_buffer = self
            .source
            .read_u8(offset_of!(Header, current_buffer))
            .ok_or_else(invalid)?;
        let index = Self::current_buffer_index(current_buffer, header.buffer_count);
        let descriptor_offset = offset_of!(Header, buffers) + index * size_of::<VariableBuffer>();
        let tick_offset = descriptor_offset + offset_of!(VariableBuffer, tick_count);
        let begin_offset = descriptor_offset + offset_of!(VariableBuffer, tick_count_begin);
        let offset_word = descriptor_offset + offset_of!(VariableBuffer, buffer_offset);
        let read = |offset| self.source.read_i32(offset).ok_or_else(invalid);
        let latest_tick = read(tick_offset)?;
        if self.last_tick_count == latest_tick {
            return Ok(None);
        }
        if self.last_tick_count > latest_tick {
            self.last_tick_count = latest_tick;
            return Ok(None);
        }

        // Retain the native selected slot across both attempts. Geometry is
        // captured and bounded afresh for each attempt, then validated afterward.
        for attempt in 0..2 {
            let session_info_update = read(offset_of!(Header, session_info_update))?;
            let buffer_count = read(offset_of!(Header, buffer_count))?;
            let frame_length = read(offset_of!(Header, buffer_length))?;
            let frame_offset = read(offset_word)?;
            if buffer_count <= 0 || buffer_count > Header::MAX_BUFFERS as i32 || frame_length <= 0 {
                return Err(invalid());
            }
            let region = FrameRegion::new(
                usize::try_from(frame_offset).map_err(|_| invalid())?,
                usize::try_from(frame_length).map_err(|_| invalid())?,
            )?;
            if region.end() > self.source.len() {
                return Err(invalid());
            }
            let tick = read(tick_offset)?;
            let mut data = vec![0; region.len()];
            // SAFETY: FrameRegion proves non-overflowing geometry and the extent
            // check proves containment. data is initialized, disjoint owned storage.
            unsafe {
                self.source.copy_unchecked(region.offset(), &mut data)?;
            }
            after_copy(&self.source, attempt);
            let begin = read(begin_offset)?;
            if tick == begin
                && tick == read(tick_offset)?
                && session_info_update == read(offset_of!(Header, session_info_update))?
                && frame_offset == read(offset_word)?
                && frame_length == read(offset_of!(Header, buffer_length))?
                && buffer_count == read(offset_of!(Header, buffer_count))?
            {
                self.last_tick_count = tick;
                return Ok(Some(LiveFrameSnapshot {
                    data,
                    tick,
                    session_info_update,
                }));
            }
        }
        Ok(None)
    }

    /// Copies session bytes, returning None on absence or acquisition failure.
    #[deprecated(
        note = "use iracing_sdk::provider::SessionInformationBytesProvider::session_info_snapshot to preserve acquisition errors"
    )]
    pub fn session_info_buffer(&self) -> Option<SessionInfoBytes> {
        self.session_info_snapshot().ok().flatten()
    }

    /// Copies headers, returning None on absence or acquisition failure.
    #[deprecated(
        note = "use iracing_sdk::provider::VariableHeadersProvider::variable_headers; absent metadata returns an empty snapshot and failures return errors"
    )]
    pub fn variable_headers_buffer(&self) -> Option<VariableHeaders> {
        let header = self.header_snapshot().ok()?;
        VariableHeadersRegion::try_from_header(&header).ok()??;
        self.variable_headers().ok()
    }

    fn copy_region(&self, region: ByteRegion) -> Option<Vec<u8>> {
        if region.end() > self.source.len() {
            return None;
        }

        let mut bytes = vec![0; region.len()];

        // SAFETY: ByteRegion guarantees non-overflowing geometry; the
        // bounds check establishes the source extent. bytes is initialized,
        // writable storage disjoint from the retained mapping.
        unsafe {
            self.source
                .copy_unchecked(region.offset(), &mut bytes)
                .ok()?;
        }

        Some(bytes)
    }

    fn current_buffer_index(current_buffer: u8, buffer_count: i32) -> usize {
        let index = usize::from(current_buffer);
        if i32::from(current_buffer) < buffer_count && index < Header::MAX_BUFFERS {
            index
        } else {
            0
        }
    }

    /// Find the buffer with the highest tick count.
    ///
    /// This legacy selection differs from the native SDK's current-buffer index.
    #[deprecated(
        note = "use the published current_buffer; get_new_data() now follows native selection"
    )]
    pub fn find_latest_buffer(&self, header: &Header) -> usize {
        let mut latest = 0;
        let num_buf = header.buffer_count.clamp(0, 4) as usize;
        for i in 1..num_buf {
            if header.buffers[latest].tick_count < header.buffers[i].tick_count {
                latest = i;
            }
        }
        latest
    }
}

impl SessionInformationBytesProvider for Connection {
    fn session_info_snapshot(&self) -> Result<Option<SessionInfoBytes>> {
        let header = self.header_snapshot()?;

        let Some(region) = SessionInfoRegion::try_from_header(&header)? else {
            return Ok(None);
        };

        let Some(bytes) = self.copy_region(region.as_region()) else {
            return Err(IRacingSDKError::parse_error(
                "Connection::session_info_snapshot",
                "Could not get session info bytes from source",
            ));
        };

        Ok(Some(SessionInfoBytes::from_checked_region(&bytes)))
    }
}

impl VariableHeadersProvider for Connection {
    fn variable_headers(&self) -> Result<VariableHeaders> {
        let header = self.header_snapshot()?;
        let Some(region) = VariableHeadersRegion::try_from_header(&header)? else {
            return Ok(VariableHeaders::default());
        };

        let Some(bytes) = self.copy_region(region.as_region()) else {
            return Err(IRacingSDKError::parse_error(
                "Connection::variable_headers",
                "Could not get variable headers bytes from source",
            ));
        };

        VariableHeaders::try_from_bytes(&bytes, region.count())
    }
}

// SAFETY: The Connection struct only holds Windows handles and a memory pointer
// that are safe to send between threads for our read-only use case
unsafe impl Send for Connection {}
unsafe impl Sync for Connection {}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::irsdk::{StatusField, VariableBuffer, constants::IRSDK_VER};
    use zerocopy::IntoBytes;

    fn test_header(num_buf: i32) -> Header {
        Header::new(
            IRSDK_VER,
            StatusField::CONNECTED,
            60,
            0,
            0,
            0,
            1,
            112,
            num_buf,
            4,
            4,
            2,
            [
                VariableBuffer::new(1, 256, 1),
                VariableBuffer::new(4, 260, 4),
                VariableBuffer::new(3, 264, 3),
                VariableBuffer::new(2, 268, 2),
            ],
        )
    }

    fn connection_with_header(header: Header, len: usize) -> Connection {
        let mut bytes = vec![0; len];
        bytes[..size_of::<Header>()].copy_from_slice(header.as_bytes());
        bytes[264..268].copy_from_slice(&[1, 2, 3, 4]);
        Connection::from_source(Source::test_source(&bytes)).unwrap()
    }

    #[test]
    fn activation_rejects_truncated_headers() {
        for len in [1, offset_of!(Header, tick_rate), size_of::<Header>() - 1] {
            assert!(Connection::from_source(Source::test_source(&vec![0; len])).is_err());
        }
    }

    #[test]
    fn acquisition_copies_published_frame_and_tracks_ticks() {
        let mut connection = connection_with_header(test_header(4), 272);
        assert_eq!(connection.tick_rate(), 60);
        assert_eq!(connection.session_info_update(), 0);
        assert!(connection.get_new_data().unwrap().is_none());
        assert_eq!(connection.last_tick_count(), 3);
        connection.last_tick_count = 2;
        assert_eq!(
            connection.get_new_data().unwrap().unwrap().data,
            &[1, 2, 3, 4]
        );
        assert!(connection.get_new_data().unwrap().is_none());
        connection.last_tick_count = 4;
        assert!(connection.get_new_data().unwrap().is_none());
        assert_eq!(connection.last_tick_count(), 3);
    }

    #[test]
    fn acquisition_rejects_torn_frames_and_invalid_regions() {
        for (offset, len, begin) in [
            (264, 4, 9),
            (-1, 4, 3),
            (270, 4, 3),
            (264, -1, 3),
            (264, 0, 3),
        ] {
            let mut header = test_header(4);
            header.buffers[2].buffer_offset = offset;
            header.buffers[2].tick_count_begin = begin;
            header.buffer_length = len;
            let mut connection = connection_with_header(header, 272);
            connection.last_tick_count = 2;
            if begin == 9 {
                assert!(connection.get_new_data().unwrap().is_none());
            } else {
                assert!(connection.get_new_data().is_err());
            }
            assert_eq!(connection.last_tick_count(), 2);
        }
        let mut header = test_header(4);
        header.session_info_offset = 270;
        header.session_info_length = 4;
        header.variable_header_offset = 270;
        let connection = connection_with_header(header, 272);
        assert!(connection.session_info_snapshot().is_err());
        assert!(connection.variable_headers().is_err());
    }

    #[test]
    fn accepted_snapshot_owns_bytes_and_matching_metadata() {
        let mut header = test_header(4);
        header.session_info_update = 7;
        let mut connection = connection_with_header(header, 268); // exact end
        connection.last_tick_count = 2;
        let frame = connection.get_new_data().unwrap().unwrap();
        connection.source.test_write(264, &[9; 4]);
        assert_eq!(frame.data, [1, 2, 3, 4]);
        assert_eq!(frame.tick, 3);
        assert_eq!(frame.session_info_update, 7);
    }

    #[test]
    fn session_transition_retries_with_metadata_from_the_accepted_attempt() {
        let mut connection = connection_with_header(test_header(4), 272);
        connection.last_tick_count = 2;
        let mut attempts = 0;
        let frame = connection
            .acquire_frame(|source, attempt| {
                attempts += 1;
                if attempt == 0 {
                    source.test_write(offset_of!(Header, session_info_update), &7i32.to_le_bytes());
                    source.test_write(264, &[9; 4]);
                }
            })
            .unwrap()
            .unwrap();
        assert_eq!(attempts, 2);
        assert_eq!(frame.data, [9; 4]);
        assert_eq!(frame.session_info_update, 7);
        assert_eq!(frame.tick, 3);
    }

    #[test]
    fn repeated_session_transitions_exhaust_two_attempts_without_accepting() {
        let mut connection = connection_with_header(test_header(4), 272);
        connection.last_tick_count = 2;
        let mut attempts = 0;
        assert!(
            connection
                .acquire_frame(|source, attempt| {
                    attempts += 1;
                    source.test_write(
                        offset_of!(Header, session_info_update),
                        &((attempt + 1) as i32).to_le_bytes(),
                    );
                })
                .unwrap()
                .is_none()
        );
        assert_eq!(attempts, 2);
        assert_eq!(connection.last_tick_count(), 2);
    }

    #[test]
    fn tick_transition_retries_and_returns_the_new_copied_frame() {
        let mut connection = connection_with_header(test_header(4), 272);
        connection.last_tick_count = 2;
        let descriptor = offset_of!(Header, buffers) + 2 * size_of::<VariableBuffer>();
        let mut attempts = 0;
        let frame = connection
            .acquire_frame(|source, attempt| {
                attempts += 1;
                if attempt == 0 {
                    source.test_write(
                        descriptor + offset_of!(VariableBuffer, tick_count),
                        &4i32.to_le_bytes(),
                    );
                    source.test_write(
                        descriptor + offset_of!(VariableBuffer, tick_count_begin),
                        &4i32.to_le_bytes(),
                    );
                    source.test_write(264, &[8; 4]);
                }
            })
            .unwrap()
            .unwrap();
        assert_eq!(attempts, 2);
        assert_eq!(frame.tick, 4);
        assert_eq!(frame.data, [8; 4]);
    }

    #[test]
    fn changed_slot_geometry_retries_before_accepting() {
        let mut connection = connection_with_header(test_header(4), 272);
        connection.last_tick_count = 2;
        let offset = offset_of!(Header, buffers)
            + 2 * size_of::<VariableBuffer>()
            + offset_of!(VariableBuffer, buffer_offset);
        let mut attempts = 0;
        let frame = connection
            .acquire_frame(|source, attempt| {
                attempts += 1;
                if attempt == 0 {
                    source.test_write(offset, &268i32.to_le_bytes());
                    source.test_write(268, &[6; 4]);
                }
            })
            .unwrap()
            .unwrap();
        assert_eq!(attempts, 2);
        assert_eq!(frame.data, [6; 4]);
    }

    #[test]
    fn invalid_buffer_counts_are_errors() {
        for count in [-1, 0, 5] {
            let mut connection = connection_with_header(test_header(count), 272);
            assert!(connection.get_new_data().is_err());
        }
    }

    #[test]
    fn current_buffer_selection_uses_published_index_instead_of_highest_tick() {
        let header = test_header(4);
        assert_eq!(
            Connection::current_buffer_index(header.current_buffer, header.buffer_count),
            2
        );
    }

    #[test]
    fn current_buffer_selection_falls_back_to_zero_for_invalid_indices() {
        for (index, count) in [(4, 4), (255, 4), (2, 2), (2, 0), (2, -1), (4, 5)] {
            assert_eq!(Connection::current_buffer_index(index, count), 0);
        }
    }

    #[test]
    #[ignore = "iracing_required"]
    fn connects_to_live_iracing() {
        let connection = Connection::try_connect().expect("Failed to connect to iRacing");
        let header = connection.header_snapshot().unwrap();

        // Validate header structure sizes match expected C SDK layout
        assert_eq!(
            std::mem::size_of::<Header>(),
            112,
            "Header size must match C SDK"
        );
        assert!(header.tick_rate > 0, "Tick rate should be positive");

        assert_eq!(header.version, IRSDK_VER);
        assert!(header.variable_count > 0);
        assert!(header.buffer_count >= 3);
        assert!(header.buffer_length > 0);
    }

    #[test]
    #[ignore = "iracing_required"]
    fn waits_for_data_updates() {
        let mut connection = Connection::try_connect().expect("Failed to connect to iRacing");

        // Try to get new data - may or may not have data immediately
        let _data = connection.get_new_data();

        // Wait for update with short timeout - should not error
        let _result = connection
            .wait_for_update(Duration::from_millis(100))
            .expect("Failed to wait for update");
    }
}
