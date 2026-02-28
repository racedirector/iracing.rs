use anyhow::Result;
use clap::Parser;
use iracing_sdk::{FrameProjection, IbtReader, IbtWriteOptions, IbtWriter};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path to the output `.ibt` telemetry file.
    #[arg(short, long)]
    output_path: PathBuf,
}

fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // ------------------------------------------------------------
    // Parse CLI arguments.
    // ------------------------------------------------------------
    let Args {
        ibt_path,
        output_path,
    } = Args::parse();

    // ------------------------------------------------------------
    // Open telemetry reader.
    // ------------------------------------------------------------
    info!(path = %ibt_path.display(), "Opening IBT file");
    let mut reader = IbtReader::open(&ibt_path)?;

    // ------------------------------------------------------------
    // Create a projection of the variables you want from the
    // source IBT
    // ------------------------------------------------------------
    let projection = FrameProjection::from_variable_names(
        reader.variables(),
        ["SessionTime", "Speed", "RPM", "OnPitRoad"],
    )?;

    let options = IbtWriteOptions::from_reader(&reader)?;
    let mut writer = IbtWriter::create(&output_path, projection.target_schema().clone(), options)?;

    while let Some((frame, _, _)) = reader.read_next_frame()? {
        writer.write_projected_frame(&frame, &projection)?;
    }

    writer.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use iracing_sdk::{VariableInfo, VariableSchema, VariableType, VarData};
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_ibt_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time moved backwards")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "iracing-sdk-{name}-{}-{nanos}.ibt",
            std::process::id()
        ))
    }

    fn build_test_schema() -> VariableSchema {
        let mut vars = HashMap::new();

        vars.insert(
            "SessionTime".to_string(),
            VariableInfo {
                name: "SessionTime".to_string(),
                data_type: VariableType::Float64,
                offset: 0,
                count: 1,
                count_as_time: false,
                units: "s".to_string(),
                description: "Session time".to_string(),
            },
        );

        vars.insert(
            "Speed".to_string(),
            VariableInfo {
                name: "Speed".to_string(),
                data_type: VariableType::Float32,
                offset: 8,
                count: 1,
                count_as_time: false,
                units: "m/s".to_string(),
                description: "Speed".to_string(),
            },
        );

        vars.insert(
            "RPM".to_string(),
            VariableInfo {
                name: "RPM".to_string(),
                data_type: VariableType::Float32,
                offset: 12,
                count: 1,
                count_as_time: false,
                units: "rpm".to_string(),
                description: "Engine RPM".to_string(),
            },
        );

        vars.insert(
            "OnPitRoad".to_string(),
            VariableInfo {
                name: "OnPitRoad".to_string(),
                data_type: VariableType::Bool,
                offset: 16,
                count: 1,
                count_as_time: false,
                units: "".to_string(),
                description: "On pit road".to_string(),
            },
        );

        vars.insert(
            "Gear".to_string(),
            VariableInfo {
                name: "Gear".to_string(),
                data_type: VariableType::Int32,
                offset: 17,
                count: 1,
                count_as_time: false,
                units: "".to_string(),
                description: "Current gear".to_string(),
            },
        );

        VariableSchema::new(vars, 21).expect("valid schema")
    }

    fn build_test_frame(
        schema: &VariableSchema,
        session_time: f64,
        speed: f32,
        rpm: f32,
        on_pit_road: bool,
        gear: i32,
    ) -> Vec<u8> {
        let mut frame = vec![0u8; schema.frame_size];

        let session_time_info = schema.get_variable("SessionTime").unwrap();
        frame[session_time_info.offset..session_time_info.offset + 8]
            .copy_from_slice(&session_time.to_le_bytes());

        let speed_info = schema.get_variable("Speed").unwrap();
        frame[speed_info.offset..speed_info.offset + 4].copy_from_slice(&speed.to_le_bytes());

        let rpm_info = schema.get_variable("RPM").unwrap();
        frame[rpm_info.offset..rpm_info.offset + 4].copy_from_slice(&rpm.to_le_bytes());

        let on_pit_road_info = schema.get_variable("OnPitRoad").unwrap();
        frame[on_pit_road_info.offset] = u8::from(on_pit_road);

        let gear_info = schema.get_variable("Gear").unwrap();
        frame[gear_info.offset..gear_info.offset + 4].copy_from_slice(&gear.to_le_bytes());

        frame
    }

    #[test]
    fn roundtrip_preserves_projected_variables() -> Result<()> {
        let source_schema = build_test_schema();
        let source_path = temp_ibt_path("roundtrip-source");
        let output_path = temp_ibt_path("roundtrip-output");

        // Create source IBT
        {
            let options = IbtWriteOptions::default();
            let mut writer = IbtWriter::create(&source_path, source_schema.clone(), options)?;

            let frame1 = build_test_frame(&source_schema, 1.0, 100.0, 5000.0, false, 3);
            let frame2 = build_test_frame(&source_schema, 2.0, 110.0, 6000.0, true, 4);

            writer.write_frame(&frame1)?;
            writer.write_frame(&frame2)?;
            writer.finish()?;
        }

        // Read and project to subset
        {
            let mut reader = IbtReader::open(&source_path)?;
            let projection = FrameProjection::from_variable_names(
                reader.variables(),
                ["SessionTime", "Speed", "RPM", "OnPitRoad"],
            )?;

            let options = IbtWriteOptions::from_reader(&reader)?;
            let mut writer =
                IbtWriter::create(&output_path, projection.target_schema().clone(), options)?;

            while let Some((frame, _, _)) = reader.read_next_frame()? {
                writer.write_projected_frame(&frame, &projection)?;
            }

            writer.finish()?;
        }

        // Verify output
        {
            let mut reader = IbtReader::open(&output_path)?;
            assert_eq!(reader.total_frames(), 2);

            // Should only have 4 variables
            assert_eq!(reader.variables().variables.len(), 4);
            assert!(reader.variables().get_variable("SessionTime").is_some());
            assert!(reader.variables().get_variable("Speed").is_some());
            assert!(reader.variables().get_variable("RPM").is_some());
            assert!(reader.variables().get_variable("OnPitRoad").is_some());
            assert!(reader.variables().get_variable("Gear").is_none());

            let speed_info = reader.variables().get_variable("Speed").unwrap().clone();
            let rpm_info = reader.variables().get_variable("RPM").unwrap().clone();

            let (frame1, _, _) = reader.read_next_frame()?.unwrap();
            let speed1: f32 = f32::from_bytes(&frame1, &speed_info)?;
            let rpm1: f32 = f32::from_bytes(&frame1, &rpm_info)?;

            assert!((speed1 - 100.0).abs() < f32::EPSILON);
            assert!((rpm1 - 5000.0).abs() < f32::EPSILON);
        }

        std::fs::remove_file(&source_path).ok();
        std::fs::remove_file(&output_path).ok();

        Ok(())
    }

    #[test]
    fn projection_handles_variable_order() -> Result<()> {
        let source_schema = build_test_schema();
        let source_path = temp_ibt_path("order-source");
        let output_path = temp_ibt_path("order-output");

        // Create source IBT
        {
            let options = IbtWriteOptions::default();
            let mut writer = IbtWriter::create(&source_path, source_schema.clone(), options)?;

            let frame = build_test_frame(&source_schema, 10.0, 50.0, 3000.0, true, 2);
            writer.write_frame(&frame)?;
            writer.finish()?;
        }

        // Project with different order
        {
            let mut reader = IbtReader::open(&source_path)?;
            let projection = FrameProjection::from_variable_names(
                reader.variables(),
                ["RPM", "Speed", "SessionTime"], // Different order
            )?;

            let options = IbtWriteOptions::from_reader(&reader)?;
            let mut writer =
                IbtWriter::create(&output_path, projection.target_schema().clone(), options)?;

            while let Some((frame, _, _)) = reader.read_next_frame()? {
                writer.write_projected_frame(&frame, &projection)?;
            }

            writer.finish()?;
        }

        // Verify values are correct despite different order
        {
            let mut reader = IbtReader::open(&output_path)?;
            let speed_info = reader.variables().get_variable("Speed").unwrap().clone();
            let rpm_info = reader.variables().get_variable("RPM").unwrap().clone();
            let session_time_info = reader.variables().get_variable("SessionTime").unwrap().clone();

            let (frame, _, _) = reader.read_next_frame()?.unwrap();

            let speed: f32 = f32::from_bytes(&frame, &speed_info)?;
            let rpm: f32 = f32::from_bytes(&frame, &rpm_info)?;
            let session_time: f64 = f64::from_bytes(&frame, &session_time_info)?;

            assert!((speed - 50.0).abs() < f32::EPSILON);
            assert!((rpm - 3000.0).abs() < f32::EPSILON);
            assert!((session_time - 10.0).abs() < f64::EPSILON);
        }

        std::fs::remove_file(&source_path).ok();
        std::fs::remove_file(&output_path).ok();

        Ok(())
    }

    #[test]
    fn projection_from_all_variables_preserves_all_data() -> Result<()> {
        let source_schema = build_test_schema();
        let source_path = temp_ibt_path("all-vars-source");
        let output_path = temp_ibt_path("all-vars-output");

        // Create source IBT
        {
            let options = IbtWriteOptions::default();
            let mut writer = IbtWriter::create(&source_path, source_schema.clone(), options)?;

            let frame = build_test_frame(&source_schema, 5.0, 75.5, 4500.0, false, 5);
            writer.write_frame(&frame)?;
            writer.finish()?;
        }

        // Project all variables
        {
            let mut reader = IbtReader::open(&source_path)?;
            let projection = FrameProjection::from_all(reader.variables())?;

            let options = IbtWriteOptions::from_reader(&reader)?;
            let mut writer =
                IbtWriter::create(&output_path, projection.target_schema().clone(), options)?;

            while let Some((frame, _, _)) = reader.read_next_frame()? {
                writer.write_projected_frame(&frame, &projection)?;
            }

            writer.finish()?;
        }

        // Verify all variables are present
        {
            let mut reader = IbtReader::open(&output_path)?;
            assert_eq!(reader.variables().variables.len(), 5);

            let gear_info = reader.variables().get_variable("Gear").unwrap().clone();
            let (frame, _, _) = reader.read_next_frame()?.unwrap();
            let gear: i32 = i32::from_bytes(&frame, &gear_info)?;

            assert_eq!(gear, 5);
        }

        std::fs::remove_file(&source_path).ok();
        std::fs::remove_file(&output_path).ok();

        Ok(())
    }

    #[test]
    fn projection_handles_single_variable() -> Result<()> {
        let source_schema = build_test_schema();
        let source_path = temp_ibt_path("single-var-source");

        // Create source IBT
        {
            let options = IbtWriteOptions::default();
            let mut writer = IbtWriter::create(&source_path, source_schema.clone(), options)?;

            let frame = build_test_frame(&source_schema, 1.0, 25.0, 2000.0, true, 1);
            writer.write_frame(&frame)?;
            writer.finish()?;
        }

        // Project single variable
        let mut reader = IbtReader::open(&source_path)?;
        let projection = FrameProjection::from_variable_names(reader.variables(), ["Speed"])?;

        assert_eq!(projection.target_schema().variables.len(), 1);
        assert_eq!(projection.target_schema().frame_size, 4); // Float32 size

        std::fs::remove_file(&source_path).ok();

        Ok(())
    }
}