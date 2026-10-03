use iracing_sdk::{ByteRegion, FieldLayout, IbtLayout, TelemetryLayout};
use serde::Serialize;
use std::fmt::Write;

/// CLI output only; runtime layout types do not need a serialization contract.
#[derive(Debug, Serialize)]
struct Region {
    offset: usize,
    end: usize,
    size: usize,
}

impl From<ByteRegion> for Region {
    fn from(region: ByteRegion) -> Self {
        Self {
            offset: region.offset(),
            end: region.end(),
            size: region.len(),
        }
    }
}

#[derive(Debug, Serialize)]
struct PhysicalLayout {
    source_length: usize,
    header: Region,
    disk_sub_header: Region,
    preamble: Region,
    variable_headers: Option<Region>,
    session_info: Option<Region>,
    frame_data: Region,
    frame_size: usize,
    frame_count: usize,
}

#[derive(Debug, Serialize)]
struct Field<'a> {
    offset: usize,
    end: usize,
    size: usize,
    data_type: iracing_irsdk::VariableType,
    count: usize,
    name: &'a str,
    unit: &'a str,
    description: &'a str,
    count_as_time: bool,
}

impl<'a> From<&'a FieldLayout> for Field<'a> {
    fn from(field: &'a FieldLayout) -> Self {
        let region = field.region().as_region();
        Self {
            offset: region.offset(),
            end: region.end(),
            size: region.len(),
            data_type: field.data_type(),
            count: field.count(),
            name: field.name(),
            unit: field.metadata().unit(),
            description: field.metadata().description(),
            count_as_time: field.metadata().count_as_time(),
        }
    }
}

#[derive(Debug, Serialize)]
struct FrameLayout<'a> {
    frame_size: usize,
    fields: Vec<Field<'a>>,
}

#[derive(Debug, Serialize)]
pub(super) struct Inspection<'a> {
    ibt_layout: PhysicalLayout,
    telemetry_frame_layout: FrameLayout<'a>,
}

impl<'a> Inspection<'a> {
    pub(super) fn new(physical: &IbtLayout, telemetry: &'a TelemetryLayout) -> Self {
        Self {
            ibt_layout: PhysicalLayout {
                source_length: physical.source_len(),
                header: physical.header_region().into(),
                disk_sub_header: physical.disk_header_region().into(),
                preamble: physical.preamble_region().into(),
                variable_headers: physical
                    .metadata()
                    .variable_headers()
                    .map(|region| region.as_region().into()),
                session_info: physical
                    .metadata()
                    .session_info()
                    .map(|region| region.as_region().into()),
                frame_data: physical.frames().as_region().into(),
                frame_size: physical.frame_size(),
                frame_count: physical.frame_count(),
            },
            telemetry_frame_layout: FrameLayout {
                frame_size: telemetry.frame_size(),
                fields: telemetry.fields().map(|(_, field)| field.into()).collect(),
            },
        }
    }

    pub(super) fn text(&self) -> String {
        let physical = &self.ibt_layout;
        let mut text = String::from("IBT layout (source byte offsets)\n");
        writeln!(text, "  source length: {}", physical.source_length).unwrap();
        for (label, region) in [
            ("header", Some(&physical.header)),
            ("disk-sub-header", Some(&physical.disk_sub_header)),
            ("preamble", Some(&physical.preamble)),
            ("variable-header", physical.variable_headers.as_ref()),
            ("session-info", physical.session_info.as_ref()),
            ("frame-data", Some(&physical.frame_data)),
        ] {
            match region {
                Some(region) => writeln!(
                    text,
                    "  {label} region: {}..{} ({} bytes)",
                    region.offset, region.end, region.size
                )
                .unwrap(),
                None => writeln!(text, "  {label} region: absent").unwrap(),
            }
        }
        writeln!(text, "  frame size: {}", physical.frame_size).unwrap();
        writeln!(text, "  frame count: {}", physical.frame_count).unwrap();
        writeln!(
            text,
            "\nTelemetry frame layout (frame-relative byte offsets)"
        )
        .unwrap();
        writeln!(text, "  offset  end  size  type  count  name").unwrap();
        for field in &self.telemetry_frame_layout.fields {
            writeln!(
                text,
                "  {}  {}  {}  {:?}  {}  {}",
                field.offset, field.end, field.size, field.data_type, field.count, field.name
            )
            .unwrap();
        }
        text.trim_end().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_sdk::{
        VariableHeaders, ibt::IbtReader, provider::VariableHeadersProvider,
        test_utils::require_smallest_ibt_fixture,
    };

    #[test]
    fn inspection_uses_runtime_geometry_and_published_header_order() -> anyhow::Result<()> {
        let path = require_smallest_ibt_fixture()?;
        let reader = IbtReader::open(&path)?;
        let headers = reader.variable_headers()?;
        let telemetry = TelemetryLayout::try_from_headers(&headers, reader.frame_size())?;
        let inspection = Inspection::new(reader.layout(), &telemetry);
        let physical = &inspection.ibt_layout;
        assert_eq!(
            physical.source_length,
            std::fs::metadata(path)?.len() as usize
        );
        assert_eq!(physical.frame_count, reader.frame_count());
        assert_eq!(
            physical.frame_data.end,
            reader.layout().frames().as_region().end()
        );
        for (field, header) in inspection
            .telemetry_frame_layout
            .fields
            .iter()
            .zip(headers.iter())
        {
            assert_eq!(field.name, header.name());
            let (_, runtime) = telemetry.field_by_name(field.name).unwrap();
            assert_eq!(field.offset, runtime.region().offset());
            assert_eq!(field.end, runtime.region().as_region().end());
            assert_eq!(field.size, runtime.region().len());
            assert_eq!(field.count, runtime.count());
            assert_eq!(field.unit, runtime.metadata().unit());
        }
        let text = inspection.text();
        assert!(text.contains("IBT layout (source byte offsets)"));
        assert!(text.contains("Telemetry frame layout (frame-relative byte offsets)"));
        assert!(text.contains("Speed"));

        // Deliberately reverse authoritative headers to catch accidental sorting.
        let mut reversed: Vec<_> = headers.iter().copied().collect();
        reversed.reverse();
        let reversed = VariableHeaders::from(reversed);
        let telemetry = TelemetryLayout::try_from_headers(&reversed, reader.frame_size())?;
        let inspection = Inspection::new(reader.layout(), &telemetry);
        let names: Vec<_> = inspection
            .telemetry_frame_layout
            .fields
            .iter()
            .map(|field| field.name)
            .collect();
        let published: Vec<_> = telemetry.fields().map(|(_, field)| field.name()).collect();
        assert_eq!(names, published);
        assert!(
            inspection.telemetry_frame_layout.fields[0].offset
                > inspection
                    .telemetry_frame_layout
                    .fields
                    .last()
                    .unwrap()
                    .offset
        );
        Ok(())
    }
}
