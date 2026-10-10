//! Provider for sequential IBT replay.

use std::{path::Path, sync::Arc};

use crate::{
    FramePacket, LayoutProvider, Result, TelemetryLayout,
    ibt::{IbtFile, IbtReader, IbtReplay},
    provider::Provider,
    types::IRacingSessionString,
};

/// A [`Provider`] that streams telemetry frames from an iRacing `.ibt` replay file.
///
/// Owns only [`IbtReplay`]. Immutable metadata and the shared layout belong to
/// its [`IbtFile`]; packet conversion uses [`crate::ibt::IbtFrame::into_packet`].
/// This compatibility adapter is retired with the recorded connection in #305.
/// `from_reader` starts at frame zero; `from_replay` preserves replay state.
pub struct IbtProvider {
    replay: IbtReplay,
}

impl IbtProvider {
    /// Open an `.ibt` file and validate its replay layout.
    ///
    /// The recording must remain unchanged while the provider is alive, as
    /// required by [`IbtFile::open`].
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::from_replay(IbtFile::open(path)?.replay())
    }

    /// Temporarily adapt an existing reader, starting replay at frame zero.
    ///
    /// Its mapped or owned storage transfers into IbtFile without a full-file
    /// copy. This constructor and the private storage bridge are removed in #305.
    ///
    /// # Errors
    /// Returns an error if variable headers cannot be read or validated against
    /// the layout's frame size, or if telemetry frames have no variable metadata.
    /// A zero-frame recording may have an empty layout.
    pub fn from_reader(reader: IbtReader) -> Result<Self> {
        Self::from_replay(reader.into_file()?.replay())
    }

    /// Adapts a validated file replay, preserving its bounds and current position.
    ///
    /// No metadata is read, copied, or reconstructed. The result signature is
    /// retained for existing compatibility callers; this operation cannot fail.
    pub fn from_replay(replay: IbtReplay) -> Result<Self> {
        Ok(Self { replay })
    }

    /// Returns an ownable layout.
    pub(crate) fn shared_layout(&self) -> Arc<TelemetryLayout> {
        Arc::clone(self.layout())
    }

    /// Returns the total number of telemetry frames in the recording.
    pub fn total_frames(&self) -> usize {
        self.replay.file().frame_count()
    }
}

impl LayoutProvider for IbtProvider {
    fn layout(&self) -> &Arc<TelemetryLayout> {
        self.replay.file().telemetry_layout()
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
    /// Returns an error if the frame index exceeds `u32`, the session revision
    /// is negative, reading fails, or the frame size differs from the layout.
    /// These failures leave the replay cursor unchanged.
    async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
        let Some(frame) = self.replay.current_frame()? else {
            return Ok(None);
        };
        let packet = frame.into_packet()?;
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
        let rate = self.replay.file().tick_rate();
        if rate > 0 { f64::from(rate) } else { 60.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
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
                    assert!(Arc::ptr_eq(packet.layout(), provider.layout()));
                    assert!(Arc::ptr_eq(
                        packet.layout(),
                        provider.replay.file().telemetry_layout()
                    ));
                }
                assert!(block_on(provider.next_frame())?.is_none());
                assert!(block_on(provider.next_frame())?.is_none());
            }
        }
        Ok(())
    }

    #[test]
    fn negative_packet_revision_preserves_replay_bounds_and_position() -> anyhow::Result<()> {
        let mut bytes = fs::read(require_smallest_ibt_fixture()?)?;
        let offset = offset_of!(Header, session_info_update);
        bytes[offset..offset + size_of::<i32>()].copy_from_slice(&(-1_i32).to_le_bytes());
        let file = IbtFile::from_bytes(bytes)?;
        assert!(file.frame(2)?.value("Speed")?.is_some());
        let layout = Arc::clone(file.telemetry_layout());
        let mut replay = file.replay();
        replay.set_range(1..4)?;
        replay.seek(2)?;
        let mut provider = IbtProvider::from_replay(replay)?;
        for _ in 0..2 {
            let error = block_on(provider.next_frame()).unwrap_err().to_string();
            assert!(error.contains("session revision"), "{error}");
            assert_eq!(provider.replay.position(), 2);
            assert_eq!(provider.replay.range(), 1..4);
            assert!(Arc::ptr_eq(provider.layout(), &layout));
        }
        Ok(())
    }

    #[test]
    fn provider_borrows_file_layout_and_reports_recorded_tick_rate() -> anyhow::Result<()> {
        for rate in [60_i32, 120] {
            let mut bytes = fs::read(require_smallest_ibt_fixture()?)?;
            let offset = offset_of!(Header, tick_rate);
            bytes[offset..offset + size_of::<i32>()].copy_from_slice(&rate.to_le_bytes());
            let file = IbtFile::from_bytes(bytes)?;
            let layout = Arc::clone(file.telemetry_layout());
            let mut provider = IbtProvider::from_replay(file.replay())?;
            assert!(Arc::ptr_eq(provider.layout(), &layout));
            assert!(Arc::ptr_eq(&provider.shared_layout(), &layout));
            assert_eq!(provider.tick_rate(), f64::from(rate));
            let packet = block_on(provider.next_frame())?.unwrap();
            assert!(Arc::ptr_eq(packet.layout(), &layout));
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
