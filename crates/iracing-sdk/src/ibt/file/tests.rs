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

#[test]
fn direct_frames_match_mapped_owned_and_legacy_reads_with_exact_layout() -> Result<()> {
    for fixture in crate::test_utils::load_fixture_manifest()?.fixtures {
        let path = fixture.fixture_path()?;
        let mapped = IbtFile::open(&path)?;
        let owned = IbtFile::from_bytes(std::fs::read(&path)?)?;
        let reference = crate::ibt::IbtReader::open(path)?;
        for index in [0, mapped.frame_count() / 2, mapped.frame_count() - 1] {
            let mapped_frame = mapped.frame(index)?;
            let owned_frame = owned.frame(index)?;
            assert_eq!(mapped_frame.index(), index);
            assert_eq!(owned_frame.index(), index);
            assert_eq!(mapped_frame.bytes(), owned_frame.bytes());
            assert_eq!(mapped_frame.bytes(), reference.frame(index)?);
            assert!(Arc::ptr_eq(
                mapped_frame.layout(),
                mapped.telemetry_layout()
            ));
            assert!(Arc::ptr_eq(owned_frame.layout(), owned.telemetry_layout()));
            assert!(!Arc::ptr_eq(mapped_frame.layout(), owned_frame.layout()));
            assert!(mapped_frame.value("Speed")?.is_some());
            assert!(mapped_frame.value("missing variable")?.is_none());
            assert_eq!(mapped_frame.value("Speed")?, owned_frame.value("Speed")?);
        }
        assert!(mapped.frame(mapped.frame_count()).is_err());
        assert!(owned.frame(usize::MAX).is_err());
    }
    Ok(())
}

#[test]
fn ranges_validate_clamp_and_keep_independent_coordinates() -> Result<()> {
    let path = require_smallest_ibt_fixture()?;
    for file in [
        IbtFile::open(&path)?,
        IbtFile::from_bytes(std::fs::read(path)?)?,
    ] {
        let count = file.frame_count();
        for range in [0..0, count..count, 0..1, 1..3, count - 1..count] {
            let mut frames = file.frames(range.clone())?;
            assert_eq!(frames.len(), range.len());
            assert_eq!(frames.size_hint(), (range.len(), Some(range.len())));
            for index in range {
                let frame = frames.next().context("expected a recorded frame")??;
                assert_eq!(frame.index(), index);
                assert_eq!(frame.bytes(), file.frame(index)?.bytes());
                assert!(Arc::ptr_eq(frame.layout(), file.telemetry_layout()));
            }
            assert_eq!(frames.len(), 0);
            assert!(frames.next().is_none());
            assert!(frames.next().is_none());
        }
        for end in [count + 1, usize::MAX] {
            let frames = file
                .frames(count - 1..end)?
                .collect::<crate::Result<Vec<_>>>()?;
            assert_eq!(frames.len(), 1);
            assert_eq!(frames[0].index(), count - 1);
            assert!(file.frames(count..end)?.next().is_none());
        }
        let mut first = file.frames(0..2)?;
        let mut second = file.frames(count - 1..count)?;
        assert_eq!(first.next().unwrap()?.index(), 0);
        file.frame(count / 2)?;
        assert_eq!(second.next().unwrap()?.index(), count - 1);
        assert_eq!(first.next().unwrap()?.index(), 1);
        assert!(file.frames(Range { start: 2, end: 1 }).is_err());
        assert!(file.frames(count + 1..usize::MAX).is_err());
        assert_eq!(file.all_frames()?.len(), count);
    }
    Ok(())
}

#[test]
fn range_creation_is_lazy_and_read_errors_advance_only_its_iterator() -> Result<()> {
    let mut file = IbtFile::from_bytes(fixture_bytes()?)?;
    let count = file.frame_count();
    let first_end = file.physical_layout().frame(0)?.end();
    let Source::Owned(bytes) = &mut file.source else {
        unreachable!();
    };
    bytes.truncate(first_end);
    assert!(file.frames(count + 1..usize::MAX).is_err());
    assert_eq!(file.all_frames()?.len(), count);
    assert!(file.frames(count..count)?.next().is_none());
    let mut frames = file.all_frames()?;
    assert_eq!(frames.next().unwrap()?.index(), 0);
    assert!(frames.next().unwrap().is_err());
    assert_eq!(frames.len(), count - 2);
    assert!(frames.next().unwrap().is_err());
    assert_eq!(frames.len(), count - 3);
    assert_eq!(file.frame(0)?.bytes().len(), file.frame_size());
    assert!(file.frame(1).is_err());
    Ok(())
}

#[test]
fn empty_recording_has_only_empty_direct_traversal() -> Result<()> {
    let bytes = fixture_bytes()?;
    let original = IbtFile::from_bytes(bytes.clone())?;
    let file =
        IbtFile::from_bytes(bytes[..original.physical_layout().frame_data_start()].to_vec())?;
    assert!(file.frame(0).is_err());
    for range in [0..0, 0..1, 0..usize::MAX] {
        let mut frames = file.frames(range)?;
        assert_eq!(frames.len(), 0);
        assert!(frames.next().is_none());
    }
    assert!(file.frames(1..usize::MAX).is_err());
    Ok(())
}

#[test]
fn packet_bridge_transfers_exact_bytes_and_layout_without_reconstruction() -> Result<()> {
    let file = IbtFile::open(require_smallest_ibt_fixture()?)?;
    let frame = file.frame(2)?;
    let bytes = frame.clone().into_bytes();
    let packet = frame.into_packet()?;
    assert_eq!(packet.tick, 2);
    assert_eq!(
        packet.session_version,
        file.header().session_info_update as u32
    );
    assert!(Arc::ptr_eq(packet.data(), &bytes));
    assert!(Arc::ptr_eq(packet.layout(), file.telemetry_layout()));
    assert_eq!(packet.value("Speed")?, file.frame(2)?.value("Speed")?);
    Ok(())
}

#[test]
fn recorded_frame_owns_bytes_and_layout_after_file_drop() -> Result<()> {
    let file = IbtFile::open(require_smallest_ibt_fixture()?)?;
    let frame = file.frame(0)?;
    let value = frame.value("Speed")?;
    let layout = Arc::clone(file.telemetry_layout());
    drop(file);
    assert!(Arc::ptr_eq(frame.layout(), &layout));
    assert_eq!(frame.bytes().len(), layout.frame_size());
    assert_eq!(frame.value("Speed")?, value);
    assert!(frame.into_packet()?.value("Speed")?.is_some());
    Ok(())
}
