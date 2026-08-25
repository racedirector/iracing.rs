//! iRacing shared memory connection aligned with C++ SDK
//!
//! This module provides direct memory mapping to iRacing's shared memory
//! following the same patterns as the official C++ SDK implementation.

use crate::{
    IRacingSDKError, Result, status_is_connected,
    types::{
        FrameBuffer, Header, SessionInfoBuffer, VariableBuffer, VariableHeader,
        VariableHeadersBuffer, WireType,
    },
};
use std::mem::{MaybeUninit, size_of};
use std::ptr::NonNull;
use std::time::Duration;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Memory::{
    FILE_MAP_READ, MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
    OpenFileMappingW, UnmapViewOfFile, VirtualQuery,
};
use windows::Win32::System::Threading::{
    OpenEventW, SYNCHRONIZATION_ACCESS_RIGHTS, WaitForSingleObject,
};
use windows::core::PCWSTR;

/// iRacing shared memory file name
const IRSDK_MEMMAPFILENAME: &str = "Local\\IRSDKMemMapFileName";
/// iRacing data valid event name
const IRSDK_DATAVALIDEVENTNAME: &str = "Local\\IRSDKDataValidEvent";

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
    mapped_length: usize,
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
            let wide_name = crate::windows::wide_string(IRSDK_MEMMAPFILENAME);
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
            let wide_name = crate::windows::wide_string(IRSDK_DATAVALIDEVENTNAME);
            OpenEventW(
                SYNCHRONIZATION_ACCESS_RIGHTS(0x0010_0000),
                false,
                PCWSTR::from_raw(wide_name.as_ptr()),
            ) // SYNCHRONIZE
            .map_err(|e| IRacingSDKError::windows_api_error("OpenEventW", e))?
        };

        // Initialize with i32::MAX to match C++ SDK's INT_MAX
        // This ensures the first frame is always accepted as "new"
        let mut connection = Self {
            mapping,
            base,
            mapped_length: 0,
            event,
            last_tick_count: i32::MAX,
        };

        connection.mapped_length = connection.query_mapped_length()?;
        connection.validate_mapped_range(0, Header::WIRE_SIZE)?;

        // Validate the connection
        connection.validate_connection()?;

        Ok(connection)
    }

    /// Get a snapshot of the header
    #[inline]
    pub fn header_snapshot(&self) -> Header {
        let bytes = unsafe { &*self.base.as_ptr().cast::<[u8; Header::WIRE_SIZE]>() };

        Header::read_from_bytes(bytes).expect("Could not parse header from live connection")
    }

    /// Wait for new telemetry data (synchronous - blocks thread)
    pub fn wait_for_update(&self, timeout: Duration) -> Result<WaitResult> {
        let ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        Self::wait_for_event(self.event, ms)
    }

    /// Wait for new telemetry data (async - cooperative, non-blocking)
    ///
    /// This method uses `spawn_blocking` to isolate the synchronous Windows event wait
    /// on a dedicated blocking thread pool, preventing starvation of other async tasks.
    /// The async worker thread yields cooperatively via `.await` while the blocking
    /// thread waits for the Windows event signal.
    ///
    /// At 60Hz (16.67ms frames), the hot path (data already available) never reaches
    /// this method, so spawn_blocking overhead is only paid during startup, pauses,
    /// or frame drops - exactly when we want cooperative yielding anyway.
    pub async fn wait_for_update_async(&self, timeout: Duration) -> Result<WaitResult> {
        // Convert HANDLE to raw pointer value (usize) to make it Send
        // SAFETY: Windows event handles are thread-safe kernel objects
        let event_raw = self.event.0 as usize;
        let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as u32;

        tokio::task::spawn_blocking(move || {
            tracing::trace!(timeout_ms, "Async waiting for Windows event");

            // Reconstruct HANDLE from raw pointer value
            // SAFETY: event_raw came from a valid HANDLE, kernel object is still alive
            let event = HANDLE(event_raw as *mut std::ffi::c_void);
            Self::wait_for_event(event, timeout_ms)
        })
        .await
        .map_err(|e| {
            IRacingSDKError::buffer_operation_error(
                format!("Event wait task panicked: {}", e),
                None,
            )
        })?
    }

    /// Get latest telemetry data if available.
    pub fn get_new_data(&mut self) -> Option<FrameBuffer> {
        if !self.is_connected() {
            tracing::debug!("Not connected to iRacing");
            self.last_tick_count = i32::MAX;
            return None;
        }

        // Cheap fast-path: has iRacing advertised a new frame?
        if self.current_buffer_tick_count() == self.last_tick_count {
            return None;
        }

        let header = self.header_snapshot();
        let buffer_length = usize::try_from(header.buffer_length).ok()?;

        for attempt in 0..2 {
            let (latest_buffer, _) = self.read_latest_buffer()?;
            if latest_buffer.tick_count == self.last_tick_count {
                return None;
            }

            let buffer_offset = usize::try_from(latest_buffer.buffer_offset).ok()?;
            let frame = self.snapshot_region(buffer_offset, buffer_length)?;

            if self.current_buffer_tick_count() != latest_buffer.tick_count {
                tracing::trace!("Telemetry advanced during copy on attempt {}", attempt + 1);
                continue;
            }

            self.last_tick_count = latest_buffer.tick_count;

            tracing::trace!(
                "Returning new data: tick={}, size={} bytes",
                latest_buffer.tick_count,
                frame.len()
            );

            return Some(FrameBuffer(frame));
        }

        tracing::warn!("Failed consistency checks, no data returned");
        None
    }

    /// Validate initial connection
    fn validate_connection(&self) -> Result<()> {
        let header = self.header_snapshot();
        header.validate_live()?;
        self.validate_header_ranges(&header)?;

        tracing::debug!(
            ver = header.version,
            num_vars = header.variable_count,
            num_buf = header.buffer_count,
            "Validated iRacing header"
        );

        Ok(())
    }

    fn query_mapped_length(&self) -> Result<usize> {
        let mut information = MaybeUninit::<MEMORY_BASIC_INFORMATION>::uninit();
        let bytes_written = unsafe {
            VirtualQuery(
                Some(self.base.as_ptr().cast()),
                information.as_mut_ptr(),
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };

        if bytes_written != size_of::<MEMORY_BASIC_INFORMATION>() {
            return Err(IRacingSDKError::windows_api_error(
                "VirtualQuery",
                windows::core::Error::from_thread(),
            ));
        }

        let information = unsafe { information.assume_init() };
        Ok(information.RegionSize)
    }

    fn validate_header_ranges(&self, header: &Header) -> Result<()> {
        let session_offset = usize::try_from(header.session_info_offset)
            .map_err(|_| IRacingSDKError::memory_access_error(0))?;
        let session_length = usize::try_from(header.session_info_len)
            .map_err(|_| IRacingSDKError::memory_access_error(session_offset))?;
        self.validate_mapped_range(session_offset, session_length)?;

        let variable_offset = usize::try_from(header.variable_header_offset)
            .map_err(|_| IRacingSDKError::memory_access_error(0))?;
        let variable_count = usize::try_from(header.variable_count)
            .map_err(|_| IRacingSDKError::memory_access_error(variable_offset))?;
        let variable_length = variable_count
            .checked_mul(VariableHeader::WIRE_SIZE)
            .ok_or_else(|| IRacingSDKError::memory_access_error(variable_offset))?;
        self.validate_mapped_range(variable_offset, variable_length)?;

        let buffer_length = usize::try_from(header.buffer_length)
            .map_err(|_| IRacingSDKError::memory_access_error(0))?;
        for buffer in &header.buffers[..header.buffer_count as usize] {
            let buffer_offset = usize::try_from(buffer.buffer_offset)
                .map_err(|_| IRacingSDKError::memory_access_error(0))?;
            self.validate_mapped_range(buffer_offset, buffer_length)?;
        }

        Ok(())
    }

    fn validate_mapped_range(&self, offset: usize, length: usize) -> Result<()> {
        let end = offset
            .checked_add(length)
            .ok_or_else(|| IRacingSDKError::memory_access_error(offset))?;

        if end > self.mapped_length {
            return Err(IRacingSDKError::memory_access_error(end));
        }

        Ok(())
    }
}

