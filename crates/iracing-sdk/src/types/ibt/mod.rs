use super::regions::SessionInfoRegion;
use crate::{
    DiskSubHeader, Header, IRacingSDKError, Result, VariableHeader, VariableInfo, VariableSchema,
    irsdk::WireType, types::regions::VariableHeaderRegion,
};
use std::{collections::HashMap, io::Cursor, path::Path};

pub struct IbtFile {
    data: Vec<u8>,
    header: Header,
    disk_header: DiskSubHeader,
    schema: VariableSchema,

    session_region: Option<SessionInfoRegion>,
    variable_header_region: VariableHeaderRegion,

    frame_data_start: usize,
    frame_data_size: usize,
    frame_count: usize,
}

impl IbtFile {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let data =
            std::fs::read(&path).map_err(|source| IRacingSDKError::file_error(path, source))?;

        Self::from_bytes(data)
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let mut cursor = Cursor::new(data.as_slice());

        // Parse IBT header
        let header = Header::try_from_reader(&mut cursor)?;
        header.validate_ibt()?;

        // Parse disk sub-header (note: may be corrupted, but we'll try)
        let disk_header = DiskSubHeader::try_from_reader(&mut cursor)?;

        let variable_header_region = VariableHeaderRegion::try_from(&header)?;
        let variable_header_range = variable_header_region.checked_range(data.len())?;

        let session_region = if header.session_info_len == 0 {
            None
        } else {
            let region = SessionInfoRegion::try_from(&header)?;
            region.checked_range(data.len())?;
            Some(region)
        };

        Ok(Self {
            data,
            header,
            disk_header,
            schema,
            session_region,
            variable_header_region,
            frame_data_start,
            frame_data_size,
            frame_count,
        })
    }
}

impl IbtFile {}
