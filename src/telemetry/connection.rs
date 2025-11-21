use encoding_rs::mem::decode_latin1;
use serde_yaml::from_str as yaml_from;
use std::convert::TryInto;
use std::error::Error;
use std::fmt::{self, Display};
use std::io::Result as IOResult;
use std::os::raw::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::raw::HANDLE;
use std::time::Duration;
use std::{ffi::OsStr, slice::from_raw_parts};
use winapi::shared::minwindef::LPVOID;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::handleapi::CloseHandle;
use winapi::um::memoryapi::{MapViewOfFile, OpenFileMappingW, FILE_MAP_READ};
use winapi::um::minwinbase::LPSECURITY_ATTRIBUTES;
use winapi::um::synchapi::{CreateEventW, ResetEvent, WaitForSingleObject};

use crate::fps::Fps;
use crate::telemetry::{
    header::{Header, Sample},
    session::SessionDetails,
};

/// System path where the shared memory map is located.
pub const TELEMETRY_PATH: &str = r"Local\IRSDKMemMapFileName";

const DATA_EVENT_NAME: &str = r"Local\IRSDKDataValidEvent";

///
/// Telemetry Error
///
/// An error which occurs when telemetry samples cannot be read from the memory buffer.
#[derive(Debug)]
pub enum TelemetryError {
    ABANDONED,
    TIMEOUT(usize),
    UNKNOWN(u32),
}

impl Display for TelemetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ABANDONED => write!(f, "Abandoned"),
            Self::TIMEOUT(ms) => write!(f, "Timeout after {}ms", ms),
            Self::UNKNOWN(v) => write!(f, "Unknown error code = {:x?}", v),
        }
    }
}

impl Error for TelemetryError {}

/// Blocking telemetry interface
///
/// Calling `sample()` on a Blocking interface will block until a new telemetry sample is made available.
///
pub struct Blocking {
    origin: *const c_void,
    header: Header,
    event_handle: HANDLE,
}

impl Blocking {
    pub fn new(location: *const c_void, head: Header) -> std::io::Result<Self> {
        let mut event_name: Vec<u16> = DATA_EVENT_NAME.encode_utf16().collect();
        event_name.push(0);

        let sc: LPSECURITY_ATTRIBUTES = unsafe { std::mem::zeroed() };

        let handle: HANDLE = unsafe { CreateEventW(sc, 0, 0, event_name.as_ptr()) };

        if handle.is_null() {
            let errno: i32 = unsafe { GetLastError() as i32 };

            return Err(std::io::Error::from_raw_os_error(errno));
        }

        Ok(Blocking {
            origin: location,
            header: head,
            event_handle: handle,
        })
    }

    pub fn is_connected(&self) -> bool {
        self.header.is_connected()
    }

    pub fn close(&self) -> std::io::Result<()> {
        if self.event_handle.is_null() {
            return Ok(());
        }

        let succ = unsafe { CloseHandle(self.event_handle) };

        if succ == 0 {
            let err: i32 = unsafe { GetLastError() as i32 };

            return Err(std::io::Error::from_raw_os_error(err));
        }

        if self.origin.is_null() {
            return Ok(());
        }

        let succ = unsafe { CloseHandle(self.origin as HANDLE) };

        if succ == 0 {
            let err: i32 = unsafe { GetLastError() as i32 };

            Err(std::io::Error::from_raw_os_error(err))
        } else {
            Ok(())
        }
    }

    ///
    /// Sample Telemetry Data FPS
    ///
    /// Same functionality as `sample`, but allows usage of the Fps struct.
    ///
    /// # Examples
    ///
    /// ```
    /// use iracing::telemetry::Connection;
    /// use iracing::fps::Fps;
    ///
    /// let sampler = Connection::new()?.blocking()?;
    /// let _ = sampler.sample_fps(Fps::new(30))?;
    /// let _ = sampler.sample_fps(Fps::MAX)?;
    /// let _ = sampler.sample_fps(Fps::MIN)?;
    /// ```
    pub fn sample_fps(&self, fps: Fps) -> Result<Sample, Box<dyn Error>> {
        self.sample(fps.to_duration())
    }

    ///
    /// Sample Telemetry Data
    ///
    /// Waits for new telemetry data up to `timeout` and returns a safe copy of the telemetry data.
    /// Returns an error on timeout or underlying system error.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use iracing::telemetry::Connection;
    /// use std::time::Duration;
    ///
    /// let sampler = Connection::new()?.blocking()?;
    /// let sample = sampler.sample(Duration::from_millis(50))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn sample(&self, timeout: Duration) -> Result<Sample, Box<dyn Error>> {
        let wait_time: u32 = match timeout.as_millis().try_into() {
            Ok(v) => v,
            Err(e) => return Err(Box::new(e)),
        };

