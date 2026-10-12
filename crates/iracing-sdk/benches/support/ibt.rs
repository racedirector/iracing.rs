//! Storage-neutral IBT benchmark workload metadata and cache preparation.
//!
//! Workload geometry is derived from the wire preamble and physical file length,
//! independently of the reader being measured. Cache preparation uses plain
//! sequential file I/O outside timing. Results therefore describe warm-cache
//! access, not cold-storage latency; the helper does not establish production
//! parser acceptance or certify an arbitrary recording.

use iracing_sdk::IbtLayout;
use iracing_sdk::irsdk::{DiskSubHeader, Header};
use std::{
    fs::{self, File},
    io::{BufReader, Read},
    mem::size_of,
    path::PathBuf,
};

pub struct Recording {
    pub name: &'static str,
    pub path: PathBuf,
    pub file_size: u64,
    pub frame_size: usize,
    pub frame_count: usize,
    pub replay_bytes: u64,
}

impl Recording {
    pub fn load(name: &'static str, relative_path: &str) -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative_path);
        let file_size = fs::metadata(&path)
            .unwrap_or_else(|error| panic!("could not stat {}: {error}", path.display()))
            .len();
        let file = File::open(&path)
            .unwrap_or_else(|error| panic!("could not open {}: {error}", path.display()));
        let mut reader = BufReader::new(file);
        let mut preamble = [0; size_of::<Header>() + size_of::<DiskSubHeader>()];
        reader
            .read_exact(&mut preamble)
            .unwrap_or_else(|error| panic!("could not read {} preamble: {error}", path.display()));
        let header = Header::try_from_bytes(&preamble[..size_of::<Header>()])
            .expect("fixture SDK header must decode");
        let disk_header = DiskSubHeader::try_from_bytes(&preamble[size_of::<Header>()..])
            .expect("fixture disk header must decode");

        let layout = IbtLayout::try_from_headers(
            &header,
            usize::try_from(file_size).expect("fixture length must fit usize"),
        )
        .expect("fixture physical layout must validate");
        let frame_size = layout.frame_size();
        let frame_count = layout.frame_count();
        if disk_header.record_count > 0 {
            assert_eq!(
                usize::try_from(disk_header.record_count)
                    .expect("fixture record count must fit usize"),
                frame_count,
                "fixture disk record count differs from EOF-derived count"
            );
        }
        let replay_bytes = (frame_size as u64)
            .checked_mul(frame_count as u64)
            .expect("fixture replay byte count overflow");
        Self {
            name,
            path,
            file_size,
            frame_size,
            frame_count,
            replay_bytes,
        }
    }

    pub fn prewarm(&self) {
        let mut file = File::open(&self.path).unwrap_or_else(|error| {
            panic!(
                "could not open {} for prewarm: {error}",
                self.path.display()
            )
        });
        std::io::copy(&mut file, &mut std::io::sink())
            .unwrap_or_else(|error| panic!("could not prewarm {}: {error}", self.path.display()));
    }
}
