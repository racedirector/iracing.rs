#![allow(dead_code)] // Shared benchmark helpers are compiled separately by each benchmark target.

//! Shared deterministic inputs and validation helpers for Criterion targets.
//!
//! The live schema capture supplies a coherent frame size, variable set, types,
//! counts, and offsets. This module generates type-correct sentinel bytes for
//! that layout; it does not reproduce values recorded from a real driving
//! session. Schema I/O, fixture construction, ordering, and verification are
//! intended for benchmark setup rather than timed loops.

pub mod telemetry_pipeline;
pub mod workloads;

use iracing_sdk::{
    BitField, FramePacket, TelemetryValue, VariableInfo, VariableSchema, VariableType,
};
use serde::Deserialize;
use std::{fs, path::PathBuf, sync::Arc};

const LIVE_SCHEMA_PATH: &str = "../../docs/reference/live-variable-schema.yml";
const BENCHMARK_TICK: u32 = 1;
const BENCHMARK_SESSION_VERSION: u32 = 1;

#[derive(Deserialize)]
struct VariableSchemaReference {
    examples: Vec<VariableSchema>,
}

/// A deterministic telemetry frame whose layout matches the checked-in live
/// iRacing variable-schema capture.
pub struct FullFrameFixture {
    /// Deterministic bytes matching `schema`'s captured layout.
    pub data: Vec<u8>,
    /// Validated metadata loaded from the checked-in live capture.
    pub schema: Arc<VariableSchema>,
}

impl FullFrameFixture {
    pub fn packet(&self) -> FramePacket {
        FramePacket::new(
            self.data.clone(),
            BENCHMARK_TICK,
            BENCHMARK_SESSION_VERSION,
            Arc::clone(&self.schema),
        )
    }
}

/// Load the full live schema outside the timed benchmark loop and populate a
/// frame with deterministic, type-correct values.
pub fn full_frame_fixture() -> FullFrameFixture {
    let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(LIVE_SCHEMA_PATH);
    let schema_yaml = fs::read_to_string(&schema_path).unwrap_or_else(|error| {
        panic!(
            "failed to read live variable schema at {}: {error}",
            schema_path.display()
        )
    });
    let reference: VariableSchemaReference = serde_yaml_ng::from_str(&schema_yaml)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", schema_path.display()));
    let schema = reference
        .examples
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{} contains no schema examples", schema_path.display()));

    schema
        .validate()
        .unwrap_or_else(|error| panic!("invalid live variable schema: {error}"));

    let mut data = vec![0; schema.frame_size];
    populate_frame(&mut data, &schema);

    FullFrameFixture {
        data,
        schema: Arc::new(schema),
    }
}

/// Require a benchmark variable with the expected telemetry type and element
/// count. Benchmark setup should fail instead of silently dropping coverage.
pub fn require_variable<'a>(
    schema: &'a VariableSchema,
    name: &str,
    expected_type: VariableType,
    expected_count: usize,
) -> &'a VariableInfo {
    let info = schema
        .get_variable(name)
        .unwrap_or_else(|| panic!("full-frame benchmark requires variable `{name}`"));

    assert_eq!(
        info.data_type, expected_type,
        "benchmark variable `{name}` has an unexpected telemetry type"
    );
    assert_eq!(
        info.count, expected_count,
        "benchmark variable `{name}` has an unexpected element count"
    );

    info
}

/// Return every captured variable in stable frame traversal order.
pub fn ordered_variables(schema: &VariableSchema) -> Vec<&VariableInfo> {
    let mut variables: Vec<_> = schema.variables.values().collect();
    variables.sort_unstable_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.name.cmp(&right.name))
    });

    assert_eq!(
        variables.len(),
        schema.variable_count(),
        "ordered full-frame workload lost schema variables"
    );
    variables
}

