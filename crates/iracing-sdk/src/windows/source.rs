use iracing_irsdk::constants::{IRSDK_DATAVALIDEVENTNAME, IRSDK_MEMMAPFILENAME};
use std::{ptr::NonNull, sync::Arc, time::Duration};
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

use crate::{IRacingSDKError, Result, windows::wide_string};

// Import the Windows implementation as an opaque external call. Unlike a
// Rust memcpy intrinsic, the compiler cannot fold this into ordinary Rust
// loads from memory which the simulator changes independently. The native
// routine performs the bulk copy; publication checks belong to LiveReader.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn RtlMoveMemory(
        destination: *mut std::ffi::c_void,
        source: *const std::ffi::c_void,
        length: usize,
    );
}

/// Result of waiting for data updates
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitResult {
    /// Wait resolved with data.
    Signaled,
    /// Wait time elapsed.
    Timeout,
}

#[derive(Debug)]
struct OwnedHandle(HANDLE);
// SAFETY: These mapping/event kernel handles have no thread affinity. The
// unique owner (or an Arc to it) keeps them open for every operation.
unsafe impl Send for OwnedHandle {}
unsafe impl Sync for OwnedHandle {}
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: This object uniquely owns a successfully opened handle.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

#[derive(Debug)]
struct MappedView(NonNull<u8>);
// SAFETY: The view is external memory accessed only through bounded volatile
// scalar reads and native copies. Ownership keeps it mapped throughout
// every synchronous read and copy.
unsafe impl Send for MappedView {}
unsafe impl Sync for MappedView {}
impl Drop for MappedView {
    fn drop(&mut self) {
        // SAFETY: This is the base returned by our successful MapViewOfFile.
        let _ = unsafe {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.0.as_ptr().cast(),
            })
        };
    }
}

fn wait_for_event(event: Arc<OwnedHandle>, timeout_ms: u32) -> Result<WaitResult> {
    // SAFETY: The closure owns an Arc, retaining the handle even if
    // its JoinHandle/future and the original source are dropped.
    match unsafe { WaitForSingleObject(event.0, timeout_ms) } {
        WAIT_OBJECT_0 => Ok(WaitResult::Signaled),
        WAIT_TIMEOUT => Ok(WaitResult::Timeout),
        _ => Err(IRacingSDKError::windows_api_error(
            "WaitForSingleObject",
            windows::core::Error::from_thread(),
        )),
    }
}

async fn wait_for_event_async(event: Arc<OwnedHandle>, timeout_ms: u32) -> Result<WaitResult> {
    tokio::task::spawn_blocking(move || wait_for_event(event, timeout_ms))
        .await
        .map_err(|e| {
            IRacingSDKError::buffer_operation_error(format!("Event wait task failed: {e}"), None)
        })?
}

#[derive(Debug)]
pub(crate) struct LiveSource {
    view: MappedView,
    _mapping: OwnedHandle,
    event: Arc<OwnedHandle>,
    len: usize,
}

