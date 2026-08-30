use anyhow::Result;
use clap::{Parser, Subcommand};
use iracing_sdk::{SessionInfo, reader::ibt::IbtReader};
use std::{fs::File, io::BufWriter, path::PathBuf};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "session",
    version,
    about = "iRacing session info utilities",
    long_about = None,
    arg_required_else_help = true,
)]
struct Args {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[cfg(windows)]
    Live {
        /// Path where the session YAML should be written.
        #[arg(short, long, default_value = "live-session-snapshot.yml")]
        output_path: PathBuf,
    },
    Ibt {
        /// Path to the input `.ibt` telemetry file.
        #[arg(short, long)]
        path: PathBuf,

        /// Path where the session YAML should be written.
        #[arg(short, long, default_value = "disk-session-snapshot.yml")]
        output_path: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();

    match args.commands {
        Commands::Ibt { path, output_path } => {
            capture_disk_session(&path, &output_path)?;
            tracing::info!(output_path=%output_path.display(),"Wrote disk session snapshot.");
        }
        #[cfg(windows)]
        Commands::Live { output_path } => {
            capture_live_session(&output_path)?;
            tracing::info!(output_path=%output_path.display(), "Wrote live session snapshot.");
        }
    }

    Ok(())
}

fn capture_disk_session(ibt_path: &PathBuf, output_path: &PathBuf) -> Result<()> {
    let reader = IbtReader::open(ibt_path)?;

    let session_info = match reader.session_info_buffer()? {
        Some(buffer) => SessionInfo::try_from(buffer)?,
        None => {
            return Err(anyhow::anyhow!(
                "Could not parse session information buffer"
            ));
        }
    };

    write_to_output(session_info, output_path)?;

    Ok(())
}

#[cfg(windows)]
fn capture_live_session(output_path: &PathBuf) -> Result<()> {
    use iracing_sdk::WindowsConnection;

    let connection = match WindowsConnection::try_connect() {
        Ok(c) if c.is_connected() => c,
        Ok(_) => {
            return Err(anyhow::anyhow!(
                "Shared memory opened but telemetry is not connected yet"
            ));
        }
        Err(e) => return Err(anyhow::anyhow!(e)),
    };

    let session_info = match connection.session_info_buffer() {
        Some(buffer) => SessionInfo::try_from(buffer)?,
        None => {
            return Err(anyhow::anyhow!(
                "Could not get session info buffer from connection"
            ));
        }
    };

    write_to_output(session_info, output_path)?;

    Ok(())
}

fn write_to_output(session_info: SessionInfo, output_path: &PathBuf) -> Result<()> {
    let output_file = File::create(output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &session_info)?;

    Ok(())
}
