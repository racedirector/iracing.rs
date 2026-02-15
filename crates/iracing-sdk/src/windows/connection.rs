//! iRacing shared memory connection aligned with C++ SDK
//!
//! This module provides direct memory mapping to iRacing's shared memory
//! following the same patterns as the official C++ SDK implementation.

use crate::{Result, TelemetryError};
use std::ptr::NonNull;
use std::time::Duration;
use tracing::{debug, trace, warn};
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
const IRSDK_VER: i32 = 2;
/// Connection status flag
const IRSDK_ST_CONNECTED: i32 = 1;
/// Maximum number of telemetry buffers
const IRSDK_MAX_BUFS: usize = 4;

/// Variable buffer containing tick count and offset information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VarBuf {
    pub tick_count: i32, // Used to detect changes in data
    pub buf_offset: i32, // Offset from header
    pub pad: [i32; 2],   // 16-byte alignment
}

/// Variable header structure from iRacing SDK
#[repr(C)]
#[derive(Debug)]
pub struct IRSDKVarHeader {
    pub var_type: i32,                    // Variable type (irsdk_VarType)
    pub offset: i32,                      // Offset from start of buffer row
    pub count: i32,                       // Number of entries (array)
    pub count_as_time: bool,              // Values in array represent timeseries data
    pub pad: [u8; 3],                     // 16-byte alignment padding
    pub name: [std::os::raw::c_char; 32], // Variable name
    pub desc: [std::os::raw::c_char; 64], // Variable description
    pub unit: [std::os::raw::c_char; 32], // Variable units
}

impl IRSDKVarHeader {
    /// Get variable name as String
    pub fn name(&self) -> String {
        unsafe {
            let cstr = std::ffi::CStr::from_ptr(self.name.as_ptr());
            cstr.to_string_lossy().into_owned()
        }
    }

    /// Get variable description as String
    pub fn description(&self) -> String {
        unsafe {
            let cstr = std::ffi::CStr::from_ptr(self.desc.as_ptr());
            cstr.to_string_lossy().into_owned()
        }
    }

    /// Get variable unit as String
    pub fn unit(&self) -> String {
        unsafe {
            let cstr = std::ffi::CStr::from_ptr(self.unit.as_ptr());
            cstr.to_string_lossy().into_owned()
        }
    }

    /// Convert iRacing variable type to our VariableType
    pub fn data_type(&self) -> crate::VariableType {
        match self.var_type {
            0 => crate::VariableType::Char,
            1 => crate::VariableType::Bool,
            2 => crate::VariableType::Int32,
            3 => crate::VariableType::BitField,
            4 => crate::VariableType::Float32,
            5 => crate::VariableType::Float64,
            _ => crate::VariableType::Int32, // Default fallback
        }
    }
}

/// Main iRacing header structure matching C++ SDK exactly
#[repr(C)]
#[derive(Debug)]
pub struct IRSDKHeader {
    pub ver: i32,       // API header version (should be IRSDK_VER)
    pub status: i32,    // Bitfield using status flags
    pub tick_rate: i32, // Ticks per second (60 or 360 etc)

    // Session information, updated periodically
    pub session_info_update: i32, // Incremented when session info changes
    pub session_info_len: i32,    // Length in bytes of session info string
    pub session_info_offset: i32, // Session info, encoded in YAML format

    // State data, output at tick_rate
    pub num_vars: i32,          // Length of array pointed to by var_header_offset
    pub var_header_offset: i32, // Offset to variable header array

    pub num_buf: i32,                      // Number of buffers (<= IRSDK_MAX_BUFS)
    pub buf_len: i32,                      // Length in bytes for one line
    pub pad1: [i32; 2],                    // 16-byte alignment
    pub var_buf: [VarBuf; IRSDK_MAX_BUFS], // Buffers of data being written to
}

/// Result of waiting for data updates
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitResult {
    Signaled,
    Timeout,
}

/// Direct connection to iRacing shared memory
pub struct Connection {
    mapping: HANDLE,
    base: NonNull<u8>,
    event: HANDLE,
    last_tick_count: i32,
}

impl Connection {
    /// Attempt to connect to iRacing shared memory
    pub fn try_connect() -> Result<Self> {
        trace!("Attempting to connect to iRacing shared memory");

        // Open the memory mapping
        let mapping = unsafe {
            let wide_name = wide_string(IRSDK_MEMMAPFILENAME);
            OpenFileMappingW(FILE_MAP_READ.0, false, PCWSTR::from_raw(wide_name.as_ptr()))
                .map_err(|e| TelemetryError::windows_api_error("OpenFileMappingW", e))?
        };

        // Map the view
        let base = unsafe {
            let ptr = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
            NonNull::new(ptr.Value as *mut u8).ok_or_else(|| {
                let win_err = windows::core::Error::from_thread();
                TelemetryError::windows_api_error("MapViewOfFile", win_err)
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
            .map_err(|e| TelemetryError::windows_api_error("OpenEventW", e))?
        };

        // Initialize with i32::MAX to match C++ SDK's INT_MAX
        // This ensures the first frame is always accepted as "new"
        let connection = Self { mapping, base, event, last_tick_count: i32::MAX };

        // Validate the connection
        connection.validate_connection()?;

        debug!("Initialized last_tick_count to i32::MAX for first frame acceptance");

        debug!("Successfully connected to iRacing shared memory");
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
        trace!(timeout_ms = ms, "Waiting for telemetry update");

        let result = unsafe { WaitForSingleObject(self.event, ms) };

        match result {
            WAIT_OBJECT_0 => {
                debug!("Telemetry update signaled");
                Ok(WaitResult::Signaled)
            }
            WAIT_TIMEOUT => {
                trace!("Wait timed out");
                Ok(WaitResult::Timeout)
            }
            _ => {
                let win_err = windows::core::Error::from_thread();
                Err(TelemetryError::windows_api_error("WaitForSingleObject", win_err))
            }
        }
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
            trace!(timeout_ms, "Async waiting for Windows event");

            // Reconstruct HANDLE from raw pointer value
            // SAFETY: event_raw came from a valid HANDLE, kernel object is still alive
            let event = HANDLE(event_raw as *mut std::ffi::c_void);
            let result = unsafe { WaitForSingleObject(event, timeout_ms) };

            match result {
                WAIT_OBJECT_0 => {
                    trace!("Event signaled");
                    Ok(WaitResult::Signaled)
                }
                WAIT_TIMEOUT => {
                    trace!("Event wait timed out");
                    Ok(WaitResult::Timeout)
                }
                _ => {
                    let win_err = windows::core::Error::from_thread();
                    Err(TelemetryError::windows_api_error("WaitForSingleObject", win_err))
                }
            }
        })
        .await
        .map_err(|e| {
            TelemetryError::buffer_operation_error(format!("Event wait task panicked: {}", e), None)
        })?
    }

