use crate::dependencies::LiveVariables;
use anyhow::Result;
use iracing_sdk::provider::VariableHeadersProvider;

use crate::{
    utils::{DiskTelemetry, SourceKind},
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
    /// Write and flush variable descriptions from the IBT or injected live source.
    ///
    /// File output is created or truncated only after headers are acquired.
    /// An empty header snapshot is written as an empty collection.
    ///
    /// # Errors
    ///
    /// Propagates source initialization, acquisition, metadata decoding,
    /// serialization, and output errors.
    pub(crate) fn run(
        self,
        #[cfg_attr(not(windows), allow(unused_variables))] dependencies: &mut impl LiveVariables,
    ) -> Result<()> {
        let headers = match self.source {
            SourceKind::Ibt { extra } => DiskTelemetry::open(&extra.path)?.variable_headers()?,
            #[cfg(windows)]
            SourceKind::Live { .. } => dependencies.live_variable_headers()?,
        };

        let mut writer = DocumentWriter::from_parts(self.output.clone(), self.format)?;
        writer.write(&headers)?;
        writer.finalize()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use clap::Parser;
    struct FakeVariables;
    impl LiveVariables for FakeVariables {
        fn live_variable_headers(&mut self) -> Result<iracing_sdk::VariableHeaders> {
            Ok(iracing_sdk::VariableHeaders::default())
        }
    }
    #[test]
    fn live_command_writes_injected_variable_snapshot() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let output = directory.path().join("variables.json");
        let crate::Command::Variables(args) = crate::Cli::try_parse_from([
            "iracing-sdk",
            "variables",
            "live",
            "--format",
            "json",
            "--output",
            output.to_str().unwrap(),
        ])?
        .command
        else {
            panic!("expected variables command")
        };
        args.run(&mut FakeVariables)?;
        assert_eq!(std::fs::read_to_string(output)?, "[]\n");
        Ok(())
    }
}
