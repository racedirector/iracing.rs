//! Provider for IBT files.

use std::{path::Path, sync::Arc};

use crate::{
    FramePacket, IRacingSDKError, IRacingSessionString, Result, SchemaProvider, VariableSchema,
    ibt::IbtReader, provider::Provider,
};

/// A [`Provider`] that streams telemetry frames from an iRacing `.ibt` replay file.
pub struct IbtProvider {
    reader: IbtReader,
    schema: Arc<VariableSchema>,
    tick_rate: f64,
    next_frame: usize,
}

impl IbtProvider {
    /// Open an `.ibt` file as a replay provider.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let reader = IbtReader::open(path)?;

        Self::from_reader(reader)
    }

    /// Create a replay provider from an already-opened reader.
    ///
    /// A file with no variable-header region is accepted only when it also
    /// contains no telemetry frames. In that case, the provider retains the
    /// advertised frame size in an empty schema so the file's validated layout
    /// remains accurately represented.
    pub fn from_reader(mut reader: IbtReader) -> Result<Self> {
        let frame_size = reader.layout().frame_size();
        let frame_count = reader.layout().frame_count();

        let schema = match reader.variable_headers_snapshot()? {
            Some(snapshot) => VariableSchema::from_snapshot(snapshot, frame_size)?,
            None if frame_count == 0 => VariableSchema::new(Default::default(), frame_size)?,
            None => {
                return Err(IRacingSDKError::parse_error(
                    "IbtProvider::from_reader",
                    format!(
                        "IBT file contains {frame_count} telemetry frames but does not advertise variable headers"
                    ),
                ));
            }
        };

        let tick_rate = f64::from(reader.header().tick_rate);

        Ok(Self {
            reader,
            schema: Arc::new(schema),
            tick_rate,
            next_frame: 0,
        })
    }

    /// Returns an ownable schema.
    pub(crate) fn shared_schema(&self) -> Arc<VariableSchema> {
        Arc::clone(&self.schema)
    }

    /// The total number of frames within the IBT file.
    pub fn total_frames(&self) -> usize {
        self.reader.layout().frame_count()
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
        let frame_index = self.next_frame;
        let total_frames = self.reader.layout().frame_count();

        // Check for EOF before asking for the next frame from the reader; the
        // reader will raise an EOF error if asked for a frame that is out of
        // range.
        if frame_index >= total_frames {
            tracing::debug!("End of IBT frames");
            return Ok(None);
        }

        let tick = u32::try_from(frame_index).map_err(|_| {
            IRacingSDKError::parse_error(
                "IbtProvider::next_frame",
                format!("Frame index {frame_index} exceeds the u32 tick range"),
            )
        })?;

        let session_version = self.reader.header().session_info_update as u32;

        let frame_data = self.reader.frame(frame_index)?;

        let packet = FramePacket::new(frame_data, tick, session_version, self.shared_schema());

        // Advance after packet is create
        self.next_frame += 1;

        tracing::trace!(
            "Frame {}/{}: tick={}, session_version={}",
            self.next_frame,
            total_frames,
            tick,
            session_version
        );

        Ok(Some(packet))
    }

    async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
        let Some(snapshot) = self.reader.session_info_snapshot()? else {
            return Ok(None);
        };

        let session = IRacingSessionString::try_from(snapshot)?;
        Ok(Some(session.into()))
    }

    fn tick_rate(&self) -> f64 {
        self.tick_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        irsdk::{DiskSubHeader, Header},
        test_utils::require_smallest_ibt_fixture,
    };
    use anyhow::Result;
    use std::mem::{offset_of, size_of};

    fn fixture_bytes() -> Result<Vec<u8>> {
        let path = require_smallest_ibt_fixture()
            .expect("generated IBT fixture should be available for provider tests");
        Ok(std::fs::read(path)?)
    }

    fn read_i32(bytes: &[u8], offset: usize) -> Result<i32> {
        Ok(i32::from_le_bytes(
            bytes[offset..offset + size_of::<i32>()].try_into()?,
        ))
    }

    fn write_i32(bytes: &mut [u8], offset: usize, value: i32) {
        bytes[offset..offset + size_of::<i32>()].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn open_constructs_provider() -> Result<()> {
        let path = require_smallest_ibt_fixture()
            .expect("generated IBT fixture should be available for provider tests");

        let provider = IbtProvider::open(&path)?;

        assert_eq!(provider.next_frame, 0);
        assert!(provider.reader.layout().frame_count() > 0);
        assert!(provider.schema().variable_count() > 0);
        Ok(())
    }

    #[test]
    fn from_reader_starts_at_first_frame() -> Result<()> {
        let path = require_smallest_ibt_fixture()
            .expect("generated IBT fixture should be available for provider tests");
        let reader = IbtReader::open(path)?;

        let provider = IbtProvider::from_reader(reader)?;

        assert_eq!(provider.next_frame, 0);
        assert!(provider.reader.layout().frame_count() > 1);
        Ok(())
    }

    #[test]
    fn constructors_produce_equivalent_providers() -> Result<()> {
        let path = require_smallest_ibt_fixture()
            .expect("generated IBT fixture should be available for provider tests");

        let from_path = IbtProvider::open(&path)?;
        let with_reader = IbtProvider::from_reader(IbtReader::open(path)?)?;

        assert_eq!(from_path.next_frame, with_reader.next_frame);
        assert_eq!(
            from_path.reader.layout().frame_count(),
            with_reader.reader.layout().frame_count()
        );
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

    #[tokio::test]
    async fn from_reader_accepts_no_variable_headers_when_there_are_no_frames() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        let session_info_length =
            usize::try_from(read_i32(&bytes, offset_of!(Header, session_info_length))?)?;
        let session_info_offset =
            usize::try_from(read_i32(&bytes, offset_of!(Header, session_info_offset))?)?;
        let metadata_end = session_info_offset
            .checked_add(session_info_length)
            .expect("fixture metadata extent should not overflow");

        write_i32(&mut bytes, offset_of!(Header, variable_count), 0);
        write_i32(
            &mut bytes,
            size_of::<Header>() + offset_of!(DiskSubHeader, record_count),
            0,
        );
        bytes.truncate(metadata_end);

        let reader = IbtReader::from_bytes(bytes)?;
        let advertised_frame_size = reader.layout().frame_size();
        assert_eq!(reader.layout().frame_count(), 0);
        assert!(reader.layout().metadata().variable_headers().is_none());

        let mut provider = IbtProvider::from_reader(reader)?;

        assert_eq!(provider.schema().variable_count(), 0);
        assert_eq!(provider.schema().frame_size, advertised_frame_size);
        assert_eq!(provider.reader.layout().frame_count(), 0);
        assert!(provider.session_yaml(0).await?.is_some());
        assert!(provider.next_frame().await?.is_none());
        Ok(())
    }

    #[test]
    fn from_reader_rejects_frames_without_variable_headers() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        write_i32(&mut bytes, offset_of!(Header, variable_count), 0);

        let reader = IbtReader::from_bytes(bytes)?;
        let frame_count = reader.layout().frame_count();
        assert!(frame_count > 0);
        assert!(reader.layout().metadata().variable_headers().is_none());

        let error = match IbtProvider::from_reader(reader) {
            Ok(_) => panic!("frames without a variable schema should be rejected"),
            Err(error) => error,
        };

        match error {
            IRacingSDKError::Parse { context, details } => {
                assert_eq!(context, "IbtProvider::from_reader");
                assert!(details.contains(&frame_count.to_string()));
                assert!(details.contains("does not advertise variable headers"));
            }
            error => panic!("expected a parse error, got {error:?}"),
        }

        Ok(())
    }
}
