//! Cross-module compatibility checks against the generated IBT fixture manifest.

use super::format::extract_variable_schema;
use crate::SessionInfoRegion;
use crate::test_utils::{IbtVariableManifest, load_fixture_manifest};
use crate::{
    VariableHeadersRegion, VariableInfo,
    irsdk::{DiskSubHeader, Header, VariableHeader, VariableType},
};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeSet, fs, path::PathBuf};

#[test]
fn indexed_and_snapshot_reads_preserve_legacy_replay() -> Result<()> {
    use crate::{SchemaProvider, ibt::IbtReader};
    use zerocopy::IntoBytes;

    for fixture in load_fixture_manifest()?.fixtures {
        let path = fixture.fixture_path()?;
        let bytes = fs::read(&path)?;
        for mut reader in [
            IbtReader::open(&path)?,
            IbtReader::from_bytes(bytes.clone())?,
        ] {
            assert_eq!(reader.layout().frame_count(), reader.total_frames());
            assert_eq!(reader.total_frames(), fixture.num_frames);
            assert_eq!(reader.schema().frame_size, fixture.frame_size);
            let first = reader.frame(0)?;
            assert_eq!(reader.current_frame(), 0);
            assert_eq!(reader.read_next_frame()?.unwrap().0, first);

            let selected = reader.total_frames() / 2;
            reader.seek_to_frame(selected)?;
            for index in [reader.total_frames() - 1, 0, selected, 1] {
                let start = reader.layout().frame_data_start() + index * fixture.frame_size;
                assert_eq!(
                    reader.frame(index)?,
                    bytes[start..start + fixture.frame_size]
                );
                assert_eq!(reader.current_frame(), selected);
            }
            assert!(reader.frame(reader.total_frames()).is_err());
            assert!(reader.frame(usize::MAX).is_err());

            let session_region = reader
                .layout()
                .metadata()
                .session_info()
                .unwrap()
                .as_region();
            let session = reader.session_info_snapshot()?.unwrap();
            assert_eq!(session.as_bytes(), &bytes[session_region.as_range()]);
            let variable_region = *reader.layout().metadata().variable_headers().unwrap();
            let headers = reader.variable_headers_snapshot()?.unwrap();
            assert_eq!(headers.len(), variable_region.count());
            assert_eq!(
                headers.as_slice().as_bytes(),
                &bytes[variable_region.as_region().as_range()]
            );
            assert_eq!(reader.current_frame(), selected);
            let (frame, tick, version) = reader.read_next_frame()?.unwrap();
            let start = reader.layout().frame_data_start() + selected * fixture.frame_size;
            assert_eq!(frame, bytes[start..start + fixture.frame_size]);
            assert_eq!(tick as usize, selected);
            assert_eq!(version, reader.header().session_info_update as u32);
            assert_eq!(reader.current_frame(), selected + 1);

            reader.seek_to_frame(reader.total_frames() - 1)?;
            reader.read_next_frame()?.unwrap();
            reader.frame(0)?;
            reader.session_info_snapshot()?;
            assert!(reader.read_next_frame()?.is_none());
            assert_eq!(reader.current_frame(), reader.total_frames());
        }
    }
    Ok(())
}

