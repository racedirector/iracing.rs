//! Independent validation of manifest-backed fixture artifacts.
//!
//! Verification is deliberately layered. It checks manifest geometry and hashes,
//! decodes foundational wire structures, compares the embedded and companion
//! YAML bytes, then consumes the same data through [`IbtReader`]. This reduces
//! the chance that matching generator and verifier mistakes accept invalid data.

use std::{fs, io::Cursor, path::Path};

use anyhow::{Context, Result, bail, ensure};
use iracing_sdk::{
    SchemaProvider,
    ibt::IbtReader,
    irsdk::{DiskSubHeader, Header, VariableHeader, WireType},
};

use crate::{VerificationReport, generate::hex_digest, model::FixtureManifest};

/// Verifies every fixture declared by the schema-version-1 manifest.
///
/// This function is read-only. The main header is decoded from bytes 0..112 and
/// the disk sub-header from bytes 112..144. Variable headers must begin at 144,
/// followed immediately by session YAML and then the declared frames.
///
/// # Errors
///
/// Returns an error when required files cannot be read, manifest paths escape
/// `repo_root`, layout/hash/header/YAML invariants disagree, SDK validation
/// fails, or `IbtReader` cannot expose the declared schema and frame count.
pub(crate) fn verify(repo_root: &Path) -> Result<VerificationReport> {
    let manifest_path = repo_root.join("test-data/ibt/manifest.json");
    let manifest_bytes = fs::read(&manifest_path)
        .with_context(|| format!("reading fixture manifest {}", manifest_path.display()))?;
    let manifest: FixtureManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("parsing fixture manifest {}", manifest_path.display()))?;
    ensure!(
        manifest.schema_version == 1,
        "{} uses unsupported schema_version {}",
        manifest_path.display(),
        manifest.schema_version
    );
    ensure!(
        manifest.fixtures.len() >= 3,
        "{} must contain at least three fixtures",
        manifest_path.display()
    );
    ensure!(
        manifest.layout.live_header_prefix_size == Header::WIRE_SIZE,
        "manifest main header size is {}, expected {}",
        manifest.layout.live_header_prefix_size,
        Header::WIRE_SIZE
    );
    ensure!(
        manifest.layout.disk_sub_header_size == DiskSubHeader::WIRE_SIZE,
        "manifest disk header size is invalid"
    );
    ensure!(
        manifest.layout.ibt_header_size == Header::WIRE_SIZE + DiskSubHeader::WIRE_SIZE,
        "manifest IBT preamble size is invalid"
    );
    ensure!(
        manifest.layout.variable_header_size == VariableHeader::WIRE_SIZE,
        "manifest variable header size is invalid"
    );

    let mut frame_count = 0usize;
    for fixture in &manifest.fixtures {
        let path = checked_join(repo_root, &fixture.path)?;
        let data =
            fs::read(&path).with_context(|| format!("reading fixture {}", path.display()))?;
        ensure!(
            hex_digest(&data) == fixture.sha256,
            "{} SHA-256 mismatch",
            path.display()
        );

        let mut cursor = Cursor::new(data.as_slice());
        let header = Header::try_from_reader(&mut cursor)
            .with_context(|| format!("decoding main header in {}", path.display()))?;
        header
            .validate_ibt()
            .with_context(|| format!("validating main header in {}", path.display()))?;
        let disk = DiskSubHeader::try_from_reader(&mut cursor)
            .with_context(|| format!("decoding disk sub-header in {}", path.display()))?;

        ensure!(
            fixture.disk_sub_header_offset == Header::WIRE_SIZE as i32,
            "{} disk sub-header offset must be {}",
            path.display(),
            Header::WIRE_SIZE
        );
        ensure!(
            fixture.var_header_offset == (Header::WIRE_SIZE + DiskSubHeader::WIRE_SIZE) as i32,
            "{} variable headers must begin at byte {}",
            path.display(),
            Header::WIRE_SIZE + DiskSubHeader::WIRE_SIZE
        );
        ensure!(
            fixture.disk_sub_header_offset
                == fixture.var_header_offset - DiskSubHeader::WIRE_SIZE as i32,
            "{} disk offset does not follow var_header_offset - disk_size",
            path.display()
        );

        compare_header(&path, &header, fixture)?;
        ensure!(
            disk.start_date == fixture.disk_header.start_date,
            "{} disk start_date mismatch",
            path.display()
        );
        ensure!(
            disk.start_time == fixture.disk_header.start_time,
            "{} disk start_time mismatch",
            path.display()
        );
        ensure!(
            disk.end_time == fixture.disk_header.end_time,
            "{} disk end_time mismatch",
            path.display()
        );
        ensure!(
            disk.lap_count == fixture.disk_header.lap_count,
            "{} disk lap_count mismatch",
            path.display()
        );
        ensure!(
            disk.record_count == fixture.disk_header.record_count,
            "{} disk record_count mismatch",
            path.display()
        );

        let variables_start = fixture.var_header_offset as usize;
        let variables_end = variables_start + fixture.num_vars as usize * VariableHeader::WIRE_SIZE;
        ensure!(
            variables_end == fixture.session_info_offset as usize,
            "{} session info must immediately follow variable headers",
            path.display()
        );
        ensure!(
            variables_end <= data.len(),
            "{} variable header region exceeds file length",
            path.display()
        );
        for (index, expected) in fixture.required_variables.iter().enumerate() {
            let start = variables_start + index * VariableHeader::WIRE_SIZE;
            let variable =
                VariableHeader::read_from_bytes(&data[start..start + VariableHeader::WIRE_SIZE])
                    .with_context(|| format!("decoding variable {index} in {}", path.display()))?;
            variable
                .validate()
                .with_context(|| format!("validating variable {index} in {}", path.display()))?;
            ensure!(
                c_string(&variable.name) == expected.name,
                "{} variable {index} name mismatch",
                path.display()
            );
            ensure!(
                variable.offset == expected.offset as i32,
                "{} variable {} offset mismatch",
                path.display(),
                expected.name
            );
            ensure!(
                variable.count == expected.count as i32,
                "{} variable {} count mismatch",
                path.display(),
                expected.name
            );
            ensure!(
                c_string(&variable.unit) == expected.units,
                "{} variable {} units mismatch",
                path.display(),
                expected.name
            );
        }

        let session_start = fixture.session_info_offset as usize;
        let session_end = session_start + fixture.session_info_len as usize;
        let expected_len = session_end + fixture.num_frames * fixture.frame_size;
        ensure!(
            data.len() == expected_len,
            "{} length is {}, expected {expected_len}",
            path.display(),
            data.len()
        );
        let yaml_path = checked_join(repo_root, &fixture.session_yaml_path)?;
        let yaml = fs::read(&yaml_path)
            .with_context(|| format!("reading session YAML {}", yaml_path.display()))?;
        ensure!(
            data[session_start..session_end] == yaml,
            "{} embedded YAML does not match {}",
            path.display(),
            yaml_path.display()
        );

        let mut reader = IbtReader::from_bytes(data)
            .with_context(|| format!("opening {} through IbtReader", path.display()))?;
        ensure!(
            reader.total_frames() == fixture.num_frames,
            "{} reader frame count mismatch",
            path.display()
        );
        ensure!(
            reader.schema().variable_count() == fixture.num_vars as usize,
            "{} reader variable count mismatch",
            path.display()
        );
        for expected in &fixture.required_variables {
            ensure!(
                reader.schema().get_variable(&expected.name).is_some(),
                "{} reader schema is missing {}",
                path.display(),
                expected.name
            );
        }
        let mut read_frames = 0usize;
        while reader.read_next_frame()?.is_some() {
            read_frames += 1;
        }
        ensure!(
            read_frames == fixture.num_frames,
            "{} yielded {read_frames} frames, expected {}",
            path.display(),
            fixture.num_frames
        );
        frame_count += read_frames;
    }

    Ok(VerificationReport {
        fixture_count: manifest.fixtures.len(),
        frame_count,
    })
}