        let signal = unsafe { WaitForSingleObject(self.event_handle, wait_time) };

        match signal {
            0x80 => Err(Box::new(TelemetryError::ABANDONED)), // Abandoned
            0x102 => Err(Box::new(TelemetryError::TIMEOUT(wait_time as usize))), // Timeout
            0xFFFFFFFF => {
                // Error
                let errno = unsafe { GetLastError() as i32 };
                Err(Box::new(std::io::Error::from_raw_os_error(errno)))
            }
            0x00 => {
                // OK
                unsafe { ResetEvent(self.event_handle) };
                self.header.telemetry(self.origin)
            }
            _ => Err(Box::new(TelemetryError::UNKNOWN(signal as u32))),
        }
    }
}

///
/// iRacing live telemetry and session data connection.
///
/// Allows retrival of live data fro iRacing.
/// The data is provided using a shared memory map, allowing the simulator
/// to deposit data roughly every 16ms to be read.
///
/// # Examples
///
/// ```
/// use iracing::telemetry::Connection;
///
/// let _ = Connection::new().expect("Unable to find telemetry data");
/// ```
pub struct Connection {
    location: *mut c_void,
}

impl Connection {
    pub fn new() -> IOResult<Connection> {
        let path: Vec<u16> = OsStr::new(TELEMETRY_PATH)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mapping: HANDLE;
        let errno: i32;

        unsafe {
            mapping = OpenFileMappingW(FILE_MAP_READ, 0, path.as_ptr());
        };

        if mapping.is_null() {
            unsafe {
                errno = GetLastError() as i32;
            }

            return Err(std::io::Error::from_raw_os_error(errno));
        }

        let view: LPVOID;

        unsafe {
            view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
        }

        if view.is_null() {
            unsafe {
                errno = GetLastError() as i32;
            }

            return Err(std::io::Error::from_raw_os_error(errno));
        }

        Ok(Connection { location: view })
    }

    pub fn is_connected(&self) -> bool {
        unsafe { Header::parse(self.location) }.is_connected()
    }

    ///
    /// Get session information
    ///
    /// Get general session information - This data is mostly static and contains
    /// overall information related to the current or replayed session
    ///
    /// # Examples
    ///
    /// ```
    /// use iracing::telemetry::Connection;
    ///
    /// match Connection::new().expect("Unable to open session").session_info() {
    ///     Ok(session) => println!("Track Name: {}", session.weekend.track_display_name),
    ///     Err(e) => println!("Invalid Session")
    /// };
    /// ```
    pub fn session_info(&mut self) -> Result<SessionDetails, Box<dyn std::error::Error>> {
        let header = unsafe { Header::parse(self.location) };

        let start = (self.location as usize + header.session_info_offset as usize) as *const u8;
        let size = header.session_info_length as usize;

        let data: &[u8] = unsafe { from_raw_parts(start, size) };

        // Decode the data as Latin-1 (Rust wants UTF-8)
        let content = decode_latin1(data);
        let details = yaml_from(&content)?;

        Ok(details)
    }

    ///
    /// Get latest telemetry.
    ///
    /// Get the latest live telemetry data, the telemetry is updated roughtly every 16ms
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use iracing::telemetry::Connection;
    ///
    /// let sample = Connection::new()?.telemetry()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn telemetry(&self) -> Result<Sample, Box<dyn std::error::Error>> {
        let header = unsafe { Header::parse(self.location) };
        header.telemetry(self.location as *const std::ffi::c_void)
    }

    ///
    /// Get Blocking Telemetry Interface.
    ///
    /// Creates a new `iracing::telemetry::Blocking` connector which allows telemetry samples to
    /// be collected, and will wait and block until a new sample is available, or a timeout is reached.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use iracing::telemetry::Connection;
    /// use std::time::Duration;
    ///
    /// let sampler = Connection::new()?.blocking()?;
    /// let sample = sampler.sample(Duration::from_millis(50))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn blocking(&self) -> IOResult<Blocking> {
        Blocking::new(self.location, unsafe { Header::parse(self.location) })
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
    fn test_session_info() {
        let session_info = Connection::new()
            .expect("Unable to open telemetry")
            .session_info();
        assert!(session_info.is_ok());
    }

    #[test]
    fn test_latest_telemetry() {
        let session_tick: u32 = Connection::new()
            .expect("Unable to open telemetry")
            .telemetry()
            .expect("Couldn't get latest telem")
            .get("SessionTick")
            .unwrap()
            .try_into()
            .unwrap();
        assert!(session_tick > 0);
    }
}
