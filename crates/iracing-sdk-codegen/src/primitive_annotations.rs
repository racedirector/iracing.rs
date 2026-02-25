use std::collections::{BTreeSet, HashMap};

use anyhow::{Result, anyhow};
use schemars::{JsonSchema, Schema, schema_for};
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone)]
pub struct PrimitiveCatalog {
    pub unit_to_def_ref: HashMap<String, String>,
    pub defs: Map<String, Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnnotationReport {
    pub annotated_variables: usize,
    pub injected_defs: usize,
    pub unknown_units: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct IrsdkPrimitivesSchema {
    #[serde(rename = "irsdk_StatusField")]
    status_field: iracing_sdk::StatusField,
    #[serde(rename = "irsdk_TrkLoc")]
    trk_loc: iracing_sdk::TrackLocation,
    #[serde(rename = "irsdk_TrkSurf")]
    trk_surf: iracing_sdk::TrackSurface,
    #[serde(rename = "irsdk_SessionState")]
    session_state: iracing_sdk::SessionState,
    #[serde(rename = "irsdk_CarLeftRight")]
    car_left_right: iracing_sdk::CarLeftRight,
    #[serde(rename = "irsdk_PitSvStatus")]
    pit_sv_status: iracing_sdk::PitServiceStatus,
    #[serde(rename = "irsdk_PaceMode")]
    pace_mode: iracing_sdk::PaceMode,
    #[serde(rename = "irsdk_TrackWetness")]
    track_wetness: iracing_sdk::TrackWetness,
    #[serde(rename = "irsdk_BroadcastMsg")]
    broadcast_msg: iracing_sdk::BroadcastMessage,
    #[serde(rename = "irsdk_ChatCommandMode")]
    chat_command_mode: iracing_sdk::ChatCommandMode,
    #[serde(rename = "irsdk_PitCommandMode")]
    pit_command_mode: iracing_sdk::PitCommandMode,
    #[serde(rename = "irsdk_TelemetryCommandMode")]
    telemetry_command_mode: iracing_sdk::TelemetryCommandMode,
    #[serde(rename = "irsdk_RpyStateMode")]
    rpy_state_mode: iracing_sdk::ReplayStateMode,
    #[serde(rename = "irsdk_ReloadTexturesMode")]
    reload_textures_mode: iracing_sdk::ReloadTexturesMode,
    #[serde(rename = "irsdk_RpySrchMode")]
    rpy_srch_mode: iracing_sdk::ReplaySearchMode,
    #[serde(rename = "irsdk_RpyPosMode")]
    rpy_pos_mode: iracing_sdk::ReplayPositionMode,
    #[serde(rename = "irsdk_FFBCommandMode")]
    ffb_command_mode: iracing_sdk::FfbCommandMode,
    #[serde(rename = "irsdk_csMode")]
    cs_mode: iracing_sdk::CameraSwitchFocus,
    #[serde(rename = "irsdk_VideoCaptureMode")]
    video_capture_mode: iracing_sdk::VideoCaptureMode,
    #[serde(rename = "irsdk_EngineWarnings")]
    engine_warnings: iracing_sdk::EngineWarnings,
    #[serde(rename = "irsdk_Flags")]
    flags: iracing_sdk::SessionFlags,
    #[serde(rename = "irsdk_CameraState")]
    camera_state: iracing_sdk::CameraState,
    #[serde(rename = "irsdk_PitSvFlags")]
    pit_sv_flags: iracing_sdk::PitServiceFlags,
    #[serde(rename = "irsdk_PaceFlags")]
    pace_flags: iracing_sdk::PaceFlags,
    #[serde(rename = "irsdk_IncidentFlags")]
    incident_flags: iracing_sdk::IncidentFlags,
}

fn schema_def_object_mut<'a>(
    schema: &'a mut Schema,
    def_name: &str,
) -> Result<&'a mut Map<String, Value>> {
    schema
        .ensure_object()
        .get_mut("$defs")
        .and_then(Value::as_object_mut)
        .and_then(|defs| defs.get_mut(def_name))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("missing $defs.{def_name} while enriching primitive schema"))
}

fn named_value_entries(values: &[(&'static str, i64)]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|(name, value)| {
                let mut entry = Map::new();
                entry.insert("name".into(), (*name).into());
                entry.insert("value".into(), (*value).into());
                Value::Object(entry)
            })
            .collect(),
    )
}