/// Compares decoded main-header fields with one manifest entry.
///
/// # Errors
///
/// Returns an error naming the fixture and first mismatched header invariant.
fn compare_header(path: &Path, header: &Header, fixture: &crate::model::IbtFixture) -> Result<()> {
    ensure!(
        header.version == 2,
        "{} SDK version mismatch",
        path.display()
    );
    ensure!(
        header.status.is_connected(),
        "{} status is not connected",
        path.display()
    );
    ensure!(
        header.tick_rate == fixture.tick_rate,
        "{} tick rate mismatch",
        path.display()
    );
    ensure!(
        header.session_info_update == fixture.session_info_update,
        "{} session update mismatch",
        path.display()
    );
    ensure!(
        header.session_info_len == fixture.session_info_len,
        "{} session length mismatch",
        path.display()
    );
    ensure!(
        header.session_info_offset == fixture.session_info_offset,
        "{} session offset mismatch",
        path.display()
    );
    ensure!(
        header.variable_count == fixture.num_vars,
        "{} variable count mismatch",
        path.display()
    );
    ensure!(
        header.variable_header_offset == fixture.var_header_offset,
        "{} variable offset mismatch",
        path.display()
    );
    ensure!(
        header.buffer_count == fixture.num_buf,
        "{} buffer count mismatch",
        path.display()
    );
    ensure!(
        header.buffer_length == fixture.frame_size as i32,
        "{} frame size mismatch",
        path.display()
    );
    Ok(())
}

/// Resolves a manifest path below `root` without permitting absolute paths or traversal.
///
/// # Errors
///
/// Returns an error when `relative` is absolute or contains a parent-directory
/// component. Other relative components are joined without filesystem access.
fn checked_join(root: &Path, relative: &str) -> Result<std::path::PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        bail!("manifest path must remain relative to the repository: {relative}");
    }
    Ok(root.join(path))
}

/// Decodes bytes through the first NUL using replacement characters for invalid UTF-8.
fn c_string(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
