use super::*;

const SMALL_SESSION: &str =
    include_str!("../../../../../test-data/session-yaml/profile_small.yaml");

#[test]
fn captured_buffers_deserialize_track_session_and_driver_fields() {
    for (bytes, track, display, track_id, current, count, driver_idx, first_driver) in [
        (
            include_bytes!("../../../../../test-data/session-yaml/utf-8-snapshot.yml").as_slice(),
            "roadatlanta full",
            "Road Atlanta",
            127,
            4,
            5,
            14,
            "Ethan Conde",
        ),
        (
            include_bytes!("../../../../../test-data/session-yaml/iso-8859-1-snapshot.yml")
                .as_slice(),
            "dover",
            "Dover Motor Speedway",
            162,
            2,
            3,
            2,
            "Elisha Whitfield",
        ),
    ] {
        let session = SessionInfo::try_from(SessionInfoBuffer::from_checked_region(bytes))
            .expect("captured session should deserialize");
        assert_eq!(session.weekend_info.track_name, track);
        assert_eq!(session.weekend_info.track_display_name, display);
        assert_eq!(session.weekend_info.track_id, Some(track_id));
        assert_eq!(session.session_info.current_session_num, current);
        assert_eq!(session.session_info.sessions.len(), count);
        assert_eq!(session.session_info.sessions[0].session_type, "Practice");
        let driver_info = session.driver_info.unwrap();
        assert_eq!(driver_info.driver_car_idx, Some(driver_idx));
        assert_eq!(driver_info.driver_user_id, Some(378767));
        let drivers = driver_info.drivers.unwrap();
        assert_eq!(drivers[0].car_idx, 0);
        assert_eq!(drivers[0].user_name, first_driver);
    }
}

#[test]
fn cleaned_yaml_preserves_string_car_numbers_and_optional_sections() {
    let minimal = SessionInfo::parse(SMALL_SESSION).unwrap();
    assert!(minimal.driver_info.is_none());
    assert!(minimal.radio_info.is_none());
    let yaml = format!(
        "{SMALL_SESSION}\nDriverInfo:\n  Drivers:\n    - CarIdx: 0\n      UserName: Test Driver\n      CarNumber: '037'\n"
    );
    let session = SessionInfo::parse(&yaml).unwrap();
    let drivers = session.driver_info.unwrap().drivers.unwrap();
    assert_eq!(drivers[0].car_number.as_deref(), Some("037"));
}

#[test]
fn buffer_conversion_decodes_sanitizes_and_ignores_padding_before_parsing() {
    let yaml = SMALL_SESSION.replace("WeekendInfo:\n", "WeekendInfo:\n  Encoding: ISO_8859_1\n");
    let mut bytes = yaml.into_bytes();
    bytes.extend_from_slice(
        b"DriverInfo:\n  Drivers:\n    - CarIdx: 0\n      UserName: Jos\xe9\x01\n      CarNumber: '037'\n\0\xffinvalid: [",
    );
    let session = SessionInfo::try_from(SessionInfoBuffer::from_checked_region(&bytes)).unwrap();
    let drivers = session.driver_info.unwrap().drivers.unwrap();
    assert_eq!(drivers[0].user_name, "Jos\u{e9}");
    assert_eq!(drivers[0].car_number.as_deref(), Some("037"));
    assert_eq!(session.weekend_info.track_name, "generated small");
}

#[test]
fn sanitized_string_conversion_deserializes_typed_fields() {
    let sanitized = IRacingSessionString::try_from(SMALL_SESSION).unwrap();
    let session = SessionInfo::try_from(sanitized).unwrap();
    assert_eq!(session.weekend_info.track_id, Some(9001));
    assert_eq!(session.session_info.sessions[0].session_type, "Practice");
}

#[test]
fn parse_rejects_malformed_yaml_and_missing_required_sections() {
    for yaml in ["WeekendInfo: [", "SessionInfo: {}", "WeekendInfo: {}"] {
        let error = SessionInfo::parse(yaml).unwrap_err();
        assert!(matches!(error, IRacingSDKError::Parse { context, .. }
            if context == "SessionInfo deserialization"));
    }
}

#[test]
fn buffer_conversion_propagates_cleanup_and_deserialization_errors() {
    for (bytes, expected_context) in [
        (b"\x01 \n\0padding".as_slice(), "YAML preprocessing"),
        (b"WeekendInfo: [".as_slice(), "SessionInfo deserialization"),
    ] {
        let error =
            SessionInfo::try_from(SessionInfoBuffer::from_checked_region(bytes)).unwrap_err();
        assert!(matches!(error, IRacingSDKError::Parse { context, .. }
            if context == expected_context));
    }
}

#[cfg(feature = "schema-discovery")]
#[test]
fn driver_tire_unknown_fields_are_preserved_and_reported() {
    // Rename a captured key to simulate a future SDK addition.
    let yaml = include_str!("../../../../../test-data/session-yaml/utf-8-snapshot.yml")
        .replace("TireCompoundType:", "UnmodeledTireCompoundType:");
    let session = SessionInfo::parse(&yaml).unwrap();
    let tire = &session
        .driver_info
        .as_ref()
        .unwrap()
        .driver_tires
        .as_ref()
        .unwrap()[0];
    assert_eq!(
        tire.unknown_fields["UnmodeledTireCompoundType"].as_str(),
        Some("AllPurpose")
    );
    // Serialization preserves unknown keys without calling the collector.
    let serialized = serde_json::to_value(&session).unwrap();
    assert_eq!(
        serialized["DriverInfo"]["DriverTires"][0]["UnmodeledTireCompoundType"],
        "AllPurpose"
    );
    assert!(session.collect_unknown_fields().iter().any(|field| {
        field.path == "DriverInfo.DriverTires[0].UnmodeledTireCompoundType"
            && field.data_type == UnknownFieldType::String
            && field.example == "AllPurpose"
    }));
}
