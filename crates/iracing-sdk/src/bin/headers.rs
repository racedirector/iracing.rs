use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use iracing_sdk::headers::{DiskSubHeader, Header, IbtHeader, VariableBuffer};
use std::{fs::File, io::BufReader, path::PathBuf};
use tracing_subscriber::EnvFilter;
use type_layout::TypeLayout;

/// iRacing header utilities.
#[derive(Parser)]
#[command(
    name = "headers",
    version,
    about = "iRacing header utilities",
    long_about = None,
    arg_required_else_help = true,
)]
struct Args {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Gets headers from a provided IBT
    Ibt {
        /// The path of the IBT
        #[arg(short, long)]
        path: PathBuf,
    },
    /// Gets headers from a live iRacing connection.
    #[cfg(windows)]
    Live {
        /// Whether to wait for the connection
        #[arg(short, long)]
        wait: bool,
        /// How long to wait before timeout. No value wait indefinitely.
        #[arg(short, long)]
        timeout_ms: Option<u32>,
    },
    /// Prints the type information for the header data structures.
    Type,
}

fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // ------------------------------------------------------------
    // Parse CLI arguments.
    // ------------------------------------------------------------
    let args = Args::parse();

    match args.commands {
        Commands::Ibt { path } => {
            print_ibt_header(path)?;
        }
        #[cfg(windows)]
        Commands::Live { wait, timeout_ms } => {
            print_live_header(wait, timeout_ms)?;
        }
        Commands::Type => print_type_layout()?,
    }

    Ok(())
}

fn print_type_layout() -> Result<()> {
    tracing::info!(
        "VariableBuffer type layout:\n{}",
        VariableBuffer::type_layout()
    );

    tracing::info!("Header type layout:\n{}", Header::type_layout());

    tracing::info!(
        "DiskSubHeader type layout:\n{}",
        DiskSubHeader::type_layout()
    );

    tracing::info!("IbtHeader type layout:\n{}", IbtHeader::type_layout());

    Ok(())
}

/**
 * Attempts to acquire a windows connection, parse the header, and print it to stdout via tracing at level info.
 */
#[cfg(windows)]
fn print_live_header(wait: bool, _timeout_ms: Option<u32>) -> Result<()> {
    use iracing_sdk::WindowsConnection;

    let connection = if wait {
        loop {
            match WindowsConnection::try_connect() {
                Ok(connection) if connection.is_connected() => break connection,
                Ok(_) => {
                    tracing::debug!("Shared memory opened but telemetry is not connected yet");
                }
                Err(error) => {
                    tracing::debug!(%error, "Waiting for iRacing shared memory");
                }
            }

            // TODO: Handle a timeout
        }
    } else {
        WindowsConnection::try_connect().expect("Could not connect to iRacing")
    };

    let header = connection.header_snapshot();

    tracing::info!(
        "Parsed live header:\nIs valid: {}\n{:#?}",
        header.validate_live().is_ok(),
        header
    );

    Ok(())
}

/**
 * Opens the file at `path`, creates `Header` and `DiskSubHeader` instances
 * and pretty-prints them to stdout.
 */
fn print_ibt_header(path: PathBuf) -> Result<()> {
    let file = File::open(&path).with_context(|| format!("Opening IBT file {}", path.display()))?;
    let mut reader = BufReader::new(file);

    let ibt_header = IbtHeader::try_from_reader(&mut reader)
        .with_context(|| format!("Parsing IBT header from {}", path.display()))?;

    tracing::info!(
        "Parsed IBT header:\nIs valid: {}\n{:#?}",
        ibt_header.is_valid(),
        ibt_header
    );

    Ok(())
}
