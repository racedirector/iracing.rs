use anyhow::Result;

#[cfg(windows)]
use clap::Parser;

#[cfg(windows)]
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Directory where numbered YAML files should be written.
    #[arg(short = 'o', long)]
    output_dir: Option<std::path::PathBuf>,

    /// Emit only car setup revisions.
    #[arg(long)]
    car_setup_only: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    #[cfg(not(windows))]
    {
        use anyhow::anyhow;

        tracing::warn!(
            "session-updates is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
        );
        Err(anyhow!("session-updates is only supported on Windows"))
    }

    #[cfg(windows)]
    {
        use anyhow::Context;
        use futures::StreamExt;
        use iracing_sdk::{LiveConnection, WindowsConnection, providers::live::LiveProvider};
        use std::{fs, thread, time::Duration};

        let Args {
            output_dir,
            car_setup_only,
        } = Args::parse();

        if let Some(output_dir) = &output_dir {
            fs::create_dir_all(output_dir).with_context(|| {
                format!("failed to create output directory {}", output_dir.display())
            })?;
        }

        let windows_connection = loop {
            match WindowsConnection::try_connect() {
                Ok(connection) if connection.is_connected() => break connection,
                Ok(_) => {
                    tracing::debug!("Shared memory opened but telemetry is not connected yet");
                }
                Err(error) => {
                    tracing::debug!(%error, "Waiting for iRacing shared memory");
                }
            }

            thread::sleep(Duration::from_secs(1));
        };

        let provider = LiveProvider::builder()
            .with_connection(windows_connection)
            .without_no_connection_limit()
            .build()?;

        let connection = LiveConnection::builder().with_provider(provider).build()?;
        let mut stream = Box::pin(connection.session_updates());
        let mut previous_session_info = None;
        let mut previous_setup_revision = None;
        let mut update_index = 0usize;

        while let Some(session) = stream.next().await {
            if car_setup_only {
                let Some(setup) = &session.car_setup else {
                    continue;
                };
                let revision = (session.session_info.current_session_num, setup.update_count);
                if previous_setup_revision == Some(revision) {
                    continue;
                }

                let setup_yaml = serde_yaml_ng::to_string(setup)?;
                if let Some(output_dir) = &output_dir {
                    let output_path = output_dir.join(format!("car_setup_{update_index}.yaml"));
                    fs::write(&output_path, setup_yaml).with_context(|| {
                        format!("failed to write car setup to {}", output_path.display())
                    })?;
                    tracing::info!(path = %output_path.display(), "Wrote car setup");
                } else {
                    print!("---\n{setup_yaml}");
                }
                previous_setup_revision = Some(revision);
                update_index += 1;
                continue;
            }

            let changed = previous_session_info
                .as_deref()
                .is_none_or(|previous_value| previous_value != session.as_ref());

            if changed {
                let session_yaml = serde_yaml_ng::to_string(session.as_ref())?;
                if let Some(output_dir) = &output_dir {
                    let output_path = output_dir.join(format!("session_info_{update_index}.yaml"));
                    fs::write(&output_path, session_yaml).with_context(|| {
                        format!("failed to write session info to {}", output_path.display())
                    })?;
                    tracing::info!(path = %output_path.display(), "Wrote session info");
                } else {
                    print!("---\n{session_yaml}");
                }

                previous_session_info = Some(session);
                update_index += 1;
            }
        }

        Ok(())
    }
}
