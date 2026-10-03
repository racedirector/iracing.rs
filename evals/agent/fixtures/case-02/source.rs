pub struct Region { start: usize, end: usize }
impl Region {
    pub fn new(start: usize, end: usize) -> Self { assert!(start <= end); Self {start, end} }
}
pub fn read(bytes: &[u8], region: Region) -> &[u8] {
    unsafe { bytes.get_unchecked(region.start..region.end) }
}
