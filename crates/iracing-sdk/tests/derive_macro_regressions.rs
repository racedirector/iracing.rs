mod support;
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use iracing_sdk::{
    BitField, FieldExtraction, FieldLayout, FrameAdapter, FramePacket, IRacingSDKError,
    TelemetryLayout,
    irsdk::{IncidentFlags, VariableType},
};
use iracing_sdk_derive::IRacingTelemetryFrame;

/// Creates a `FieldLayout` for a single scalar telemetry variable.
///
/// # Examples
///
/// ```
/// let info = make_variable_info("Speed", VariableType::Float, 0);
/// assert_eq!(info.name(), "Speed");
/// assert_eq!(info.data_type(), VariableType::Float);
/// assert_eq!(info.region().offset(), 0);
/// assert_eq!(info.count(), 1);
/// ```
fn make_variable_info(name: &str, data_type: VariableType, offset: usize) -> FieldLayout {
    support::field(
        name.to_string(),
        data_type,
        offset,
        1,
        false,
        String::new(),
        String::new(),
    )
}

/// Builds a TelemetryLayout from a list of `(name, VariableType, offset)` entries.
///
/// The provided entries are converted into `FieldLayout` records and assembled into a
/// `TelemetryLayout` with the given `frame_size`. This function will panic if the
/// constructed layout is invalid.
///
/// # Examples
///
/// ```
/// let entries = &[("Speed", VariableType::Float, 0usize)];
/// let layout = make_schema(entries, 4);
/// let _ = layout; // use layout for validation/adaptation in tests
/// ```
fn make_schema(entries: &[(&str, VariableType, usize)], frame_size: usize) -> TelemetryLayout {
    let variables = entries
        .iter()
        .map(|(name, data_type, offset)| {
            (
                (*name).to_string(),
                make_variable_info(name, *data_type, *offset),
            )
        })
        .collect::<HashMap<_, _>>();

    support::layout(variables.into_values(), frame_size).expect("layout should be valid")
}

