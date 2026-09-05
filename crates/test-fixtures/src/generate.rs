//! Deterministic construction of session YAML, IBT bytes, and the manifest.
//!
//! SDK structure sizes come from [`WireType::WIRE_SIZE`]. The module keeps the
//! 112-byte main header separate from the 144-byte composite IBT preamble to
//! prevent variable headers from overlapping the disk sub-header.

use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use iracing_sdk::irsdk::{
    DiskSubHeader, Header, StatusField, VariableBuffer, VariableHeader, VariableType, WireType,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sha2::{Digest, Sha256};

use crate::{
    GenerationReport,
    model::{
        FixtureManifest, FixtureManifestLayout, IbtDiskHeaderManifest, IbtFixture,
        IbtVariableManifest, Profile, Variable, profiles,
    },
};

/// Size of the SDK header common to live and recorded telemetry.
const MAIN_HEADER_SIZE: usize = Header::WIRE_SIZE;
/// Size of the metadata present only in recorded IBT files.
const DISK_HEADER_SIZE: usize = DiskSubHeader::WIRE_SIZE;
/// Offset at which the first variable header begins in generated IBT files.
const IBT_PREAMBLE_SIZE: usize = MAIN_HEADER_SIZE + DISK_HEADER_SIZE;

/// One completely materialized output waiting to be written below the root.
struct Artifact {
    /// Repository-relative destination path.
    relative_path: String,
    /// Complete file contents.
    bytes: Vec<u8>,
}

/// Generates and writes every canonical artifact below `repo_root`.
///
/// All profiles, IBT bytes, YAML bytes, hashes, and manifest bytes are built
/// successfully before the first file is written. This prevents profile or
/// serialization failures from producing a partially computed artifact set,
/// though an operating-system write failure can still interrupt the final write
/// loop.
///
/// # Errors
///
/// Returns an error for invalid profile geometry, SDK wire construction or
/// encoding failures, manifest serialization failures, and filesystem errors.
pub(crate) fn generate(repo_root: &Path) -> Result<GenerationReport> {
    let profiles = profiles();
    let mut artifacts = Vec::with_capacity(profiles.len() * 2 + 1);
    let mut fixtures = Vec::with_capacity(profiles.len());

    for profile in &profiles {
        let yaml = session_yaml(profile).into_bytes();
        let ibt = build_ibt(profile, &yaml)?;
        let ibt_path = format!("test-data/ibt/{}.ibt", profile.name);
        let yaml_path = format!("test-data/session-yaml/{}.yaml", profile.name);
        fixtures.push(manifest_fixture(
            profile, &yaml, &ibt, &ibt_path, &yaml_path,
        ));
        artifacts.push(Artifact {
            relative_path: yaml_path,
            bytes: yaml,
        });
        artifacts.push(Artifact {
            relative_path: ibt_path,
            bytes: ibt,
        });
    }

    let manifest = FixtureManifest {
        schema_version: 1,
        generated_by: "cargo test-fixtures".to_owned(),
        layout: FixtureManifestLayout {
            live_header_prefix_size: MAIN_HEADER_SIZE,
            ibt_header_size: IBT_PREAMBLE_SIZE,
            disk_sub_header_size: DISK_HEADER_SIZE,
            variable_header_size: VariableHeader::WIRE_SIZE,
            disk_sub_header_offset_rule: "var_header_offset - disk_sub_header_size".to_owned(),
        },
        fixtures,
    };
    let mut manifest_bytes =
        serde_json::to_vec_pretty(&manifest).context("serializing fixture manifest")?;
    manifest_bytes.push(b'\n');
    artifacts.push(Artifact {
        relative_path: "test-data/ibt/manifest.json".to_owned(),
        bytes: manifest_bytes,
    });

    for artifact in &artifacts {
        let path = repo_root.join(&artifact.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating fixture directory {}", parent.display()))?;
        }
        fs::write(&path, &artifact.bytes)
            .with_context(|| format!("writing generated fixture {}", path.display()))?;
    }

    Ok(GenerationReport {
        fixture_count: profiles.len(),
        file_count: artifacts.len(),
    })
}

/// Builds the companion session YAML for one profile.
///
/// Formatting and the final newline are part of fixture determinism because the
/// exact bytes are embedded in the IBT and hashed as part of the full file.
fn session_yaml(profile: &Profile) -> String {
    format!(
        "WeekendInfo:\n  TrackName: {}\n  TrackDisplayName: {}\n  TrackLength: \"1.00 km\"\n  TrackID: 9001\nSessionInfo:\n  CurrentSessionNum: 0\n  Sessions:\n    - SessionNum: 0\n      SessionLaps: unlimited\n      SessionTime: \"600 sec\"\n      SessionType: {}\n      SessionName: {}\n",
        profile.track_name, profile.track_display_name, profile.session_name, profile.session_name
    )
}

/// Constructs one complete IBT byte stream without touching the filesystem.
///
/// Layout is `Header`, `DiskSubHeader`, ordered `VariableHeader` values, session
/// YAML, then fixed-size frames. Header fields advertise the same derived offsets
/// used during assembly.
///
/// # Errors
///
/// Returns an error if the profile is invalid, counts or offsets do not fit SDK
/// integer fields, a wire header cannot be constructed/encoded, or the final
/// byte length disagrees with the calculated geometry.
fn build_ibt(profile: &Profile, yaml: &[u8]) -> Result<Vec<u8>> {
    validate_profile(profile)?;
    let variable_headers_len = profile.variables.len() * VariableHeader::WIRE_SIZE;
    let session_info_offset = IBT_PREAMBLE_SIZE + variable_headers_len;
    let end_time = profile.start_time + profile.frame_count as f64 / f64::from(profile.tick_rate);
    let buffers = [VariableBuffer::new(0, 0, 0); Header::MAX_BUFFERS];
    let header = Header::new(
        2,
        StatusField::CONNECTED,
        profile.tick_rate,
        0,
        i32::try_from(yaml.len()).context("session YAML is too large")?,
        i32::try_from(session_info_offset).context("session offset is too large")?,
        i32::try_from(profile.variables.len()).context("variable count is too large")?,
        IBT_PREAMBLE_SIZE as i32,
        1,
        i32::try_from(profile.frame_size).context("frame size is too large")?,
        0,
        0,
        buffers,
    );
    let disk = DiskSubHeader::new(
        profile.start_date,
        profile.start_time,
        end_time,
        profile.lap_count,
        i32::try_from(profile.frame_count).context("frame count is too large")?,
    );

    let expected_len = session_info_offset + yaml.len() + profile.frame_count * profile.frame_size;
    let mut bytes = Vec::with_capacity(expected_len);
    header
        .write_to(&mut bytes)
        .context("encoding main header")?;
    disk.write_to(&mut bytes)
        .context("encoding disk sub-header")?;
    for variable in &profile.variables {
        VariableHeader::new(
            variable.data_type,
            variable.offset,
            variable.count,
            variable.count_as_time,
            variable.name,
            variable.description,
            variable.units,
        )?
        .write_to(&mut bytes)
        .context("encoding variable header")?;
    }
    bytes.extend_from_slice(yaml);
    let mut random = ChaCha8Rng::seed_from_u64(profile.seed);
    for frame_index in 0..profile.frame_count {
        bytes.extend_from_slice(&build_frame(profile, frame_index, &mut random));
    }
    ensure!(
        bytes.len() == expected_len,
        "generated {} with length {}, expected {expected_len}",
        profile.name,
        bytes.len()
    );
    Ok(bytes)
}

/// Checks profile values that must be valid before frame construction.
///
/// # Errors
///
/// Returns an error for a non-positive tick rate, a non-storage SDK type, a
/// negative offset/count, or a variable whose declared range exceeds the frame.
fn validate_profile(profile: &Profile) -> Result<()> {
    ensure!(
        profile.tick_rate > 0,
        "{} tick rate must be positive",
        profile.name
    );
    for variable in &profile.variables {
        let width = variable
            .data_type
            .byte_size()
            .context("variable uses non-storage type")?;
        let end = usize::try_from(variable.offset).context("negative variable offset")?
            + width * usize::try_from(variable.count).context("negative variable count")?;
        ensure!(
            end <= profile.frame_size,
            "{} variable {} exceeds frame size",
            profile.name,
            variable.name
        );
    }
    Ok(())
}

/// Builds one fixed-size telemetry frame from deterministic formulas.
///
/// `ChaCha8Rng` is passed by mutable reference so each profile consumes one
/// stable stream seeded from [`Profile::seed`]. The explicit algorithm is part
/// of the generated-data contract; replacing it intentionally changes hashes.
fn build_frame(profile: &Profile, index: usize, random: &mut ChaCha8Rng) -> Vec<u8> {
    let mut frame = vec![0; profile.frame_size];
    write_f64(&mut frame, 0, index as f64 / f64::from(profile.tick_rate));
    write_f32(&mut frame, 8, 35.0 + index as f64 * 0.25);
    write_f32(&mut frame, 12, index as f64 * 18.5);
    let frames_per_lap =
        (profile.frame_count / usize::try_from(profile.lap_count.max(1)).unwrap()).max(1);
    write_i32(&mut frame, 16, (index / frames_per_lap) as i32);
    write_f32(&mut frame, 20, 0.15 + (index % 4) as f64 * 0.1);
    write_f32(&mut frame, 24, 0.55 + (index % 5) as f64 * 0.05);
    write_f32(&mut frame, 28, 3200.0 + index as f64 * 12.0);
    write_i32(&mut frame, 32, 1 + (index % 5) as i32);
    if profile.frame_size >= 44 {
        write_f32(&mut frame, 36, -0.12 + random.random::<f64>() * 0.24);
        write_f32(&mut frame, 40, 45.0 - index as f64 * 0.02);
    }
    if profile.frame_size >= 56 {
        write_f32(&mut frame, 44, 31.5 + index as f64 * 0.01);
        frame[48] = u8::from(index == 0 || index == profile.frame_count - 1);
        write_i32(
            &mut frame,
            52,
            if index.is_multiple_of(2) { 0x1 } else { 0x5 },
        );
    }
    frame
}

/// Writes a little-endian signed integer into a known-valid frame range.
fn write_i32(frame: &mut [u8], offset: usize, value: i32) {
    frame[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// Rounds a generator `f64` to `f32` and writes its little-endian bytes.
fn write_f32(frame: &mut [u8], offset: usize, value: f64) {
    frame[offset..offset + 4].copy_from_slice(&(value as f32).to_le_bytes());
}

/// Writes a little-endian double into a known-valid frame range.
fn write_f64(frame: &mut [u8], offset: usize, value: f64) {
    frame[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

/// Derives the schema-version-1 manifest entry for a generated profile.
fn manifest_fixture(
    profile: &Profile,
    yaml: &[u8],
    ibt: &[u8],
    ibt_path: &str,
    yaml_path: &str,
) -> IbtFixture {
    let end_time = profile.start_time + profile.frame_count as f64 / f64::from(profile.tick_rate);
    IbtFixture {
        name: profile.name.to_owned(),
        path: ibt_path.to_owned(),
        session_yaml_path: yaml_path.to_owned(),
        seed: profile.seed,
        tick_rate: profile.tick_rate,
        num_vars: profile.variables.len() as i32,
        frame_size: profile.frame_size,
        num_frames: profile.frame_count,
        var_header_offset: IBT_PREAMBLE_SIZE as i32,
        disk_sub_header_offset: MAIN_HEADER_SIZE as i32,
        session_info_update: 0,
        session_info_len: yaml.len() as i32,
        session_info_offset: (IBT_PREAMBLE_SIZE
            + profile.variables.len() * VariableHeader::WIRE_SIZE)
            as i32,
        num_buf: 1,
        disk_header: IbtDiskHeaderManifest {
            start_date: profile.start_date,
            start_time: profile.start_time,
            end_time,
            lap_count: profile.lap_count,
            record_count: profile.frame_count as i32,
        },
        sha256: hex_digest(ibt),
        required_variables: profile.variables.iter().map(manifest_variable).collect(),
    }
}

/// Converts generator metadata to the manifest's historical type vocabulary.
fn manifest_variable(variable: &Variable) -> IbtVariableManifest {
    IbtVariableManifest {
        name: variable.name.to_owned(),
        data_type: match variable.data_type {
            VariableType::Character => "Char",
            VariableType::Boolean => "Bool",
            VariableType::Integer => "Int32",
            VariableType::BitField => "BitField",
            VariableType::Float => "Float32",
            VariableType::Double => "Float64",
            VariableType::ElementTypeCount => unreachable!("profile validation rejects sentinel"),
        }
        .to_owned(),
        offset: variable.offset as usize,
        count: variable.count as usize,
        units: variable.units.to_owned(),
    }
}

/// Returns a lowercase SHA-256 digest for complete fixture bytes.
pub(crate) fn hex_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_binary_hashes_are_stable() {
        let expected = [
            (
                "profile_small",
                "c0ce28dd236e9f8abbe7b7139201af49aaac321fdec0536c56c6c3f50ca3a5a2",
            ),
            (
                "profile_medium",
                "d7e751dd0b08a444a97a53cecead3dc8a450823ae4636598d88a4a133521046e",
            ),
            (
                "profile_large",
                "1eb6b5c6c971b034e0193ad510cb6c93b86643416e7dca9091a433f6812e4d1d",
            ),
        ];
        for (profile, (name, hash)) in profiles().iter().zip(expected) {
            let yaml = session_yaml(profile);
            let ibt = build_ibt(profile, yaml.as_bytes()).unwrap();
            assert_eq!(profile.name, name);
            assert_eq!(hex_digest(&ibt), hash);
        }
    }
}
