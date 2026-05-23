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
    match value {
        Some(value) => u32_to_u16(field_name, value),
        None => missing(field_name),
    }
}

pub(crate) fn required_i16(field_name: &'static str, value: Option<i32>) -> Result<i16, Status> {
    match value {
        Some(value) => i32_to_i16(field_name, value),
        None => missing(field_name),
    }
}

pub(crate) fn required_u32(field_name: &'static str, value: Option<u32>) -> Result<u32, Status> {
    value.ok_or_else(|| missing_status(field_name))
}

pub(crate) fn required_bool(field_name: &'static str, value: Option<bool>) -> Result<bool, Status> {
    value.ok_or_else(|| missing_status(field_name))
}

pub(crate) fn required_string(
    field_name: &'static str,
    value: Option<String>,
) -> Result<String, Status> {
    match value {
        Some(value) if !value.is_empty() => Ok(value),
        Some(_) => Err(Status::invalid_argument(format!(
            "`{field_name}` must not be empty"
        ))),
        None => missing(field_name),
    }
}

pub(crate) fn required_enum<E>(field_name: &'static str, value: Option<i32>) -> Result<E, Status>
where
    E: TryFrom<i32>,
{
    let value = value.ok_or_else(|| missing_status(field_name))?;

    if value == 0 {
        return Err(Status::invalid_argument(format!(
            "`{field_name}` must not be UNKNOWN"
        )));
    }

    E::try_from(value).map_err(|_| {
        Status::invalid_argument(format!("Invalid `{field_name}` enum value: {value}"))
    })
}

pub(crate) fn required_f32(field_name: &'static str, value: Option<f32>) -> Result<f32, Status> {
    let value = value.ok_or_else(|| missing_status(field_name))?;

    if value.is_finite() {
        Ok(value)
    } else {
        Err(Status::invalid_argument(format!(
            "`{field_name}` must be finite, got {value}"
        )))
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

fn missing<T>(field_name: &'static str) -> Result<T, Status> {
    Err(missing_status(field_name))
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
    fn required_u16_accepts_present_values_in_range() {
        assert_eq!(required_u16("position", Some(0)).unwrap(), 0);
        assert_eq!(
            required_u16("position", Some(u32::from(u16::MAX))).unwrap(),
            u16::MAX
        );
    }

    #[test]
    fn required_u16_rejects_missing_and_overflow_values() {
        assert_invalid_argument(required_u16("position", None).unwrap_err(), "position");
        assert_invalid_argument(
            required_u16("position", Some(u32::from(u16::MAX) + 1)).unwrap_err(),
            "position",
        );
    }

    #[test]
    fn required_i16_accepts_present_values_in_range() {
        assert_eq!(
            required_i16("speed", Some(i32::from(i16::MIN))).unwrap(),
            i16::MIN
        );
        assert_eq!(
            required_i16("speed", Some(i32::from(i16::MAX))).unwrap(),
            i16::MAX
        );
    }

    #[test]
    fn required_i16_rejects_missing_and_overflow_values() {
        assert_invalid_argument(required_i16("speed", None).unwrap_err(), "speed");
        assert_invalid_argument(
            required_i16("speed", Some(i32::from(i16::MAX) + 1)).unwrap_err(),
            "speed",
        );
        assert_invalid_argument(
            required_i16("speed", Some(i32::from(i16::MIN) - 1)).unwrap_err(),
            "speed",
        );
    }

    #[test]
    fn required_string_rejects_missing_and_empty_values() {
        assert_eq!(
            required_string("car_number", Some("012".to_string())).unwrap(),
            "012"
        );
        assert_invalid_argument(
            required_string("car_number", None).unwrap_err(),
            "car_number",
        );
        assert_invalid_argument(
            required_string("car_number", Some(String::new())).unwrap_err(),
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
    fn required_f32_accepts_only_finite_values() {
        assert_eq!(required_f32("value", Some(12.5)).unwrap(), 12.5);
        assert_invalid_argument(required_f32("value", None).unwrap_err(), "value");
        assert_invalid_argument(
            required_f32("value", Some(f32::INFINITY)).unwrap_err(),
            "value",
        );
        assert_invalid_argument(required_f32("value", Some(f32::NAN)).unwrap_err(), "value");
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
