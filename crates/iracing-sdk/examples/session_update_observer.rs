use anyhow::Result;

#[cfg(windows)]
use clap::Parser;

#[cfg(windows)]
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Directory where numbered session info YAML files should be written.
    #[arg(short = 'o', long)]
    output_dir: Option<std::path::PathBuf>,

    /// Emit only car-setup revisions instead of every session-info revision.
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
    tracing_subscriber::fmt().with_env_filter(filter).init();

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
        let mut previous_setup_update = None;
        let mut previous_session_num = None;
        let mut update_index = 0usize;

        while let Some(session) = stream.next().await {
            let setup_update = session.car_setup.as_ref().map(|setup| setup.update_count);
            let current_session_num = session.session_info.current_session_num;
            if car_setup_only && setup_update.is_none() {
                previous_setup_update = None;
            }

            let changed = if car_setup_only {
                setup_update.is_some()
                    && (setup_update != previous_setup_update
                        || previous_session_num != Some(current_session_num))
            } else {
                previous_session_info
                    .as_deref()
                    .is_none_or(|previous_value| previous_value != session.as_ref())
            };

            if changed {
                let serialized = if car_setup_only {
                    serde_yaml_ng::to_string(
                        session
                            .car_setup
                            .as_ref()
                            .expect("a car-setup update is required when this branch is selected"),
                    )?
                } else {
                    serde_yaml_ng::to_string(session.as_ref())?
                };

                if let Some(output_dir) = &output_dir {
                    let stem = if car_setup_only {
                        "car_setup"
                    } else {
                        "session_info"
                    };
                    let output_path = output_dir.join(format!("{stem}_{update_index}.yaml"));
                    fs::write(&output_path, serialized).with_context(|| {
                        format!("failed to write {stem} to {}", output_path.display())
                    })?;
                    tracing::info!(path = %output_path.display(), kind = stem, "Wrote update");
                    update_index += 1;
                } else {
                    print!("{serialized}");
                }

                previous_setup_update = setup_update;
                previous_session_num = Some(current_session_num);
                previous_session_info = Some(session);
            }
        }

        Ok(())
    }
}
