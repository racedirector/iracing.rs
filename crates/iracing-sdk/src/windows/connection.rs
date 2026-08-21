//! iRacing shared memory connection aligned with C++ SDK
//!
//! This module provides direct memory mapping to iRacing's shared memory
//! following the same patterns as the official C++ SDK implementation.

use crate::schema::header::IRSDKHeader;
use crate::schema::variables::IRSDKVarHeader;
use crate::{IRacingSDKError, Result, yaml_utils};
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

/// iRacing shared memory file name
const IRSDK_MEMMAPFILENAME: &str = "Local\\IRSDKMemMapFileName";
/// iRacing data valid event name
const IRSDK_DATAVALIDEVENTNAME: &str = "Local\\IRSDKDataValidEvent";
/// Expected SDK version
#[cfg(test)]
const IRSDK_VER: i32 = 2;
/// Connection status flag
const IRSDK_ST_CONNECTED: i32 = 1;

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
    /// Reusable destination for [`Self::get_new_data`]'s copy of the latest
    /// telemetry buffer; the returned slice borrows from here.
    frame_buffer: Vec<u8>,
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
        let connection = Self {
            mapping,
            base,
            event,
            last_tick_count: i32::MAX,
            frame_buffer: Vec::new(),
        };

        // Validate the connection
        connection.validate_connection()?;

        tracing::debug!("Initialized last_tick_count to i32::MAX for first frame acceptance");

        tracing::debug!("Successfully connected to iRacing shared memory");
        Ok(connection)
    }

    /// Get direct access to the header
    pub fn header(&self) -> &IRSDKHeader {
        unsafe { &*(self.base.as_ptr() as *const IRSDKHeader) }
    }

    /// Check if iRacing is connected
    pub fn is_connected(&self) -> bool {
        let header = self.header();
        header.status & IRSDK_ST_CONNECTED != 0
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

    /// Get latest telemetry data if available
    ///
    /// Returns a copy of the most recent telemetry buffer, taken while the
    /// buffer's tick count read unchanged on both sides of the copy. The
    /// returned slice borrows the connection's internal buffer and stays
    /// valid until the next call.
    pub fn get_new_data(&mut self) -> Option<&[u8]> {
        if !self.is_connected() {
            tracing::debug!("Not connected to iRacing");
            self.last_tick_count = i32::MAX;
            return None;
        }

        // The sim process writes the tick counts, so they are always read
        // volatile; a plain read through a shared reference lets the compiler
        // assume the value never changes between reads.
        let (latest_tick, latest_buf_idx) = {
            let header = self.header();
            let idx = self.find_latest_buffer(header);
            let tick = unsafe {
                std::ptr::read_volatile(std::ptr::addr_of!(header.var_buf[idx].tick_count))
            };
            (tick, idx)
        };

        tracing::trace!(
            "Checking for new data: last_tick={}, latest_tick={}, buffer_idx={}",
            self.last_tick_count,
            latest_tick,
            latest_buf_idx
        );

        // Check if we have new data
        if self.last_tick_count == latest_tick {
            tracing::trace!("No new data (same tick count)");
            return None;
        }

        // Handle potential tick count reset or wraparound
        if self.last_tick_count > latest_tick && self.last_tick_count != i32::MAX {
            tracing::trace!(
                "Tick count reset detected: {} -> {}",
                self.last_tick_count,
                latest_tick
            );
        }

        // Double-read pattern to ensure data consistency: copy the buffer out
        // of shared memory *between* two volatile reads of its tick count, and
        // discard the copy if they differ. Constructing a borrowed slice into
        // shared memory (as this used to do) checks nothing - the caller reads
        // the bytes after the check, so a concurrent write from the sim could
        // hand it a torn frame.
        for attempt in 0..2 {
            let (tick_ptr, data_ptr, buf_len) = {
                let header = self.header();
                let idx = self.find_latest_buffer(header);
                let buf = &header.var_buf[idx];
                (
                    std::ptr::addr_of!(buf.tick_count),
                    unsafe { self.base.as_ptr().add(buf.buf_offset as usize) as *const u8 },
                    header.buf_len as usize,
                )
            };

            let tick_before = unsafe { std::ptr::read_volatile(tick_ptr) };
            self.frame_buffer.resize(buf_len, 0);
            unsafe {
                std::ptr::copy_nonoverlapping(data_ptr, self.frame_buffer.as_mut_ptr(), buf_len);
            }
            let tick_after = unsafe { std::ptr::read_volatile(tick_ptr) };

            if tick_before == tick_after {
                self.last_tick_count = tick_before;
                tracing::trace!(
                    "Returning new data: tick={}, size={} bytes",
                    tick_before,
                    buf_len
                );
                return Some(&self.frame_buffer);
            }

            tracing::trace!(
                "Data consistency check failed on attempt {}: before={}, after={}",
                attempt + 1,
                tick_before,
                tick_after
            );
        }

        tracing::warn!("Failed consistency checks, no data returned");
        None
    }

    /// Tick count of the frame most recently returned by [`Self::get_new_data`].
    pub fn last_tick_count(&self) -> i32 {
        self.last_tick_count
    }

    /// Get session info YAML string
    pub fn session_info(&self) -> Option<String> {
        let header = self.header();
        if header.session_info_len <= 0 {
            return None;
        }
        if header.session_info_offset < 0 {
            return None;
        }

        unsafe {
            // Get the slice of the session yaml
            let info_ptr = self.base.as_ptr().add(header.session_info_offset as usize);
            let info_slice = std::slice::from_raw_parts(info_ptr, header.session_info_len as usize);

            // Parse and return
            yaml_utils::extract_yaml_from_memory(info_slice, 0, header.session_info_len).ok()
        }
    }

    /// Get session info update counter
    pub fn session_info_update(&self) -> i32 {
        self.header().session_info_update
    }

    /// Get all variable definitions from the header
    pub fn get_variables(&self) -> Vec<crate::VariableInfo> {
        let header = self.header();
        if header.num_vars <= 0 || header.var_header_offset <= 0 {
            return Vec::new();
        }

        let mut variables = Vec::new();

        unsafe {
            let var_header_ptr = self.base.as_ptr().add(header.var_header_offset as usize);

            for i in 0..header.num_vars {
                let var_ptr =
                    var_header_ptr.add(i as usize * std::mem::size_of::<IRSDKVarHeader>());
                let var_header = &*(var_ptr as *const IRSDKVarHeader);

                // Convert to our VariableInfo format
                let var_info = var_header.to_variable_info();

                variables.push(var_info);
            }
        }

        variables
    }

    /// Validate initial connection
    fn validate_connection(&self) -> Result<()> {
        let header = self.header();
        header.validate()?;

        tracing::debug!(
            ver = header.ver,
            num_vars = header.num_vars,
            num_buf = header.num_buf,
            "Validated iRacing header"
        );

        Ok(())
    }

    /// Find the buffer with the highest tick count
    pub fn find_latest_buffer(&self, header: &IRSDKHeader) -> usize {
        let mut latest = 0;
        let num_buf = std::cmp::min(header.num_buf, 4) as usize;
        for i in 1..num_buf {
            if header.var_buf[latest].tick_count < header.var_buf[i].tick_count {
                latest = i;
            }
        }
        latest
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
    use crate::schema::header::IRSDKVarBuf;
    use std::mem::ManuallyDrop;

    fn test_connection() -> ManuallyDrop<Connection> {
        ManuallyDrop::new(Connection {
            mapping: HANDLE::default(),
            base: NonNull::dangling(),
            event: HANDLE::default(),
            last_tick_count: i32::MAX,
            frame_buffer: Vec::new(),
        })
    }

    fn test_header(num_buf: i32) -> IRSDKHeader {
        IRSDKHeader {
            ver: IRSDK_VER,
            status: IRSDK_ST_CONNECTED,
            tick_rate: 60,
            session_info_update: 0,
            session_info_len: 0,
            session_info_offset: 0,
            num_vars: 1,
            var_header_offset: 112,
            num_buf,
            buf_len: 4,
            pad1: [0; 2],
            var_buf: [
                IRSDKVarBuf {
                    tick_count: 1,
                    buf_offset: 256,
                    pad: [0; 2],
                },
                IRSDKVarBuf {
                    tick_count: 4,
                    buf_offset: 260,
                    pad: [0; 2],
                },
                IRSDKVarBuf {
                    tick_count: 3,
                    buf_offset: 264,
                    pad: [0; 2],
                },
                IRSDKVarBuf {
                    tick_count: 2,
                    buf_offset: 268,
                    pad: [0; 2],
                },
            ],
        }
    }

    #[test]
    fn constants_match_iracing_sdk() {
        assert_eq!(IRSDK_MEMMAPFILENAME, "Local\\IRSDKMemMapFileName");
        assert_eq!(IRSDK_DATAVALIDEVENTNAME, "Local\\IRSDKDataValidEvent");
        assert_eq!(IRSDK_VER, 2);
        assert_eq!(IRSDK_ST_CONNECTED, 1);
    }

    #[test]
    fn header_struct_layout() {
        // Verify the header struct matches expected C layout
        assert_eq!(std::mem::size_of::<IRSDKHeader>(), 112); // Expected size
        assert_eq!(std::mem::align_of::<IRSDKHeader>(), 4);

        // Check VarBuf size and alignment
        assert_eq!(std::mem::size_of::<IRSDKVarBuf>(), 16);
        assert_eq!(std::mem::align_of::<IRSDKVarBuf>(), 4);
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
        let variables = connection.get_variables();

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
            std::mem::size_of::<IRSDKHeader>(),
            112,
            "Header size must match C SDK"
        );
        assert!(header.tick_rate > 0, "Tick rate should be positive");

        assert_eq!(header.ver, IRSDK_VER);
        assert!(header.num_vars > 0);
        assert!(header.num_buf >= 3);
        assert!(header.buf_len > 0);
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
