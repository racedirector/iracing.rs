use super::*;
use crate::{irsdk::VariableHeader, test_utils::require_smallest_ibt_fixture};
use anyhow::{Context, Result};
use std::mem::offset_of;
use zerocopy::IntoBytes;

fn fixture_bytes() -> Result<Vec<u8>> {
    Ok(std::fs::read(require_smallest_ibt_fixture()?)?)
}

fn write_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + size_of::<i32>()].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn mapped_and_owned_metadata_and_layouts_match() -> Result<()> {
    for fixture in crate::test_utils::load_fixture_manifest()?.fixtures {
        let path = fixture.fixture_path()?;
        let mapped = IbtFile::open(&path)?;
        let owned = IbtFile::from_bytes(std::fs::read(path)?)?;
        assert!(matches!(mapped.source, Source::Mapped(_)));
        assert!(matches!(owned.source, Source::Owned(_)));
        assert_eq!(mapped.header().as_bytes(), owned.header().as_bytes());
        assert_eq!(
            mapped.disk_header().as_bytes(),
            owned.disk_header().as_bytes()
        );
        assert_eq!(
            mapped.physical_layout().frames(),
            owned.physical_layout().frames()
        );
        assert_eq!(
            mapped.variable_headers().as_slice().as_bytes(),
            owned.variable_headers().as_slice().as_bytes()
        );
        assert_eq!(
            mapped.session_info_bytes().unwrap().as_bytes(),
            owned.session_info_bytes().unwrap().as_bytes()
        );
        assert_eq!(mapped.frame_count(), fixture.num_frames);
        assert_eq!(mapped.frame_size(), fixture.frame_size);
        assert_eq!(mapped.telemetry_layout().frame_size(), mapped.frame_size());
        assert_eq!(mapped.telemetry_layout().len(), fixture.num_vars as usize);
        for (wire, (_, field)) in mapped
            .variable_headers()
            .iter()
            .zip(mapped.telemetry_layout().fields())
        {
            assert_eq!(field.name(), wire.name());
            assert_eq!(field.region().offset(), wire.offset as usize);
        }
    }
    Ok(())
}

#[test]
fn owned_source_allocation_is_transferred_without_copying_frames() -> Result<()> {
    let bytes = fixture_bytes()?;
    let pointer = bytes.as_ptr();
    let file = IbtFile::from_bytes(bytes)?;
    let Source::Owned(bytes) = &file.source else {
        panic!("expected owned source");
    };
    assert_eq!(pointer, bytes.as_ptr());
    assert_eq!(bytes.len(), file.physical_layout().source_len());
    Ok(())
}

#[test]
fn metadata_snapshots_are_exact_owned_and_reused() -> Result<()> {
    let bytes = fixture_bytes()?;
    let mut file = IbtFile::from_bytes(bytes.clone())?;
    let variable_region = file
        .physical_layout()
        .metadata()
        .variable_headers()
        .unwrap()
        .as_region()
        .as_range();
    let session_region = file
        .physical_layout()
        .metadata()
        .session_info()
        .unwrap()
        .as_region()
        .as_range();
    let variables = file.variable_headers().as_slice().as_bytes().to_vec();
    let session = file.session_info_bytes().unwrap().as_bytes().to_vec();
    assert_eq!(variables, bytes[variable_region]);
    assert_eq!(session, bytes[session_region]);
    let layout = Arc::clone(file.telemetry_layout());
    let variable_pointer = file.variable_headers().as_slice().as_ptr();
    let session_pointer = file.session_info_bytes().unwrap().as_bytes().as_ptr();
    // Fault injection only into owned storage. Mapped files remain immutable.
    match &mut file.source {
        Source::Owned(bytes) => bytes.fill(0xff),
        Source::Mapped(_) => unreachable!(),
    }
    assert_eq!(file.variable_headers().as_slice().as_bytes(), variables);
    assert_eq!(file.session_info_bytes().unwrap().as_bytes(), session);
    assert_eq!(
        file.variable_headers().as_slice().as_ptr(),
        variable_pointer
    );
    assert_eq!(
        file.session_info_bytes().unwrap().as_bytes().as_ptr(),
        session_pointer
    );
    assert!(Arc::ptr_eq(&layout, file.telemetry_layout()));
    Ok(())
}

#[test]
fn session_snapshot_preserves_padding_and_does_not_parse_or_sanitize() -> Result<()> {
    let mut bytes = fixture_bytes()?;
    let original = IbtFile::from_bytes(bytes.clone())?;
    let region = original
        .physical_layout()
        .metadata()
        .session_info()
        .unwrap()
        .as_region()
        .as_range();
    let payload = b"\x01not YAML\0\xffpadding";
    bytes[region.clone()].fill(0xaa);
    bytes[region.start..region.start + payload.len()].copy_from_slice(payload);
    let file = IbtFile::from_bytes(bytes.clone())?;
    let snapshot = file.session_info_bytes().unwrap();
    assert_eq!(snapshot.as_bytes(), &bytes[region]);
    assert_eq!(snapshot.payload().decode(), "\x01not YAML");
    Ok(())
}

