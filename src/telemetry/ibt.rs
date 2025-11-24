use std::ffi::OsStr;
use std::io::{Error, ErrorKind, Result as IOResult};
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

use crate::telemetry::data::header::{DiskSubHeader, Header, Sample};
use crate::telemetry::session::SessionDetails;

pub struct IBT {
    pub header: Header,
    pub sub_header: DiskSubHeader,
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

        let header: Header = unsafe { Header::parse(view) };
        let sub_header: DiskSubHeader = unsafe { DiskSubHeader::parse(view) };

        Ok(IBT {
            location: view,
            header,
            sub_header,
        })
    }

    pub fn session_info(&self) -> Result<SessionDetails, Box<dyn std::error::Error>> {
        self.header.session_info(self.location as usize)
    }

    ///
    /// Gets an individual telemetry frame at the given index.
    pub fn telemetry_sample(&self, index: u32) -> Result<Sample, Box<dyn std::error::Error>> {
        if self.sub_header.session_record_count <= 0 {
            return Err(Box::new(Error::new(
                ErrorKind::InvalidData,
                "IBT file reports no telemetry records",
            )));
        }

        let record_count = self.sub_header.session_record_count as u32;

        if index >= record_count {
            return Err(Box::new(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "Requested telemetry sample {} is out of bounds for {} records",
                    index, record_count
                ),
            )));
        }

        self.header.telemetry_sample(self.location)
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

    const TELEMETRY_PATH: &str = "./telemetry.ibt";

    #[test]
    fn test_ibt() {
        let ibt = IBT::open(TELEMETRY_PATH);
        assert!(ibt.is_ok());
    }

    #[test]
    fn test_session_info() {
        let ibt = IBT::open(TELEMETRY_PATH).expect("Could not open IBT");
        let session_info = ibt.session_info();
        assert!(session_info.is_ok());
    }
}