    /// Get latest telemetry data if available
    pub fn get_new_data(&mut self) -> Option<&[u8]> {
        if !self.is_connected() {
            debug!("Not connected to iRacing");
            self.last_tick_count = i32::MAX;
            return None;
        }

        let header = self.header();

        // Find the buffer with the highest tick count (most recent)
        let latest_buf_idx = self.find_latest_buffer(header);
        let latest_buf = &header.var_buf[latest_buf_idx];

        debug!(
            "Checking for new data: last_tick={}, latest_tick={}, buffer_idx={}",
            self.last_tick_count, latest_buf.tick_count, latest_buf_idx
        );

        // Check if we have new data
        if self.last_tick_count == latest_buf.tick_count {
            trace!("No new data (same tick count)");
            return None;
        }

        // Handle potential tick count reset or wraparound
        if self.last_tick_count > latest_buf.tick_count && self.last_tick_count != i32::MAX {
            debug!(
                "Tick count reset detected: {} -> {}",
                self.last_tick_count, latest_buf.tick_count
            );
        }

        // Double-read pattern to ensure data consistency
        for attempt in 0..2 {
            let tick_before = latest_buf.tick_count;
            let data_ptr = unsafe { self.base.as_ptr().add(latest_buf.buf_offset as usize) };
            let data_slice =
                unsafe { std::slice::from_raw_parts(data_ptr, header.buf_len as usize) };
            let tick_after = latest_buf.tick_count;

            if tick_before == tick_after {
                self.last_tick_count = tick_before;
                debug!("Returning new data: tick={}, size={} bytes", tick_before, data_slice.len());
                return Some(data_slice);
            } else {
                debug!(
                    "Data consistency check failed on attempt {}: before={}, after={}",
                    attempt + 1,
                    tick_before,
                    tick_after
                );
            }
        }

        warn!("Failed consistency checks, no data returned");
        None
    }

    /// Get session info YAML string
    pub fn session_info(&self) -> Option<&str> {
        let header = self.header();
        if header.session_info_len <= 0 {
            return None;
        }

        unsafe {
            let info_ptr = self.base.as_ptr().add(header.session_info_offset as usize);
            let info_slice = std::slice::from_raw_parts(info_ptr, header.session_info_len as usize);

            // Find null terminator - iRacing YAML is null-terminated
            let null_pos = info_slice.iter().position(|&b| b == 0).unwrap_or(info_slice.len());
            let yaml_bytes = &info_slice[..null_pos];

            std::str::from_utf8(yaml_bytes).ok()
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
                let var_info = crate::VariableInfo {
                    name: var_header.name(),
                    description: var_header.description(),
                    units: var_header.unit(),
                    data_type: var_header.data_type(),
                    offset: var_header.offset as usize,
                    count: var_header.count as usize,
                    count_as_time: var_header.count_as_time,
                };

                variables.push(var_info);
            }
        }

        variables
    }

    /// Validate initial connection
    fn validate_connection(&self) -> Result<()> {
        let header = self.header();

        // Check SDK version
        if header.ver != IRSDK_VER {
            return Err(TelemetryError::Version {
                expected: IRSDK_VER as u32,
                found: header.ver as u32,
            });
        }

        debug!(
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
        for i in 1..(header.num_buf as usize) {
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
            let addr = MEMORY_MAPPED_VIEW_ADDRESS { Value: self.base.as_ptr() as *mut _ };
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

/// Convert string to null-terminated wide string for Windows APIs
fn wide_string(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

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
        assert_eq!(std::mem::size_of::<VarBuf>(), 16);
        assert_eq!(std::mem::align_of::<VarBuf>(), 4);
    }

    #[test]
    #[ignore = "iracing_required"]
    fn test_read_rpm_variable() {
        let connection = Connection::try_connect().expect("Failed to connect to iRacing");
        let variables = connection.get_variables();

        // Look for exact "RPM" match to verify variable schema
        let exact_rpm = variables.iter().find(|v| v.name == "RPM");
        assert!(exact_rpm.is_some(), "RPM variable should be available in iRacing");

        assert!(!variables.is_empty(), "Should have some variables");
    }

    #[test]
    #[ignore = "iracing_required"]
    fn connects_to_live_iracing() {
        let connection = Connection::try_connect().expect("Failed to connect to iRacing");
        let header = connection.header();

        // Validate header structure sizes match expected C SDK layout
        assert_eq!(std::mem::size_of::<IRSDKHeader>(), 112, "Header size must match C SDK");
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
