mod header;
mod layout;

pub(crate) use header::ParsedIbtHeader;
pub use layout::IbtLayout;

pub(crate) const IBT_PREAMBLE_SIZE: usize =
    size_of::<iracing_irsdk::Header>() + size_of::<iracing_irsdk::DiskSubHeader>();