#[test]
fn absent_metadata_preserves_empty_legacy_schema_and_optional_snapshots() -> Result<()> {
    use crate::{SchemaProvider, ibt::IbtReader};
    use zerocopy::IntoBytes;

    let bytes = fs::read(crate::test_utils::require_smallest_ibt_fixture()?)?;
    let original = IbtReader::from_bytes(bytes.clone())?;
    let preamble = size_of::<Header>() + size_of::<DiskSubHeader>();
    for keep_variables in [false, true] {
        for keep_session in [false, true] {
            for keep_frames in [false, true] {
                let mut header = Header::try_from_reader(&mut std::io::Cursor::new(&bytes))?;
                let mut data = bytes[..preamble].to_vec();
                header.variable_header_offset = 0;
                header.variable_count = 0;
                header.session_info_offset = 0;
                header.session_info_length = 0;
                if keep_variables {
                    let region = original.layout().metadata().variable_headers().unwrap();
                    header.variable_header_offset = i32::try_from(data.len())?;
                    header.variable_count = i32::try_from(region.count())?;
                    data.extend_from_slice(&bytes[region.as_region().as_range()]);
                }
                if keep_session {
                    let region = original.layout().metadata().session_info().unwrap();
                    header.session_info_offset = i32::try_from(data.len())?;
                    header.session_info_length = i32::try_from(region.as_region().len())?;
                    data.extend_from_slice(&bytes[region.as_region().as_range()]);
                }
                if keep_frames {
                    data.extend_from_slice(
                        &bytes[original.layout().frames().as_region().as_range()],
                    );
                }
                data[..size_of::<Header>()].copy_from_slice(header.as_bytes());
                let mut reader = IbtReader::from_bytes(data)?;
                assert_eq!(
                    reader.variable_headers_snapshot()?.is_some(),
                    keep_variables
                );
                assert_eq!(
                    reader.schema().variable_count(),
                    if keep_variables {
                        original.schema().variable_count()
                    } else {
                        0
                    }
                );
                assert_eq!(reader.schema().frame_size, original.schema().frame_size);
                assert_eq!(reader.session_info_snapshot()?.is_some(), keep_session);
                assert_eq!(reader.session_info_buffer().is_some(), keep_session);
                assert_eq!(reader.session_yaml().is_some(), keep_session);
                assert_eq!(
                    reader.total_frames(),
                    if keep_frames {
                        original.total_frames()
                    } else {
                        0
                    }
                );
                assert_eq!(reader.current_frame(), 0);
                assert_eq!(reader.read_next_frame()?.is_some(), keep_frames);
            }
        }
    }
    Ok(())
}

#[test]
fn fresh_snapshots_reread_source_but_leave_legacy_caches_owned() -> Result<()> {
    use crate::{SchemaProvider, ibt::IbtReader};
    use std::io::{Seek, SeekFrom, Write};
    use zerocopy::IntoBytes;

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("snapshots.ibt");
    fs::copy(crate::test_utils::require_smallest_ibt_fixture()?, &path)?;
    let mut reader = IbtReader::open(&path)?;
    let session = reader.session_info_snapshot()?.unwrap();
    let headers = reader.variable_headers_snapshot()?.unwrap();
    let session_region = reader
        .layout()
        .metadata()
        .session_info()
        .unwrap()
        .as_region();
    let variable_region = reader
        .layout()
        .metadata()
        .variable_headers()
        .unwrap()
        .as_region();
    let mut file = fs::OpenOptions::new().write(true).open(&path)?;
    file.seek(SeekFrom::Start(u64::try_from(session_region.offset())?))?;
    file.write_all(&vec![0; session_region.len()])?;
    let mut changed_headers = headers.as_slice().as_bytes().to_vec();
    // Change a description byte, preserving all wire and schema geometry.
    let description = std::mem::offset_of!(VariableHeader, description);
    changed_headers[description] = b'!';
    file.seek(SeekFrom::Start(u64::try_from(variable_region.offset())?))?;
    file.write_all(&changed_headers)?;
    file.flush()?;

    assert!(
        reader
            .session_info_snapshot()?
            .unwrap()
            .as_bytes()
            .iter()
            .all(|&byte| byte == 0)
    );
    assert_eq!(
        reader
            .variable_headers_snapshot()?
            .unwrap()
            .as_slice()
            .as_bytes(),
        changed_headers
    );
    assert_eq!(
        reader.session_info_buffer().unwrap().as_bytes(),
        session.as_bytes()
    );
    assert_ne!(headers.as_slice().as_bytes(), changed_headers);
    assert_eq!(reader.schema().variable_count(), headers.len());
    assert_eq!(reader.current_frame(), 0);
    assert!(reader.read_next_frame()?.is_some());

    file.set_len(u64::try_from(variable_region.offset())?)?;
    assert!(reader.variable_headers_snapshot().is_err());
    assert!(reader.frame(0).is_err());
    assert_eq!(reader.current_frame(), 1);
    Ok(())
}

