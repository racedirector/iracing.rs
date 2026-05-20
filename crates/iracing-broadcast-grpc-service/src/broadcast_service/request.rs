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
