//! Provider for IBT files.

use std::{path::Path, sync::Arc};

use crate::{
    FramePacket, Result, SchemaProvider, VariableSchema, ibt::IbtReader, provider::Provider,
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
        Ok(Self::from_reader(IbtReader::open(path)?))
    }

    /// Create a replay provider from an already-opened reader.
    pub fn from_reader(reader: IbtReader) -> Self {
        let tick_rate = reader.tick_rate();
        let schema = Arc::new(reader.schema().clone());
        Self {
            reader,
            schema,
            tick_rate,
        }
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

        let (frame_data, tick, session_version) = match self.reader.read_next_frame()? {
            Some(data) => data,
            None => {
                tracing::debug!("No more frames from reader");
                return Ok(None);
            }
        };

        let packet = FramePacket::new(frame_data, tick, session_version, Arc::clone(&self.schema));

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
        self.reader.session_yaml()
    }

    fn tick_rate(&self) -> f64 {
        self.tick_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_utils::require_smallest_ibt_fixture;

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

        let provider = IbtProvider::from_reader(reader);

        assert_eq!(provider.current_frame(), 1);
        assert!(provider.total_frames() > 1);
        Ok(())
    }

    #[test]
    fn constructors_produce_equivalent_providers() -> Result<()> {
        let path = require_smallest_ibt_fixture()
            .expect("generated IBT fixture should be available for provider tests");

        let from_path = IbtProvider::open(&path)?;
        let with_reader = IbtProvider::from_reader(IbtReader::open(path)?);

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
