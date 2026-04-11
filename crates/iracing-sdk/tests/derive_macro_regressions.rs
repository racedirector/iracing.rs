use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use iracing_sdk::{
    BitField, FieldExtraction, FrameAdapter, FramePacket, IRacingSDKError, VariableInfo,
    VariableSchema, VariableType,
};
use iracing_sdk_derive::IRacingTelemetryFrame;

/// Creates a `VariableInfo` for a single scalar telemetry variable.
///
/// # Examples
///
/// ```
/// let info = make_variable_info("Speed", VariableType::Float32, 0);
/// assert_eq!(info.name, "Speed");
/// assert_eq!(info.data_type, VariableType::Float32);
/// assert_eq!(info.offset, 0);
/// assert_eq!(info.count, 1);
/// ```
fn make_variable_info(name: &str, data_type: VariableType, offset: usize) -> VariableInfo {
    VariableInfo {
        name: name.to_string(),
        data_type,
        offset,
        count: 1,
        count_as_time: false,
        units: String::new(),
        description: String::new(),
    }
}

/// Builds a VariableSchema from a list of `(name, VariableType, offset)` entries.
///
/// The provided entries are converted into `VariableInfo` records and assembled into a
/// `VariableSchema` with the given `frame_size`. This function will panic if the
/// constructed schema is invalid.
///
/// # Examples
///
/// ```
/// let entries = &[("Speed", VariableType::Float32, 0usize)];
/// let schema = make_schema(entries, 4);
/// let _ = schema; // use schema for validation/adaptation in tests
/// ```
fn make_schema(entries: &[(&str, VariableType, usize)], frame_size: usize) -> VariableSchema {
    let variables = entries
        .iter()
        .map(|(name, data_type, offset)| {
            (
                (*name).to_string(),
                make_variable_info(name, *data_type, *offset),
            )
        })
        .collect::<HashMap<_, _>>();

    VariableSchema::new(variables, frame_size).expect("schema should be valid")
}

/// Constructs a FramePacket from raw frame bytes and an associated VariableSchema.
///
/// # Examples
///
/// ```
/// // `schema` should be an `Arc<VariableSchema>` obtained from schema construction/validation.
/// // This example demonstrates the call; concrete schema construction is omitted for brevity.
/// let schema = /* Arc<VariableSchema> */ unimplemented!();
/// let packet = make_packet(schema, vec![0u8; 16]);
/// // `packet` is ready for adaptation/inspection.
/// ```
fn make_packet(schema: Arc<VariableSchema>, data: Vec<u8>) -> FramePacket {
    FramePacket::new(data, 7, 11, schema)
}

/// Checks whether the least-significant bit (0b1) is set in the bitfield.
///
/// # Examples
///
/// ```
/// let bits = BitField::from(0b1);
/// assert!(decode_low_bit(bits));
///
/// let bits = BitField::from(0b0);
/// assert!(!decode_low_bit(bits));
/// ```
///
/// # Returns
///
/// `true` if the least-significant bit is set, `false` otherwise.
fn decode_low_bit(bits: BitField) -> bool {
    bits.has_flag(0b1)
}

/// Convert speed from miles per hour to kilometers per hour.
///
/// # Returns
///
/// The equivalent speed in kilometers per hour.
///
/// # Examples
///
/// ```
/// let kph = mph_to_kph(60.0);
/// assert!((kph - 96.5604).abs() < 1e-6);
/// ```
fn mph_to_kph(speed: f32) -> f32 {
    speed * 1.609_34
}

#[derive(IRacingTelemetryFrame, Debug, PartialEq)]
struct GenericRow<T>
where
    T: Clone,
{
    #[field_name = "Speed"]
    speed: f32,
    #[skip]
    marker: PhantomData<T>,
}

#[derive(IRacingTelemetryFrame, Debug)]
struct CalculatedRow {
    #[field_name = "Speed"]
    speed: f32,
    #[calculated = "mph_to_kph(Speed)"]
    speed_kph: f32,
}

#[derive(IRacingTelemetryFrame, Debug)]
struct FallbackRow {
    #[field_name = "OptionalInt"]
    optional_int: Option<i32>,
    #[field_name = "DefaultedFloat"]
    #[missing = "7.5"]
    defaulted_float: f32,
    #[field_name = "TypeDefaultFloat"]
    type_default_float: f32,
    #[bitfield(name = "HasFlagField", has = "0b1")]
    has_flag: bool,
    #[bitfield_map(name = "MappedFlagField", decoder = "decode_low_bit")]
    mapped_flag: bool,
}

