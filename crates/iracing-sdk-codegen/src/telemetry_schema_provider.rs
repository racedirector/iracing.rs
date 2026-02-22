use anyhow::Result;
use iracing_sdk::{VariableInfo, VariableSchema, VariableType};
use schemars::Schema;
use serde_json::{Map, Value};

pub struct TelemetrySchemaProvider {
    variable_schema: VariableSchema,
}

impl TelemetrySchemaProvider {
    pub fn new(variable_schema: VariableSchema) -> Result<Self> {
        Ok(Self { variable_schema })
    }

    pub fn build_schema(&self) -> Schema {
        schema_for_variable_schema(&self.variable_schema)
    }
}

fn schema_with_type(instance_type: &str) -> Schema {
    let mut obj = Map::new();
    obj.insert("type".into(), instance_type.into());
    Schema::from(obj)
}

fn scalar_schema_for_variable_type(ty: &VariableType) -> Schema {
    let instance_type = match ty {
        VariableType::Char => "string",
        VariableType::Bool => "boolean",
        VariableType::Float32 | VariableType::Float64 => "number",
        VariableType::Int8
        | VariableType::UInt8
        | VariableType::Int16
        | VariableType::UInt16
        | VariableType::Int32
        | VariableType::UInt32
        | VariableType::BitField => "integer",
    };

    schema_with_type(instance_type)
}

fn is_char_type(info: &VariableInfo) -> bool {
    info.data_type == VariableType::Char
}

fn array_schema_for_count(item_schema: Schema, count: usize) -> Schema {
    let mut obj = Map::new();
    obj.insert("type".into(), "array".into());
    obj.insert("items".into(), item_schema.to_value());
    obj.insert("minItems".into(), (count as u64).into());
    obj.insert("maxItems".into(), (count as u64).into());
    Schema::from(obj)
}

fn annotate_telemetry_schema(
    schema: &mut Schema,
    info: &VariableInfo,
    include_count_as_time: bool,
) {
    let obj = schema.ensure_object();
    obj.insert("description".into(), info.description.clone().into());
    obj.insert("x-units".into(), info.units.clone().into());
    obj.insert(
        "x-iracing-var-type".into(),
        format!("{:?}", info.data_type).into(),
    );
    obj.insert("x-count".into(), (info.count as u64).into());
    obj.insert("x-offset".into(), (info.offset as u64).into());

    if include_count_as_time {
        obj.insert("x-count-as-time".into(), info.count_as_time.into());
    }
}

fn telemetry_property_schema(info: &VariableInfo) -> Schema {
    if is_char_type(info) {
        let mut schema = schema_with_type("string");
        annotate_telemetry_schema(&mut schema, info, false);
        return schema;
    }

    let scalar_schema = scalar_schema_for_variable_type(&info.data_type);
    let mut schema = if info.count > 1 {
        array_schema_for_count(scalar_schema, info.count)
    } else {
        scalar_schema
    };

    annotate_telemetry_schema(&mut schema, info, true);
    schema
}

