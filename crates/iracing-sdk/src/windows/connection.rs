//! Windows ownership of one iRacing shared-memory mapping generation.
//!
//! The operating-system handles and mapping lifetime live here. Parsing,
//! static layout validation, ordered control reads, and coherent acquisition
//! are delegated to [`LiveReader`]. A newly opened or replaced mapping always
//! constructs a new reader; reconnecting never reuses layout state from an old
//! generation.

use std::{
    mem::{MaybeUninit, size_of},
    ptr::NonNull,
    time::Duration,
};

use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{
            Memory::{
                FILE_MAP_READ, MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
                OpenFileMappingW, UnmapViewOfFile, VirtualQuery,
            },
            Threading::{OpenEventW, SYNCHRONIZATION_ACCESS_RIGHTS, WaitForSingleObject},
        },
    },
    core::PCWSTR,
};

use super::utils::wide_string;
use crate::{
    IRacingSDKError, Result,
    irsdk::{Header, StatusField, VariableHeadersBuffer},
    reader::live::{LiveFrameSnapshot, LiveReader, LiveSessionSnapshot, MappedView},
    types::irsdk::constants::{IRSDK_DATAVALIDEVENTNAME, IRSDK_MEMMAPFILENAME},
};

/// Result of waiting for data updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitResult {
    /// Wait resolved with data.
    Signaled,
    /// Wait time elapsed.
    Timeout,
}

/// Direct connection to one generation of iRacing shared memory.
#[derive(Debug)]
pub struct Connection {
    mapping: HANDLE,
    base: NonNull<u8>,
    mapped_length: usize,
    event: HANDLE,
    reader: LiveReader,
}

impl Connection {
    /// Attempts to open and validate the current iRacing mapping generation.
    pub fn try_connect() -> Result<Self> {
        tracing::trace!("Attempting to connect to iRacing shared memory");

        let mapping = unsafe {
            let wide_name = wide_string(IRSDK_MEMMAPFILENAME);
            OpenFileMappingW(FILE_MAP_READ.0, false, PCWSTR::from_raw(wide_name.as_ptr()))
                .map_err(|error| IRacingSDKError::windows_api_error("OpenFileMappingW", error))?
        };

        let base = unsafe {
            let address = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
            match NonNull::new(address.Value.cast::<u8>()) {
                Some(base) => base,
                None => {
                    let error = IRacingSDKError::windows_api_error(
                        "MapViewOfFile",
                        windows::core::Error::from_thread(),
                    );
                    let _ = CloseHandle(mapping);
                    return Err(error);
                }
            }
        };

        let mapped_length = match Self::query_mapped_length(base) {
            Ok(length) => length,
            Err(error) => {
                unsafe { Self::release_mapping(mapping, base) };
                return Err(error);
            }
        };
        // SAFETY: `base` is the view returned above and remains mapped until
        // `Connection::drop`; this temporary view cannot outlive this scope.
        let view = unsafe { MappedView::from_raw_parts(base, mapped_length) };
        let reader = match LiveReader::new(&view) {
            Ok(reader) => reader,
            Err(error) => {
                unsafe { Self::release_mapping(mapping, base) };
                return Err(error);
            }
        };

        let event = unsafe {
            let wide_name = wide_string(IRSDK_DATAVALIDEVENTNAME);
            match OpenEventW(
                SYNCHRONIZATION_ACCESS_RIGHTS(0x0010_0000),
                false,
                PCWSTR::from_raw(wide_name.as_ptr()),
            ) {
                Ok(event) => event,
                Err(error) => {
                    Self::release_mapping(mapping, base);
                    return Err(IRacingSDKError::windows_api_error("OpenEventW", error));
                }
            }
        };

        tracing::debug!(
            tick_rate = reader.tick_rate(),
            buffer_count = reader.buffer_count(),
            frame_length = reader.frame_length(),
            "Validated iRacing mapping generation"
        );

        Ok(Self {
            mapping,
            base,
            mapped_length,
            event,
            reader,
        })
    }

    /// Returns a current header snapshot without revalidating static layout.
    pub fn header_snapshot(&self) -> Result<Header> {
        let view = self.mapped_view();
        self.reader.header_snapshot(&view)
    }

