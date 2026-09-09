//! Cross-module compatibility checks against the generated IBT fixture manifest.

use super::format::extract_variable_schema;
use crate::test_utils::{IbtVariableManifest, load_fixture_manifest};
use crate::{
    DiskSubHeader, Header, VariableHeader, VariableHeaderRegion, VariableInfo, VariableType,
    irsdk::WireType,
};
use anyhow::{Context, Result, ensure};

#[test]
fn test_generated_fixture_headers_match_manifest() -> Result<()> {
    use crate::StatusField;

    let manifest = load_fixture_manifest()?;
    assert_eq!(manifest.layout.live_header_prefix_size, Header::WIRE_SIZE);
    assert_eq!(
        manifest.layout.ibt_header_size,
        Header::WIRE_SIZE + DiskSubHeader::WIRE_SIZE
    );
    assert_eq!(
        manifest.layout.disk_sub_header_size,
        DiskSubHeader::WIRE_SIZE
    );
    assert_eq!(
        manifest.layout.variable_header_size,
        VariableHeader::WIRE_SIZE
    );

    for fixture in &manifest.fixtures {
        let mut reader = std::io::BufReader::new(std::fs::File::open(fixture.fixture_path()?)?);
        let header = Header::try_from_reader(&mut reader)?;
        let disk_header = DiskSubHeader::try_from_reader(&mut reader)?;

        assert_eq!(header.version, 2);
        assert_eq!(header.status, StatusField::CONNECTED);
        assert_eq!(header.tick_rate, fixture.tick_rate);
        assert_eq!(header.variable_count, fixture.num_vars);
        assert_eq!(header.variable_header_offset, fixture.var_header_offset);
        assert_eq!(header.variable_header_offset, 144);
        assert_eq!(header.buffer_length, fixture.frame_size as i32);
        assert_eq!(header.buffer_count, fixture.num_buf);
        assert_eq!(header.session_info_len, fixture.session_info_len);
        assert_eq!(header.session_info_offset, fixture.session_info_offset);
        assert_eq!(header.session_info_update, fixture.session_info_update);

        assert_eq!(
            fixture.disk_sub_header_offset,
            header.variable_header_offset - DiskSubHeader::WIRE_SIZE as i32
        );
        assert_eq!(disk_header.start_date, fixture.disk_header.start_date);
        assert!((disk_header.start_time - fixture.disk_header.start_time).abs() < f64::EPSILON);
        assert!((disk_header.end_time - fixture.disk_header.end_time).abs() < f64::EPSILON);
        assert_eq!(disk_header.lap_count, fixture.disk_header.lap_count);
        assert_eq!(disk_header.record_count, fixture.disk_header.record_count);

        header.validate()?;
    }

    Ok(())
}

#[test]
fn test_generated_fixture_profiles_cover_increasing_shapes() -> Result<()> {
    let manifest = load_fixture_manifest()?;
    ensure!(
        manifest.fixtures.len() == 3,
        "Expected exactly three generated IBT fixtures"
    );

    let small = manifest
        .fixtures
        .iter()
        .find(|fixture| fixture.name == "profile_small")
        .context("Missing profile_small fixture")?;
    let medium = manifest
        .fixtures
        .iter()
        .find(|fixture| fixture.name == "profile_medium")
        .context("Missing profile_medium fixture")?;
    let large = manifest
        .fixtures
        .iter()
        .find(|fixture| fixture.name == "profile_large")
        .context("Missing profile_large fixture")?;

    assert!(small.num_vars < medium.num_vars);
    assert!(medium.num_vars < large.num_vars);
    assert!(small.frame_size < medium.frame_size);
    assert!(medium.frame_size < large.frame_size);
    assert!(small.num_frames < medium.num_frames);
    assert!(medium.num_frames < large.num_frames);

    Ok(())
}

fn variable_type(expected: &str) -> VariableType {
    match expected {
        "Char" => VariableType::Char,
        "Bool" => VariableType::Bool,
        "Int32" => VariableType::Int32,
        "BitField" => VariableType::BitField,
        "Float32" => VariableType::Float32,
        "Float64" => VariableType::Float64,
        other => panic!("Unsupported manifest variable type: {}", other),
    }
}

fn assert_required_variable(actual: &VariableInfo, expected: &IbtVariableManifest) {
    assert_eq!(actual.name, expected.name);
    assert_eq!(actual.data_type, variable_type(&expected.data_type));
    assert_eq!(actual.offset, expected.offset);
    assert_eq!(actual.count, expected.count);
    assert_eq!(actual.units, expected.units);
}

#[test]
fn test_generated_fixture_variables_match_manifest() -> Result<()> {
    let manifest = load_fixture_manifest()?;

    for fixture in &manifest.fixtures {
        let path = fixture.fixture_path()?;
        let mut reader = std::io::BufReader::new(
            std::fs::File::open(&path).with_context(|| format!("Opening {}", path.display()))?,
        );
        let header = Header::try_from_reader(&mut reader)?;
        let region = VariableHeaderRegion::try_from(&header)?;
        let frame_size = usize::try_from(header.buffer_length)?;
        let schema = extract_variable_schema(&mut reader, &region, frame_size)?;

        assert_eq!(schema.frame_size, fixture.frame_size);
        assert_eq!(schema.variable_count(), fixture.num_vars as usize);

        for expected in &fixture.required_variables {
            let actual = schema.variables.get(&expected.name).with_context(|| {
                format!(
                    "Fixture {} missing variable {}",
                    fixture.name, expected.name
                )
            })?;
            assert_required_variable(actual, expected);
        }
    }

    Ok(())
}
