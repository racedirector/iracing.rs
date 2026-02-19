use std::collections::BTreeMap;

use schemars::schema::{
    InstanceType, Metadata, ObjectValidation, RootSchema, Schema, SchemaObject, SingleOrVec,
};
use schemars::schema_for;

use crate::{SessionInfo, VariableInfo, VariableSchema, VariableType};

fn scalar_schema_for_variable_type(ty: &VariableType) -> Schema {
    let instance_type = match ty {
        VariableType::Char => InstanceType::String,
        VariableType::Bool => InstanceType::Boolean,
        VariableType::Float32 | VariableType::Float64 => InstanceType::Number,
        VariableType::Int8
        | VariableType::UInt8
        | VariableType::Int16
        | VariableType::UInt16
        | VariableType::Int32
        | VariableType::UInt32
        | VariableType::BitField => InstanceType::Integer,
    };

    Schema::Object(SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(instance_type))),
        ..Default::default()
    })
}

fn is_char_buffer(info: &VariableInfo) -> bool {
    info.data_type == VariableType::Char && info.count > 1
}

fn telemetry_property_schema(info: &VariableInfo) -> Schema {
    // If it's a char buffer, treat it as a string (not an array of single-char strings).
    if is_char_buffer(info) {
        let mut obj = SchemaObject {
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::String))),
            ..Default::default()
        };

        obj.metadata
            .get_or_insert_with(Default::default)
            .description = Some(info.description.clone());

        obj.extensions
            .insert("x-units".into(), info.units.clone().into());
        obj.extensions.insert(
            "x-iracing-var-type".into(),
            format!("{:?}", info.data_type).into(),
        );
        obj.extensions
            .insert("x-count".into(), (info.count as u64).into());
        obj.extensions
            .insert("x-offset".into(), (info.offset as u64).into());

        return Schema::Object(obj);
    }

    // Otherwise: scalar type, possibly array if count > 1.
    let base = scalar_schema_for_variable_type(&info.data_type);

    let mut obj = match base {
        Schema::Object(o) => o,
        _ => SchemaObject::default(),
    };

    obj.metadata
        .get_or_insert_with(Default::default)
        .description = Some(info.description.clone());

    obj.extensions
        .insert("x-units".into(), info.units.clone().into());
    obj.extensions.insert(
        "x-iracing-var-type".into(),
        format!("{:?}", info.data_type).into(),
    );
    obj.extensions
        .insert("x-count-as-time".into(), info.count_as_time.into());
    obj.extensions
        .insert("x-count".into(), (info.count as u64).into());
    obj.extensions
        .insert("x-offset".into(), (info.offset as u64).into());

    if info.count > 1 {
        Schema::Object(array_schema_for_count(obj, info.count))
    } else {
        Schema::Object(obj)
    }
}

fn array_schema_for_count(item_schema: SchemaObject, count: usize) -> SchemaObject {
    use schemars::schema::ArrayValidation;

    SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Array))),
        array: Some(Box::new(ArrayValidation {
            items: Some(SingleOrVec::Single(Box::new(Schema::Object(item_schema)))),
            min_items: Some(count as u32),
            max_items: Some(count as u32),
            ..Default::default()
        })),
        ..Default::default()
    }
}

fn schema_for_variable_schema(schema: &VariableSchema) -> RootSchema {
    let mut properties: BTreeMap<String, Schema> = BTreeMap::new();

    for (name, info) in &schema.variables {
        let prop_name = name.clone();
        properties.insert(prop_name, telemetry_property_schema(info));
    }

    RootSchema {
        schema: SchemaObject {
            metadata: Some(Box::new(Metadata {
                title: Some("Telemetry".to_string()),
                description: Some("Telemetry from iRacing".to_string()),
                ..Default::default()
            })),
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Object))),
            object: Some(Box::new(ObjectValidation {
                properties,
                additional_properties: Some(Box::new(Schema::Bool(false))),
                ..Default::default()
            })),
            ..Default::default()
        },
        definitions: Default::default(),
        ..Default::default()
    }
}

pub fn session_root_schema() -> RootSchema {
    schema_for!(SessionInfo)
}

#[cfg(feature = "schema-discovery")]
pub fn session_root_schema_with_discovery(session: &SessionInfo) -> RootSchema {
    let mut schema = session_root_schema();

    for field in session.collect_unknown_fields() {
        overlay_discovered_field(&mut schema, &field.path, &field.data_type, &field.example);
    }

    schema
}

