use tonic::Status;

pub(crate) fn u32_to_u16(field_name: &'static str, value: u32) -> Result<u16, Status> {
    u16::try_from(value).map_err(|_| {
        Status::invalid_argument(format!(
            "`{field_name}` must be in the range 0..={}, got {value}",
            u16::MAX,
        ))
    })
}

pub(crate) fn i32_to_i16(field_name: &'static str, value: i32) -> Result<i16, Status> {
    i16::try_from(value).map_err(|_| {
        Status::invalid_argument(format!(
            "`{field_name}` must be in the range {}..={}, got {value}",
            i16::MIN,
            i16::MAX,
        ))
    })
}

pub(crate) fn required_u16(field_name: &'static str, value: Option<u32>) -> Result<u16, Status> {
    optional_u16(field_name, value)?.ok_or_else(|| missing_status(field_name))
}

pub(crate) fn optional_u16(
    field_name: &'static str,
    value: Option<u32>,
) -> Result<Option<u16>, Status> {
    value.map(|value| u32_to_u16(field_name, value)).transpose()
}

pub(crate) fn optional_i16(
    field_name: &'static str,
    value: Option<i32>,
) -> Result<Option<i16>, Status> {
    value.map(|value| i32_to_i16(field_name, value)).transpose()
}

pub(crate) fn required_u32(field_name: &'static str, value: Option<u32>) -> Result<u32, Status> {
    value.ok_or_else(|| missing_status(field_name))
}

pub(crate) fn optional_u32(value: Option<u32>) -> Option<u32> {
    value
}

pub(crate) fn optional_string(
    field_name: &'static str,
    value: Option<String>,
) -> Result<Option<String>, Status> {
    match value {
        Some(value) if !value.is_empty() => Ok(Some(value)),
        Some(_) => Err(Status::invalid_argument(format!(
            "`{field_name}` must not be empty"
        ))),
        None => Ok(None),
    }
}

