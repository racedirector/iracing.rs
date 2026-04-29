use thiserror::Error;
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    },
};

/// Default executable name used by the iRacing simulation process on Windows.
pub const DEFAULT_IRACING_PROCESS_NAME: &str = "iRacingSim64DX11.exe";

/// A running process visible to the current user via the Windows Tool Help API.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RunningProcess {
    /// Process identifier.
    pub pid: u32,
    /// Executable file name reported by Windows.
    pub exe_name: String,
}

/// Errors returned when enumerating processes on Windows fails.
#[derive(Debug, Clone, Error)]
pub enum ProcessDetectionError {
    /// Windows failed to create a snapshot of the running process list.
    #[error("failed to create process snapshot: {message}")]
    Snapshot {
        /// Windows error message returned by the snapshot API.
        message: String,
    },
    /// Windows failed to read the first process entry from the snapshot.
    #[error("failed to enumerate first process entry: {message}")]
    EnumerateFirst {
        /// Windows error message returned by the first-entry API.
        message: String,
    },
    /// Windows failed while iterating the process snapshot.
    #[error("failed to enumerate process entries: {message}")]
    EnumerateNext {
        /// Windows error message returned while advancing the snapshot iterator.
        message: String,
    },
}

struct Snapshot(HANDLE);

impl Snapshot {
    fn create() -> Result<Self, ProcessDetectionError> {
        unsafe {
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
                .map(Self)
                .map_err(|err| ProcessDetectionError::Snapshot {
                    message: err.message().to_string(),
                })
        }
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Return the current Windows process list.
fn list_running_processes() -> Result<Vec<RunningProcess>, ProcessDetectionError> {
    let snapshot = Snapshot::create()?;
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut processes = Vec::new();

    unsafe {
        match Process32FirstW(snapshot.0, &mut entry) {
            Ok(()) => loop {
                processes.push(RunningProcess {
                    pid: entry.th32ProcessID,
                    exe_name: wide_c_string_to_string(&entry.szExeFile),
                });

                if let Err(err) = Process32NextW(snapshot.0, &mut entry) {
                    if err.code() == windows::Win32::Foundation::ERROR_NO_MORE_FILES.to_hresult() {
                        break;
                    }

                    return Err(ProcessDetectionError::EnumerateNext {
                        message: err.message().to_string(),
                    });
                }
            },
            Err(err)
                if err.code() == windows::Win32::Foundation::ERROR_NO_MORE_FILES.to_hresult() =>
            {
                return Ok(processes);
            }
            Err(err) => {
                return Err(ProcessDetectionError::EnumerateFirst {
                    message: err.message().to_string(),
                });
            }
        }
    }

    Ok(processes)
}

/// Return `true` when the target executable is present in the process list.
fn is_process_running(exe_name: &str) -> Result<bool, ProcessDetectionError> {
    Ok(list_running_processes()?
        .iter()
        .any(|process| exe_name_matches(&process.exe_name, exe_name)))
}

/// Return `true` when the default iRacing simulation executable is running.
pub fn is_iracing_process_running() -> Result<bool, ProcessDetectionError> {
    is_process_running(DEFAULT_IRACING_PROCESS_NAME)
}

fn wide_c_string_to_string(buffer: &[u16]) -> String {
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}

fn exe_name_matches(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_c_string_to_string_stops_at_nul() {
        let bytes = [
            b'i' as u16,
            b'R' as u16,
            b'a' as u16,
            b'c' as u16,
            b'i' as u16,
            b'n' as u16,
            b'g' as u16,
            0,
            b'x' as u16,
        ];

        assert_eq!(wide_c_string_to_string(&bytes), "iRacing");
    }

    #[test]
    fn exe_name_matches_is_case_insensitive() {
        assert!(exe_name_matches(
            "IRACINGSIM64DX11.EXE",
            "iRacingSim64DX11.exe"
        ));
    }

    #[test]
    fn exe_name_matches_requires_exact_name() {
        assert!(!exe_name_matches(
            "iRacingSim64DX11.exe.bak",
            "iRacingSim64DX11.exe"
        ));
        assert!(!exe_name_matches("crew-chief.exe", "iRacingSim64DX11.exe"));
    }

    #[test]
    fn default_process_name_matches_expected_binary() {
        assert_eq!(DEFAULT_IRACING_PROCESS_NAME, "iRacingSim64DX11.exe");
    }
}
