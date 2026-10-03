#![allow(dead_code)] // Shared benchmark helpers are compiled separately by each benchmark target.

//! Shared deterministic inputs and validation helpers for Criterion targets.
//!
//! The live layout capture supplies a coherent frame size, variable set, types,
//! counts, and offsets. This module generates type-correct sentinel bytes for
//! that layout; it does not reproduce values recorded from a real driving
//! session. Schema I/O, fixture construction, ordering, and verification are
//! intended for benchmark setup rather than timed loops.

pub mod ibt;
pub mod telemetry_pipeline;
pub mod workloads;

use iracing_sdk::{
    BitField, FieldLayout, FramePacket, TelemetryLayout, TelemetryValue, irsdk::VariableType,
};
use serde::Deserialize;
use std::{fs, path::PathBuf, sync::Arc};

const LIVE_SCHEMA_PATH: &str = "../../docs/reference/live-variable-schema.yml";
const BENCHMARK_TICK: u32 = 1;
const BENCHMARK_SESSION_VERSION: u32 = 1;

#[derive(Deserialize)]
struct TelemetryLayoutReference {
    examples: Vec<Capture>,
}

// Capture DTOs exist only at benchmark setup; runtime layouts have no serde contract.
#[derive(Deserialize)]
struct Capture {
    frame_size: usize,
    variables: std::collections::BTreeMap<String, CapturedField>,
}
#[derive(Deserialize)]
struct CapturedField {
    name: String,
    data_type: VariableType,
    offset: i32,
    count: i32,
    count_as_time: bool,
    units: String,
    description: String,
}
impl Capture {
    fn into_layout(self) -> TelemetryLayout {
        let mut fields: Vec<_> = self.variables.into_values().collect();
        fields.sort_by_key(|f| f.offset);
        let headers: Vec<_> = fields
            .into_iter()
            .map(|f| {
                iracing_sdk::irsdk::VariableHeader::new(
                    f.data_type,
                    f.offset,
                    f.count,
                    f.count_as_time,
                    &f.name,
                    &f.description,
                    &f.units,
                )
                .unwrap()
            })
            .collect();
        TelemetryLayout::try_from_headers(&headers.into(), self.frame_size).unwrap()
    }
}

/// A deterministic telemetry frame whose layout matches the checked-in live
/// iRacing variable-layout capture.
pub struct FullFrameFixture {
    /// Deterministic bytes matching `layout`'s captured layout.
    pub data: Vec<u8>,
    /// Validated metadata loaded from the checked-in live capture.
    pub layout: Arc<TelemetryLayout>,
}

impl FullFrameFixture {
    pub fn packet(&self) -> FramePacket {
        FramePacket::new(
            self.data.clone(),
            BENCHMARK_TICK,
            BENCHMARK_SESSION_VERSION,
            Arc::clone(&self.layout),
        )
        .unwrap()
    }
}

/// Load the full live layout outside the timed benchmark loop and populate a
/// frame with deterministic, type-correct values.
pub fn full_frame_fixture() -> FullFrameFixture {
    let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(LIVE_SCHEMA_PATH);
    let schema_yaml = fs::read_to_string(&schema_path).unwrap_or_else(|error| {
        panic!(
            "failed to read live variable layout at {}: {error}",
            schema_path.display()
        )
    });
    let reference: TelemetryLayoutReference = serde_yaml_ng::from_str(&schema_yaml)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", schema_path.display()));
    let layout = reference
        .examples
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{} contains no layout examples", schema_path.display()))
        .into_layout();

    let mut data = vec![0; layout.frame_size()];
    populate_frame(&mut data, &layout);

    FullFrameFixture {
        data,
        layout: Arc::new(layout),
    }
}

/// Require a benchmark variable with the expected telemetry type and element
/// count. Benchmark setup should fail instead of silently dropping coverage.
pub fn require_variable<'a>(
    layout: &'a TelemetryLayout,
    name: &str,
    expected_type: VariableType,
    expected_count: usize,
) -> &'a FieldLayout {
    let info = layout
        .field_by_name(name)
        .map(|(_, field)| field)
        .unwrap_or_else(|| panic!("full-frame benchmark requires variable `{name}`"));

    assert_eq!(
        info.data_type(),
        expected_type,
        "benchmark variable `{name}` has an unexpected telemetry type"
    );
    assert_eq!(
        info.count(),
        expected_count,
        "benchmark variable `{name}` has an unexpected element count"
    );

    info
}

