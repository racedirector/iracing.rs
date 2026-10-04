/// Builds a single field through the wire-to-runtime validation boundary.
#[allow(clippy::too_many_arguments)]
pub fn field(
    name: String,
    data_type: iracing_sdk::irsdk::VariableType,
    offset: usize,
    count: usize,
    count_as_time: bool,
    units: String,
    description: String,
) -> iracing_sdk::FieldLayout {
    let header = iracing_sdk::irsdk::VariableHeader::new(
        data_type,
        i32::try_from(offset).unwrap(),
        i32::try_from(count).unwrap(),
        count_as_time,
        &name,
        &description,
        &units,
    )
    .unwrap();
    iracing_sdk::FieldLayout::try_from_header(&header, usize::MAX).unwrap()
}
/// Builds a validated layout from synthetic test field descriptions.
pub fn layout(
    fields: impl IntoIterator<Item = iracing_sdk::FieldLayout>,
    frame_size: usize,
) -> iracing_sdk::Result<iracing_sdk::TelemetryLayout> {
    let headers = fields
        .into_iter()
        .map(|field| {
            iracing_sdk::irsdk::VariableHeader::new(
                field.data_type(),
                i32::try_from(field.region().offset()).unwrap(),
                i32::try_from(field.count()).unwrap(),
                field.metadata().count_as_time(),
                field.name(),
                field.metadata().description(),
                field.metadata().unit(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    iracing_sdk::TelemetryLayout::try_from_headers(
        &iracing_sdk::VariableHeaders::from(headers),
        frame_size,
    )
}