#[derive(IRacingTelemetryFrame, Debug)]
#[allow(dead_code)]
struct CriticalRow {
    #[field_name = "Speed"]
    #[fail_if_missing]
    speed: f32,
}

#[derive(IRacingTelemetryFrame, Debug)]
#[allow(dead_code)]
struct CriticalBitfieldRow {
    #[bitfield(name = "SessionFlags", has = "0b1")]
    #[fail_if_missing]
    flag: bool,
}

#[test]
fn derive_supports_generic_structs() {
    let schema = Arc::new(make_schema(&[("Speed", VariableType::Float32, 0)], 4));
    let packet = make_packet(Arc::clone(&schema), 42.25f32.to_le_bytes().to_vec());

    let validation = GenericRow::<u8>::validate_schema(&schema).expect("validation should pass");
    let row = GenericRow::<u8>::adapt(&packet, &validation);

    assert_eq!(row.speed, 42.25);
    assert_eq!(row.marker, PhantomData);
}

#[test]
fn calculated_expressions_preserve_non_telemetry_identifiers() {
    let schema = Arc::new(make_schema(&[("Speed", VariableType::Float32, 0)], 4));
    let packet = make_packet(Arc::clone(&schema), 100.0f32.to_le_bytes().to_vec());

    let validation = CalculatedRow::validate_schema(&schema).expect("validation should pass");
    let row = CalculatedRow::adapt(&packet, &validation);

    assert_eq!(row.speed, 100.0);
    assert!((row.speed_kph - 160.934).abs() < 1e-3);
}

#[test]
fn validate_schema_treats_incompatible_optional_and_default_fields_as_missing() {
    let schema = Arc::new(make_schema(
        &[
            ("OptionalInt", VariableType::Float32, 0),
            ("DefaultedFloat", VariableType::Int32, 4),
            ("TypeDefaultFloat", VariableType::Bool, 8),
            ("HasFlagField", VariableType::Int32, 12),
            ("MappedFlagField", VariableType::UInt32, 16),
        ],
        20,
    ));
    let packet = make_packet(Arc::clone(&schema), vec![0; 20]);

    let validation = FallbackRow::validate_schema(&schema).expect("validation should pass");

    for field_name in [
        "OptionalInt",
        "DefaultedFloat",
        "TypeDefaultFloat",
        "HasFlagField",
        "MappedFlagField",
    ] {
        let index = validation
            .index_of(field_name)
            .expect("field should be indexed");
        let extraction = validation
            .extraction_plan
            .get(index)
            .expect("field extraction should exist");
        match extraction {
            FieldExtraction::Optional { var_info, .. }
            | FieldExtraction::WithDefault { var_info, .. } => {
                assert!(
                    var_info.is_none(),
                    "{field_name} should be treated as missing after type validation"
                );
            }
            other => panic!("unexpected extraction for {field_name}: {other:?}"),
        }
    }

    let row = FallbackRow::adapt(&packet, &validation);
    assert_eq!(row.optional_int, None);
    assert_eq!(row.defaulted_float, 7.5);
    assert_eq!(row.type_default_float, 0.0);
    assert!(!row.has_flag);
    assert!(!row.mapped_flag);
}

#[test]
fn validate_schema_rejects_incompatible_required_fields() {
    let schema = make_schema(&[("Speed", VariableType::Int32, 0)], 4);

    let err = CriticalRow::validate_schema(&schema).expect_err("validation should fail");
    match err {
        IRacingSDKError::Parse { context, details } => {
            assert_eq!(context, "Frame adapter validation");
            assert!(details.contains("Field 'Speed' has incompatible telemetry type"));
            assert!(details.contains("Expected Float32, got Int32"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn validate_schema_rejects_incompatible_required_bitfields() {
    let schema = make_schema(&[("SessionFlags", VariableType::Int32, 0)], 4);

    let err = CriticalBitfieldRow::validate_schema(&schema).expect_err("validation should fail");
    match err {
        IRacingSDKError::Parse { context, details } => {
            assert_eq!(context, "Frame adapter validation");
            assert!(details.contains("Field 'SessionFlags' has incompatible telemetry type"));
            assert!(details.contains("Expected BitField, got Int32"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
