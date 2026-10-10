//! Provider for sequential IBT replay.

use std::{path::Path, sync::Arc};

use crate::{
    FramePacket, IRacingSDKError, LayoutProvider, Result, TelemetryLayout,
    ibt::{IbtReader, IbtReplay},
    provider::Provider,
    types::IRacingSessionString,
};

/// A [`Provider`] that streams telemetry frames from an iRacing `.ibt` replay file.
///
/// Retains its compatibility layout and delegates cursor state to [`IbtReplay`].
/// Replay owns an [`IbtFile`](crate::ibt::IbtFile); the duplicated provider
/// layout and packet bridge are consolidated by #304.
/// `from_reader` starts at frame zero; `from_replay` preserves replay state.
pub struct IbtProvider {
    replay: IbtReplay,
    layout: Arc<TelemetryLayout>,
    tick_rate: f64,
}

impl IbtProvider {
    /// Open an `.ibt` file and validate its replay layout.
    ///
    /// The recording must remain unchanged while the provider is alive, as
    /// required by [`IbtReader::open`].
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::from_reader(IbtReader::open(path)?)
    }

    /// Create a replay provider starting at frame zero.
    ///
    /// # Errors
    /// Returns an error if variable headers cannot be read or validated against
    /// the layout's frame size, or if telemetry frames have no variable metadata.
    /// A zero-frame recording may have an empty layout.
    pub fn from_reader(reader: IbtReader) -> Result<Self> {
        Self::from_replay(reader.into_file()?.replay())
    }

    /// Adapts a replay, preserving its configured bounds and current position.
    /// Schema validation uses the complete recording metadata, as in
    /// [`Self::from_reader`]. A nonpositive recorded tick rate defaults to 60 Hz.
    ///
    /// # Errors
    /// Propagates compatibility-layout validation errors. The replay's file
    /// already validated immutable metadata during construction.
    pub fn from_replay(replay: IbtReplay) -> Result<Self> {
        let file = replay.file();
        let frame_size = file.frame_size();

        let headers = file.variable_headers();
        if headers.is_empty() && file.frame_count() > 0 {
            return Err(IRacingSDKError::parse_error(
                "IBT replay layout",
                "Telemetry frames require variable-header metadata",
            ));
        }
        let layout = TelemetryLayout::try_from_headers(headers, frame_size)?;

        let tick_rate = if file.header().tick_rate > 0 {
            f64::from(file.header().tick_rate)
        } else {
            60.0
        };
        Ok(Self {
            replay,
            layout: Arc::new(layout),
            tick_rate,
        })
    }

    /// Returns an ownable layout.
    pub(crate) fn shared_layout(&self) -> Arc<TelemetryLayout> {
        Arc::clone(&self.layout)
    }

    /// Returns the total number of telemetry frames in the recording.
    pub fn total_frames(&self) -> usize {
        self.replay.file().frame_count()
    }

    fn tick_for_frame(index: usize) -> Result<u32> {
        u32::try_from(index).map_err(|_| {
            IRacingSDKError::parse_error(
                "IbtProvider::next_frame",
                format!("Frame index {index} exceeds the u32 tick range"),
            )
        })
    }
}

impl LayoutProvider for IbtProvider {
    fn layout(&self) -> &Arc<TelemetryLayout> {
        &self.layout
    }
}

