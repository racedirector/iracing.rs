//! iRacing shared memory connection aligned with C++ SDK
//!
//! This module provides direct memory mapping to iRacing's shared memory
//! following the same patterns as the official C++ SDK implementation.

use crate::{
    ByteParser, ByteRegion, IRacingSDKError, IRacingSessionString, Result, SessionInfoBuffer,
    SessionInfoRegion, VariableHeaderRegion, VariableHeadersBuffer, VariableInfo,
    irsdk::{
        Header,
        constants::{IRSDK_DATAVALIDEVENTNAME, IRSDK_MEMMAPFILENAME},
    },
    windows::wide_string,
};
use std::ptr::NonNull;
use std::time::Duration;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Memory::{
    FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile,
};
use windows::Win32::System::Threading::{
    OpenEventW, SYNCHRONIZATION_ACCESS_RIGHTS, WaitForSingleObject,
};
use windows::core::PCWSTR;

/// Result of waiting for data updates
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitResult {
    /// Wait resolved with data.
    Signaled,
    /// Wait time elapsed.
    Timeout,
}

/// Direct connection to iRacing shared memory
#[derive(Debug)]
pub struct Connection {
    mapping: HANDLE,
    base: NonNull<u8>,
    event: HANDLE,
    last_tick_count: i32,
}

impl Connection {
    fn wait_for_event(event: HANDLE, timeout_ms: u32) -> Result<WaitResult> {
        tracing::trace!(timeout_ms = timeout_ms, "Waiting for telemetry update");

        let result = unsafe { WaitForSingleObject(event, timeout_ms) };

        match result {
            WAIT_OBJECT_0 => {
                tracing::trace!("Telemetry update signaled");
                Ok(WaitResult::Signaled)
            }
            WAIT_TIMEOUT => {
                tracing::trace!("Wait timed out");
                Ok(WaitResult::Timeout)
            }
            _ => {
                let win_err = windows::core::Error::from_thread();
                Err(IRacingSDKError::windows_api_error(
                    "WaitForSingleObject",
                    win_err,
                ))
            }
        }
    }

    /// Attempt to connect to iRacing shared memory
    pub fn try_connect() -> Result<Self> {
        tracing::trace!("Attempting to connect to iRacing shared memory");

        // Open the memory mapping
        let mapping = unsafe {
            let wide_name = wide_string(IRSDK_MEMMAPFILENAME);
            OpenFileMappingW(FILE_MAP_READ.0, false, PCWSTR::from_raw(wide_name.as_ptr()))
                .map_err(|e| IRacingSDKError::windows_api_error("OpenFileMappingW", e))?
        };

        // Map the view
        let base = unsafe {
            let ptr = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
            NonNull::new(ptr.Value as *mut u8).ok_or_else(|| {
                let win_err = windows::core::Error::from_thread();
                IRacingSDKError::windows_api_error("MapViewOfFile", win_err)
            })?
        };

        // Open the data valid event
        let event = unsafe {
            let wide_name = wide_string(IRSDK_DATAVALIDEVENTNAME);
            OpenEventW(
                SYNCHRONIZATION_ACCESS_RIGHTS(0x0010_0000),
                false,
                PCWSTR::from_raw(wide_name.as_ptr()),
            ) // SYNCHRONIZE
            .map_err(|e| IRacingSDKError::windows_api_error("OpenEventW", e))?
        };

        // Initialize with i32::MAX to match C++ SDK's INT_MAX
        // This ensures the first frame is always accepted as "new"
        let connection = Self {
            mapping,
            base,
            event,
            last_tick_count: i32::MAX,
        };

        // Validate the connection
        connection.validate_connection()?;

        tracing::debug!("Initialized last_tick_count to i32::MAX for first frame acceptance");
        tracing::debug!("Successfully connected to iRacing shared memory");

        Ok(connection)
    }

    /// Get direct access to the header
    pub fn header(&self) -> &Header {
        unsafe { &*(self.base.as_ptr() as *const Header) }
    }

    /// Check if iRacing is connected
    pub fn is_connected(&self) -> bool {
        self.header().status.is_connected()
    }

