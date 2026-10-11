//! Temporary ownership attribution control; never replaces the existing IBT experiments.
//!
//! All modes copy the same cached representative frame from each existing recording.
//! They retain their entire owned result through black_box and include its destruction.
//! There are no file reads, metadata construction, YAML parsing, adapter work, or
//! changed fixtures inside the timed loops. Case order rotates and reverses each
//! three rounds; paired differences and raw observations are reported separately.
//! This diagnostic isolates ownership costs, not complete File::frame performance.

use iracing_sdk::{IbtFile, TelemetryLayout};
use std::{hint::black_box, path::PathBuf, sync::Arc, time::Instant};

#[derive(Clone, Copy)]
enum Case {
    VecPayload,
    ArcPayload,
    ArcPayloadAndLayout,
}

impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::VecPayload => "vec_payload",
            Self::ArcPayload => "arc_payload",
            Self::ArcPayloadAndLayout => "arc_payload_exact_layout",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::VecPayload => 0,
            Self::ArcPayload => 1,
            Self::ArcPayloadAndLayout => 2,
        }
    }
}

fn measure(case: Case, payload: &[u8], layout: &Arc<TelemetryLayout>, iterations: u32) -> f64 {
    let started = Instant::now();
    // Match once outside the loops so case dispatch does not add per-record work.
    match case {
        Case::VecPayload => {
            for _ in 0..iterations {
                let owned = black_box(payload).to_vec();
                drop(black_box(owned));
            }
        }
        Case::ArcPayload => {
            for _ in 0..iterations {
                let owned = Arc::<[u8]>::from(black_box(payload));
                drop(black_box(owned));
            }
        }
        Case::ArcPayloadAndLayout => {
            for _ in 0..iterations {
                let owned = (
                    Arc::<[u8]>::from(black_box(payload)),
                    Arc::clone(black_box(layout)),
                );
                drop(black_box(owned));
            }
        }
    }
    started.elapsed().as_secs_f64() * 1_000_000_000.0 / f64::from(iterations)
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

fn main() -> iracing_sdk::Result<()> {
    // --test supplies a bounded smoke mode without changing diagnostic defaults.
    let smoke = std::env::args().any(|arg| arg == "--test");
    let iterations = if smoke { 1_000 } else { 1_000_000 };
    let rounds = if smoke { 2 } else { 12 };
    println!(
        "ownership diagnostic: iterations={iterations}, rounds={rounds}; ns/owned copy including drop"
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, path) in [
        (
            "small_5_9mb",
            "test-data/fordmustanggt4_donington national 2026-04-09 21-57-46.ibt",
        ),
        ("large_142_6mb", "test-data/ibt/b_mustang_bristol_race.ibt"),
    ] {
        let file = IbtFile::open(root.join(path))?;
        let source = file.frame(file.frame_count() / 2)?;
        assert!(Arc::ptr_eq(source.layout(), file.telemetry_layout()));
        let payload = source.bytes();
        let layout = source.layout();
        println!(
            "fixture={name}, representative_frame={}, payload_bytes={}",
            source.index(),
            payload.len()
        );
        for case in [
            Case::VecPayload,
            Case::ArcPayload,
            Case::ArcPayloadAndLayout,
        ] {
            black_box(measure(case, payload, layout, 10_000));
        }
        let mut observed: Vec<[f64; 3]> = Vec::new();
        for round in 0..rounds {
            let mut order = [
                Case::VecPayload,
                Case::ArcPayload,
                Case::ArcPayloadAndLayout,
            ];
            order.rotate_left(round % 3);
            if (round / 3) % 2 == 1 {
                order.reverse();
            }
            let mut times = [0.0; 3];
            for (position, case) in order.into_iter().enumerate() {
                let nanos = measure(case, payload, layout, iterations);
                times[case.index()] = nanos;
                println!("raw,{name},{round},{position},{},{nanos:.3}", case.name());
            }
            observed.push(times);
        }
        for case in [
            Case::VecPayload,
            Case::ArcPayload,
            Case::ArcPayloadAndLayout,
        ] {
            let mut values: Vec<_> = observed.iter().map(|times| times[case.index()]).collect();
            println!("median,{name},{},{:.3}", case.name(), median(&mut values));
        }
        for (description, lower, upper) in [
            ("arc_payload_over_vec", 0, 1),
            ("layout_clone_and_drop", 1, 2),
            ("all_shared_ownership_over_vec", 0, 2),
        ] {
            let mut differences: Vec<_> = observed
                .iter()
                .map(|times| times[upper] - times[lower])
                .collect();
            println!(
                "paired_median_delta,{name},{description},{:.3}",
                median(&mut differences)
            );
        }
    }
    Ok(())
}