#[cfg(feature = "schema-discovery")]
fn overlay_discovered_field(
    root: &mut RootSchema,
    path: &str,
    data_type: &crate::schema::session::UnknownFieldType,
    example: &str,
) {
    let tokens = parse_discovery_path(path);
    if tokens.is_empty() {
        return;
    }

    let (leaf, parents) = match tokens.split_last() {
        Some(parts) => parts,
        None => return,
    };

    let mut current = &mut root.schema;

    for parent in parents {
        if !parent.key.is_empty() {
            let properties = ensure_object_properties(current);
            let entry = properties
                .entry(parent.key.clone())
                .or_insert_with(default_object_schema);

            let Some(next) = schema_object_mut(entry) else {
                return;
            };
            current = next;
        }

        for _ in 0..parent.array_depth {
            current = ensure_array_item_object(current);
        }
    }

    if !leaf.key.is_empty() {
        let properties = ensure_object_properties(current);
        let entry = if leaf.array_depth == 0 {
            properties
                .entry(leaf.key.clone())
                .or_insert_with(|| Schema::Object(SchemaObject::default()))
        } else {
            properties
                .entry(leaf.key.clone())
                .or_insert_with(default_object_schema)
        };

        if leaf.array_depth == 0 {
            apply_discovered_leaf(entry, data_type, example);
            return;
        }

        let Some(mut leaf_object) = schema_object_mut(entry) else {
            return;
        };

        for idx in 0..leaf.array_depth {
            if idx + 1 == leaf.array_depth {
                let item_schema = ensure_array_item_schema(leaf_object);
                apply_discovered_leaf(item_schema, data_type, example);
            } else {
                leaf_object = ensure_array_item_object(leaf_object);
            }
        }
    }
}

#[cfg(feature = "schema-discovery")]
#[derive(Debug)]
struct PathToken {
    key: String,
    array_depth: usize,
}

#[cfg(feature = "schema-discovery")]
fn parse_discovery_path(path: &str) -> Vec<PathToken> {
    path.split('.')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut key = String::new();
            let mut array_depth = 0usize;

            for ch in segment.chars() {
                match ch {
                    '[' => array_depth += 1,
                    ']' => {}
                    _ if array_depth == 0 => key.push(ch),
                    _ => {}
                }
            }

            PathToken { key, array_depth }
        })
        .collect()
}

#[cfg(feature = "schema-discovery")]
fn ensure_object_properties(schema_obj: &mut SchemaObject) -> &mut BTreeMap<String, Schema> {
    schema_obj.instance_type.get_or_insert_with(|| {
        SingleOrVec::Single(Box::new(InstanceType::Object))
    });

    let object = schema_obj
        .object
        .get_or_insert_with(|| Box::new(ObjectValidation::default()));

    &mut object.properties
}

#[cfg(feature = "schema-discovery")]
fn ensure_array_item_object(schema_obj: &mut SchemaObject) -> &mut SchemaObject {
    let item = ensure_array_item_schema(schema_obj);

    match item {
        Schema::Object(obj) => obj,
        Schema::Bool(_) => {
            *item = default_object_schema();
            match item {
                Schema::Object(obj) => obj,
                Schema::Bool(_) => unreachable!(),
            }
        }
    }
}

#[cfg(feature = "schema-discovery")]
fn ensure_array_item_schema(schema_obj: &mut SchemaObject) -> &mut Schema {
    use schemars::schema::ArrayValidation;

    schema_obj.instance_type.get_or_insert_with(|| {
        SingleOrVec::Single(Box::new(InstanceType::Array))
    });

    let array = schema_obj.array.get_or_insert_with(|| {
        Box::new(ArrayValidation {
            items: Some(SingleOrVec::Single(Box::new(default_object_schema()))),
            ..Default::default()
        })
    });

    if array.items.is_none() {
        array.items = Some(SingleOrVec::Single(Box::new(default_object_schema())));
    }

    let items = array.items.as_mut().expect("array items initialized");

    match items {
        SingleOrVec::Single(item) => item.as_mut(),
        SingleOrVec::Vec(vec) => {
            if vec.is_empty() {
                vec.push(default_object_schema());
            }
            vec.first_mut().expect("item inserted")
        }
    }
}

#[cfg(feature = "schema-discovery")]
fn schema_object_mut(schema: &mut Schema) -> Option<&mut SchemaObject> {
    match schema {
        Schema::Object(obj) => Some(obj),
        Schema::Bool(_) => None,
    }
}

#[cfg(feature = "schema-discovery")]
fn default_object_schema() -> Schema {
    Schema::Object(SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Object))),
        ..Default::default()
    })
}

#[cfg(feature = "schema-discovery")]
fn apply_discovered_leaf(
    schema: &mut Schema,
    data_type: &crate::schema::session::UnknownFieldType,
    example: &str,
) {
    let expected_type = match data_type {
        crate::schema::session::UnknownFieldType::String => InstanceType::String,
        crate::schema::session::UnknownFieldType::Number => InstanceType::Number,
        crate::schema::session::UnknownFieldType::Boolean => InstanceType::Boolean,
        crate::schema::session::UnknownFieldType::Null => InstanceType::Null,
        crate::schema::session::UnknownFieldType::Object => InstanceType::Object,
        crate::schema::session::UnknownFieldType::Array => InstanceType::Array,
    };

    let Some(obj) = schema_object_mut(schema) else {
        return;
    };

    if obj.instance_type.is_none() {
        obj.instance_type = Some(SingleOrVec::Single(Box::new(expected_type)));
    }

    obj.extensions.insert("x-discovered".into(), true.into());
    obj.extensions
        .insert("x-example".into(), example.to_string().into());
}