fn representative_ibt_paths() -> Result<Vec<PathBuf>> {
    let test_data = crate::test_utils::get_test_data_dir()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let mut paths = BTreeSet::new();

    for directory in [test_data.clone(), test_data.join("ibt")] {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.extension().is_some_and(|extension| extension == "ibt") {
                paths.insert(path);
            }
        }
    }

    Ok(paths.into_iter().collect())
}

#[test]
fn representative_fixtures_follow_frame_region_semantics() -> Result<()> {
    let paths = representative_ibt_paths()?;
    ensure!(!paths.is_empty(), "Expected representative IBT fixtures");

    for path in paths {
        let mut reader = std::io::BufReader::new(
            fs::File::open(&path).with_context(|| format!("Opening {}", path.display()))?,
        );
        let source_len = usize::try_from(reader.get_ref().metadata()?.len())?;
        let header = Header::try_from_reader(&mut reader)?;
        let disk_header = DiskSubHeader::try_from_reader(&mut reader)?;
        let headers_region = VariableHeadersRegion::try_from(&header)?;
        let session_info_region = SessionInfoRegion::try_from_header(&header)?;

        let variable_end = headers_region.end();

        let frame_start = if let Some(session_region) = session_info_region {
            variable_end.max(session_region.end())
        } else {
            variable_end
        };

        let frame_size = usize::try_from(header.buffer_length)?;
        let telemetry_bytes = source_len
            .checked_sub(frame_start)
            .with_context(|| format!("Frame start exceeds source length for {}", path.display()))?;

        ensure!(frame_size > 0, "{} has zero frame size", path.display());
        assert_eq!(
            telemetry_bytes % frame_size,
            0,
            "{} has a partial trailing frame",
            path.display()
        );
        assert_eq!(
            telemetry_bytes / frame_size,
            usize::try_from(disk_header.record_count)?,
            "{} record_count disagrees with EOF-derived frame count",
            path.display()
        );
    }

    Ok(())
}

#[test]
fn test_generated_fixture_headers_match_manifest() -> Result<()> {
    use crate::irsdk::StatusField;

    let manifest = load_fixture_manifest()?;
    assert_eq!(manifest.layout.live_header_prefix_size, size_of::<Header>());
    assert_eq!(
        manifest.layout.ibt_header_size,
        size_of::<Header>() + size_of::<DiskSubHeader>()
    );
    assert_eq!(
        manifest.layout.disk_sub_header_size,
        size_of::<DiskSubHeader>()
    );
    assert_eq!(
        manifest.layout.variable_header_size,
        size_of::<VariableHeader>()
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
        assert_eq!(header.session_info_length, fixture.session_info_len);
        assert_eq!(header.session_info_offset, fixture.session_info_offset);
        assert_eq!(header.session_info_update, fixture.session_info_update);

        assert_eq!(
            fixture.disk_sub_header_offset,
            header.variable_header_offset - size_of::<DiskSubHeader>() as i32
        );
        assert_eq!(disk_header.start_date, fixture.disk_header.start_date);
        assert!((disk_header.start_time - fixture.disk_header.start_time).abs() < f64::EPSILON);
        assert!((disk_header.end_time - fixture.disk_header.end_time).abs() < f64::EPSILON);
        assert_eq!(disk_header.lap_count, fixture.disk_header.lap_count);
        assert_eq!(disk_header.record_count, fixture.disk_header.record_count);
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
        "Char" => VariableType::Character,
        "Bool" => VariableType::Boolean,
        "Int32" => VariableType::Integer,
        "BitField" => VariableType::BitField,
        "Float32" => VariableType::Float,
        "Float64" => VariableType::Double,
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
        let region = VariableHeadersRegion::try_from(&header)?;
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