    /// Waits synchronously for a telemetry update event.
    pub fn wait_for_update(&self, timeout: Duration) -> Result<WaitResult> {
        let milliseconds = timeout.as_millis().min(u32::MAX as u128) as u32;
        Self::wait_for_event(self.event, milliseconds)
    }

    /// Waits asynchronously for a telemetry update without blocking a worker.
    pub async fn wait_for_update_async(&self, timeout: Duration) -> Result<WaitResult> {
        let event_raw = self.event.0 as usize;
        let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as u32;

        tokio::task::spawn_blocking(move || {
            tracing::trace!(timeout_ms, "Async waiting for Windows event");

            // SAFETY: `event_raw` came from the live Connection's kernel event
            // handle, which remains open while this awaited task is running.
            let event = HANDLE(event_raw as *mut std::ffi::c_void);
            Self::wait_for_event(event, timeout_ms)
        })
        .await
        .map_err(|error| {
            IRacingSDKError::buffer_operation_error(
                format!("Event wait task panicked: {error}"),
                None,
            )
        })?
    }

    /// Acquires the next coherent owned frame snapshot, if one is available.
    pub fn next_frame(&mut self) -> Result<Option<LiveFrameSnapshot>> {
        let base = self.base;
        let mapped_length = self.mapped_length;
        // SAFETY: The Connection owns this mapping and the temporary view is
        // dropped before the mapping can be unmapped.
        let view = unsafe { MappedView::from_raw_parts(base, mapped_length) };
        self.reader.next_frame(&view)
    }

    /// Acquires one stable owned session-information snapshot.
    pub fn session_info_snapshot(&self) -> Result<Option<LiveSessionSnapshot>> {
        let view = self.mapped_view();
        self.reader.session_info(&view)
    }

    /// Copies the variable headers validated for this mapping generation.
    pub fn variable_headers_buffer(&self) -> Result<Option<VariableHeadersBuffer>> {
        let view = self.mapped_view();
        self.reader.variable_headers_buffer(&view)
    }

    /// Returns the validated source tick rate.
    pub fn tick_rate(&self) -> i32 {
        self.reader.tick_rate()
    }

    /// Returns the validated telemetry frame length.
    pub fn frame_length(&self) -> usize {
        self.reader.frame_length()
    }

    /// Reads the current session-information update counter.
    pub fn session_info_update(&self) -> i32 {
        let view = self.mapped_view();
        self.reader
            .session_info_update(&view)
            .expect("connection retains its validated mapping generation")
    }

    /// Reads the cached tick count for the current buffer.
    pub fn current_buffer_tick_count(&self) -> i32 {
        let view = self.mapped_view();
        self.reader
            .current_buffer_tick_count(&view)
            .expect("connection retains its validated mapping generation")
    }

    /// Reads the current buffer index.
    pub fn current_buffer_index(&self) -> u8 {
        let view = self.mapped_view();
        u8::try_from(
            self.reader
                .current_buffer_index(&view)
                .expect("connection retains its validated mapping generation"),
        )
        .expect("SDK buffer index fits in u8")
    }

    /// Returns the validated buffer count.
    pub fn buffer_count(&self) -> usize {
        self.reader.buffer_count()
    }

    /// Reads the current connection status.
    pub fn connection_status(&self) -> StatusField {
        let view = self.mapped_view();
        self.reader
            .connection_status(&view)
            .expect("connection retains its validated mapping generation")
    }

    /// Returns whether iRacing currently advertises an active connection.
    pub fn is_connected(&self) -> bool {
        self.connection_status().is_connected()
    }