/// Return every captured variable in stable frame traversal order.
pub fn ordered_variables(layout: &TelemetryLayout) -> Vec<&FieldLayout> {
    let mut variables: Vec<_> = layout.fields().map(|(_, f)| f).collect();
    variables.sort_unstable_by(|left, right| {
        left.region()
            .offset()
            .cmp(&right.region().offset())
            .then_with(|| left.name().cmp(right.name()))
    });

    assert_eq!(
        variables.len(),
        layout.len(),
        "ordered full-frame workload lost layout variables"
    );
    variables
}

/// Verify that every variable in the captured frame decodes to its generated
/// sentinel before the benchmark timer starts.
pub fn verify_full_frame(packet: &FramePacket, variables: &[&FieldLayout]) {
    assert_eq!(packet.data().len(), packet.layout().frame_size());
    assert_eq!(variables.len(), packet.layout().len());

    for info in variables {
        let byte_len = info
            .data_type()
            .byte_size()
            .checked_mul(info.count())
            .unwrap_or_else(|| {
                panic!(
                    "byte length overflow for benchmark variable `{}`",
                    info.name()
                )
            });
        let end = info
            .region()
            .offset()
            .checked_add(byte_len)
            .unwrap_or_else(|| {
                panic!(
                    "end offset overflow for benchmark variable `{}`",
                    info.name()
                )
            });
        assert!(
            end <= packet.data().len(),
            "benchmark variable `{}` at offset {} with type {:?} and count {} exceeds frame size {}",
            info.name(),
            info.region().offset(),
            info.data_type(),
            info.count(),
            packet.data().len()
        );

        let actual = TelemetryValue::decode_field(packet.data().as_ref(), info).unwrap_or_else(|error| {
            panic!(
                "failed to decode benchmark variable `{}` at offset {} with type {:?} and count {}: {error}",
                info.name(), info.region().offset(), info.data_type(), info.count()
            )
        });
        let expected = expected_value(info);
        assert_eq!(
            actual,
            expected,
            "decoded sentinel mismatch for benchmark variable `{}` with type {:?} and count {}",
            info.name(),
            info.data_type(),
            info.count()
        );
    }
}

/// Count scalar values represented by scalars and array elements together.
pub fn total_elements(variables: &[&FieldLayout]) -> usize {
    variables
        .iter()
        .try_fold(0_usize, |total, info| total.checked_add(info.count()))
        .expect("full-frame benchmark element count overflow")
}

fn expected_value(info: &FieldLayout) -> TelemetryValue {
    if info.count() == 1 {
        expected_scalar(info.data_type(), 0)
    } else {
        TelemetryValue::Array(
            (0..info.count())
                .map(|index| expected_scalar(info.data_type(), index))
                .collect(),
        )
    }
}

fn expected_scalar(data_type: VariableType, index: usize) -> TelemetryValue {
    let integer = (index as u32).wrapping_add(1);

    match data_type {
        VariableType::Character => TelemetryValue::Char(integer as u8),
        VariableType::Integer => TelemetryValue::Int32(integer as i32),
        VariableType::Float => TelemetryValue::Float32(index as f32 + 0.5),
        VariableType::Double => TelemetryValue::Float64(index as f64 + 0.5),
        VariableType::Boolean => TelemetryValue::Bool(index.is_multiple_of(2)),
        VariableType::BitField => {
            TelemetryValue::BitField(BitField::new(1_u32 << (index % u32::BITS as usize)))
        }
    }
}

fn populate_frame(data: &mut [u8], layout: &TelemetryLayout) {
    for info in ordered_variables(layout) {
        for index in 0..info.count() {
            let offset = info.region().offset() + index * info.data_type().byte_size();
            let value = (index as u32).wrapping_add(1);

            match info.data_type() {
                VariableType::Character => data[offset] = value as u8,
                VariableType::Integer => {
                    data[offset..offset + 4].copy_from_slice(&(value as i32).to_le_bytes());
                }
                VariableType::Float => {
                    data[offset..offset + 4].copy_from_slice(&((index as f32) + 0.5).to_le_bytes());
                }
                VariableType::Double => {
                    data[offset..offset + 8].copy_from_slice(&((index as f64) + 0.5).to_le_bytes());
                }
                VariableType::Boolean => data[offset] = u8::from(index % 2 == 0),
                VariableType::BitField => {
                    let bit = 1_u32 << (index % u32::BITS as usize);
                    data[offset..offset + 4].copy_from_slice(&bit.to_le_bytes());
                }
            }
        }
    }
}
