//! Provider for IBT files.

use std::{path::Path, sync::Arc};

use crate::{
    FramePacket, IRacingSDKError, Result, SchemaProvider, VariableSchema, provider::Provider,
    reader::ibt::IbtReader, yaml_utils,
};

/// A [`Provider`] that streams telemetry frames from an iRacing `.ibt` replay file.
pub struct IbtProvider {
    reader: IbtReader,
    schema: Arc<VariableSchema>,
    tick_rate: f64,
}

impl IbtProvider {
    /// Open an `.ibt` file as a replay provider.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::from_reader(IbtReader::open(path)?)
    }

    /// Create a replay provider from an already-opened reader.
    pub fn from_reader(reader: IbtReader) -> Result<Self> {
        let tick_rate = reader.tick_rate();
        let schema = VariableSchema::from_reader(&reader).map(Arc::new)?;
        Ok(Self {
            reader,
            schema,
            tick_rate,
        })
    }

    /// Seek to a specific frame
    pub fn seek_to_frame(&mut self, frame: usize) -> Result<()> {
        self.reader.seek_to_frame(frame)
    }

    /// Returns the index of the next frame that will be read (0-based).
    pub fn current_frame(&self) -> usize {
        self.reader.current_frame()
    }

    /// Returns the total number of telemetry frames in the file.
    pub fn total_frames(&self) -> usize {
        self.reader.total_frames()
    }

    /// Get current playback time in seconds.
    pub fn current_time(&self) -> f64 {
        self.reader.current_time()
    }

    /// Get total duration in seconds.
    pub fn duration(&self) -> f64 {
        self.reader.duration()
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
    async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
        let total_frames = self.total_frames();
        if self.current_frame() >= total_frames {
            tracing::debug!("End of IBT frames");
            return Ok(None);
        }

        let frame = match self.reader.read_next_frame()? {
            Some(frame) => frame,
            None => {
                tracing::debug!("No more frames from reader");
                return Ok(None);
            }
        };
        let tick = u32::try_from(frame.index()).map_err(|_| {
            IRacingSDKError::parse_error(
                "IBT provider",
                format!(
                    "Frame index {} cannot be represented as a tick",
                    frame.index()
                ),
            )
        })?;
        let session_version =
            u32::try_from(self.reader.header().session_info_update).map_err(|_| {
                IRacingSDKError::parse_error(
                    "IBT provider",
                    format!(
                        "Session version cannot be negative: {}",
                        self.reader.header().session_info_update
                    ),
                )
            })?;
        let frame_data: Vec<u8> = frame.into_buffer().into();

        let packet = FramePacket::new(frame_data, tick, session_version, self.shared_schema());

        tracing::trace!(
            "Frame {}/{}: tick={}, session_version={}",
            self.reader.current_frame(),
            total_frames,
            tick,
            session_version
        );

        Ok(Some(packet))
    }

    async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
        let Some(buffer) = self.reader.session_info_buffer()? else {
            return Ok(None);
        };
        let raw_yaml: String = buffer.try_into()?;
        if raw_yaml.trim().is_empty() {
            return Ok(None);
        }

        yaml_utils::preprocess_iracing_yaml(&raw_yaml).map(Some)
    }

    fn tick_rate(&self) -> f64 {
        self.tick_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::require_smallest_ibt_fixture;

    #[test]
    fn open_constructs_provider() -> Result<()> {
        let path = require_smallest_ibt_fixture()
            .expect("generated IBT fixture should be available for provider tests");

        let provider = IbtProvider::open(&path)?;

        assert_eq!(provider.current_frame(), 0);
        assert!(provider.total_frames() > 0);
        assert!(provider.schema().variable_count() > 0);
        Ok(())
    }

    #[test]
    fn from_reader_preserves_explicit_reader_position() -> Result<()> {
        let path = require_smallest_ibt_fixture()
            .expect("generated IBT fixture should be available for provider tests");
        let mut reader = IbtReader::open(path)?;
        reader.seek_to_frame(1)?;

        let provider = IbtProvider::from_reader(reader)?;

        assert_eq!(provider.current_frame(), 1);
        assert!(provider.total_frames() > 1);
        Ok(())
    }

    #[test]
    fn constructors_produce_equivalent_providers() -> Result<()> {
        let path = require_smallest_ibt_fixture()
            .expect("generated IBT fixture should be available for provider tests");

        let from_path = IbtProvider::open(&path)?;
        let with_reader = IbtProvider::from_reader(IbtReader::open(path)?)?;

        assert_eq!(from_path.current_frame(), with_reader.current_frame());
        assert_eq!(from_path.total_frames(), with_reader.total_frames());
        assert_eq!(
            from_path.schema().variable_count(),
            with_reader.schema().variable_count()
        );
        assert_eq!(
            from_path.schema().frame_size,
            with_reader.schema().frame_size
        );
        Ok(())
    }
}
