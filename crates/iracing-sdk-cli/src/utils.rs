use anyhow::Result;
use clap::Subcommand;

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
    /// Empty ranges are valid. Packet ticks retain the zero-based recorded indices.
    ///
    /// # Errors
    ///
    /// Invalid ranges return an error immediately. Iterator items return errors
    /// for failed frame reads, indices or session update counters that do not fit
    /// in `u32`, or frame sizes that differ from the retained layout.
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
    ///
    /// Construction reads no frame bytes and always succeeds. Each item follows
    /// the packet and error semantics of [`Self::frames`].
    pub(crate) fn all_frames(
        &self,
    ) -> Result<impl ExactSizeIterator<Item = Result<FramePacket>> + std::iter::FusedIterator + '_>
    {
        self.frames(0..self.reader.frame_count())
    }

    /// Consumes a recorded frame, preserving its index as the packet tick.
    /// Propagates conversion and layout errors from [`Self::packet_from_bytes`].
    fn packet_from_recorded(&self, frame: RecordedFrame) -> Result<FramePacket> {
        let index = frame.index();
        self.packet_from_bytes(index, frame.into_bytes())
    }

    /// Wraps owned bytes with the retained layout and recording session version.
    /// `index` is the zero-based recorded coordinate used as the packet tick.
    /// Returns an error if either counter cannot fit in `u32` or the byte count
    /// differs from the layout's frame size.
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
