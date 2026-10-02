use std::path::Path;

use anyhow::Result;
#[cfg(windows)]
use iracing_sdk::WindowsConnection;
use iracing_sdk::ibt::IbtReader;

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