    /// Wait for new telemetry data (synchronous - blocks thread)
    pub fn wait_for_update(&self, timeout: Duration) -> Result<WaitResult> {
        let ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        Self::wait_for_event(self.event, ms)
    }

    /// Wait for new telemetry data (async - waits on the calling thread).
    ///
    /// The event wait runs directly on the calling thread, so the Windows
    /// event resumes the caller with no intermediate scheduler hop. This
    /// matters for latency-sensitive consumers that run the provider on a
    /// dedicated runtime, where the extra wake-up per 60 Hz frame is pure
    /// overhead.
    ///
    /// Blocking the executor is kept safe by splitting the requested timeout
    /// into 50 ms chunks with a `yield_now` between them, so expired timers
    /// and ready sibling tasks are serviced while the wait is in progress.
    /// The event ends the current chunk immediately when it fires; `Timeout`
    /// is returned only once the full requested duration has elapsed. In the
    /// connected steady state the event fires within one 60 Hz frame
    /// (~16.7 ms), so the wait completes inside the first chunk.
    pub async fn wait_for_update_async(&self, timeout: Duration) -> Result<WaitResult> {
        const WAIT_CHUNK: Duration = Duration::from_millis(50);

        let mut remaining = timeout;
        loop {
            // Let the runtime service expired timers and sibling tasks before
            // the thread commits to the blocking wait.
            tokio::task::yield_now().await;

            let chunk = remaining.min(WAIT_CHUNK);
            let chunk_ms = chunk.as_millis() as u32;
            tracing::trace!(chunk_ms, "Waiting for Windows event on the calling thread");
            match Self::wait_for_event(self.event, chunk_ms)? {
                WaitResult::Timeout => {
                    remaining = remaining.saturating_sub(chunk);
                    if remaining.is_zero() {
                        return Ok(WaitResult::Timeout);
                    }
                }
                signaled => return Ok(signaled),
            }
        }
    }

    /// Get latest telemetry data if available
    pub fn get_new_data(&mut self) -> Option<&[u8]> {
        if !self.is_connected() {
            tracing::debug!("Not connected to iRacing");
            self.last_tick_count = i32::MAX;
            return None;
        }

        let header = self.header();

        // Find the buffer with the highest tick count (most recent)
        let latest_buf_idx = self.find_latest_buffer(header);
        let latest_buf = &header.buffers[latest_buf_idx];

        tracing::trace!(
            "Checking for new data: last_tick={}, latest_tick={}, buffer_idx={}",
            self.last_tick_count,
            latest_buf.tick_count,
            latest_buf_idx
        );

        // Check if we have new data
        if self.last_tick_count == latest_buf.tick_count {
            tracing::trace!("No new data (same tick count)");
            return None;
        }

        // Handle potential tick count reset or wraparound
        if self.last_tick_count > latest_buf.tick_count && self.last_tick_count != i32::MAX {
            tracing::trace!(
                "Tick count reset detected: {} -> {}",
                self.last_tick_count,
                latest_buf.tick_count
            );
        }

        // Double-read pattern to ensure data consistency
        for attempt in 0..2 {
            let tick_before = latest_buf.tick_count;
            let data_ptr = unsafe { self.base.as_ptr().add(latest_buf.buffer_offset as usize) };
            let data_slice =
                unsafe { std::slice::from_raw_parts(data_ptr, header.buffer_length as usize) };
            let tick_after = latest_buf.tick_count;

            if tick_before == tick_after {
                self.last_tick_count = tick_before;
                tracing::trace!(
                    "Returning new data: tick={}, size={} bytes",
                    tick_before,
                    data_slice.len()
                );
                return Some(data_slice);
            } else {
                tracing::trace!(
                    "Data consistency check failed on attempt {}: before={}, after={}",
                    attempt + 1,
                    tick_before,
                    tick_after
                );
            }
        }

        tracing::warn!("Failed consistency checks, no data returned");
        None
    }

    /// Copies the session-information region advertised by the live header.
    ///
    /// Returns `None` when the header advertises no usable region.
    pub fn session_info_buffer(&self) -> Option<SessionInfoBuffer> {
        let header = self.header();

        let region = SessionInfoRegion::try_from(header)
            .ok()
            .filter(|r| r.is_valid())
            .map(|r| r.as_region())?;

        let session_info_bytes = self.bytes_at_region(region);

        Some(SessionInfoBuffer::from_checked_region(session_info_bytes))
    }

