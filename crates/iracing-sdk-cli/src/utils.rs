use anyhow::Result;
use clap::Subcommand;

#[cfg(windows)]
use iracing_sdk::WindowsConnection;
use iracing_sdk::{
    FramePacket, LayoutProvider, TelemetryLayout,
    ibt::{IbtReader, RecordedFrame},
    provider::{SessionInformationBytesProvider, VariableHeadersProvider},
};
use std::{
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};

#[cfg(windows)]
use std::time::Duration;

pub struct DiskTelemetry {
    pub reader: IbtReader,
    pub layout: Arc<TelemetryLayout>,
}

impl DiskTelemetry {
    /// Open a completed IBT recording and validate its telemetry layout.
    ///
    /// The file must remain unmodified and untruncated while the reader is alive.
    ///
    /// # Errors
    ///
    /// Propagates file access, memory mapping, IBT parsing, variable header, and
    /// telemetry layout validation errors.
    pub(crate) fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let reader = IbtReader::open(path)?;
        Self::from_reader(reader)
    }

    pub(crate) fn from_reader(reader: IbtReader) -> Result<Self> {
        let frame_size = reader.frame_size();
        let headers = reader.variable_headers()?;
        let layout = TelemetryLayout::try_from_headers(&headers, frame_size)?;

        Ok(Self {
            reader,
            layout: Arc::new(layout),
        })
    }

    /// Iterates a validated half-open range of recorded frames as schema-backed packets.
    ///
    /// Bounds follow `IbtReader::frames`: the end clamps to the recording length;
    /// reversed ranges and starts beyond EOF fail before iteration. Each item
    /// reads one frame on demand. Item errors consume their recorded coordinate,
    /// and independent iterators do not share cursor state.
    pub(crate) fn frames(
        &self,
        range: Range<usize>,
    ) -> Result<impl ExactSizeIterator<Item = Result<FramePacket>> + std::iter::FusedIterator + '_>
    {
        Ok(self
            .reader
            .frames(range)?
            .map(|frame| self.packet_from_recorded(frame?)))
    }

    /// Iterates all recorded frames in file order, starting at zero.
    pub(crate) fn all_frames(
        &self,
    ) -> Result<impl ExactSizeIterator<Item = Result<FramePacket>> + std::iter::FusedIterator + '_>
    {
        self.frames(0..self.reader.frame_count())
    }

    fn packet_from_recorded(&self, frame: RecordedFrame) -> Result<FramePacket> {
        let index = frame.index();
        self.packet_from_bytes(index, frame.into_bytes())
    }

    fn packet_from_bytes(&self, index: usize, data: Vec<u8>) -> Result<FramePacket> {
        Ok(FramePacket::new(
            data,
            u32::try_from(index)?,
            u32::try_from(self.reader.header().session_info_update)?,
            Arc::clone(&self.layout),
        )?)
    }

    /// Read the zero-based IBT frame without advancing a replay cursor.
    ///
    /// The returned packet owns its bytes and uses `index` as its tick counter.
    ///
    /// # Errors
    ///
    /// Returns an error if `index` is outside the recording, the frame cannot be
    /// read, `index` or the session update counter cannot fit in `u32`, or the
    /// frame size does not match the retained layout.
    pub(crate) fn frame_at(&self, index: usize) -> Result<FramePacket> {
        self.packet_from_bytes(index, self.reader.frame(index)?)
    }
}

impl SessionInformationBytesProvider for DiskTelemetry {
    fn session_info_snapshot(&self) -> iracing_sdk::Result<Option<iracing_sdk::SessionInfoBytes>> {
        self.reader.session_info_snapshot()
    }
}

impl VariableHeadersProvider for DiskTelemetry {
    fn variable_headers(&self) -> iracing_sdk::Result<iracing_sdk::VariableHeaders> {
        self.reader.variable_headers()
    }
}

impl LayoutProvider for DiskTelemetry {
    fn layout(&self) -> &std::sync::Arc<TelemetryLayout> {
        &self.layout
    }
}

#[cfg(windows)]
pub struct LiveTelemetry {
    pub connection: WindowsConnection,
    pub layout: Arc<TelemetryLayout>,
}

#[cfg(windows)]
impl LiveTelemetry {
    /// Open live shared memory and validate its telemetry layout without waiting
    /// for the simulator to become connected.
    ///
    /// # Errors
    ///
    /// Returns an error if telemetry is not connected. Propagates shared-memory
    /// setup, header access, frame-size conversion, variable header, and layout
    /// validation errors.
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

