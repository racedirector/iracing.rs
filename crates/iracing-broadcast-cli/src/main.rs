use clap::Parser;
#[cfg(windows)]
use iracing_broadcast_cli::Command;

#[derive(Parser)]
#[command(name = "iracing-broadcast", version, about = "iRacing Broadcast SDK CLI", long_about = None, arg_required_else_help = true)]
struct Cli {
    #[cfg(windows)]
    #[command(subcommand)]
    command: Command,
}

fn main() -> anyhow::Result<()> {
    #[cfg(not(windows))]
    {
        Err(anyhow::anyhow!("Broadcast commands only run on Windows."))
    }

    #[cfg(windows)]
    {
        Cli::parse().command.run()
    }
}
