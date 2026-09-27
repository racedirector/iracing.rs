use crate::{ByteRegion, IRacingSDKError, Result};

use std::{
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom},
};

pub(crate) enum IbtSource {
    File(File),
    Memory(Cursor<Vec<u8>>),
}

// Keep the conversion separate so the address-space boundary can be tested
// without creating or allocating a multi-gigabyte source.
pub(super) fn layout_source_len(source_len: u64) -> Result<usize> {
    usize::try_from(source_len).map_err(|_| {
        IRacingSDKError::parse_error(
            "IBT source length",
            format!(
                "Source length {source_len} exceeds usize::MAX ({}) on this target",
                usize::MAX
            ),
        )
    })
}

impl IbtSource {
    pub(super) fn len(&self) -> Result<u64> {
        match self {
            Self::File(file) => file
                .metadata()
                .map(|metadata| metadata.len())
                .map_err(|error| {
                    IRacingSDKError::parse_error("IBT source length", error.to_string())
                }),
            Self::Memory(cursor) => u64::try_from(cursor.get_ref().len()).map_err(|_| {
                IRacingSDKError::parse_error("IBT source length", "Memory length exceeds u64")
            }),
        }
    }

    pub(super) fn seek_to_region_start(&mut self, region: ByteRegion) -> Result<u64> {
        let offset = u64::try_from(region.offset()).map_err(|_| {
            IRacingSDKError::parse_error("IBT region seek", "Region offset exceeds u64")
        })?;
        self.seek(SeekFrom::Start(offset)).map_err(|error| {
            IRacingSDKError::parse_error(
                "IBT region seek",
                format!("Failed to seek to {offset}: {error}"),
            )
        })
    }

    pub(super) fn read_region(&mut self, region: ByteRegion) -> Result<Vec<u8>> {
        let mut bytes = vec![0; region.len()];
        self.read_region_into(region, &mut bytes)?;
        Ok(bytes)
    }

    pub(super) fn read_region_into(
        &mut self,
        region: ByteRegion,
        destination: &mut [u8],
    ) -> Result<()> {
        if destination.len() != region.len() {
            return Err(IRacingSDKError::parse_error(
                "IBT region read",
                "Destination length differs from region length",
            ));
        }
        self.seek_to_region_start(region)?;
        if let Err(error) = self.read_exact(destination) {
            let _ = self.seek_to_region_start(region);
            return Err(IRacingSDKError::parse_error(
                "IBT region read",
                format!(
                    "Failed to read {} bytes at {}: {error}",
                    region.len(),
                    region.offset()
                ),
            ));
        }
        Ok(())
    }
}

impl Read for IbtSource {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::File(source) => source.read(buffer),
            Self::Memory(source) => source.read(buffer),
        }
    }
}

impl Seek for IbtSource {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        match self {
            Self::File(source) => source.seek(position),
            Self::Memory(source) => source.seek(position),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_length_representability_boundary() {
        let maximum = u64::try_from(usize::MAX).unwrap();
        assert_eq!(layout_source_len(maximum).unwrap(), usize::MAX);
        if let Some(too_large) = maximum.checked_add(1) {
            let error = layout_source_len(too_large).unwrap_err().to_string();
            assert!(error.contains(&too_large.to_string()));
            assert!(error.contains("exceeds usize::MAX"));
        }
        assert_eq!(
            layout_source_len(u64::from(u32::MAX)).unwrap(),
            u32::MAX as usize
        );
        assert_eq!(
            layout_source_len(u64::from(u32::MAX) + 1).is_ok(),
            usize::BITS > 32
        );
    }
}
