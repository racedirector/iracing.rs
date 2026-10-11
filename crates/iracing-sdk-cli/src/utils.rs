use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args, Debug, Default)]
pub(crate) struct NoArgs {}

#[derive(clap::Args, Debug)]
pub(crate) struct IbtArgs<Extra = NoArgs>
where
    Extra: clap::Args,
{
    /// The path of the IBT
    #[arg(short, long)]
    pub path: PathBuf,

    #[command(flatten)]
    pub extra: Extra,
}

#[cfg_attr(
    doc,
    doc = "Per-command source development contracts are indexed in the [command guide](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-sdk-cli/docs/commands/README.md); clap owns source vocabulary and platform gates."
)]
#[derive(Subcommand, Debug)]
pub(crate) enum SourceKind<
    IbtExtra: clap::Args = NoArgs,
    #[cfg(windows)] LiveExtra: clap::Args = NoArgs,
> {
    Ibt {
        #[command(flatten)]
        extra: IbtArgs<IbtExtra>,
    },

    #[cfg(windows)]
    Live {
        #[command(flatten)]
        extra: LiveExtra,
    },
}