/// Utilities and helpers for reading from the header efficiently
impl Connection {
    // Reads a value directly from the header memory
    #[inline]
    unsafe fn read_header_at<T: Copy>(&self, offset: usize) -> T {
        debug_assert!(
            offset
                .checked_add(std::mem::size_of::<T>())
                .is_some_and(|end| end <= Header::WIRE_SIZE)
        );

        unsafe {
            let ptr = self.base.as_ptr().add(offset).cast::<T>();

            debug_assert_eq!(ptr.align_offset(std::mem::align_of::<T>()), 0);

            std::ptr::read_volatile(ptr)
        }
    }

    // Unsafe read directly from base pointer
    #[inline]
    fn snapshot_region(&self, offset: usize, length: usize) -> Option<Vec<u8>> {
        self.validate_mapped_range(offset, length).ok()?;
        let mut buffer = Vec::with_capacity(length);

        unsafe {
            std::ptr::copy_nonoverlapping(
                self.base.as_ptr().add(offset),
                buffer.as_mut_ptr(),
                length,
            );

            buffer.set_len(length);
        }

        Some(buffer)
    }

    #[inline]
    fn buffer_snapshot(&self, index: usize) -> VariableBuffer {
        debug_assert!(index < Header::MAX_BUFFERS);

        // Get the offset to the buffer at `index`
        let offset = std::mem::offset_of!(Header, buffers) + index * VariableBuffer::WIRE_SIZE;

        let bytes = unsafe {
            &*self
                .base
                .as_ptr()
                .add(offset)
                .cast::<[u8; VariableBuffer::WIRE_SIZE]>()
        };

        VariableBuffer::read_from_bytes(bytes)
            .expect("Could not parse valid buffer from connection")
    }

