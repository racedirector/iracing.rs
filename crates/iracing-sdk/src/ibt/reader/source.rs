use memmap2::Mmap;
use std::ops::Range;
use zerocopy::IntoBytes;

pub(crate) enum IbtSource {
    Mapped(Mmap),
    Owned(Vec<u8>),
}

impl IbtSource {
    pub fn len(&self) -> usize {
        match self {
            Self::Mapped(source) => source.len(),
            Self::Owned(source) => source.len(),
        }
    }

    fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Mapped(source) => source.as_bytes(),
            Self::Owned(source) => source.as_bytes(),
        }
    }

    pub fn get(&self, range: Range<usize>) -> Option<&[u8]> {
        self.as_bytes().get(range)
    }
}

#[cfg(test)]
mod tests {
    use super::IbtSource;

    #[test]
    fn owned_reads_check_destination_and_available_bytes() {
        let source = IbtSource::Owned(vec![1, 2, 3, 4]);
        assert_eq!(source.get(1..3), Some([2, 3].as_slice()));
        assert_eq!(source.get(3..5), None);
        assert_eq!(source.get(1..3), Some([2, 3].as_slice()));
    }

    #[test]
    fn checked_ranges_enforce_source_bounds() {
        let bytes = vec![10, 20, 30, 40];
        let mut mapping = memmap2::MmapMut::map_anon(bytes.len()).unwrap();
        mapping.copy_from_slice(&bytes);
        for source in [
            IbtSource::Owned(bytes),
            IbtSource::Mapped(mapping.make_read_only().unwrap()),
        ] {
            assert_eq!(source.len(), 4);
            assert_eq!(source.get(0..4), Some([10, 20, 30, 40].as_slice()));
            assert_eq!(source.get(1..3), Some([20, 30].as_slice()));
            assert_eq!(source.get(4..4), Some([].as_slice()));
            assert_eq!(source.get(0..5), None);
            assert_eq!(source.get(5..5), None);
            let start = 3;
            assert_eq!(source.get(start..2), None);
            assert_eq!(source.get(0..usize::MAX), None);
            assert_eq!(source.get(0..1), Some([10].as_slice()));
        }
    }

    #[test]
    fn empty_owned_source_has_only_an_empty_range() {
        let source = IbtSource::Owned(Vec::new());
        assert_eq!(source.len(), 0);
        assert_eq!(source.get(0..0), Some([].as_slice()));
        assert_eq!(source.get(0..1), None);
    }
}
