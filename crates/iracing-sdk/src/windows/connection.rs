//! iRacing shared memory connection aligned with C++ SDK
//!
//! This module provides direct memory mapping to iRacing's shared memory
//! following the same patterns as the official C++ SDK implementation.

use iracing_irsdk::{StatusField, VariableBuffer};

use super::source::WaitResult;
use crate::ByteRegion;
use crate::{
    IRacingSDKError, Result, SessionInfoBuffer, SessionInfoRegion, VariableHeadersBuffer,
    VariableHeadersRegion, irsdk::Header, windows::source::LiveSource,
};
use std::mem::offset_of;
use std::time::Duration;

/// Direct connection to iRacing shared memory
#[derive(Debug)]
pub struct Connection {
    source: LiveSource,

    frame_data: Vec<u8>,
    last_tick_count: i32,
}

impl Connection {
    /// Attempt to connect to iRacing shared memory
    ///
    /// # Errors
    /// Returns an error when opening the mapping/event fails or the mapped
    /// extent cannot hold the complete fixed SDK header.
    pub fn try_connect() -> Result<Self> {
        tracing::trace!("Attempting to connect to iRacing shared memory");

        // Initialize with i32::MAX to match C++ SDK's INT_MAX
        // The first observed tick establishes the baseline without a frame.
        let connection = Self::from_source(LiveSource::try_connect()?)?;

        tracing::trace!("Successfully connected to iRacing shared memory");

        Ok(connection)
    }

    fn from_source(source: LiveSource) -> Result<Self> {
        if source.len() < size_of::<Header>() {
            return Err(IRacingSDKError::parse_error(
                "Header",
                "Mapped header is truncated",
            ));
        }
        Ok(Self {
            source,

            frame_data: Vec::new(),
            last_tick_count: i32::MAX,
        })
    }

