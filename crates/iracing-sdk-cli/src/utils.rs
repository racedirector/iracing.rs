use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use anyhow::Result;
#[cfg(windows)]
use iracing_sdk::WindowsConnection;
use iracing_sdk::{ibt::IbtReader, schema::SessionInfo};

use crate::types::{DocumentFormat, OutputTarget};

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

#[cfg(windows)]
pub(crate) fn get_live_session_info(connection: &WindowsConnection) -> Result<SessionInfo> {
    let buffer = connection
        .session_info_buffer()
        .ok_or_else(|| anyhow::anyhow!("Live connection contains no session information"))?;

    Ok(SessionInfo::try_from(buffer)?)
}

pub(crate) fn get_disk_reader<P: AsRef<Path>>(path: &P) -> Result<IbtReader> {
    Ok(IbtReader::open(path)?)
}

pub(crate) fn get_disk_session_info(reader: &mut IbtReader) -> Result<SessionInfo> {
    let buffer = reader
        .session_info_snapshot()?
        .ok_or_else(|| anyhow::anyhow!("IBT contains no session information"))?;

    Ok(SessionInfo::try_from(buffer)?)
}

pub(super) fn write_to_output<T>(
    value: &T,
    target: &OutputTarget,
    encoding: DocumentFormat,
) -> Result<()>
where
    T: ?Sized + serde::Serialize,
{
    match target {
        OutputTarget::Stdout => {
            let stdout = std::io::stdout();
            write_to_writer(value, stdout.lock(), encoding)
        }
        OutputTarget::File(path) => {
            let writer = BufWriter::new(File::create(path)?);
            write_to_writer(value, writer, encoding)
        }
    }
}

fn write_to_writer<T, W>(value: &T, mut writer: W, encoding: DocumentFormat) -> Result<()>
where
    T: ?Sized + serde::Serialize,
    W: Write,
{
    match encoding {
        DocumentFormat::Yaml => serde_yaml_ng::to_writer(&mut writer, value)?,
        DocumentFormat::Json => serde_json::to_writer(&mut writer, value)?,
        DocumentFormat::JsonPretty => serde_json::to_writer_pretty(&mut writer, value)?,
    }
    writer.flush()?;
    Ok(())
}
