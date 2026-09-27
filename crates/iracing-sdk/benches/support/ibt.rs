//! Storage-neutral IBT benchmark workload metadata and cache preparation.

use iracing_sdk::irsdk::{DiskSubHeader, Header, VariableHeader};
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

        let positive = |value: i32, field: &str| -> usize {
            usize::try_from(value)
                .unwrap_or_else(|_| panic!("invalid {field} in {}", path.display()))
        };
        let frame_size = positive(header.buffer_length, "frame size");
        assert!(frame_size > 0, "zero frame size in {}", path.display());
        let variable_end = positive(header.variable_header_offset, "variable offset")
            .checked_add(
                positive(header.variable_count, "variable count")
                    .checked_mul(size_of::<VariableHeader>())
                    .expect("fixture variable region length overflow"),
            )
            .expect("fixture variable region end overflow");
        let session_end = positive(header.session_info_offset, "session offset")
            .checked_add(positive(header.session_info_length, "session length"))
            .expect("fixture session region end overflow");
        let frame_data_start = u64::try_from(preamble.len().max(variable_end).max(session_end))
            .expect("fixture frame data offset does not fit u64");
        let remaining = file_size
            .checked_sub(frame_data_start)
            .expect("fixture metadata extends past EOF");
        let frame_size_u64 = u64::try_from(frame_size).expect("frame size does not fit u64");
        assert_eq!(
            remaining % frame_size_u64,
            0,
            "partial final frame in {}",
            path.display()
        );
        let frame_count = usize::try_from(remaining / frame_size_u64)
            .expect("fixture frame count does not fit usize");
        if disk_header.record_count > 0 {
            assert_eq!(
                usize::try_from(disk_header.record_count).expect("invalid record count"),
                frame_count,
                "fixture disk record count differs from EOF-derived count"
            );
        }
        let replay_bytes = frame_size_u64
            .checked_mul(u64::try_from(frame_count).expect("frame count does not fit u64"))
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
