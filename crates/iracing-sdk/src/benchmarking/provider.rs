//! Provider for sequential IBT replay.

use std::{path::Path, sync::Arc};

use crate::{
    FramePacket, LayoutProvider, Result, TelemetryLayout,
    ibt::{IbtFile, IbtReplay},
    provider::Provider,
};

/// A [`Provider`] that streams telemetry frames from an iRacing `.ibt` replay file.
///
/// Owns only [`IbtReplay`]. Immutable metadata and the shared layout belong to
/// its [`IbtFile`]; packet conversion uses [`crate::ibt::IbtFrame::into_packet`].
/// Benchmark-only adapter preserving the historical asynchronous packet workload.
/// `from_replay` preserves the supplied replay state.
pub struct RecordedProvider {
    replay: IbtReplay,
}

impl RecordedProvider {
    /// Open an `.ibt` file and validate its replay layout.
    ///
    /// The recording must remain unchanged while the provider is alive, as
    /// required by [`IbtFile::open`].
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Ok(Self::from_replay(IbtFile::open(path)?.replay()))
    }

    /// Adapts a validated file replay, preserving its bounds and current position.
    ///
    /// No metadata is read, copied, or reconstructed.
    pub fn from_replay(replay: IbtReplay) -> Self {
        Self { replay }
    }
}

impl LayoutProvider for RecordedProvider {
    fn layout(&self) -> &Arc<TelemetryLayout> {
        self.replay.file().telemetry_layout()
    }
}

#[async_trait::async_trait]
impl Provider for RecordedProvider {
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

        snapshot.sanitized_yaml().map(Some)
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
        irsdk::Header,
        test_utils::{load_fixture_manifest, require_smallest_ibt_fixture},
    };
    use futures::executor::block_on;
    use std::{
        fs,
        mem::{offset_of, size_of},
    };

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
        let mut provider = RecordedProvider::from_replay(replay);
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
            let mut provider = RecordedProvider::from_replay(file.replay());
            assert!(Arc::ptr_eq(provider.layout(), &layout));
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
        let mut provider = RecordedProvider::from_replay(replay);
        assert_eq!(block_on(provider.next_frame())?.unwrap().tick, 2);
        assert_eq!(block_on(provider.next_frame())?.unwrap().tick, 3);
        assert!(block_on(provider.next_frame())?.is_none());
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
        let mut provider = RecordedProvider::from_replay(IbtFile::from_bytes(bytes)?.replay());

        let error = block_on(provider.session_yaml(0)).unwrap_err().to_string();
        assert!(
            error.contains("YAML is empty after preprocessing"),
            "{error}"
        );
        Ok(())
    }
    #[test]
    fn mapped_and_owned_providers_replay_equivalent_packets() -> anyhow::Result<()> {
        for fixture in load_fixture_manifest()?.fixtures {
            let path = fixture.fixture_path()?;
            let bytes = fs::read(&path)?;
            let reference = IbtFile::from_bytes(bytes.clone())?;
            for mut provider in [
                RecordedProvider::open(&path)?,
                RecordedProvider::from_replay(IbtFile::from_bytes(bytes)?.replay()),
            ] {
                assert_eq!(provider.layout().frame_size(), fixture.frame_size);
                assert_eq!(provider.layout().len(), fixture.num_vars as usize);
                assert_eq!(provider.tick_rate(), f64::from(fixture.tick_rate));
                for index in 0..reference.frame_count() {
                    if index == 1 {
                        let yaml = block_on(provider.session_yaml(0))?.unwrap();
                        assert!(
                            !crate::schema::SessionInfo::parse(&yaml)?
                                .weekend_info
                                .track_name
                                .is_empty()
                        );
                    }
                    let packet = block_on(provider.next_frame())?.unwrap();
                    assert_eq!(packet.tick as usize, index);
                    assert_eq!(packet.session_version, fixture.session_info_update as u32);
                    assert_eq!(packet.data().as_ref(), reference.frame(index)?.bytes());
                    assert!(Arc::ptr_eq(packet.layout(), provider.layout()));
                }
                assert!(block_on(provider.next_frame())?.is_none());
                assert!(block_on(provider.next_frame())?.is_none());
            }
        }
        Ok(())
    }
}
