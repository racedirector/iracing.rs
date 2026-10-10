use anyhow::Result;
use iracing_sdk::IbtFile;

use crate::{
    commands::dependencies::LiveHeaders,
    utils::SourceKind,
    writer::{DocumentFormat, DocumentWriter, OutputTarget},
};

#[derive(clap::Args, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    source: SourceKind,

    /// Output destination. Use `-` for stdout.
    #[arg(short, long, default_value = "-", global = true)]
    output: OutputTarget,

    /// The encoding for the session string.
    #[arg(long, default_value = "yaml", global = true, value_enum)]
    format: DocumentFormat,
}

impl Args {
    /// Write and flush the IBT header followed by its disk header, or one live header.
    ///
    /// Uses the injected capability for live input. File output is created or
    /// truncated before source acquisition, even if acquisition fails.
    ///
    /// # Errors
    ///
    /// Propagates output creation, source initialization and acquisition,
    /// serialization, and flush errors.
    pub(crate) fn run(
        self,
        #[cfg_attr(not(windows), allow(unused_variables))] dependencies: &mut impl LiveHeaders,
    ) -> Result<()> {
        let mut writer = DocumentWriter::from_parts(self.output.clone(), self.format)?;

        match self.source {
            SourceKind::Ibt { extra } => {
                let telemetry = IbtFile::open(&extra.path)?;
                writer.write(&telemetry.header())?;
                writer.write(&telemetry.disk_header())?;
            }
            #[cfg(windows)]
            SourceKind::Live { .. } => {
                let header = dependencies.live_header()?;
                writer.write(&header)?;
            }
        }

        writer.finalize()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use clap::Parser;
    struct FakeHeaders {
        calls: usize,
    }
    impl LiveHeaders for FakeHeaders {
        fn live_header(&mut self) -> Result<iracing_irsdk::Header> {
            self.calls += 1;
            anyhow::bail!("fake header acquisition")
        }
    }
    #[test]
    fn live_command_uses_injected_header_capability() {
        let crate::Command::Headers(args) =
            crate::Cli::try_parse_from(["iracing-sdk", "headers", "live", "--format", "json"])
                .unwrap()
                .command
        else {
            panic!("expected headers command")
        };
        let mut fake = FakeHeaders { calls: 0 };
        assert_eq!(
            args.run(&mut fake).unwrap_err().to_string(),
            "fake header acquisition"
        );
        assert_eq!(fake.calls, 1);
    }
}
