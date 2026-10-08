use anyhow::Result;
use clap::Subcommand;

#[cfg(windows)]
use iracing_sdk::WindowsConnection;
use iracing_sdk::{
    // FramePacket, TelemetryLayout,
    ibt::IbtReader,
    provider::{SessionInformationProvider, VariableHeadersProvider},
};
use std::{
    path::{Path, PathBuf},
    // sync::Arc,
    // time::Duration,
};

pub struct DiskTelemetry {
    pub reader: IbtReader,
    // pub layout: Arc<TelemetryLayout>,
}

impl DiskTelemetry {
    pub(crate) fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let reader = IbtReader::open(path)?;
        // let frame_size = reader.frame_size();
        // let headers = reader.variable_headers()?;
        // let layout = TelemetryLayout::try_from_headers(&headers, frame_size)?;

        Ok(Self {
            reader,
            // layout: Arc::new(layout),
        })
    }

    // pub(crate) fn frame_at(&self, index: usize) -> Result<FramePacket> {
    //     let data = self.reader.frame(index)?;

    //     Ok(FramePacket::new(
    //         data,
    //         u32::try_from(index)?,
    //         u32::try_from(self.reader.header().session_info_update)?,
    //         Arc::clone(&self.layout),
    //     )?)
    // }
}

#[cfg(windows)]
pub struct LiveTelemetry {
    pub connection: WindowsConnection,
    // pub layout: Arc<TelemetryLayout>,
}

#[cfg(windows)]
impl LiveTelemetry {
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

        // let frame_size = usize::try_from(connection.header_snapshot()?.buffer_length)?;
        // let headers = connection.variable_headers()?;
        // let layout = TelemetryLayout::try_from_headers(&headers, frame_size)?;

        Ok(Self {
            connection,
            // layout: Arc::new(layout),
        })
    }

    // pub(crate) fn next_frame(&mut self) -> Result<FramePacket> {
    //     loop {
    //         if let Some(frame) = self.connection.get_new_data()? {
    //             return Ok(FramePacket::new(
    //                 frame.data,
    //                 u32::try_from(frame.tick)?,
    //                 u32::try_from(frame.session_info_update)?,
    //                 Arc::clone(&self.layout),
    //             )?);
    //         }

    //         // Wait up to 500ms for an update
    //         self.connection
    //             .wait_for_update(Duration::from_millis(500))?;
    //     }
    // }
}

pub(crate) enum TelemetrySource {
    Disk(Box<DiskTelemetry>),
    #[cfg(windows)]
    Live(LiveTelemetry),
}

impl SessionInformationProvider for TelemetrySource {
    fn session_info(&self) -> iracing_sdk::Result<Option<iracing_sdk::schema::SessionInfo>> {
        match self {
            Self::Disk(telemetry) => telemetry.reader.session_info(),
            #[cfg(windows)]
            Self::Live(telemetry) => telemetry.connection.session_info(),
        }
    }
}

impl VariableHeadersProvider for TelemetrySource {
    fn variable_headers(&self) -> iracing_sdk::Result<iracing_sdk::VariableHeaders> {
        match self {
            Self::Disk(telemetry) => telemetry.reader.variable_headers(),
            #[cfg(windows)]
            Self::Live(telemetry) => telemetry.connection.variable_headers(),
        }
    }
}

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

impl SourceKind {
    pub(crate) fn open(&self) -> Result<TelemetrySource> {
        match self {
            Self::Ibt { extra } => Ok(TelemetrySource::Disk(Box::new(DiskTelemetry::open(
                &extra.path,
            )?))),
            #[cfg(windows)]
            Self::Live { .. } => Ok(TelemetrySource::Live(LiveTelemetry::try_connect()?)),
        }
    }
}