    fn mapped_view(&self) -> MappedView<'_> {
        // SAFETY: The returned view borrows this Connection and therefore cannot
        // outlive the mapping that Connection::drop unmaps.
        unsafe { MappedView::from_raw_parts(self.base, self.mapped_length) }
    }

    fn wait_for_event(event: HANDLE, timeout_ms: u32) -> Result<WaitResult> {
        tracing::trace!(timeout_ms, "Waiting for telemetry update");
        let result = unsafe { WaitForSingleObject(event, timeout_ms) };

        match result {
            WAIT_OBJECT_0 => Ok(WaitResult::Signaled),
            WAIT_TIMEOUT => Ok(WaitResult::Timeout),
            _ => Err(IRacingSDKError::windows_api_error(
                "WaitForSingleObject",
                windows::core::Error::from_thread(),
            )),
        }
    }

    fn query_mapped_length(base: NonNull<u8>) -> Result<usize> {
        let mut information = MaybeUninit::<MEMORY_BASIC_INFORMATION>::uninit();
        let bytes_written = unsafe {
            VirtualQuery(
                Some(base.as_ptr().cast()),
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

        // SAFETY: VirtualQuery reported that it initialized the complete value.
        Ok(unsafe { information.assume_init() }.RegionSize)
    }

    unsafe fn release_mapping(mapping: HANDLE, base: NonNull<u8>) {
        let address = MEMORY_MAPPED_VIEW_ADDRESS {
            Value: base.as_ptr().cast(),
        };
        unsafe {
            let _ = UnmapViewOfFile(address);
            let _ = CloseHandle(mapping);
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            let address = MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.base.as_ptr().cast(),
            };
            let _ = UnmapViewOfFile(address);
            let _ = CloseHandle(self.mapping);
            let _ = CloseHandle(self.event);
        }
    }
}

// SAFETY: The connection owns stable Windows handles and a read-only mapping.
// Mutating acquisition state requires `&mut self`; shared session reads protect
// their diagnostics with a mutex.
unsafe impl Send for Connection {}
unsafe impl Sync for Connection {}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::{VarData, VariableHeader, VariableSchema};

    #[test]
    #[ignore = "iracing_required"]
    fn test_read_rpm_variable() {
        let connection = Connection::try_connect().expect("Failed to connect to iRacing");
        let buffer = connection
            .variable_headers_buffer()
            .expect("Failed to read VariableInfo from connection")
            .expect("Could not get VariableInfo from connection");

        let headers: Vec<VariableHeader> = buffer.into();
        assert!(!headers.is_empty(), "Buffer should have some variables");

        let variables = VariableSchema::from_connection(&connection)
            .expect("Could not get variable schema")
            .variables();
        assert!(variables.iter().any(|variable| variable.name == "RPM"));
    }

    #[test]
    #[ignore = "iracing_required"]
    fn connects_to_live_iracing() {
        let connection = Connection::try_connect().expect("Failed to connect to iRacing");
        let header = connection
            .header_snapshot()
            .expect("Failed to copy live header");

        assert_eq!(std::mem::size_of::<Header>(), 112);
        assert!(header.tick_rate > 0);
        assert_eq!(header.version, crate::IRSDK_VERSION);
        assert!(header.variable_count > 0);
        assert!(header.buffer_count >= 3);
    }

    #[test]
    #[ignore = "iracing_required"]
    fn waits_for_data_updates() {
        let mut connection = Connection::try_connect().expect("Failed to connect to iRacing");
        let _ = connection.next_frame();
        connection
            .wait_for_update(Duration::from_millis(100))
            .expect("Failed to wait for update");
    }

    #[test]
    #[ignore = "iracing_required"]
    fn accepted_tick_correlates_with_decoded_session_tick() -> Result<()> {
        let mut connection = Connection::try_connect()?;
        let schema = VariableSchema::from_connection(&connection)?;
        let session_tick = schema
            .get_variable("SessionTick")
            .expect("live schema should contain SessionTick")
            .clone();

        for _ in 0..120 {
            if let Some(snapshot) = connection.next_frame()? {
                let accepted_tick = snapshot.tick_count();
                let bytes: Vec<u8> = snapshot.into_buffer().into();
                let decoded_tick = i32::from_bytes(&bytes, &session_tick)?;
                assert_eq!(decoded_tick, accepted_tick);
                return Ok(());
            }
            connection.wait_for_update(Duration::from_millis(100))?;
        }

        Err(IRacingSDKError::parse_error(
            "live correlation test",
            "No accepted frame arrived within 12 seconds",
        ))
    }
}
