//! Temporary controls compiled against main 943f06d, where Reader and File coexist.
//! This binary never changes the existing Criterion workloads or their boundaries.
//! Drop controls construct outside timing; borrowed reads destroy files after timing.

use iracing_sdk::{
    IbtFile, SessionInfoBytes, TelemetryLayout, VariableHeaders,
    ibt::IbtReader,
    provider::{SessionInformationBytesProvider, VariableHeadersProvider},
};
use std::{
    hint::black_box,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

const POSITIONS: usize = 1_024;
const ROUNDS: usize = 12;
const DROP_SAMPLES: usize = 128;
const READ_LOOPS: usize = 1_000;

struct CompleteReader {
    _reader: IbtReader,
    _headers: VariableHeaders,
    _session: Option<SessionInfoBytes>,
    _layout: Arc<TelemetryLayout>,
}

impl CompleteReader {
    fn open(path: &Path) -> iracing_sdk::Result<Self> {
        let reader = IbtReader::open(path)?;
        let headers = VariableHeadersProvider::variable_headers(&reader)?;
        let session = SessionInformationBytesProvider::session_info_snapshot(&reader)?;
        let layout = Arc::new(TelemetryLayout::try_from_headers(
            &headers,
            reader.frame_size(),
        )?);
        Ok(Self {
            _reader: reader,
            _headers: headers,
            _session: session,
            _layout: layout,
        })
    }
}

fn timed_drop<T>(mut values: Vec<T>) -> f64 {
    let count = values.len();
    let started = Instant::now();
    // Pop leaves the batch Vec allocation alive until after elapsed is captured.
    while let Some(value) = values.pop() {
        drop(black_box(value));
    }
    started.elapsed().as_secs_f64() * 1_000_000.0 / count as f64
}

fn timed_reads<T>(mut frame: impl FnMut(usize) -> T, positions: &[usize], loops: usize) -> f64 {
    let started = Instant::now();
    for _ in 0..loops {
        for &index in positions {
            drop(black_box(frame(black_box(index))));
        }
    }
    started.elapsed().as_secs_f64() * 1_000_000_000.0 / (loops * positions.len()) as f64
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len() & 1 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

fn main() -> iracing_sdk::Result<()> {
    let reverse = std::env::args().any(|arg| arg == "--reverse");
    let smoke = std::env::args().any(|arg| arg == "--test");
    let rounds = if smoke { 2 } else { ROUNDS };
    let drop_samples = if smoke { 2 } else { DROP_SAMPLES };
    let read_loops = if smoke { 2 } else { READ_LOOPS };
    println!(
        "configuration,rounds={rounds},drop_samples={drop_samples},read_loops={read_loops},positions={POSITIONS},reverse={reverse}"
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (fixture, path) in [
        (
            "small_5_9mb",
            "test-data/fordmustanggt4_donington national 2026-04-09 21-57-46.ibt",
        ),
        ("large_142_6mb", "test-data/ibt/b_mustang_bristol_race.ibt"),
    ] {
        let path = root.join(path);
        let mut prewarm = std::fs::File::open(&path).unwrap();
        std::io::copy(&mut prewarm, &mut std::io::sink()).unwrap();
        drop(prewarm);
        let reader = IbtReader::open(&path)?;
        let file = IbtFile::open(&path)?;
        assert_eq!(reader.frame_count(), file.frame_count());
        let positions: Vec<_> = (0..POSITIONS)
            .map(|index| {
                index.wrapping_mul(1_103_515_245).wrapping_add(12_345) % file.frame_count()
            })
            .collect();
        for &index in &positions {
            assert_eq!(reader.frame(index)?.as_slice(), file.frame(index)?.bytes());
        }
        println!(
            "metadata,{fixture},frames={},frame_bytes={},fields={}",
            file.frame_count(),
            file.frame_size(),
            file.telemetry_layout().len()
        );
        // Warm both actual paths while keeping both maps alive for every timed round.
        black_box(timed_reads(
            |index| reader.frame(index).unwrap(),
            &positions,
            10,
        ));
        black_box(timed_reads(
            |index| file.frame(index).unwrap(),
            &positions,
            10,
        ));
        let mut read_observed = Vec::new();
        for round in 0..rounds {
            let order = if (round % 2 == 0) ^ reverse {
                [0, 1]
            } else {
                [1, 0]
            };
            let mut values = [0.0; 2];
            for (order_index, case) in order.into_iter().enumerate() {
                let (name, nanos) = if case == 0 {
                    (
                        "partial_reader",
                        timed_reads(|index| reader.frame(index).unwrap(), &positions, read_loops),
                    )
                } else {
                    (
                        "canonical_file",
                        timed_reads(|index| file.frame(index).unwrap(), &positions, read_loops),
                    )
                };
                values[case] = nanos;
                println!("raw_read_ns,{fixture},{round},{order_index},{name},{nanos:.3}");
            }
            read_observed.push(values);
        }
        let mut read_delta: Vec<_> = read_observed
            .iter()
            .map(|times| times[1] - times[0])
            .collect();
        println!(
            "paired_median_read_delta_ns,{fixture},{:.3}",
            median(&mut read_delta)
        );
        for (case, name) in ["partial_reader", "canonical_file"].into_iter().enumerate() {
            let mut values: Vec<_> = read_observed.iter().map(|times| times[case]).collect();
            println!("median_read_ns,{fixture},{name},{:.3}", median(&mut values));
        }
        // The retained read-control files drop here, explicitly outside their timers.
        drop(reader);
        drop(file);
        let mut drop_observed = Vec::new();
        for round in 0..rounds {
            let mut order = [0, 1, 2];
            order.rotate_left(round % 3);
            if ((round / 3) % 2 == 1) ^ reverse {
                order.reverse();
            }
            let mut values = [0.0; 3];
            for (order_index, case) in order.into_iter().enumerate() {
                // All mmap/header/metadata/layout construction occurs before timed_drop.
                let (name, micros) = match case {
                    0 => (
                        "partial_reader",
                        timed_drop(
                            (0..drop_samples)
                                .map(|_| IbtReader::open(&path).unwrap())
                                .collect(),
                        ),
                    ),
                    1 => (
                        "complete_reader",
                        timed_drop(
                            (0..drop_samples)
                                .map(|_| CompleteReader::open(&path).unwrap())
                                .collect(),
                        ),
                    ),
                    _ => (
                        "canonical_file",
                        timed_drop(
                            (0..drop_samples)
                                .map(|_| IbtFile::open(&path).unwrap())
                                .collect(),
                        ),
                    ),
                };
                values[case] = micros;
                println!("raw_drop_us,{fixture},{round},{order_index},{name},{micros:.3}");
            }
            drop_observed.push(values);
        }
        for (case, name) in ["partial_reader", "complete_reader", "canonical_file"]
            .into_iter()
            .enumerate()
        {
            let mut values: Vec<_> = drop_observed.iter().map(|times| times[case]).collect();
            println!("median_drop_us,{fixture},{name},{:.3}", median(&mut values));
        }
        for (name, baseline) in [
            ("canonical_over_partial", 0),
            ("canonical_over_complete", 1),
        ] {
            let mut deltas: Vec<_> = drop_observed
                .iter()
                .map(|times| times[2] - times[baseline])
                .collect();
            println!(
                "paired_median_drop_delta_us,{fixture},{name},{:.3}",
                median(&mut deltas)
            );
        }
    }
    Ok(())
}
