//! Provider for sequential IBT replay.

use std::{path::Path, sync::Arc};

use crate::{
    FramePacket, IRacingSDKError, Result, SchemaProvider, VariableSchema, ibt::IbtReader,
    provider::Provider, types::IRacingSessionString,
};

/// A [`Provider`] that streams telemetry frames from an iRacing `.ibt` replay file.
///
/// Owns the validated variable schema and sequential replay cursor. Construction
/// always starts at frame zero, regardless of previous indexed reader operations.
pub struct IbtProvider {
    reader: IbtReader,
    schema: Arc<VariableSchema>,
    current_frame: usize,
    tick_rate: f64,
}

impl IbtProvider {
    /// Open an `.ibt` file and validate its replay schema.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::from_reader(IbtReader::open(path)?)
    }

    /// Create a replay provider starting at frame zero.
    ///
    /// # Errors
    /// Returns an error if variable headers cannot be read or validated against
    /// the layout's frame size, or if telemetry frames have no variable metadata.
    /// A zero-frame recording may have an empty schema.
    pub fn from_reader(mut reader: IbtReader) -> Result<Self> {
        let frame_size = reader.layout().frame_size();
        let frame_count = reader.layout().frame_count();
        let schema = match reader.variable_headers_snapshot()? {
            Some(snapshot) => VariableSchema::from_snapshot(snapshot, frame_size)?,
            None if frame_count == 0 => VariableSchema::from_headers(&[], frame_size)?,
            None => {
                return Err(IRacingSDKError::parse_error(
                    "IBT replay schema",
                    "Telemetry frames require variable-header metadata",
                ));
            }
        };
        let tick_rate = if reader.header().tick_rate > 0 {
            f64::from(reader.header().tick_rate)
        } else {
            60.0
        };
        Ok(Self {
            reader,
            schema: Arc::new(schema),
            current_frame: 0,
            tick_rate,
        })
    }

    /// Returns an ownable schema.
    pub(crate) fn shared_schema(&self) -> Arc<VariableSchema> {
        Arc::clone(&self.schema)
    }
}

impl SchemaProvider for IbtProvider {
    fn schema(&self) -> &VariableSchema {
        self.schema.as_ref()
    }
}

#[async_trait::async_trait]
impl Provider for IbtProvider {
    /// Emits each frame once in index order, then returns permanent EOF.
    ///
    /// The synthetic tick is the zero-based frame index (cast to `u32`), and
    /// the session version is the recording header's update counter. Failed
    /// reads leave the replay cursor unchanged so the same frame can be retried.
    async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
        if self.current_frame >= self.reader.layout().frame_count() {
            return Ok(None);
        }
        let frame_data = self.reader.frame(self.current_frame)?;
        let packet = FramePacket::new(
            frame_data,
            self.current_frame as u32,
            self.reader.header().session_info_update as u32,
            self.shared_schema(),
        );
        self.current_frame += 1;
        Ok(Some(packet))
    }

    async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
        Ok(self
            .reader
            .session_info_snapshot()?
            .and_then(|snapshot| IRacingSessionString::try_from(snapshot).ok())
            .map(Into::into))
    }

    fn tick_rate(&self) -> f64 {
        self.tick_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{load_fixture_manifest, require_smallest_ibt_fixture};
    use futures::executor::block_on;
    use std::fs;

    #[test]
    fn constructors_replay_equivalent_frames_and_schema_from_zero() -> anyhow::Result<()> {
        for fixture in load_fixture_manifest()?.fixtures {
            let path = fixture.fixture_path()?;
            let bytes = fs::read(&path)?;
            let mut reference = IbtReader::from_bytes(bytes.clone())?;
            let mut moved = IbtReader::open(&path)?;
            moved.frame(fixture.num_frames - 1)?;
            moved.session_info_snapshot()?;
            moved.variable_headers_snapshot()?;
            for mut provider in [
                IbtProvider::open(&path)?,
                IbtProvider::from_reader(IbtReader::from_bytes(bytes)?)?,
                IbtProvider::from_reader(moved)?,
            ] {
                assert_eq!(provider.schema().frame_size, fixture.frame_size);
                assert_eq!(
                    provider.schema().variable_count(),
                    fixture.num_vars as usize
                );
                assert_eq!(provider.tick_rate(), f64::from(fixture.tick_rate));
                for expected in &fixture.required_variables {
                    let actual = provider.schema().get_variable(&expected.name).unwrap();
                    assert_eq!(actual.offset, expected.offset);
                    assert_eq!(actual.count, expected.count);
                    assert_eq!(actual.units, expected.units);
                }
                for index in 0..reference.layout().frame_count() {
                    // Session snapshots move the source cursor between frame reads.
                    if index == 1 {
                        let yaml = block_on(provider.session_yaml(0))?.unwrap();
                        let session = crate::schema::SessionInfo::parse(&yaml)?;
                        assert!(!session.weekend_info.track_name.is_empty());
                    }
                    let packet = block_on(provider.next_frame())?.unwrap();
                    assert_eq!(packet.tick as usize, index);
                    assert_eq!(packet.session_version, fixture.session_info_update as u32);
                    assert_eq!(packet.data.as_ref(), reference.frame(index)?);
                    assert!(Arc::ptr_eq(&packet.schema, &provider.schema));
                }
                assert!(block_on(provider.next_frame())?.is_none());
                assert!(block_on(provider.next_frame())?.is_none());
            }
        }
        Ok(())
    }

    #[test]
    fn failed_read_does_not_advance_replay() -> anyhow::Result<()> {
        let bytes = fs::read(require_smallest_ibt_fixture()?)?;
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("retry.ibt");
        fs::write(&path, &bytes)?;
        let mut reader = IbtReader::open(&path)?;
        let expected = reader.frame(0)?;
        let start = reader.layout().frame_data_start();
        let mut provider = IbtProvider::from_reader(reader)?;
        fs::OpenOptions::new()
            .write(true)
            .open(&path)?
            .set_len(start as u64)?;
        assert!(block_on(provider.next_frame()).is_err());
        assert_eq!(provider.current_frame, 0);
        fs::write(&path, &bytes)?;
        let frame = block_on(provider.next_frame())?.unwrap();
        assert_eq!(frame.tick, 0);
        assert_eq!(frame.data.as_ref(), expected);
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
        // Byte geometry remains valid; the provider owns semantic validation.
        let reader = IbtReader::from_bytes(bytes)?;
        let error = IbtProvider::from_reader(reader).err().unwrap().to_string();
        assert!(error.contains("beyond frame size"), "{error}");
        Ok(())
    }
}