        Self::from_connection(connection)
    }

    pub(crate) fn from_connection(connection: WindowsConnection) -> Result<Self> {
        let frame_size = usize::try_from(connection.header_snapshot()?.buffer_length)?;
        let headers = connection.variable_headers()?;
        let layout = TelemetryLayout::try_from_headers(&headers, frame_size)?;

        Ok(Self {
            connection,
            layout: Arc::new(layout),
        })
    }

    /// Wait cooperatively for a frame, or finish when the simulator disconnects.
    /// Canceling this future leaves at most one bounded native wait in progress.
    pub(crate) async fn next_frame_async(&mut self) -> Result<Option<FramePacket>> {
        loop {
            if !self.connection.is_connected() {
                return Ok(None);
            }
            if let Some(frame) = self.connection.get_new_data()? {
                return Ok(Some(FramePacket::new(
                    frame.data,
                    u32::try_from(frame.tick)?,
                    u32::try_from(frame.session_info_update)?,
                    Arc::clone(&self.layout),
                )?));
            }
            self.connection
                .wait_for_update_async(Duration::from_millis(500))
                .await?;
        }
    }

    /// Block until a new live frame can be returned with the retained layout.
    ///
    /// The first observed tick establishes a baseline without yielding a frame.
    /// Waits retry after each 500 ms timeout while connected; there is no overall
    /// timeout. A disconnect ends capture with an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the source disconnects. Propagates acquisition and
    /// wait errors. Also returns an error if the tick or session update counter
    /// cannot fit in `u32`, or the frame size differs from the retained layout.
    pub(crate) fn next_frame(&mut self) -> Result<FramePacket> {
        loop {
            if !self.connection.is_connected() {
                anyhow::bail!("Live telemetry disconnected before a frame was available");
            }
            if let Some(frame) = self.connection.get_new_data()? {
                return Ok(FramePacket::new(
                    frame.data,
                    u32::try_from(frame.tick)?,
                    u32::try_from(frame.session_info_update)?,
                    Arc::clone(&self.layout),
                )?);
            }

            // Wait up to 500ms for an update
            self.connection
                .wait_for_update(Duration::from_millis(500))?;
        }
    }
}

#[cfg(windows)]
impl SessionInformationBytesProvider for LiveTelemetry {
    fn session_info_snapshot(&self) -> iracing_sdk::Result<Option<iracing_sdk::SessionInfoBytes>> {
        self.connection.session_info_snapshot()
    }
}

#[cfg(windows)]
impl VariableHeadersProvider for LiveTelemetry {
    fn variable_headers(&self) -> iracing_sdk::Result<iracing_sdk::VariableHeaders> {
        self.connection.variable_headers()
    }
}

#[cfg(windows)]
impl LayoutProvider for LiveTelemetry {
    fn layout(&self) -> &std::sync::Arc<TelemetryLayout> {
        &self.layout
    }
}

pub(crate) enum TelemetrySource {
    Disk(Box<DiskTelemetry>),
    #[cfg(windows)]
    Live(LiveTelemetry),
}

impl SessionInformationBytesProvider for TelemetrySource {
    fn session_info_snapshot(&self) -> iracing_sdk::Result<Option<iracing_sdk::SessionInfoBytes>> {
        match self {
            Self::Disk(telemetry) => telemetry.session_info_snapshot(),
            #[cfg(windows)]
            Self::Live(telemetry) => telemetry.session_info_snapshot(),
        }
    }
}

impl VariableHeadersProvider for TelemetrySource {
    fn variable_headers(&self) -> iracing_sdk::Result<iracing_sdk::VariableHeaders> {
        match self {
            Self::Disk(telemetry) => telemetry.variable_headers(),
            #[cfg(windows)]
            Self::Live(telemetry) => telemetry.variable_headers(),
        }
    }
}

impl LayoutProvider for TelemetrySource {
    fn layout(&self) -> &std::sync::Arc<TelemetryLayout> {
        match self {
            Self::Disk(telemetry) => telemetry.layout(),
            #[cfg(windows)]
            Self::Live(telemetry) => telemetry.layout(),
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

#[cfg(test)]
mod disk_tests {
    use super::*;
    use iracing_sdk::test_utils::require_named_ibt_fixture;

    #[test]
    fn packet_iterators_preserve_coordinates_layout_and_independence() -> Result<()> {
        let telemetry = DiskTelemetry::open(require_named_ibt_fixture("profile_small.ibt")?)?;
        let count = telemetry.reader.frame_count();
        let mut first = telemetry.all_frames()?;
        let mut tail = telemetry.frames(count - 1..usize::MAX)?;
        assert_eq!(first.len(), count);
        assert_eq!(tail.len(), 1);
        let packet = tail.next().unwrap()?;
        assert_eq!(packet.tick as usize, count - 1);
        assert_eq!(packet.data(), telemetry.frame_at(count - 1)?.data());
        assert!(Arc::ptr_eq(packet.layout(), &telemetry.layout));
        assert_eq!(
            packet.session_version,
            telemetry.frame_at(0)?.session_version
        );
        assert!(tail.next().is_none());
        assert!(tail.next().is_none());
        assert_eq!(first.next().unwrap()?.tick, 0);
        telemetry.frame_at(count - 1)?;
        assert_eq!(first.next().unwrap()?.tick, 1);
        assert_eq!(first.len(), count - 2);
        assert_eq!(telemetry.frames(count..count)?.len(), 0);
        assert!(telemetry.frames(Range { start: 2, end: 1 }).is_err());
        assert!(telemetry.frames(count + 1..usize::MAX).is_err());
        Ok(())
    }
}