#[async_trait::async_trait]
impl Provider for IbtProvider {
    /// Emits the remaining frames within the replay bounds in index order,
    /// then returns permanent EOF.
    ///
    /// The synthetic tick is the zero-based frame index (checked against `u32`), and
    /// the session version is the recording header's update counter. Failed
    /// reads leave the replay cursor unchanged so the same frame can be retried.
    ///
    /// # Errors
    /// Returns an error if the frame index exceeds `u32`, reading fails, or the
    /// returned byte count differs from the telemetry layout's frame size.
    /// These failures leave the replay cursor unchanged.
    async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
        if self.replay.is_eof() {
            return Ok(None);
        }
        let tick = Self::tick_for_frame(self.replay.position())?;
        let Some(frame) = self.replay.current_frame()? else {
            return Ok(None);
        };
        let frame_data = frame.into_bytes();
        let packet = FramePacket::new(
            frame_data,
            tick,
            self.replay.file().header().session_info_update as u32,
            self.shared_layout(),
        )?;
        self.replay.advance();
        Ok(Some(packet))
    }

    /// Returns the recording's decoded, sanitized session YAML without advancing.
    /// Ignores `_version` and returns `None` only when session metadata is absent.
    ///
    /// # Errors
    /// Rejects text that is empty or whitespace-only after sanitization.
    /// YAML syntax is not validated here.
    async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
        let Some(snapshot) = self.replay.file().session_info_bytes() else {
            return Ok(None);
        };

        Ok(Some(
            IRacingSessionString::try_from(snapshot.payload().decode())?.into(),
        ))
    }

    fn tick_rate(&self) -> f64 {
        self.tick_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ibt::IbtFile,
        irsdk::{DiskSubHeader, Header},
        provider::VariableHeadersProvider,
        test_utils::{load_fixture_manifest, require_smallest_ibt_fixture},
    };
    use futures::executor::block_on;
    use std::fs;
    use std::mem::{offset_of, size_of};

    #[test]
    fn constructors_replay_equivalent_frames_and_schema_from_zero() -> anyhow::Result<()> {
        for fixture in load_fixture_manifest()?.fixtures {
            let path = fixture.fixture_path()?;
            let bytes = fs::read(&path)?;
            let reference = IbtReader::from_bytes(bytes.clone())?;
            let moved = IbtReader::open(&path)?;
            moved.frame(fixture.num_frames - 1)?;
            crate::provider::SessionInformationBytesProvider::session_info_snapshot(&moved)?;
            moved.variable_headers()?;
            for mut provider in [
                IbtProvider::open(&path)?,
                IbtProvider::from_reader(IbtReader::from_bytes(bytes)?)?,
                IbtProvider::from_reader(moved)?,
            ] {
                assert_eq!(provider.total_frames(), fixture.num_frames);
                assert_eq!(provider.layout().frame_size(), fixture.frame_size);
                assert_eq!(provider.layout().len(), fixture.num_vars as usize);
                assert_eq!(provider.tick_rate(), f64::from(fixture.tick_rate));
                for expected in &fixture.required_variables {
                    let actual = provider
                        .layout()
                        .field_by_name(&expected.name)
                        .map(|(_, field)| field)
                        .unwrap();
                    assert_eq!(actual.region().offset(), expected.offset);
                    assert_eq!(actual.count(), expected.count);
                    assert_eq!(actual.metadata().unit(), expected.units);
                }
                for index in 0..reference.frame_count() {
                    // Session snapshots move the source cursor between frame reads.
                    if index == 1 {
                        let yaml = block_on(provider.session_yaml(0))?.unwrap();
                        let session = crate::schema::SessionInfo::parse(&yaml)?;
                        assert!(!session.weekend_info.track_name.is_empty());
                    }
                    let packet = block_on(provider.next_frame())?.unwrap();
                    assert_eq!(packet.tick as usize, index);
                    assert_eq!(packet.session_version, fixture.session_info_update as u32);
                    assert_eq!(packet.data().as_ref(), reference.frame(index)?);
                    assert!(Arc::ptr_eq(packet.layout(), &provider.layout));
                }
                assert!(block_on(provider.next_frame())?.is_none());
                assert!(block_on(provider.next_frame())?.is_none());
            }
        }
        Ok(())
    }

    #[test]
    fn configured_replay_preserves_bounds_position_and_ticks() -> anyhow::Result<()> {
        let mut replay = IbtFile::open(require_smallest_ibt_fixture()?)?.replay();
        replay.set_range(1..4)?;
        replay.seek(2)?;
        let mut provider = IbtProvider::from_replay(replay)?;
        assert_eq!(block_on(provider.next_frame())?.unwrap().tick, 2);
        assert_eq!(block_on(provider.next_frame())?.unwrap().tick, 3);
        assert!(block_on(provider.next_frame())?.is_none());
        Ok(())
    }

    #[test]
    fn failed_read_does_not_advance_replay() -> anyhow::Result<()> {
        let bytes = fs::read(require_smallest_ibt_fixture()?)?;
        let reader = IbtReader::from_bytes(bytes.clone())?;
        let expected = reader.frame(0)?;
        let start = reader.layout().frame_data_start();
        let mut provider = IbtProvider::from_reader(reader)?;
        // Inject a short read without mutating a live mapped file.
        provider.replay.file.owned_bytes_mut().truncate(start);
        assert!(block_on(provider.next_frame()).is_err());
        assert_eq!(provider.replay.position(), 0);
        *provider.replay.file.owned_bytes_mut() = bytes;
        let frame = block_on(provider.next_frame())?.unwrap();
        assert_eq!(frame.tick, 0);
        assert_eq!(frame.data().as_ref(), expected);
        Ok(())
    }

    #[test]
    fn schema_validation_uses_layout_frame_size() -> anyhow::Result<()> {
        use crate::irsdk::VariableHeader;
        let mut bytes = fs::read(require_smallest_ibt_fixture()?)?;
        let reader = IbtReader::from_bytes(bytes.clone())?;
        let offset = reader
            .layout()
            .metadata()
            .variable_headers()
            .unwrap()
            .as_region()
            .offset()
            + std::mem::offset_of!(VariableHeader, offset);
        bytes[offset..offset + 4]
            .copy_from_slice(&(reader.layout().frame_size() as i32).to_le_bytes());
        // Byte geometry remains valid; conversion to IbtFile validates semantics.
        let reader = IbtReader::from_bytes(bytes)?;
        let error = IbtProvider::from_reader(reader).err().unwrap().to_string();
        assert!(error.contains("past frame size"), "{error}");
        Ok(())
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn frame_index_beyond_tick_range_is_rejected() {
        assert_eq!(
            IbtProvider::tick_for_frame(u32::MAX as usize).unwrap(),
            u32::MAX
        );
        let error = IbtProvider::tick_for_frame(u32::MAX as usize + 1)
            .unwrap_err()
            .to_string();
        assert!(error.contains("exceeds the u32 tick range"), "{error}");
    }

    #[test]
    fn invalid_session_snapshot_returns_an_error() -> anyhow::Result<()> {
        let mut bytes = fs::read(require_smallest_ibt_fixture()?)?;
        let file = IbtFile::from_bytes(bytes.clone())?;
        let offset = file
            .physical_layout()
            .metadata()
            .session_info()
            .expect("fixture has session information")
            .offset();
        // Immutable metadata is captured during construction, before replay.
        bytes[offset] = 0;
        let mut provider = IbtProvider::from_reader(IbtReader::from_bytes(bytes)?)?;

        let error = block_on(provider.session_yaml(0)).unwrap_err().to_string();
        assert!(
            error.contains("YAML is empty after preprocessing"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn zero_frame_recording_without_variable_headers_has_empty_schema() -> anyhow::Result<()> {
        let mut bytes = fs::read(require_smallest_ibt_fixture()?)?;
        let reader = IbtReader::from_bytes(bytes.clone())?;
        let metadata_end = reader
            .layout()
            .metadata()
            .session_info()
            .expect("fixture has session information")
            .end();
        let frame_size = reader.frame_size();

        bytes[offset_of!(Header, variable_count)..offset_of!(Header, variable_count) + 4]
            .copy_from_slice(&0_i32.to_le_bytes());
        let record_count_offset = size_of::<Header>() + offset_of!(DiskSubHeader, record_count);
        bytes[record_count_offset..record_count_offset + 4].copy_from_slice(&0_i32.to_le_bytes());
        bytes.truncate(metadata_end);

        let mut provider = IbtProvider::from_reader(IbtReader::from_bytes(bytes)?)?;
        assert_eq!(provider.replay.file().frame_count(), 0);
        assert!(
            provider
                .replay
                .file
                .physical_layout()
                .metadata()
                .variable_headers()
                .is_none()
        );
        assert_eq!(provider.layout().len(), 0);
        assert_eq!(provider.layout().frame_size(), frame_size);
        assert!(block_on(provider.next_frame())?.is_none());
        Ok(())
    }
}