    /// Returns decoded live session-information text with invalid control characters removed.
    ///
    /// Returns `None` when no usable region exists or the NUL-bounded payload is
    /// empty after sanitization.
    pub fn session_info(&self) -> Option<String> {
        let buffer = self.session_info_buffer()?;
        let session_info = IRacingSessionString::try_from(buffer).ok()?;

        Some(session_info.into())
    }

    /// Get session info update counter
    pub fn session_info_update(&self) -> i32 {
        self.header().session_info_update
    }

    /// Copies the variable-header region advertised by the live header.
    ///
    /// Returns `None` when the header advertises no usable region.
    pub fn variable_headers_buffer(&self) -> Option<VariableHeadersBuffer> {
        let header = self.header();

        let region = VariableHeaderRegion::try_from(header)
            .ok()
            .filter(|r| r.is_valid())
            .map(|r| r.region)?;

        let variable_header_bytes = self.bytes_at_region(region);

        Some(VariableHeadersBuffer::from_checked_region(
            variable_header_bytes,
        ))
    }

    /// Decodes all variable definitions from a copied variable-header region.
    ///
    /// Returns an empty vector when no usable variable-header region exists.
    ///
    /// # Errors
    ///
    /// Returns an error if any variable header contains invalid metadata.
    pub fn get_variables(&self) -> Result<Vec<VariableInfo>> {
        let buffer = match self.variable_headers_buffer() {
            Some(b) => b,
            _ => return Ok(Vec::new()),
        };

        buffer.iter_headers().map(VariableInfo::try_from).collect()
    }

    /// Validate initial connection
    fn validate_connection(&self) -> Result<()> {
        let header = self.header();
        header.validate_live()?;

        tracing::debug!(
            ver = header.version,
            num_vars = header.variable_count,
            num_buf = header.buffer_count,
            "Validated iRacing header"
        );

        Ok(())
    }

    /// Find the buffer with the highest tick count
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

impl ByteParser for Connection {
    fn bytes_at_region(&self, region: ByteRegion) -> &[u8] {
        unsafe {
            let bytes_ptr = self.base.as_ptr().add(region.offset);
            std::slice::from_raw_parts(bytes_ptr, region.length)
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            let addr = MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.base.as_ptr() as *mut _,
            };
            let _ = UnmapViewOfFile(addr);
            let _ = CloseHandle(self.mapping);
            let _ = CloseHandle(self.event);
        }
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
    use std::mem::ManuallyDrop;

    fn test_connection() -> ManuallyDrop<Connection> {
        ManuallyDrop::new(Connection {
            mapping: HANDLE::default(),
            base: NonNull::dangling(),
            event: HANDLE::default(),
            last_tick_count: i32::MAX,
        })
    }

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

    #[test]
    fn find_latest_buffer_caps_count_at_backing_array_length() {
        let connection = test_connection();
        let header = test_header(5);

        assert_eq!(connection.find_latest_buffer(&header), 1);
    }

    #[test]
    #[ignore = "known bug: a negative num_buf is cast to usize after applying only an upper bound"]
    fn find_latest_buffer_does_not_panic_for_negative_count() {
        let connection = test_connection();
        let header = test_header(-1);

        let result = std::panic::catch_unwind(|| connection.find_latest_buffer(&header));

        assert!(result.is_ok(), "negative num_buf must not cause a panic");
    }

    #[test]
    #[ignore = "iracing_required"]
    fn test_read_rpm_variable() {
        let connection = Connection::try_connect().expect("Failed to connect to iRacing");
        let variables = connection
            .get_variables()
            .expect("Could not get variables from connection");

        // Look for exact "RPM" match to verify variable schema
        let exact_rpm = variables.iter().find(|v| v.name == "RPM");
        assert!(
            exact_rpm.is_some(),
            "RPM variable should be available in iRacing"
        );

        assert!(!variables.is_empty(), "Should have some variables");
    }

    #[test]
    #[ignore = "iracing_required"]
    fn connects_to_live_iracing() {
        let connection = Connection::try_connect().expect("Failed to connect to iRacing");
        let header = connection.header();

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
