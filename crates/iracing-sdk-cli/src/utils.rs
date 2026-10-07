use std::path::Path;

use anyhow::Result;
#[cfg(windows)]
use iracing_sdk::WindowsConnection;
use iracing_sdk::{
    ibt::IbtReader,
    provider::{SessionInformationBytesProvider, VariableHeadersProvider},
};

enum TelemetryProvider {
    #[cfg(windows)]
    Live(WindowsConnection),
    Disk(IbtReader),
}

impl SessionInformationBytesProvider for TelemetryProvider {
    fn session_info_snapshot(&self) -> iracing_sdk::Result<Option<iracing_sdk::SessionInfoBytes>> {
        match self {
            Self::Disk(reader) => reader.session_info_snapshot(),
            #[cfg(windows)]
            Self::Live(connection) => connection.session_info_snapsho(),
        }
    }
}

impl VariableHeadersProvider for TelemetryProvider {
    fn variable_headers(&self) -> iracing_sdk::Result<iracing_sdk::VariableHeaders> {
        match self {
            Self::Disk(reader) => reader.variable_headers(),
            #[cfg(windows)]
            Self::Live(connection) => connection.variable_headers(),
        }
    }
}

#[cfg(windows)]
pub(crate) fn get_connection() -> Result<WindowsConnection> {
    let connection = match WindowsConnection::try_connect() {
        Ok(c) if c.is_connected() => c,
        Ok(_) => {
            return Err(anyhow::anyhow!(
                "Shared memory opened but telemetry is not connected yet"
            ));
        }
        Err(e) => return Err(anyhow::anyhow!(e)),
    };

    Ok(connection)
}

pub(crate) fn get_disk_reader<P: AsRef<Path>>(path: &P) -> Result<IbtReader> {
    Ok(IbtReader::open(path)?)
}
