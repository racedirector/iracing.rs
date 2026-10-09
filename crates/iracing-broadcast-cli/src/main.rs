mod application;

use clap::Parser;

#[derive(Parser)]
#[command(name = "iracing-broadcast", version, about = "iRacing Broadcast SDK CLI", long_about = None, arg_required_else_help = true)]
struct Cli {
    #[cfg(windows)]
    #[command(subcommand)]
    command: iracing_broadcast_cli::Command,
}

/// Parse and execute the broadcast command on Windows.
///
/// Returns an unsupported-platform error before argument parsing on non-Windows
/// systems. On Windows, propagates command execution errors.
fn main() -> anyhow::Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let mut application = application::Application::new();

    #[cfg(not(windows))]
    {
        let _ = &mut application;
        Err(anyhow::anyhow!("Broadcast commands only run on Windows."))
    }

    #[cfg(windows)]
    {
        Cli::parse().command.run(&mut application)
    }
}
