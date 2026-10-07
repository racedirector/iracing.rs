//! Cross-module compatibility checks against the generated IBT fixture manifest.

use crate::SessionInfoRegion;
use crate::provider::VariableHeadersProvider;
use crate::test_utils::{IbtVariableManifest, load_fixture_manifest};
use crate::{
    FieldLayout, VariableHeadersRegion,
    irsdk::{DiskSubHeader, Header, VariableHeader, VariableType},
};
use crate::{TelemetryLayout, ibt::IbtReader};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeSet, fs, path::PathBuf};

#[test]
fn indexed_and_snapshot_reads_match_source_bytes() -> Result<()> {
    use zerocopy::IntoBytes;

    for fixture in load_fixture_manifest()?.fixtures {
        let path = fixture.fixture_path()?;
        let bytes = fs::read(&path)?;
        for mut reader in [
            IbtReader::open(&path)?,
            IbtReader::from_bytes(bytes.clone())?,
        ] {
            assert_eq!(reader.frame_count(), fixture.num_frames);
            assert_eq!(reader.frame_size(), fixture.frame_size);

            let selected = reader.frame_count() / 2;
            for index in [reader.frame_count() - 1, 0, selected, 1] {
                let start = reader.layout().frame_data_start() + index * fixture.frame_size;
                assert_eq!(
                    reader.frame(index)?,
                    bytes[start..start + fixture.frame_size]
                );
            }
            assert!(reader.frame(reader.frame_count()).is_err());
            assert!(reader.frame(usize::MAX).is_err());

            let session_region = reader
                .layout()
                .metadata()
                .session_info()
                .unwrap()
                .as_region();
            let session =
                crate::provider::SessionInformationBytesProvider::session_info_snapshot(&reader)?
                    .unwrap();
            assert_eq!(session.as_bytes(), &bytes[session_region.as_range()]);
            let variable_region = *reader.layout().metadata().variable_headers().unwrap();
            let headers = reader.variable_headers()?;
            assert_eq!(headers.len(), variable_region.count());
            assert_eq!(
                headers.as_slice().as_bytes(),
                &bytes[variable_region.as_region().as_range()]
            );
        }
    }
    Ok(())
}

#[test]
fn optional_metadata_and_empty_replay_follow_provider_contract() -> Result<()> {
    use crate::{LayoutProvider, provider::Provider};
    use futures::executor::block_on;
    use zerocopy::IntoBytes;

    let bytes = fs::read(crate::test_utils::require_smallest_ibt_fixture()?)?;
    let original = IbtReader::from_bytes(bytes.clone())?;
    let preamble = size_of::<Header>() + size_of::<DiskSubHeader>();
    for keep_variables in [false, true] {
        for keep_session in [false, true] {
            for keep_frames in [false, true] {
                let mut header = Header::try_from_bytes(&bytes[..size_of::<Header>()])?;
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
                let reader = IbtReader::from_bytes(data)?;
                assert_eq!(!reader.variable_headers()?.is_empty(), keep_variables);
                assert_eq!(reader.layout().frame_size(), original.layout().frame_size());
                assert_eq!(
                    crate::provider::SessionInformationBytesProvider::session_info_snapshot(
                        &reader
                    )?
                    .is_some(),
                    keep_session
                );
                let expected_frames = if keep_frames {
                    original.layout().frame_count()
                } else {
                    0
                };
                assert_eq!(reader.layout().frame_count(), expected_frames);
                let provider = crate::providers::ibt::IbtProvider::from_reader(reader);
                if keep_frames && !keep_variables {
                    let error = provider.err().unwrap().to_string();
                    assert!(error.contains("variable-header metadata"), "{error}");
                    continue;
                }
                let mut provider = provider?;
                assert_eq!(
                    provider.layout().len(),
                    if keep_variables {
                        original.header().variable_count as usize
                    } else {
                        0
                    }
                );
                assert_eq!(
                    provider.layout().frame_size(),
                    original.layout().frame_size()
                );
                assert_eq!(block_on(provider.session_yaml(0))?.is_some(), keep_session);
                let mut count = 0;
                while let Some(frame) = block_on(provider.next_frame())? {
                    assert_eq!(frame.tick as usize, count);
                    count += 1;
                }
                assert_eq!(count, expected_frames);
                assert!(block_on(provider.next_frame())?.is_none());
            }
        }
    }
    Ok(())
}

#[test]
fn snapshots_remain_owned_after_reader_drop_and_file_changes() -> Result<()> {
    use std::io::{Seek, SeekFrom, Write};
    use zerocopy::IntoBytes;

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("snapshots.ibt");
    fs::copy(crate::test_utils::require_smallest_ibt_fixture()?, &path)?;
    let reader = IbtReader::open(&path)?;
    let session =
        crate::provider::SessionInformationBytesProvider::session_info_snapshot(&reader)?.unwrap();
    let headers = reader.variable_headers()?;
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
    drop(reader);
    let mut file = fs::OpenOptions::new().write(true).open(&path)?;
    file.seek(SeekFrom::Start(u64::try_from(session_region.offset())?))?;
    file.write_all(&vec![0; session_region.len()])?;
    let first = &headers.as_slice()[0];
    let original_description = first.description().into_owned();
    let changed_description = "Changed by snapshot test";
    let changed_header = VariableHeader::new(
        first.variable_type,
        first.offset,
        first.count,
        first.count_as_time != 0,
        &first.name(),
        changed_description,
        &first.unit(),
    )?;
    file.seek(SeekFrom::Start(u64::try_from(variable_region.offset())?))?;
    file.write_all(changed_header.as_bytes())?;
    file.flush()?;
    drop(file);
    let mut reader = IbtReader::open(&path)?;

    assert!(
        crate::provider::SessionInformationBytesProvider::session_info_snapshot(&reader)?
            .unwrap()
            .as_bytes()
            .iter()
            .all(|&byte| byte == 0)
    );
    assert_eq!(
        reader.variable_headers()?.as_slice()[0].description(),
        changed_description
    );
    assert!(session.as_bytes().iter().any(|&byte| byte != 0));
    assert_eq!(headers.as_slice()[0].description(), original_description);
    assert_ne!(headers.as_slice()[0].description(), changed_description);
    assert!(!reader.frame(0)?.is_empty());

    drop(reader);
    fs::OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(u64::try_from(variable_region.offset())?)?;
    assert!(IbtReader::open(&path).is_err());
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
        assert_eq!(
            usize::try_from(header.variable_header_offset)?,
            size_of::<Header>() + size_of::<DiskSubHeader>()
        );
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

fn assert_required_variable(actual: &FieldLayout, expected: &IbtVariableManifest) {
    assert_eq!(actual.name(), expected.name);
    assert_eq!(actual.data_type(), variable_type(&expected.data_type));
    assert_eq!(actual.region().offset(), expected.offset);
    assert_eq!(actual.count(), expected.count);
    assert_eq!(actual.metadata().unit(), expected.units);
}

#[test]
fn test_generated_fixture_variables_match_manifest() -> Result<()> {
    let manifest = load_fixture_manifest()?;

    for fixture in &manifest.fixtures {
        let path = fixture.fixture_path()?;
        let reader = IbtReader::open(&path)?;
        let snapshot = reader.variable_headers()?;
        let layout = TelemetryLayout::try_from_headers(&snapshot, reader.layout().frame_size())?;

        assert_eq!(layout.frame_size(), fixture.frame_size);
        assert_eq!(layout.len(), fixture.num_vars as usize);

        for expected in &fixture.required_variables {
            let actual = layout
                .field_by_name(&expected.name)
                .map(|(_, field)| field)
                .with_context(|| {
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