    #[inline]
    fn read_latest_buffer(&self) -> Option<(VariableBuffer, usize)> {
        loop {
            let index = self.current_buffer_index() as usize;
            let buffer_count = self.buffer_count();

            if !(3..=Header::MAX_BUFFERS).contains(&buffer_count) || index >= buffer_count {
                tracing::warn!(
                    index,
                    buffer_count,
                    "iRacing advertised an invalid current buffer index"
                );
                return None;
            }

            let buffer = self.buffer_snapshot(index);
            let current_tick = self.current_buffer_tick_count();

            if buffer.tick_count == current_tick && buffer.tick_count_begin == current_tick {
                return Some((buffer, index));
            }
        }
    }

    /// Get the tick rate for the session
    pub fn tick_rate(&self) -> i32 {
        unsafe { self.read_header_at::<i32>(std::mem::offset_of!(Header, tick_rate)) }
    }

    /// Get session info update counter
    pub fn session_info_update(&self) -> i32 {
        unsafe { self.read_header_at::<i32>(std::mem::offset_of!(Header, session_info_update)) }
    }

    /// Reads the YAML session string out of the pointer
    pub fn session_info_buffer(&self) -> Option<SessionInfoBuffer> {
        let header = self.header_snapshot();

        let offset = usize::try_from(header.session_info_offset).ok()?;
        let length = usize::try_from(header.session_info_len).ok()?;

        if length == 0 {
            return None;
        }

        Some(SessionInfoBuffer(self.snapshot_region(offset, length)?))
    }

    /// Reads an available variables snapshot out of the pointer
    pub(crate) fn variable_headers_buffer(&self) -> Option<VariableHeadersBuffer> {
        let header = self.header_snapshot();

        let offset = usize::try_from(header.variable_header_offset).ok()?;
        let count = usize::try_from(header.variable_count).ok()?;

        if count == 0 {
            return None;
        }

        let length = count.checked_mul(VariableHeader::WIRE_SIZE)?;

        Some(VariableHeadersBuffer::from_snapshot(
            self.snapshot_region(offset, length)?,
        ))
    }

    /// The latest buffer's tick count
    pub fn current_buffer_tick_count(&self) -> i32 {
        unsafe {
            self.read_header_at::<i32>(std::mem::offset_of!(Header, current_buffer_tick_count))
        }
    }

    /// The index of the current buffer
    pub fn current_buffer_index(&self) -> u8 {
        unsafe { self.read_header_at::<u8>(std::mem::offset_of!(Header, current_buffer)) }
    }

    /// The current buffer count.
    pub fn buffer_count(&self) -> usize {
        unsafe { self.read_header_at::<i32>(std::mem::offset_of!(Header, buffer_count)) }
            .try_into()
            .unwrap_or(0)
    }

    /// The status bit from the header.
    pub fn connection_status(&self) -> i32 {
        unsafe { self.read_header_at::<i32>(std::mem::offset_of!(Header, status)) }
    }

    /// Check if iRacing is connected
    pub fn is_connected(&self) -> bool {
        let status = self.connection_status();
        status_is_connected(status)
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
    use crate::{IRSDK_VERSION, VariableBuffer, VariableSchema, types::irsdk::StatusField};

    use super::*;
    use std::mem::ManuallyDrop;

    fn test_connection() -> ManuallyDrop<Connection> {
        ManuallyDrop::new(Connection {
            mapping: HANDLE::default(),
            base: NonNull::dangling(),
            mapped_length: 0,
            event: HANDLE::default(),
            last_tick_count: i32::MAX,
        })
    }

    fn test_header(buffer_count: i32) -> Header {
        Header::new(
            IRSDK_VERSION,
            StatusField::CONNECTED,
            60,
            0,
            0,
            0,
            1,
            112,
            buffer_count,
            4,
            0,
            0,
            [
                VariableBuffer::new(1, 256, 0),
                VariableBuffer::new(4, 260, 0),
                VariableBuffer::new(3, 264, 0),
                VariableBuffer::new(2, 268, 0),
            ],
        )
    }

    #[test]
    #[ignore = "iracing_required"]
    fn test_read_rpm_variable() {
        let connection = Connection::try_connect().expect("Failed to connect to iRacing");
        let buffer = connection
            .variable_headers_buffer()
            .expect("Could not get VariableInfo from connection");

        let headers: Vec<VariableHeader> = buffer.into();
        assert!(!headers.is_empty(), "Buffer should have some variables");

        let variables = VariableSchema::from_connection(&connection)
            .expect("Could not get variable schema")
            .variables();

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
        let header = connection.header_snapshot();

        // Validate header structure sizes match expected C SDK layout
        assert_eq!(
            std::mem::size_of::<Header>(),
            112,
            "Header size must match C SDK"
        );
        assert!(header.tick_rate > 0, "Tick rate should be positive");

        assert_eq!(header.version, IRSDK_VERSION);
        assert!(header.variable_count > 0);
        assert!(header.buffer_count >= 3);
        assert!(header.buffer_count > 0);
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