impl LiveSource {
    #[cfg(test)]
    pub(crate) fn test_source(bytes: &[u8]) -> Self {
        use windows::Win32::{
            Foundation::INVALID_HANDLE_VALUE,
            System::{
                Memory::{CreateFileMappingW, FILE_MAP_WRITE, PAGE_READWRITE},
                Threading::CreateEventW,
            },
        };
        assert!(!bytes.is_empty());
        // SAFETY: Create a private, page-file-backed mapping and unnamed event.
        // The view is writable while initialized here, then only read by tests.
        unsafe {
            let mapping = OwnedHandle(
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    None,
                    PAGE_READWRITE,
                    0,
                    u32::try_from(bytes.len()).unwrap(),
                    PCWSTR::null(),
                )
                .unwrap(),
            );
            let raw = MapViewOfFile(mapping.0, FILE_MAP_WRITE, 0, 0, bytes.len());
            let view = MappedView(NonNull::new(raw.Value.cast()).unwrap());
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), view.0.as_ptr(), bytes.len());
            Self {
                view,
                _mapping: mapping,
                event: Arc::new(OwnedHandle(
                    CreateEventW(None, true, false, PCWSTR::null()).unwrap(),
                )),
                len: bytes.len(),
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn test_write(&self, offset: usize, bytes: &[u8]) {
        assert!(offset.checked_add(bytes.len()).unwrap() <= self.len);
        // SAFETY: Only private writable test mappings call this synchronous
        // helper; the checked destination fits and the input is disjoint.
        unsafe {
            RtlMoveMemory(
                self.view.0.as_ptr().add(offset).cast(),
                bytes.as_ptr().cast(),
                bytes.len(),
            );
        }
    }

    pub fn try_connect() -> Result<Self> {
        let name = wide_string(IRSDK_MEMMAPFILENAME);
        // SAFETY: name is a live NUL-terminated UTF-16 string.
        let mapping = OwnedHandle(
            unsafe { OpenFileMappingW(FILE_MAP_READ.0, false, PCWSTR(name.as_ptr())) }
                .map_err(|e| IRacingSDKError::windows_api_error("OpenFileMappingW", e))?,
        );
        // SAFETY: mapping is owned and open; request a read-only full view.
        let raw = unsafe { MapViewOfFile(mapping.0, FILE_MAP_READ, 0, 0, 0) };
        let view = MappedView(NonNull::new(raw.Value.cast()).ok_or_else(|| {
            IRacingSDKError::windows_api_error("MapViewOfFile", windows::core::Error::from_thread())
        })?);
        let mut info = MEMORY_BASIC_INFORMATION::default();
        // SAFETY: view is mapped and info is writable for its exact size.
        if unsafe {
            VirtualQuery(
                Some(view.0.as_ptr().cast()),
                &mut info,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        } == 0
        {
            return Err(IRacingSDKError::windows_api_error(
                "VirtualQuery",
                windows::core::Error::from_thread(),
            ));
        }
        // A page-file-backed SDK view is one committed region. Conservatively
        // bound reads to this queried region even if a larger view exists.
        let len = info.RegionSize;
        if info.BaseAddress != view.0.as_ptr().cast() || len > isize::MAX as usize {
            return Err(IRacingSDKError::parse_error(
                "LiveSource",
                "Invalid mapped view extent",
            ));
        }
        let name = wide_string(IRSDK_DATAVALIDEVENTNAME);
        // SAFETY: name is NUL-terminated; only SYNCHRONIZE access is needed.
        let event = OwnedHandle(
            unsafe {
                OpenEventW(
                    SYNCHRONIZATION_ACCESS_RIGHTS(0x0010_0000),
                    false,
                    PCWSTR(name.as_ptr()),
                )
            }
            .map_err(|e| IRacingSDKError::windows_api_error("OpenEventW", e))?,
        );

        Ok(Self {
            view,
            _mapping: mapping,
            event: Arc::new(event),
            len,
        })
    }

    unsafe fn copy_raw_unchecked(&self, offset: usize, destination: *mut u8, len: usize) {
        if len == 0 {
            return;
        }

        // SAFETY:
        // - Caller guarantees `offset + len` fits within the retained mapping.
        // - Caller guarantees `destination` is valid for writes of `len` bytes.
        // - Caller guarantees the destination does not overlap the mapped source.
        // - `self.view` remains mapped for the duration of this synchronous copy.
        //
        // RtlMoveMemory performs the copy without constructing Rust references
        // into memory that may be concurrently modified by iRacing.
        unsafe {
            RtlMoveMemory(
                destination.cast(),
                self.view.0.as_ptr().add(offset).cast(),
                len,
            );
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn wait_for_update(&self, timeout: Duration) -> Result<WaitResult> {
        // INFINITE is u32::MAX; keep even enormous requested waits bounded.
        let timeout_ms = timeout.as_millis().min(u128::from(u32::MAX - 1)) as u32;
        wait_for_event(Arc::clone(&self.event), timeout_ms)
    }

    pub async fn wait_for_update_async(&self, timeout: Duration) -> Result<WaitResult> {
        // INFINITE is u32::MAX; keep even enormous requested waits bounded.
        let timeout_ms = timeout.as_millis().min(u128::from(u32::MAX - 1)) as u32;
        wait_for_event_async(Arc::clone(&self.event), timeout_ms).await
    }

    pub(crate) fn read_i32(&self, offset: usize) -> Option<i32> {
        if !offset.is_multiple_of(align_of::<i32>())
            || offset.checked_add(size_of::<i32>())? > self.len()
        {
            return None;
        }

        // SAFETY: The mapped base is page-aligned. The checks above establish
        // alignment and bounds; self.source retains the initialized mapping.
        unsafe { self.read_i32_unchecked(offset).ok() }
    }

    /// Read a synchronization word using geometry validated at activation.
    ///
    /// # Safety
    /// `offset` must be aligned for i32 and its four bytes must fit in this
    /// source, which must retain its mapping throughout the read.
    pub(crate) unsafe fn read_i32_unchecked(&self, offset: usize) -> Result<i32> {
        let bytes = unsafe {
            self.view
                .0
                .as_ptr()
                .add(offset)
                .cast::<i32>()
                .read_volatile()
        };

        // SAFETY: The caller establishes bounds and alignment. The
        // external mapping is initialized and all i32 bits are valid.
        Ok(i32::from_le(bytes))
    }

    pub(crate) fn read_u8(&self, offset: usize) -> Option<u8> {
        if !offset.is_multiple_of(align_of::<u8>())
            || offset.checked_add(size_of::<u8>())? > self.len()
        {
            return None;
        }

        unsafe { self.read_u8_unchecked(offset).ok() }
    }

    /// # Safety
    /// `offset` must identify a byte within this source's retained mapping.
    pub(crate) unsafe fn read_u8_unchecked(&self, offset: usize) -> Result<u8> {
        // SAFETY: The caller establishes bounds. u8 has no alignment or
        // representation restrictions; the mapping remains owned.
        Ok(unsafe { self.view.0.as_ptr().add(offset).read_volatile() })
    }

    /// Copy bytes from an offset validated at activation, without building
    /// a range or repeating geometry checks.
    ///
    /// # Safety
    /// `offset + destination.len()` must not overflow and must fit within
    /// this retained source. The destination must be disjoint from it.
    /// Publication checks are required before accepting a possibly torn copy.
    pub(crate) unsafe fn copy_unchecked(
        &self,
        offset: usize,
        destination: &mut [u8],
    ) -> Result<()> {
        // SAFETY: The caller establishes that the source range fits within the
        // retained mapping. `destination` is valid for its own length and cannot
        // overlap the externally owned mapped view.
        unsafe {
            self.copy_raw_unchecked(offset, destination.as_mut_ptr(), destination.len());
        }

        Ok(())
    }

    pub(crate) unsafe fn copy_value_unchecked<T>(&self, offset: usize) -> Result<T>
    where
        T: zerocopy::FromBytes,
    {
        let mut value = std::mem::MaybeUninit::<T>::uninit();

        // SAFETY:
        // - Caller guarantees the source range for `T` fits within the mapping.
        // - `value` provides writable storage for exactly `size_of::<T>()` bytes.
        // - The destination is stack/local storage and cannot overlap the mapping.
        unsafe {
            self.copy_raw_unchecked(offset, value.as_mut_ptr().cast(), size_of::<T>());
        }

        // SAFETY: `copy_raw_unchecked` initialized every byte of `value`.
        // `T: FromBytes` guarantees that every initialized byte pattern is a
        // valid instance of `T`.
        Ok(unsafe { value.assume_init() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Threading::SetEvent;

    #[tokio::test]
    async fn canceled_async_wait_retains_event_until_worker_finishes() {
        let source = LiveSource::test_source(&[0; 112]);
        let weak = Arc::downgrade(&source.event);
        let mut wait = Box::pin(source.wait_for_update_async(Duration::from_secs(5)));
        // Poll once to submit the blocking worker, then cancel its awaiting future.
        assert!(matches!(
            futures::poll!(wait.as_mut()),
            std::task::Poll::Pending
        ));
        drop(wait);
        drop(source);
        let event = weak.upgrade().expect("worker must retain the event");
        // SAFETY: The upgraded owner keeps the private event open.
        unsafe { SetEvent(event.0).unwrap() };
        drop(event);
        tokio::time::timeout(Duration::from_secs(5), async {
            while weak.upgrade().is_some() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("worker must release the event after signaling");
    }

    #[test]
    fn private_event_wait_times_out_and_signals() {
        let source = LiveSource::test_source(&[0; 112]);
        assert_eq!(
            source.wait_for_update(Duration::ZERO).unwrap(),
            WaitResult::Timeout
        );
        // SAFETY: source owns this private event.
        unsafe { SetEvent(source.event.0).unwrap() };
        assert_eq!(
            source.wait_for_update(Duration::ZERO).unwrap(),
            WaitResult::Signaled
        );
    }
}