/// Verify that every variable in the captured frame decodes to its generated
/// sentinel before the benchmark timer starts.
pub fn verify_full_frame(packet: &FramePacket, variables: &[&VariableInfo]) {
    assert_eq!(packet.data.len(), packet.schema.frame_size);
    assert_eq!(variables.len(), packet.schema.variable_count());

    for info in variables {
        let byte_len = info
            .data_type
            .size()
            .checked_mul(info.count)
            .unwrap_or_else(|| {
                panic!(
                    "byte length overflow for benchmark variable `{}`",
                    info.name
                )
            });
        let end = info.offset.checked_add(byte_len).unwrap_or_else(|| {
            panic!("end offset overflow for benchmark variable `{}`", info.name)
        });
        assert!(
            end <= packet.data.len(),
            "benchmark variable `{}` at offset {} with type {:?} and count {} exceeds frame size {}",
            info.name,
            info.offset,
            info.data_type,
            info.count,
            packet.data.len()
        );

        let actual = TelemetryValue::decode(packet.data.as_ref(), info).unwrap_or_else(|error| {
            panic!(
                "failed to decode benchmark variable `{}` at offset {} with type {:?} and count {}: {error}",
                info.name, info.offset, info.data_type, info.count
            )
        });
        let expected = expected_value(info);
        assert_eq!(
            actual, expected,
            "decoded sentinel mismatch for benchmark variable `{}` with type {:?} and count {}",
            info.name, info.data_type, info.count
        );
    }
}

/// Count scalar values represented by scalars and array elements together.
pub fn total_elements(variables: &[&VariableInfo]) -> usize {
    variables
        .iter()
        .try_fold(0_usize, |total, info| total.checked_add(info.count))
        .expect("full-frame benchmark element count overflow")
}

fn expected_value(info: &VariableInfo) -> TelemetryValue {
    if info.count == 1 {
        expected_scalar(info.data_type, 0)
    } else {
        TelemetryValue::Array(
            (0..info.count)
                .map(|index| expected_scalar(info.data_type, index))
                .collect(),
        )
    }
}

fn expected_scalar(data_type: VariableType, index: usize) -> TelemetryValue {
    let integer = (index as u32).wrapping_add(1);

    match data_type {
        VariableType::Char => TelemetryValue::Char(integer as u8),
        VariableType::Int8 => TelemetryValue::Int8(integer as i8),
        VariableType::UInt8 => TelemetryValue::UInt8(integer as u8),
        VariableType::Int16 => TelemetryValue::Int16(integer as i16),
        VariableType::UInt16 => TelemetryValue::UInt16(integer as u16),
        VariableType::Int32 => TelemetryValue::Int32(integer as i32),
        VariableType::UInt32 => TelemetryValue::UInt32(integer),
        VariableType::Float32 => TelemetryValue::Float32(index as f32 + 0.5),
        VariableType::Float64 => TelemetryValue::Float64(index as f64 + 0.5),
        VariableType::Bool => TelemetryValue::Bool(index.is_multiple_of(2)),
        VariableType::BitField => {
            TelemetryValue::BitField(BitField::new(1_u32 << (index % u32::BITS as usize)))
        }
    }
}

fn populate_frame(data: &mut [u8], schema: &VariableSchema) {
    for info in ordered_variables(schema) {
        for index in 0..info.count {
            let offset = info.offset + index * info.data_type.size();
            let value = (index as u32).wrapping_add(1);

            match info.data_type {
                VariableType::Char | VariableType::UInt8 => data[offset] = value as u8,
                VariableType::Int8 => data[offset] = (value as i8).to_le_bytes()[0],
                VariableType::Int16 => {
                    data[offset..offset + 2].copy_from_slice(&(value as i16).to_le_bytes());
                }
                VariableType::UInt16 => {
                    data[offset..offset + 2].copy_from_slice(&(value as u16).to_le_bytes());
                }
                VariableType::Int32 => {
                    data[offset..offset + 4].copy_from_slice(&(value as i32).to_le_bytes());
                }
                VariableType::UInt32 => {
                    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                }
                VariableType::Float32 => {
                    data[offset..offset + 4].copy_from_slice(&((index as f32) + 0.5).to_le_bytes());
                }
                VariableType::Float64 => {
                    data[offset..offset + 8].copy_from_slice(&((index as f64) + 0.5).to_le_bytes());
                }
                VariableType::Bool => data[offset] = u8::from(index % 2 == 0),
                VariableType::BitField => {
                    let bit = 1_u32 << (index % u32::BITS as usize);
                    data[offset..offset + 4].copy_from_slice(&bit.to_le_bytes());
                }
            }
        }
    }
}
