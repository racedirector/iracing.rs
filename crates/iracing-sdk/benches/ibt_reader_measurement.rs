//! Retained-heap and throughput diagnostic for file-backed IBT replay.
//!
//! Run with:
//!
//! ```text
//! cargo bench -p iracing-sdk --features benchmark --bench ibt-reader-memory
//! ```
//!
//! The `file` mode uses [`IbtReader::open`]. The `memory_baseline` mode models
//! the pre-refactor ownership behavior with `std::fs::read` followed by
//! [`IbtReader::from_bytes`]. It is benchmark-only comparison code, not a
//! second production reader.
//!
//! Retained and peak bytes are application-heap allocations observed through
//! the global allocator. They exclude kernel/filesystem page cache and include
//! any allocator-visible state retained by reader construction. The file mode
//! retains a file handle and the validated fixed layout without a source-sized
//! heap buffer; the memory baseline retains the complete source buffer. Timing
//! is sensitive to filesystem cache and machine load; compare results only on
//! the same machine and revision.

use anyhow::{Context, Result, ensure};
use iracing_sdk::ibt::IbtReader;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    fs,
    hint::black_box,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant},
};

struct TrackingAllocator;

static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static PEAK_BYTES: AtomicU64 = AtomicU64::new(0);
static TRACK_PEAK: AtomicBool = AtomicBool::new(false);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The allocation request is forwarded unchanged.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record_deallocation(layout.size());
        // SAFETY: The pointer and layout originated from the system allocator.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The allocation request is forwarded unchanged.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: The pointer and layout originated from the system allocator.
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if !replacement.is_null() {
            record_deallocation(layout.size());
            record_allocation(new_size);
        }
        replacement
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

fn record_allocation(bytes: usize) {
    let live = LIVE_BYTES.fetch_add(bytes as u64, Ordering::Relaxed) + bytes as u64;
    if TRACK_PEAK.load(Ordering::Relaxed) {
        PEAK_BYTES.fetch_max(live, Ordering::Relaxed);
    }
}

fn record_deallocation(bytes: usize) {
    LIVE_BYTES.fetch_sub(bytes as u64, Ordering::Relaxed);
}

struct Measurement {
    mode: &'static str,
    path: PathBuf,
    file_size: u64,
    frame_size: usize,
    frame_count: usize,
    open_time: Duration,
    retained_heap: u64,
    peak_open_heap: u64,
    replay_time: Duration,
    replay_bytes: u64,
}

impl Measurement {
    fn mib_per_second(&self) -> f64 {
        self.replay_bytes as f64 / 1_048_576.0 / self.replay_time.as_secs_f64()
    }

    fn report(&self) {
        println!(
            "ibt_reader mode={} file={} file_bytes={} frame_size={} frames={} open_ms={:.3} retained_heap_bytes={} peak_open_heap_bytes={} replay_ms={:.3} replay_mib_per_s={:.1}",
            self.mode,
            self.path.display(),
            self.file_size,
            self.frame_size,
            self.frame_count,
            self.open_time.as_secs_f64() * 1_000.0,
            self.retained_heap,
            self.peak_open_heap,
            self.replay_time.as_secs_f64() * 1_000.0,
            self.mib_per_second(),
        );
    }
}

fn measure(
    mode: &'static str,
    path: &Path,
    open: impl FnOnce() -> Result<IbtReader>,
) -> Result<Measurement> {
    let file_size = fs::metadata(path)?.len();
    let before = LIVE_BYTES.load(Ordering::SeqCst);
    PEAK_BYTES.store(before, Ordering::SeqCst);
    TRACK_PEAK.store(true, Ordering::SeqCst);
    let open_started = Instant::now();
    let mut reader = open()?;
    let open_time = open_started.elapsed();
    TRACK_PEAK.store(false, Ordering::SeqCst);
    let retained_heap = LIVE_BYTES.load(Ordering::SeqCst).saturating_sub(before);
    let peak_open_heap = PEAK_BYTES.load(Ordering::SeqCst).saturating_sub(before);
    let frame_size = reader.layout().frame_size();
    let frame_count = reader.layout().frame_count();

    let replay_started = Instant::now();
    let mut replay_bytes = 0u64;
    for frame_index in 0..frame_count {
        let frame = reader.frame(frame_index)?;
        replay_bytes = replay_bytes
            .checked_add(u64::try_from(frame.len())?)
            .context("replay byte count overflowed")?;
        black_box((frame, frame_index));
    }
    let replay_time = replay_started.elapsed();

    Ok(Measurement {
        mode,
        path: path.to_path_buf(),
        file_size,
        frame_size,
        frame_count,
        open_time,
        retained_heap,
        peak_open_heap,
        replay_time,
        replay_bytes,
    })
}

fn representative_recordings() -> Result<[PathBuf; 2]> {
    let test_data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-data");
    let mut recordings = Vec::new();
    for directory in [test_data.clone(), test_data.join("ibt")] {
        for entry in fs::read_dir(&directory)
            .with_context(|| format!("reading fixture directory {}", directory.display()))?
        {
            let path = entry?.path();
            if path.extension().is_some_and(|extension| extension == "ibt") {
                let size = fs::metadata(&path)?.len();
                if size >= 1_048_576 {
                    recordings.push((size, path));
                }
            }
        }
    }
    recordings.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    ensure!(
        recordings.len() >= 2,
        "memory measurement requires at least two IBT recordings of 1 MiB or larger under test-data"
    );

    let smallest = recordings.first().expect("length checked").1.clone();
    let largest = recordings.last().expect("length checked").1.clone();
    Ok([smallest, largest])
}

fn main() -> Result<()> {
    for path in representative_recordings()? {
        let file = measure("file", &path, || Ok(IbtReader::open(&path)?))?;
        file.report();
        ensure!(
            file.retained_heap < file.file_size / 2,
            "file-backed reader retained unexpectedly recording-proportional heap"
        );

        let memory = measure("memory_baseline", &path, || {
            Ok(IbtReader::from_bytes(fs::read(&path)?)?)
        })?;
        memory.report();
        ensure!(
            memory.retained_heap >= memory.file_size,
            "memory baseline did not retain at least the recording byte length"
        );
    }
    Ok(())
}