    /// Borrows the legacy header directly from the shared-memory mapping.
    ///
    /// The header contents are volatile: iRacing can change them independently
    /// of this connection. This compatibility accessor does not perform volatile
    /// loads or provide a coherent snapshot. Ordinary Rust references require
    /// the referent to remain unchanged, which the live simulator does not
    /// guarantee; retaining this API does not resolve that legacy limitation.
    /// New code should use [`Self::header_snapshot`] or scalar accessors.
    #[deprecated(
        note = "use header_snapshot() or scalar accessors; mapped header contents can change"
    )]
    pub fn header(&self) -> &Header {
        assert!(
            self.source.len() >= size_of::<Header>(),
            "mapped header is truncated"
        );
        // Legacy compatibility only: bounds and page alignment are established,
        // but concurrent simulator mutation remains the documented limitation.
        unsafe { &*self.source.legacy_header_ptr().cast::<Header>() }
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

    /// Copies the published current telemetry buffer into connection-owned storage.
    ///
    /// Returns a slice only after the completed tick read before the copy matches
    /// the begin tick read afterward, retrying acquisition at most twice. Equal
    /// ticks return no data; older ticks establish a new baseline without a frame.
    pub fn get_new_data(&mut self) -> Option<&[u8]> {
        if !self.is_connected() {
            tracing::debug!("Not connected to iRacing");
            self.last_tick_count = i32::MAX;
            return None;
        }

        let header = self.header_snapshot().ok()?;
        // Use the published current buffer, with the native fallback to zero.
        // The backing array also bounds malformed advertised buffer counts.
        let current_buffer = self.source.read_u8(offset_of!(Header, current_buffer))?;
        let latest_buf_idx = Self::current_buffer_index(current_buffer, header.buffer_count);
        let descriptor_offset =
            offset_of!(Header, buffers) + latest_buf_idx * size_of::<VariableBuffer>();
        let tick_offset = descriptor_offset + offset_of!(VariableBuffer, tick_count);
        let begin_offset = descriptor_offset + offset_of!(VariableBuffer, tick_count_begin);

        let latest_tick = self.source.read_i32(tick_offset)?;

        if self.last_tick_count == latest_tick {
            return None;
        }

        // Match the native SDK, including its initial INT_MAX sentinel:
        // record an older tick, but do not return that frame.
        if self.last_tick_count > latest_tick {
            tracing::trace!(
                "Tick count reset detected: {} -> {}",
                self.last_tick_count,
                latest_tick
            );
            self.last_tick_count = latest_tick;
            return None;
        }

        let frame_offset = usize::try_from(header.buffers[latest_buf_idx].buffer_offset).ok()?;
        let frame_len = usize::try_from(header.buffer_length).ok()?;

        if frame_offset.checked_add(frame_len)? > self.source.len() {
            return None;
        }

        self.frame_data.resize(frame_len, 0);

        // Keep the selected descriptor fixed across both attempts,
        // as the native SDK does.
        for attempt in 0..2 {
            let tick_count = self.source.read_i32(tick_offset)?;

            // SAFETY: The source range was checked above. frame_data is
            // initialized Rust-owned storage, disjoint from the mapping.
            // Geometry is assumed stable during this acquisition, as in
            // the native SDK.
            unsafe {
                self.source
                    .copy_unchecked(frame_offset, &mut self.frame_data)
                    .ok()?;
            }

            let tick_count_begin = self.source.read_i32(begin_offset)?;

            if tick_count == tick_count_begin {
                self.last_tick_count = tick_count;
                return Some(self.frame_data.as_slice());
            }

            tracing::trace!(
                "Data consistency check failed on attempt {}: \
             tick_count={}, tick_count_begin={}",
                attempt + 1,
                tick_count,
                tick_count_begin
            );
        }

        tracing::warn!("Failed consistency checks, no data returned");
        None
    }

    /// Copies the session-information region advertised by the live header.
    ///
    /// Returns `None` when the header advertises no usable region.
    pub fn session_info_buffer(&self) -> Option<SessionInfoBuffer> {
        let header = self.header_snapshot().ok()?;

        let region = SessionInfoRegion::try_from_header(&header).ok()??;

        let bytes = self.copy_region(region.as_region())?;

        Some(SessionInfoBuffer::from_checked_region(&bytes))
    }

    /// Copies the variable-header region advertised by the live header.
    ///
    /// Returns `None` when the header advertises no usable region.
    pub fn variable_headers_buffer(&self) -> Option<VariableHeadersBuffer> {
        let header = self.header_snapshot().ok()?;
        let region = VariableHeadersRegion::try_from_header(&header).ok()??;

        let variable_header_bytes = self.copy_region(region.as_region())?;

        VariableHeadersBuffer::try_from_region_bytes(&variable_header_bytes, region.count()).ok()
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
        Connection::from_source(LiveSource::test_source(&bytes)).unwrap()
    }

    #[test]
    fn activation_rejects_truncated_headers() {
        for len in [1, offset_of!(Header, tick_rate), size_of::<Header>() - 1] {
            assert!(Connection::from_source(LiveSource::test_source(&vec![0; len])).is_err());
        }
    }

    #[test]
    fn acquisition_copies_published_frame_and_tracks_ticks() {
        let mut connection = connection_with_header(test_header(4), 272);
        assert_eq!(connection.tick_rate(), 60);
        assert_eq!(connection.session_info_update(), 0);
        assert!(connection.get_new_data().is_none());
        assert_eq!(connection.last_tick_count(), 3);
        connection.last_tick_count = 2;
        assert_eq!(connection.get_new_data().unwrap(), &[1, 2, 3, 4]);
        assert!(connection.get_new_data().is_none());
        connection.last_tick_count = 4;
        assert!(connection.get_new_data().is_none());
        assert_eq!(connection.last_tick_count(), 3);
    }

    #[test]
    fn acquisition_rejects_torn_frames_and_invalid_regions() {
        for (offset, len, begin) in [(264, 4, 9), (-1, 4, 3), (270, 4, 3), (264, -1, 3)] {
            let mut header = test_header(4);
            header.buffers[2].buffer_offset = offset;
            header.buffers[2].tick_count_begin = begin;
            header.buffer_length = len;
            let mut connection = connection_with_header(header, 272);
            connection.last_tick_count = 2;
            assert!(connection.get_new_data().is_none());
            assert_eq!(connection.last_tick_count(), 2);
        }
        let mut header = test_header(4);
        header.session_info_offset = 270;
        header.session_info_length = 4;
        header.variable_header_offset = 270;
        let connection = connection_with_header(header, 272);
        assert!(connection.session_info_buffer().is_none());
        assert!(connection.variable_headers_buffer().is_none());
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