pub(crate) fn required_enum<E>(field_name: &'static str, value: Option<i32>) -> Result<E, Status>
where
    E: TryFrom<i32>,
{
    let value = value.ok_or_else(|| missing_status(field_name))?;
    enum_value(field_name, value)
}

pub(crate) fn required_f32(field_name: &'static str, value: Option<f32>) -> Result<f32, Status> {
    optional_f32(field_name, value)?.ok_or_else(|| missing_status(field_name))
}

pub(crate) fn optional_f32(
    field_name: &'static str,
    value: Option<f32>,
) -> Result<Option<f32>, Status> {
    match value {
        Some(value) if value.is_finite() => Ok(Some(value)),
        Some(value) => Err(Status::invalid_argument(format!(
            "`{field_name}` must be finite, got {value}"
        ))),
        None => Ok(None),
    }
}

pub(crate) fn f32_to_u16(field_name: &'static str, value: f32) -> Result<u16, Status> {
    if value < 0.0 || value > f32::from(u16::MAX) || value.fract() != 0.0 {
        return Err(Status::invalid_argument(format!(
            "`{field_name}` must be an integer in the range 0..={}, got {value}",
            u16::MAX,
        )));
    }

    Ok(value as u16)
}

fn enum_value<E>(field_name: &'static str, value: i32) -> Result<E, Status>
where
    E: TryFrom<i32>,
{
    if value == 0 {
        return Err(Status::invalid_argument(format!(
            "`{field_name}` must not be UNKNOWN"
        )));
    }

    E::try_from(value).map_err(|_| {
        Status::invalid_argument(format!("Invalid `{field_name}` enum value: {value}"))
    })
}

fn missing_status(field_name: &'static str) -> Status {
    Status::invalid_argument(format!("Missing `{field_name}`"))
}

#[cfg(test)]
mod tests {
    use tonic::Code;

    use super::*;
    use crate::broadcast::ReplaySearchMode;

    fn assert_invalid_argument(error: Status, field: &str) {
        assert_eq!(error.code(), Code::InvalidArgument);
        assert!(
            error.message().contains(field),
            "error message should mention `{field}`: {}",
            error.message()
        );
    }

    #[test]
    fn optional_u16_accepts_present_values_and_preserves_missing() {
        assert_eq!(optional_u16("position", None).unwrap(), None);
        assert_eq!(optional_u16("position", Some(0)).unwrap(), Some(0));
        assert_eq!(
            optional_u16("position", Some(u32::from(u16::MAX))).unwrap(),
            Some(u16::MAX)
        );
    }

    #[test]
    fn optional_u16_rejects_overflow_values() {
        assert_invalid_argument(
            optional_u16("position", Some(u32::from(u16::MAX) + 1)).unwrap_err(),
            "position",
        );
    }

    #[test]
    fn optional_i16_accepts_present_values_and_preserves_missing() {
        assert_eq!(optional_i16("speed", None).unwrap(), None);
        assert_eq!(
            optional_i16("speed", Some(i32::from(i16::MIN))).unwrap(),
            Some(i16::MIN)
        );
        assert_eq!(
            optional_i16("speed", Some(i32::from(i16::MAX))).unwrap(),
            Some(i16::MAX)
        );
    }

    #[test]
    fn optional_i16_rejects_overflow_values() {
        assert_invalid_argument(
            optional_i16("speed", Some(i32::from(i16::MAX) + 1)).unwrap_err(),
            "speed",
        );
        assert_invalid_argument(
            optional_i16("speed", Some(i32::from(i16::MIN) - 1)).unwrap_err(),
            "speed",
        );
    }

    #[test]
    fn required_helpers_reject_missing_values() {
        assert_invalid_argument(required_u16("position", None).unwrap_err(), "position");
        assert_invalid_argument(required_u32("frame", None).unwrap_err(), "frame");
        assert_invalid_argument(required_f32("value", None).unwrap_err(), "value");
    }

    #[test]
    fn optional_u32_preserves_presence() {
        assert_eq!(optional_u32(None), None);
        assert_eq!(optional_u32(Some(7)), Some(7));
    }

    #[test]
    fn string_helpers_validate_present_values_and_preserve_missing() {
        assert_eq!(optional_string("car_number", None).unwrap(), None);
        assert_eq!(
            optional_string("car_number", Some("012".to_string())).unwrap(),
            Some("012".to_string())
        );
        assert_invalid_argument(
            optional_string("car_number", Some(String::new())).unwrap_err(),
            "car_number",
        );
    }

    #[test]
    fn required_enum_rejects_missing_unknown_and_invalid_values() {
        assert_eq!(
            required_enum::<ReplaySearchMode>("mode", Some(ReplaySearchMode::NextLap as i32))
                .unwrap(),
            ReplaySearchMode::NextLap
        );
        assert_invalid_argument(
            required_enum::<ReplaySearchMode>("mode", None).unwrap_err(),
            "mode",
        );
        assert_invalid_argument(
            required_enum::<ReplaySearchMode>("mode", Some(0)).unwrap_err(),
            "mode",
        );
        assert_invalid_argument(
            required_enum::<ReplaySearchMode>("mode", Some(999)).unwrap_err(),
            "mode",
        );
    }

    #[test]
    fn optional_f32_accepts_finite_values_and_preserves_missing() {
        assert_eq!(optional_f32("value", None).unwrap(), None);
        assert_eq!(optional_f32("value", Some(12.5)).unwrap(), Some(12.5));
        assert_invalid_argument(
            optional_f32("value", Some(f32::INFINITY)).unwrap_err(),
            "value",
        );
        assert_invalid_argument(optional_f32("value", Some(f32::NAN)).unwrap_err(), "value");
    }

    #[test]
    fn f32_to_u16_requires_integer_in_range() {
        assert_eq!(f32_to_u16("value", 0.0).unwrap(), 0);
        assert_eq!(f32_to_u16("value", f32::from(u16::MAX)).unwrap(), u16::MAX);
        assert_invalid_argument(f32_to_u16("value", -1.0).unwrap_err(), "value");
        assert_invalid_argument(f32_to_u16("value", 1.5).unwrap_err(), "value");
        assert_invalid_argument(
            f32_to_u16("value", f32::from(u16::MAX) + 1.0).unwrap_err(),
            "value",
        );
    }
}
