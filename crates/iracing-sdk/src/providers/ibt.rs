use crate::{FramePacket, IbtReader, Provider, Result, VariableSchema};
use std::{path::Path, sync::Arc};

/// A [`Provider`] that streams telemetry frames from an iRacing `.ibt` file.
pub struct IbtProvider {
    pub(super) reader: IbtReader,
    schema: Arc<VariableSchema>,
}

impl IbtProvider {
    /// Opens an `.ibt` file at `path` and constructs an `IbtProvider`.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let reader = IbtReader::open(path)?;
        Self::with_reader(reader)
    }

    /// Constructs an `IbtProvider` from an already-opened [`IbtReader`].
    pub fn with_reader(reader: IbtReader) -> Result<Self> {
        let schema = Arc::new(reader.variables().clone());
        Ok(Self { reader, schema })
    }

    /// Returns a shared reference to the telemetry variable schema.
    pub fn schema(&self) -> Arc<VariableSchema> {
        Arc::clone(&self.schema)
    }

    /// Returns the index of the next frame that will be read (0-based).
    pub fn current_frame(&self) -> usize {
        self.reader.current_frame()
    }

    /// Returns the total number of telemetry frames in the file.
    pub fn total_frames(&self) -> usize {
        self.reader.total_frames()
    }
}

#[async_trait::async_trait(?Send)]
impl Provider for IbtProvider {
    async fn next_frame(&mut self) -> Result<Option<crate::FramePacket>> {
        let total_frames = self.reader.total_frames();
        if self.reader.current_frame() >= total_frames {
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
        // Get cleaned YAML from IBT file
        // IBT files have static session info, version parameter is ignored
        self.reader.session_yaml()
    }

    fn tick_rate(&self) -> f64 {
        self.reader.tick_rate()
    }
}
