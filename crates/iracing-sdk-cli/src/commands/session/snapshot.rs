use crate::{
    commands::dependencies::LiveSessions,
    utils::SourceKind,
    writer::{DocumentFormat, DocumentWriter, OutputTarget},
};
use anyhow::{Context, Result};
use iracing_sdk::IbtFile;

#[derive(clap::Args, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    source: SourceKind,

    /// Output destination. Use `-` for stdout.
    #[arg(short, long, default_value = "-", global = true)]
    output: OutputTarget,

    /// The format for the output.
    #[arg(long, default_value = "yaml", global = true, value_enum)]
    format: DocumentFormat,
}

impl Args {
    /// Write and flush the decoded session from the IBT or injected live source.
    ///
    /// File output is created or truncated only after a session has been acquired
    /// and parsed.
    ///
    /// # Errors
    ///
    /// Returns an error when no session is available. Propagates source
    /// initialization, acquisition, session parsing, serialization, and output errors.
    pub(crate) fn run(
        self,
        #[cfg_attr(not(windows), allow(unused_variables))] dependencies: &mut impl LiveSessions,
    ) -> Result<()> {
        let session_info = match self.source {
            SourceKind::Ibt { extra } => super::parse_ibt_session(&IbtFile::open(&extra.path)?)?,
            #[cfg(windows)]
            SourceKind::Live { .. } => dependencies.live_session()?,
        }
        .context("Session information is unavailable")?;

        let mut writer = DocumentWriter::from_parts(self.output.clone(), self.format)?;
        writer.write(&session_info)?;
        writer.finalize()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use clap::Parser;
    struct FakeSessions;
    impl LiveSessions for FakeSessions {
        fn live_session(&mut self) -> Result<Option<iracing_sdk::schema::SessionInfo>> {
            Ok(None)
        }
    }
    #[test]
    fn live_command_uses_injected_session_capability() {
        let crate::Command::Session { command } =
            crate::Cli::try_parse_from(["iracing-sdk", "session", "snapshot", "live"])
                .unwrap()
                .command
        else {
            panic!("expected session command")
        };
        assert_eq!(
            command.run(&mut FakeSessions).unwrap_err().to_string(),
            "Session information is unavailable"
        );
    }
}