impl From<&VariableSchema> for RootSchema {
    fn from(v: &VariableSchema) -> Self {
        schema_for_variable_schema(v)
    }
}

impl From<VariableSchema> for RootSchema {
    fn from(v: VariableSchema) -> Self {
        RootSchema::from(&v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_root_schema_has_expected_top_level_properties() {
        let schema = session_root_schema();
        let object = schema
            .schema
            .object
            .as_ref()
            .expect("Session schema should be an object");

        assert!(object.properties.contains_key("WeekendInfo"));
        assert!(object.properties.contains_key("SessionInfo"));
    }

    #[cfg(feature = "schema-discovery")]
    #[test]
    fn session_discovery_overlay_injects_nested_path() {
        use std::collections::HashMap;

        let mut session = SessionInfo::default();
        session.unknown_fields = HashMap::new();
        session.unknown_fields.insert(
            "CustomRoot".to_string(),
            serde_yaml_ng::Value::String("example".to_string()),
        );

        let schema = session_root_schema_with_discovery(&session);
        let object = schema.schema.object.as_ref().unwrap();
        let custom = object.properties.get("CustomRoot").unwrap();

        match custom {
            Schema::Object(obj) => {
                assert!(obj.extensions.contains_key("x-discovered"));
            }
            Schema::Bool(_) => panic!("Expected object schema"),
        }
    }

    #[cfg(feature = "schema-discovery")]
    #[test]
    fn session_discovery_overlay_handles_array_path() {
        use std::collections::HashMap;

        let mut session = SessionInfo::default();
        session.session_info.sessions.push(Default::default());
        session.session_info.sessions[0].unknown_fields = HashMap::new();
        session.session_info.sessions[0].unknown_fields.insert(
            "NewMetric".to_string(),
            serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(42)),
        );

        let schema = session_root_schema_with_discovery(&session);
        let root = schema.schema.object.as_ref().unwrap();
        let session_info = root.properties.get("SessionInfo").unwrap();
        let session_info_obj = match session_info {
            Schema::Object(obj) => obj,
            Schema::Bool(_) => panic!("Expected object"),
        };
        let sessions = session_info_obj
            .object
            .as_ref()
            .unwrap()
            .properties
            .get("Sessions")
            .unwrap();

        let sessions_obj = match sessions {
            Schema::Object(obj) => obj,
            Schema::Bool(_) => panic!("Expected array object"),
        };

        let item = match sessions_obj.array.as_ref().unwrap().items.as_ref().unwrap() {
            SingleOrVec::Single(schema) => schema,
            SingleOrVec::Vec(_) => panic!("Expected homogeneous array"),
        };

        let item_obj = match item.as_ref() {
            Schema::Object(obj) => obj,
            Schema::Bool(_) => panic!("Expected object item"),
        };

        let new_metric = item_obj
            .object
            .as_ref()
            .unwrap()
            .properties
            .get("NewMetric")
            .unwrap();

        match new_metric {
            Schema::Object(obj) => {
                assert!(obj.extensions.contains_key("x-discovered"));
            }
            Schema::Bool(_) => panic!("Expected object leaf"),
        }
    }

    #[cfg(feature = "schema-discovery")]
    #[test]
    fn session_discovery_does_not_overwrite_known_property_types() {
        use std::collections::HashMap;

        let mut session = SessionInfo::default();
        session.weekend_info.unknown_fields = HashMap::new();
        session.weekend_info.unknown_fields.insert(
            "TrackName".to_string(),
            serde_yaml_ng::Value::Mapping(Default::default()),
        );

        let schema = session_root_schema_with_discovery(&session);
        let root = schema.schema.object.as_ref().unwrap();
        let weekend_info = root.properties.get("WeekendInfo").unwrap();
        let weekend_obj = match weekend_info {
            Schema::Object(obj) => obj,
            Schema::Bool(_) => panic!("Expected object"),
        };
        let track_name = weekend_obj
            .object
            .as_ref()
            .unwrap()
            .properties
            .get("TrackName")
            .unwrap();

        match track_name {
            Schema::Object(obj) => {
                assert!(matches!(
                    obj.instance_type,
                    Some(SingleOrVec::Single(ref ty)) if **ty == InstanceType::String
                ));
            }
            Schema::Bool(_) => panic!("Expected object leaf"),
        }
    }
}