fn annotate_named_values(
    schema: &mut Schema,
    def_name: &str,
    kind: &str,
    values: &[(&'static str, i64)],
    known_mask: Option<u32>,
) -> Result<()> {
    let def = schema_def_object_mut(schema, def_name)?;
    def.insert("x-irsdk-kind".into(), kind.into());
    def.insert("x-irsdk-values".into(), named_value_entries(values));
    if let Some(mask) = known_mask {
        def.insert("x-irsdk-known-mask".into(), (mask as u64).into());
    }
    Ok(())
}

fn annotate_incident_values(schema: &mut Schema) -> Result<()> {
    let def = schema_def_object_mut(schema, "IncidentFlags")?;
    def.insert("x-irsdk-kind".into(), "incident-flags".into());

    let mut masks = Map::new();
    masks.insert(
        "report".into(),
        (iracing_sdk::IncidentFlags::REP_MASK as u64).into(),
    );
    masks.insert(
        "penalty".into(),
        (iracing_sdk::IncidentFlags::PEN_MASK as u64).into(),
    );
    def.insert("x-irsdk-masks".into(), Value::Object(masks));
    def.insert(
        "x-irsdk-report-codes".into(),
        named_value_entries(iracing_sdk::IncidentFlags::SCHEMA_REPORT_CODES),
    );
    def.insert(
        "x-irsdk-penalty-codes".into(),
        named_value_entries(iracing_sdk::IncidentFlags::SCHEMA_PENALTY_CODES),
    );
    Ok(())
}

fn annotate_primitive_values(schema: &mut Schema) -> Result<()> {
    type SchemaValues = &'static [(&'static str, i64)];

    let enum_entries: [(&str, SchemaValues); 19] = [
        ("StatusField", iracing_sdk::StatusField::SCHEMA_VALUES),
        ("TrackLocation", iracing_sdk::TrackLocation::SCHEMA_VALUES),
        ("TrackSurface", iracing_sdk::TrackSurface::SCHEMA_VALUES),
        ("SessionState", iracing_sdk::SessionState::SCHEMA_VALUES),
        ("CarLeftRight", iracing_sdk::CarLeftRight::SCHEMA_VALUES),
        (
            "PitServiceStatus",
            iracing_sdk::PitServiceStatus::SCHEMA_VALUES,
        ),
        ("PaceMode", iracing_sdk::PaceMode::SCHEMA_VALUES),
        ("TrackWetness", iracing_sdk::TrackWetness::SCHEMA_VALUES),
        (
            "BroadcastMessage",
            iracing_sdk::BroadcastMessage::SCHEMA_VALUES,
        ),
        (
            "ChatCommandMode",
            iracing_sdk::ChatCommandMode::SCHEMA_VALUES,
        ),
        ("PitCommandMode", iracing_sdk::PitCommandMode::SCHEMA_VALUES),
        (
            "TelemetryCommandMode",
            iracing_sdk::TelemetryCommandMode::SCHEMA_VALUES,
        ),
        (
            "ReplayStateMode",
            iracing_sdk::ReplayStateMode::SCHEMA_VALUES,
        ),
        (
            "ReloadTexturesMode",
            iracing_sdk::ReloadTexturesMode::SCHEMA_VALUES,
        ),
        (
            "ReplaySearchMode",
            iracing_sdk::ReplaySearchMode::SCHEMA_VALUES,
        ),
        (
            "ReplayPositionMode",
            iracing_sdk::ReplayPositionMode::SCHEMA_VALUES,
        ),
        ("FfbCommandMode", iracing_sdk::FfbCommandMode::SCHEMA_VALUES),
        (
            "CameraSwitchFocus",
            iracing_sdk::CameraSwitchFocus::SCHEMA_VALUES,
        ),
        (
            "VideoCaptureMode",
            iracing_sdk::VideoCaptureMode::SCHEMA_VALUES,
        ),
    ];

    for (name, values) in enum_entries {
        annotate_named_values(schema, name, "enum", values, None)?;
    }

    let bitflag_entries: [(&str, SchemaValues, u32); 5] = [
        (
            "EngineWarnings",
            iracing_sdk::EngineWarnings::SCHEMA_VALUES,
            iracing_sdk::EngineWarnings::SCHEMA_KNOWN_MASK,
        ),
        (
            "SessionFlags",
            iracing_sdk::SessionFlags::SCHEMA_VALUES,
            iracing_sdk::SessionFlags::SCHEMA_KNOWN_MASK,
        ),
        (
            "CameraState",
            iracing_sdk::CameraState::SCHEMA_VALUES,
            iracing_sdk::CameraState::SCHEMA_KNOWN_MASK,
        ),
        (
            "PitServiceFlags",
            iracing_sdk::PitServiceFlags::SCHEMA_VALUES,
            iracing_sdk::PitServiceFlags::SCHEMA_KNOWN_MASK,
        ),
        (
            "PaceFlags",
            iracing_sdk::PaceFlags::SCHEMA_VALUES,
            iracing_sdk::PaceFlags::SCHEMA_KNOWN_MASK,
        ),
    ];

    for (name, values, mask) in bitflag_entries {
        annotate_named_values(schema, name, "bitflags", values, Some(mask))?;
    }

    annotate_incident_values(schema)?;

    Ok(())
}

