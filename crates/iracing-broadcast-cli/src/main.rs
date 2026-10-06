use clap::Parser;
use iracing_broadcast_cli::Command;

#[derive(Parser)]
#[command(name = "iracing-broadcast", version, about = "iRacing Broadcast SDK CLI", long_about = None, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

fn main() -> anyhow::Result<()> {
    Cli::parse().command.run()
}
