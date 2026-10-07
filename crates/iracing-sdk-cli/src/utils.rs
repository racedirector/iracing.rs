use anyhow::Result;
use clap::Subcommand;

#[cfg(windows)]
use iracing_sdk::WindowsConnection;
use iracing_sdk::{
    ibt::IbtReader,
    provider::{SessionInformationBytesProvider, VariableHeadersProvider},
};
use std::path::{Path, PathBuf};

#[derive(Subcommand, Debug)]
pub(crate) enum SourceKind {
    Ibt {
        /// The path of the IBT
        #[arg(short, long)]
        path: PathBuf,
    },
    #[cfg(windows)]
    Live,
}

impl SourceKind {
    pub(crate) fn open(self) -> Result<TelemetrySource> {
        match self {
            Self::Ibt { path } => TelemetrySource::open(path),
            #[cfg(windows)]
            Self::Live => TelemetrySource::try_connect(),
        }
    }
}

// pub(crate) enum TelemetryFrameRequest {
//     Next,
//     Index(usize),
// }

pub(crate) enum TelemetrySource {
    #[cfg(windows)]
    Live(WindowsConnection),
    Disk(IbtReader),
}

impl TelemetrySource {
    pub(crate) fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Ok(TelemetrySource::Disk(IbtReader::open(path)?))
    }

    #[cfg(windows)]
    pub(crate) fn try_connect() -> Result<Self> {
        let connection = match WindowsConnection::try_connect() {
            Ok(c) if c.is_connected() => c,
            Ok(_) => {
                return Err(anyhow::anyhow!(
                    "Shared memory opened but telemetry is not connected yet"
                ));
            }
            Err(e) => return Err(anyhow::anyhow!(e)),
        };

        Ok(TelemetrySource::Live(connection))
    }

    // /// Returns an owned byte-snapshot for a frame request.
    // pub(crate) fn frame(self, request: TelemetryFrameRequest) -> Result<Vec<u8>> {
    //     match self {
    //         #[cfg(windows)]
    //         Self::Live(mut connection) => {
    //             let Some(data) = connection.get_new_data()? else {
    //                 return Err(anyhow::anyhow!("Could not retrieve live frame snapshot"));
    //             };

    //             Ok(data.data)
    //         }
    //         Self::Disk(mut reader) => {
    //             let data = match request {
    //                 TelemetryFrameRequest::Index(index) => reader.frame(index)?,
    //                 TelemetryFrameRequest::Next => reader.frame(0)?,
    //             };

    //             Ok(data)
    //         }
    //     }
    // }

    // pub(crate) fn to_provider(self) -> Result<TelemetryProvider> {
    //     match self {
    //         #[cfg(windows)]
    //         Self::Live(connection) => {
    //             let provider = LiveProvider::builder()
    //                 .with_connection(connection)
    //                 .build()
    //                 .map_err(|e| anyhow::anyhow!(e))?;

    //             Ok(TelemetryProvider::Live(provider))
    //         }
    //         Self::Disk(reader) => {
    //             let provider = IbtProvider::from_reader(reader)?;

    //             Ok(TelemetryProvider::Disk(provider))
    //         }
    //     }
    // }
}

impl SessionInformationBytesProvider for TelemetrySource {
    fn session_info_snapshot(&self) -> iracing_sdk::Result<Option<iracing_sdk::SessionInfoBytes>> {
        match self {
            Self::Disk(reader) => reader.session_info_snapshot(),
            #[cfg(windows)]
            Self::Live(connection) => connection.session_info_snapshot(),
        }
    }
}

impl VariableHeadersProvider for TelemetrySource {
    fn variable_headers(&self) -> iracing_sdk::Result<iracing_sdk::VariableHeaders> {
        match self {
            Self::Disk(reader) => reader.variable_headers(),
            #[cfg(windows)]
            Self::Live(connection) => connection.variable_headers(),
        }
    }
}

// pub(crate) enum TelemetryProvider {
//     #[cfg(windows)]
//     Live(LiveProvider),
//     Disk(IbtProvider),
// }

// impl TelemetryProvider {
//     pub(crate) async fn to_connection(self) -> Result<TelemetryConnection> {
//         match self {
//             Self::Disk(provider) => {
//                 let connection = IbtConnection::builder()
//                     .with_provider(provider)
//                     .build()
//                     .await?;

//                 Ok(TelemetryConnection::Disk(connection))
//             }
//             #[cfg(windows)]
//             Self::Live(provider) => {
//                 let connection = LiveConnection::builder().with_provider(provider).build()?;

//                 Ok(TelemetryConnection::Live(connection))
//             }
//         }
//     }
// }

// pub(crate) enum TelemetryConnection {
//     #[cfg(windows)]
//     Live(LiveConnection),
//     Disk(IbtConnection),
// }
