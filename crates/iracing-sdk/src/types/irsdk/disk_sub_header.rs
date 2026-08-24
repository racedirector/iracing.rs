use type_layout::TypeLayout;

use crate::types::irsdk::wire_type::WireType;

/// IBT disk sub-header (IBT-specific structure, `irsdk_diskSubHeader`).
///
/// Stored just before the variable header array (at
/// `header.var_header_offset - IRSDK_DISK_SUBHEADER_SIZE`) and provides timing and record-count
/// metadata specific to `.ibt` replay files.
#[repr(C)]
#[derive(Debug, Clone, Copy, TypeLayout)]
pub struct DiskSubHeader {
    /// Unix timestamp (`time_t`) of the session start date.
    pub start_date: i64,
    /// Session start time in seconds since session midnight.
    pub start_time: f64,
    /// Session end time in seconds since session midnight.
    pub end_time: f64,
    /// Number of laps completed during the recorded session.
    pub lap_count: i32,
    /// Total number of telemetry frames (records) in the file.
    pub record_count: i32,
}

unsafe impl WireType for DiskSubHeader {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of};

    #[test]
    fn disk_sub_header_layout_matches_iracing_abi() {
        assert_eq!(DiskSubHeader::WIRE_SIZE, 32);
        assert_eq!(align_of::<DiskSubHeader>(), 8);

        assert_eq!(offset_of!(DiskSubHeader, start_date), 0);
        assert_eq!(offset_of!(DiskSubHeader, start_time), 8);
        assert_eq!(offset_of!(DiskSubHeader, end_time), 16);
        assert_eq!(offset_of!(DiskSubHeader, lap_count), 24);
        assert_eq!(offset_of!(DiskSubHeader, record_count), 28);
    }
}