#[test]
fn rejects_frames_without_variable_metadata() -> Result<()> {
    let mut bytes = fixture_bytes()?;
    write_i32(&mut bytes, offset_of!(Header, variable_count), 0);
    let error = IbtFile::from_bytes(bytes).unwrap_err().to_string();
    assert!(
        error.contains("Telemetry frames require variable-header metadata"),
        "{error}"
    );
    Ok(())
}

#[test]
fn zero_frames_preserve_present_metadata_or_allow_absent_metadata() -> Result<()> {
    let bytes = fixture_bytes()?;
    let original = IbtFile::from_bytes(bytes.clone())?;
    let empty =
        IbtFile::from_bytes(bytes[..original.physical_layout().frame_data_start()].to_vec())?;
    assert_eq!(empty.frame_count(), 0);
    assert!(!empty.variable_headers().is_empty());
    assert!(empty.session_info_bytes().is_some());

    let mut absent = bytes;
    write_i32(&mut absent, offset_of!(Header, variable_count), 0);
    write_i32(&mut absent, offset_of!(Header, session_info_length), 0);
    absent.truncate(size_of::<Header>() + size_of::<DiskSubHeader>());
    let empty = IbtFile::from_bytes(absent)?;
    assert_eq!(empty.frame_count(), 0);
    assert!(empty.variable_headers().is_empty());
    assert!(empty.session_info_bytes().is_none());
    assert!(empty.telemetry_layout().is_empty());
    assert_eq!(empty.telemetry_layout().frame_size(), original.frame_size());
    Ok(())
}

#[test]
fn construction_rejects_semantic_fields_outside_physical_frame() -> Result<()> {
    let mut bytes = fixture_bytes()?;
    let original = IbtFile::from_bytes(bytes.clone())?;
    let offset = original
        .physical_layout()
        .metadata()
        .variable_headers()
        .unwrap()
        .as_region()
        .offset()
        + offset_of!(VariableHeader, offset);
    write_i32(&mut bytes, offset, original.frame_size() as i32);
    let error = IbtFile::from_bytes(bytes).unwrap_err().to_string();
    assert!(error.contains("past frame size"), "{error}");
    Ok(())
}

#[test]
fn mapped_and_owned_reject_malformed_sources_equivalently() -> Result<()> {
    let original = fixture_bytes()?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("malformed.ibt");
    let preamble_len = size_of::<Header>() + size_of::<DiskSubHeader>();
    let mut cases = vec![original[..original.len() - 1].to_vec()];
    for len in 1..preamble_len {
        cases.push(original[..len].to_vec());
    }
    for (offset, value) in [
        (offset_of!(Header, variable_header_offset), i32::MAX),
        (offset_of!(Header, session_info_offset), i32::MAX),
        (offset_of!(Header, buffer_length), 0),
        (offset_of!(Header, variable_count), 0),
    ] {
        let mut bytes = original.clone();
        write_i32(&mut bytes, offset, value);
        cases.push(bytes);
    }
    let file = IbtFile::from_bytes(original.clone())?;
    let variable_start = file
        .physical_layout()
        .metadata()
        .variable_headers()
        .unwrap()
        .as_region()
        .offset();
    for (offset, value) in [
        (offset_of!(VariableHeader, offset), file.frame_size() as i32),
        (offset_of!(VariableHeader, variable_type), i32::MAX),
        (offset_of!(VariableHeader, count), 0),
    ] {
        let mut bytes = original.clone();
        write_i32(&mut bytes, variable_start + offset, value);
        cases.push(bytes);
    }
    for bytes in cases {
        std::fs::write(&path, &bytes)?;
        let mapped_error = IbtFile::open(&path).err().context("mapped must fail")?;
        let owned_error = IbtFile::from_bytes(bytes)
            .err()
            .context("owned must fail")?;
        assert_eq!(mapped_error.to_string(), owned_error.to_string());
    }
    std::fs::write(&path, [])?;
    assert!(IbtFile::open(&path).is_err());
    assert!(IbtFile::from_bytes(Vec::new()).is_err());
    Ok(())
}

#[test]
fn advisory_disk_counts_and_tick_rate_do_not_override_physical_geometry() -> Result<()> {
    let original = fixture_bytes()?;
    let reference = IbtFile::from_bytes(original.clone())?;
    for record_count in [-1, 0, 1, i32::MAX] {
        let mut bytes = original.clone();
        write_i32(
            &mut bytes,
            size_of::<Header>() + offset_of!(DiskSubHeader, record_count),
            record_count,
        );
        let file = IbtFile::from_bytes(bytes)?;
        assert_eq!(file.frame_count(), reference.frame_count());
        assert_eq!(file.disk_header().record_count, record_count);
        assert_eq!(file.tick_rate(), reference.header().tick_rate);
    }
    Ok(())
}

#[test]
fn dropping_file_releases_mapping_resources() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("drop.ibt");
    std::fs::copy(require_smallest_ibt_fixture()?, &path)?;
    let file = IbtFile::open(&path)?;
    let session = file.session_info_bytes().unwrap().clone();
    let layout = Arc::clone(file.telemetry_layout());
    drop(file);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(0)?;
    std::fs::remove_file(&path)?;
    assert!(!session.as_bytes().is_empty());
    assert!(!layout.is_empty());
    Ok(())
}
