mod ibt;
mod metadata;

pub(crate) use ibt::ParsedIbtHeader;
pub use {ibt::IbtLayout, metadata::MetadataRegions};

const IBT_PREAMBLE_SIZE: usize =
    size_of::<iracing_irsdk::Header>() + size_of::<iracing_irsdk::DiskSubHeader>();
