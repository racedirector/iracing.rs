#![allow(dead_code)] // Shared benchmark helpers are compiled separately by each benchmark target.

use iracing_sdk::{FramePacket, VariableInfo, VariableSchema, VariableType};
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
    pub data: Vec<u8>,
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

fn populate_frame(data: &mut [u8], schema: &VariableSchema) {
    let mut variables: Vec<_> = schema.variables.values().collect();
    variables.sort_unstable_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.name.cmp(&right.name))
    });

    for info in variables {
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