fn schema_for_variable_schema(schema: &VariableSchema) -> Schema {
    let mut names: Vec<_> = schema.variables.keys().collect();
    names.sort();

    let mut properties = Map::new();
    for name in names {
        if let Some(info) = schema.variables.get(name) {
            properties.insert(name.clone(), telemetry_property_schema(info).to_value());
        }
    }

    let mut root = schema_with_type("object");
    let obj = root.ensure_object();
    obj.insert("title".into(), "Telemetry".into());
    obj.insert("description".into(), "Telemetry from iRacing".into());
    obj.insert("additionalProperties".into(), Value::Bool(false));
    obj.insert("properties".into(), Value::Object(properties));

    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_sdk::{VariableInfo, VariableType};
    use serde_json::Value;
    use std::collections::HashMap;

    fn make_var(
        name: &str,
        data_type: VariableType,
        count: usize,
        offset: usize,
        count_as_time: bool,
    ) -> VariableInfo {
        VariableInfo {
            name: name.to_string(),
            data_type,
            offset,
            count,
            count_as_time,
            units: "unit".to_string(),
            description: format!("{name} description"),
        }
    }

    fn test_schema(vars: Vec<VariableInfo>, frame_size: usize) -> VariableSchema {
        let mut map = HashMap::new();
        for var in vars {
            map.insert(var.name.clone(), var);
        }
        VariableSchema::new(map, frame_size).expect("valid schema")
    }

    fn get_property<'a>(root: &'a Value, field: &str) -> &'a Value {
        root.get("properties")
            .and_then(Value::as_object)
            .and_then(|props| props.get(field))
            .expect("property to exist")
    }

    #[test]
    fn builds_object_root_with_additional_properties_false() {
        let schema = test_schema(
            vec![make_var("Speed", VariableType::Float32, 1, 0, false)],
            4,
        );
        let root = TelemetrySchemaProvider::new(schema)
            .expect("provider init")
            .build_schema()
            .to_value();

        assert_eq!(root.get("type"), Some(&Value::String("object".to_string())));
        assert_eq!(root.get("additionalProperties"), Some(&Value::Bool(false)));
        assert_eq!(
            root.get("title"),
            Some(&Value::String("Telemetry".to_string()))
        );
    }

    #[test]
    fn maps_scalar_types_correctly() {
        let schema = test_schema(
            vec![
                make_var("Flag", VariableType::Bool, 1, 0, false),
                make_var("Speed", VariableType::Float32, 1, 4, false),
                make_var("Rpm", VariableType::Int32, 1, 8, false),
                make_var("Code", VariableType::Char, 1, 12, false),
            ],
            13,
        );

        let root = TelemetrySchemaProvider::new(schema)
            .expect("provider init")
            .build_schema()
            .to_value();

        assert_eq!(
            get_property(&root, "Flag").get("type"),
            Some(&Value::String("boolean".to_string()))
        );
        assert_eq!(
            get_property(&root, "Speed").get("type"),
            Some(&Value::String("number".to_string()))
        );
        assert_eq!(
            get_property(&root, "Rpm").get("type"),
            Some(&Value::String("integer".to_string()))
        );
        assert_eq!(
            get_property(&root, "Code").get("type"),
            Some(&Value::String("string".to_string()))
        );
    }

    #[test]
    fn maps_non_char_arrays_with_fixed_bounds() {
        let schema = test_schema(
            vec![make_var("TireTemp", VariableType::Float32, 4, 0, true)],
            16,
        );

        let root = TelemetrySchemaProvider::new(schema)
            .expect("provider init")
            .build_schema()
            .to_value();
        let tire_temp = get_property(&root, "TireTemp");

        assert_eq!(
            tire_temp.get("type"),
            Some(&Value::String("array".to_string()))
        );
        assert_eq!(tire_temp.get("minItems"), Some(&Value::from(4u64)));
        assert_eq!(tire_temp.get("maxItems"), Some(&Value::from(4u64)));
        assert_eq!(
            tire_temp
                .get("items")
                .and_then(|v| v.get("type"))
                .cloned()
                .unwrap_or(Value::Null),
            Value::String("number".to_string())
        );
    }

    #[test]
    fn maps_char_buffers_to_string() {
        let schema = test_schema(
            vec![make_var("TrackName", VariableType::Char, 64, 0, false)],
            64,
        );
        let root = TelemetrySchemaProvider::new(schema)
            .expect("provider init")
            .build_schema()
            .to_value();
        let track_name = get_property(&root, "TrackName");

        assert_eq!(
            track_name.get("type"),
            Some(&Value::String("string".to_string()))
        );
        assert!(track_name.get("items").is_none());
        assert!(track_name.get("x-count-as-time").is_none());
    }

    #[test]
    fn includes_expected_x_metadata_fields() {
        let schema = test_schema(
            vec![make_var("Speed", VariableType::Float32, 1, 24, true)],
            28,
        );
        let root = TelemetrySchemaProvider::new(schema)
            .expect("provider init")
            .build_schema()
            .to_value();
        let speed = get_property(&root, "Speed");

        assert_eq!(
            speed.get("description"),
            Some(&Value::String("Speed description".to_string()))
        );
        assert_eq!(
            speed.get("x-units"),
            Some(&Value::String("unit".to_string()))
        );
        assert_eq!(
            speed.get("x-iracing-var-type"),
            Some(&Value::String("Float32".to_string()))
        );
        assert_eq!(speed.get("x-count"), Some(&Value::from(1u64)));
        assert_eq!(speed.get("x-offset"), Some(&Value::from(24u64)));
        assert_eq!(speed.get("x-count-as-time"), Some(&Value::Bool(true)));
    }

    #[test]
    fn produces_deterministic_property_order() {
        let schema = test_schema(
            vec![
                make_var("Zulu", VariableType::Float32, 1, 0, false),
                make_var("Alpha", VariableType::Float32, 1, 4, false),
                make_var("Mike", VariableType::Float32, 1, 8, false),
            ],
            12,
        );

        let root = TelemetrySchemaProvider::new(schema)
            .expect("provider init")
            .build_schema()
            .to_value();
        let keys: Vec<String> = root
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties object")
            .keys()
            .cloned()
            .collect();

        assert_eq!(keys, vec!["Alpha", "Mike", "Zulu"]);
    }
}