fn parse_def_name(def_ref: &str) -> Option<&str> {
    def_ref.strip_prefix("#/$defs/")
}

pub fn build_primitive_schema() -> Result<Schema> {
    let mut schema = schema_for!(IrsdkPrimitivesSchema);
    annotate_primitive_values(&mut schema)?;
    Ok(schema)
}

pub fn build_primitive_catalog() -> Result<PrimitiveCatalog> {
    let schema = build_primitive_schema()?;
    let root = schema
        .as_value()
        .as_object()
        .ok_or_else(|| anyhow!("primitive schema root is not an object"))?;

    let properties = root
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("primitive schema missing properties object"))?;

    let mut unit_to_def_ref = HashMap::new();
    for (name, value) in properties {
        let def_ref = value
            .as_object()
            .and_then(|obj| obj.get("$ref"))
            .and_then(Value::as_str);
        if let Some(def_ref) = def_ref {
            unit_to_def_ref.insert(name.clone(), def_ref.to_string());
        }
    }

    let defs = root
        .get("$defs")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| anyhow!("primitive schema missing $defs object"))?;

    Ok(PrimitiveCatalog {
        unit_to_def_ref,
        defs,
    })
}

pub fn annotate_variable_schema(schema: &mut Schema) -> Result<AnnotationReport> {
    let catalog = build_primitive_catalog()?;
    let mut annotated_variables = 0usize;
    let mut used_defs = BTreeSet::new();
    let mut unknown_units = BTreeSet::new();

    if let Some(examples) = schema
        .ensure_object()
        .get_mut("examples")
        .and_then(Value::as_array_mut)
    {
        for example in examples {
            let Some(example_obj) = example.as_object_mut() else {
                continue;
            };
            let Some(variables) = example_obj
                .get_mut("variables")
                .and_then(Value::as_object_mut)
            else {
                continue;
            };

            for variable in variables.values_mut() {
                let Some(variable_obj) = variable.as_object_mut() else {
                    continue;
                };
                let Some(units) = variable_obj.get("units").and_then(Value::as_str) else {
                    continue;
                };

                if let Some(def_ref) = catalog.unit_to_def_ref.get(units) {
                    variable_obj.insert("x-irsdk-unit-ref".into(), def_ref.clone().into());
                    annotated_variables += 1;

                    if let Some(def_name) = parse_def_name(def_ref) {
                        used_defs.insert(def_name.to_string());
                    }
                } else if units.starts_with("irsdk_") {
                    unknown_units.insert(units.to_string());
                }
            }
        }
    }

    let mut injected_defs = 0usize;
    if !used_defs.is_empty() {
        let root = schema.ensure_object();
        let defs_value = root
            .entry("$defs")
            .or_insert_with(|| Value::Object(Map::new()));
        let defs_obj = defs_value
            .as_object_mut()
            .ok_or_else(|| anyhow!("schema root $defs is not an object"))?;

        for def_name in used_defs {
            let def_value = catalog
                .defs
                .get(&def_name)
                .ok_or_else(|| anyhow!("missing primitive definition for {def_name}"))?;
            defs_obj.insert(def_name, def_value.clone());
            injected_defs += 1;
        }
    }

    Ok(AnnotationReport {
        annotated_variables,
        injected_defs,
        unknown_units: unknown_units.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use iracing_sdk::{VariableInfo, VariableSchema, VariableType};
    use schemars::schema_for_value;
    use serde_json::Value;

    use super::*;

    fn variable_schema_with_units(units: &[&str]) -> Schema {
        let mut variables = HashMap::new();
        for (index, unit) in units.iter().enumerate() {
            let name = format!("Var{index}");
            variables.insert(
                name.clone(),
                VariableInfo {
                    name,
                    data_type: VariableType::Int32,
                    offset: index * 4,
                    count: 1,
                    count_as_time: false,
                    units: (*unit).to_string(),
                    description: format!("description {index}"),
                },
            );
        }

        let schema =
            VariableSchema::new(variables, units.len() * 4).expect("valid variable schema");
        schema_for_value!(schema)
    }

    fn first_variable<'a>(
        schema: &'a Schema,
        variable_name: &str,
    ) -> &'a serde_json::Map<String, Value> {
        schema
            .as_value()
            .get("examples")
            .and_then(Value::as_array)
            .and_then(|examples| examples.first())
            .and_then(Value::as_object)
            .and_then(|example| example.get("variables"))
            .and_then(Value::as_object)
            .and_then(|variables| variables.get(variable_name))
            .and_then(Value::as_object)
            .expect("variable example should exist")
    }

    #[test]
    fn annotates_known_unit_with_ref() {
        let mut schema = variable_schema_with_units(&["irsdk_Flags"]);
        let report = annotate_variable_schema(&mut schema).expect("annotation should succeed");

        assert_eq!(report.annotated_variables, 1);
        assert_eq!(report.injected_defs, 1);
        assert!(report.unknown_units.is_empty());

        let variable = first_variable(&schema, "Var0");
        assert_eq!(
            variable.get("x-irsdk-unit-ref").and_then(Value::as_str),
            Some("#/$defs/SessionFlags")
        );
        assert!(
            schema
                .as_value()
                .get("$defs")
                .and_then(Value::as_object)
                .and_then(|defs| defs.get("SessionFlags"))
                .is_some()
        );
    }

    #[test]
    fn injects_only_used_defs() {
        let mut schema = variable_schema_with_units(&["irsdk_Flags", "irsdk_TrkLoc"]);
        let report = annotate_variable_schema(&mut schema).expect("annotation should succeed");

        assert_eq!(report.injected_defs, 2);
        let defs = schema
            .as_value()
            .get("$defs")
            .and_then(Value::as_object)
            .expect("$defs should be present");
        assert_eq!(defs.len(), 2);
        assert!(defs.contains_key("SessionFlags"));
        assert!(defs.contains_key("TrackLocation"));
    }

    #[test]
    fn warns_on_unknown_irsdk_unit_without_failing() {
        let mut schema = variable_schema_with_units(&["irsdk_UnknownThing"]);
        let report = annotate_variable_schema(&mut schema).expect("annotation should not fail");

        assert_eq!(report.annotated_variables, 0);
        assert_eq!(report.injected_defs, 0);
        assert_eq!(report.unknown_units, vec!["irsdk_UnknownThing".to_string()]);
        assert!(
            schema
                .as_value()
                .get("$defs")
                .and_then(Value::as_object)
                .is_none()
        );
    }

    #[test]
    fn ignores_non_irsdk_units() {
        let mut schema = variable_schema_with_units(&["m/s"]);
        let report = annotate_variable_schema(&mut schema).expect("annotation should succeed");

        assert_eq!(report.annotated_variables, 0);
        assert_eq!(report.injected_defs, 0);
        assert!(report.unknown_units.is_empty());
    }

    #[test]
    fn catalog_mapping_matches_primitives_schema_properties() {
        let catalog = build_primitive_catalog().expect("catalog should build");
        assert_eq!(
            catalog.unit_to_def_ref.get("irsdk_Flags"),
            Some(&"#/$defs/SessionFlags".to_string())
        );
        assert_eq!(
            catalog.unit_to_def_ref.get("irsdk_TrkLoc"),
            Some(&"#/$defs/TrackLocation".to_string())
        );
    }
}
