use std::ffi::OsStr;
use std::io::Result as IOResult;
use std::os::raw::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::raw::HANDLE;
use std::ptr::null_mut;
use winapi::shared::minwindef::LPVOID;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::memoryapi::{CreateFileMappingW, MapViewOfFile, FILE_MAP_READ};
use winapi::um::winnt::{FILE_SHARE_READ, GENERIC_READ, PAGE_READONLY};

use crate::telemetry::header::{DiskSubHeader, Header};

pub struct IBT {
    location: *mut c_void,
}

impl IBT {
    pub fn open(path_string: &str) -> IOResult<IBT> {
        let mapping: HANDLE;
        let errno: i32;
        let path: Vec<u16> = OsStr::new(path_string)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let handle: HANDLE = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ,
                null_mut(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }

        mapping =
            unsafe { CreateFileMappingW(handle, null_mut(), PAGE_READONLY, 0, 0, null_mut()) };

        if mapping.is_null() {
            unsafe {
                errno = GetLastError() as i32;
            }

            return Err(std::io::Error::from_raw_os_error(errno));
        }

        unsafe { CloseHandle(handle) };

        let view: LPVOID;
        unsafe {
            view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
        }

        unsafe { CloseHandle(mapping) };

        if view.is_null() {
            unsafe {
                errno = GetLastError() as i32;
            }

            return Err(std::io::Error::from_raw_os_error(errno));
        }

        Ok(IBT { location: view })
    }

    pub fn header(&self) -> Result<Header, Box<dyn std::error::Error>> {
        unsafe { Ok(Header::parse(self.location)) }
    }

    pub fn sub_header(&self) -> Result<DiskSubHeader, Box<dyn std::error::Error>> {
        unsafe { Ok(DiskSubHeader::parse(self.location)) }
    }

    pub fn close(&self) -> IOResult<()> {
        if unsafe { CloseHandle(self.location) } != 0 {
            Ok(())
        } else {
            let errno: i32 = unsafe { GetLastError() as i32 };
            Err(std::io::Error::from_raw_os_error(errno))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ibt() {
        let ibt = IBT::open("./telemetry.ibt");
        assert!(ibt.is_ok());
    }
}