/// Constructs a FramePacket from raw frame bytes and an associated TelemetryLayout.
///
/// # Examples
///
/// ```
/// // `layout` should be an `Arc<TelemetryLayout>` obtained from layout construction/validation.
/// // This example demonstrates the call; concrete layout construction is omitted for brevity.
/// let layout = /* Arc<TelemetryLayout> */ unimplemented!();
/// let packet = make_packet(layout, vec![0u8; 16]);
/// // `packet` is ready for adaptation/inspection.
/// ```
fn make_packet(layout: Arc<TelemetryLayout>, data: Vec<u8>) -> FramePacket {
    FramePacket::new(data, 7, 11, layout).unwrap()
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

#[derive(IRacingTelemetryFrame, Debug, PartialEq)]
struct IncidentRow {
    #[field_name = "PlayerIncidents"]
    incidents: IncidentFlags,
}

#[test]
fn derive_supports_generic_structs() {
    let layout = Arc::new(make_schema(&[("Speed", VariableType::Float, 0)], 4));
    let packet = make_packet(Arc::clone(&layout), 42.25f32.to_le_bytes().to_vec());

    let validation = GenericRow::<u8>::validate_layout(&layout).expect("validation should pass");
    let row = GenericRow::<u8>::adapt(&packet, &validation);

    assert_eq!(row.speed, 42.25);
    assert_eq!(row.marker, PhantomData);
}

#[test]
fn calculated_expressions_preserve_non_telemetry_identifiers() {
    let layout = Arc::new(make_schema(&[("Speed", VariableType::Float, 0)], 4));
    let packet = make_packet(Arc::clone(&layout), 100.0f32.to_le_bytes().to_vec());

    let validation = CalculatedRow::validate_layout(&layout).expect("validation should pass");
    let row = CalculatedRow::adapt(&packet, &validation);

    assert_eq!(row.speed, 100.0);
    assert!((row.speed_kph - 160.934).abs() < 1e-3);
}

#[test]
fn validate_layout_treats_incompatible_optional_and_default_fields_as_missing() {
    let layout = Arc::new(make_schema(
        &[
            ("OptionalInt", VariableType::Float, 0),
            ("DefaultedFloat", VariableType::Integer, 4),
            ("TypeDefaultFloat", VariableType::Boolean, 8),
            ("HasFlagField", VariableType::Integer, 12),
            ("MappedFlagField", VariableType::Float, 16),
        ],
        20,
    ));
    let packet = make_packet(Arc::clone(&layout), vec![0; 20]);

    let validation = FallbackRow::validate_layout(&layout).expect("validation should pass");

    for (index, field_name) in [
        "OptionalInt",
        "DefaultedFloat",
        "TypeDefaultFloat",
        "HasFlagField",
        "MappedFlagField",
    ]
    .into_iter()
    .enumerate()
    {
        let extraction = validation
            .extraction_plan()
            .get(index)
            .expect("field extraction should exist");
        match extraction {
            FieldExtraction::Optional(var_info) | FieldExtraction::WithDefault(var_info) => {
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
fn validate_layout_rejects_incompatible_required_fields() {
    let layout = make_schema(&[("Speed", VariableType::Integer, 0)], 4);

    let err = CriticalRow::validate_layout(&Arc::new(layout)).expect_err("validation should fail");
    match err {
        IRacingSDKError::Parse { context, details } => {
            assert_eq!(context, "Frame adapter validation");
            assert!(details.contains("Field 'Speed' has incompatible telemetry type"));
            assert!(details.contains("f32") && details.contains("Integer"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn validate_layout_rejects_incompatible_required_bitfields() {
    let layout = make_schema(&[("SessionFlags", VariableType::Integer, 0)], 4);

    let err = CriticalBitfieldRow::validate_layout(&Arc::new(layout))
        .expect_err("validation should fail");
    match err {
        IRacingSDKError::Parse { context, details } => {
            assert_eq!(context, "Frame adapter validation");
            assert!(details.contains("Field 'SessionFlags' has incompatible telemetry type"));
            assert!(details.contains("BitField") && details.contains("Integer"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn incident_flags_validate_and_adapt_from_bitfield_or_int32_storage() {
    const RAW: u32 = 0x8000_0408;

    for data_type in [VariableType::BitField, VariableType::Integer] {
        let layout = Arc::new(make_schema(&[("PlayerIncidents", data_type, 0)], 4));
        let packet = make_packet(Arc::clone(&layout), RAW.to_le_bytes().to_vec());

        let validation = IncidentRow::validate_layout(&layout)
            .expect("IncidentFlags storage type should validate");
        let row = IncidentRow::adapt(&packet, &validation);

        assert_eq!(row.incidents.bits(), RAW);
        assert_eq!(row.incidents.report_bits(), 8);
        assert_eq!(row.incidents.penalty_bits(), 4);
        assert_eq!(
            row.incidents.classify(),
            iracing_sdk::irsdk::IncidentClassification {
                report: iracing_sdk::irsdk::IncidentReport::CollisionWithCar,
                penalty: iracing_sdk::irsdk::IncidentPenalty::FourX,
            }
        );
    }
}

#[test]
fn validation_rejects_equal_geometry_from_another_layout() {
    let layout = Arc::new(make_schema(&[("Speed", VariableType::Float, 0)], 4));
    let validation = CalculatedRow::validate_layout(&layout).unwrap();
    assert!(Arc::ptr_eq(validation.layout(), &layout));
    let other = Arc::new(make_schema(&[("Speed", VariableType::Float, 0)], 4));
    let packet = make_packet(other, 3f32.to_le_bytes().to_vec());
    assert!(validation.ensure_packet(&packet).is_err());
    assert!(validation.for_packet(&packet).is_err());
    assert!(validation.decode::<f32>(&packet, 0).is_err());
    assert!(std::panic::catch_unwind(|| CalculatedRow::adapt(&packet, &validation)).is_err());
}

#[test]
fn scalar_shape_is_checked_before_adaptation() {
    let array = support::field(
        "Speed".into(),
        VariableType::Float,
        0,
        2,
        false,
        "m/s".into(),
        String::new(),
    );
    let layout = Arc::new(support::layout([array], 8).unwrap());
    assert!(CriticalRow::validate_layout(&layout).is_err());
    let validation = CalculatedRow::validate_layout(&layout).unwrap();
    assert_eq!(validation.extraction_plan()[0].field_id(), None);
    let packet = make_packet(layout, vec![0; 8]);
    let row = CalculatedRow::adapt(&packet, &validation);
    assert_eq!(row.speed, 0.0);
    assert_eq!(row.speed_kph, 0.0);
}

#[test]
fn plan_slots_follow_declarations_instead_of_header_order() {
    let speed = make_variable_info("Speed", VariableType::Float, 0);
    let gear = make_variable_info("Gear", VariableType::Integer, 4);
    let layout = Arc::new(support::layout([gear, speed], 8).unwrap());
    let validation = CalculatedRow::validate_layout(&layout).unwrap();
    let speed_id = layout.field_by_name("Speed").unwrap().0;
    assert_eq!(validation.extraction_plan()[0].field_id(), Some(speed_id));
    assert_eq!(validation.extraction_plan()[1], FieldExtraction::Calculated);
    let mut data = 10f32.to_le_bytes().to_vec();
    data.extend(4i32.to_le_bytes());
    let row = CalculatedRow::adapt(&make_packet(layout, data), &validation);
    assert_eq!(row.speed, 10.0);
    assert_eq!(row.speed_kph, mph_to_kph(10.0));
}
