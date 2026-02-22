//! JSON schema generation helpers for iRacing session and telemetry models.
//!
//! This module is compiled when the crate `codegen` feature is enabled and provides:
//! - Baseline session schema generation from [`SessionInfo`] (`session_root_schema`)
//! - Telemetry schema generation from [`VariableSchema`] (`From<VariableSchema> for Schema`)
//!
//! When `schema-discovery` is also enabled, this module can overlay unknown fields collected
//! during session parsing onto the baseline schema (`session_root_schema_with_discovery`).
//!
//! Custom metadata is carried through schema extension fields:
//! - Telemetry properties: `x-units`, `x-iracing-var-type`, `x-count`, `x-offset`,
//!   `x-count-as-time`
//! - Discovery overlays: `x-discovered`, `x-example`
//!
//! These helpers are intended for tooling and code generation flows that need stable,
//! machine-readable schema output.

use schemars::Schema;
use schemars::schema_for;
use serde_json::{Map, Value};

use crate::{SessionInfo, VariableInfo, VariableSchema, VariableType};

/// Build a scalar JSON Schema for a single iRacing [`VariableType`].
///
/// This maps telemetry primitives to their JSON Schema `type`:
/// - `Char` -> `string`
/// - `Bool` -> `boolean`
/// - `Float32`/`Float64` -> `number`
/// - Integer and bitfield types -> `integer`
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

/// Return `true` when a variable is represented as a fixed-size character buffer.
///
/// In iRacing telemetry this shape (`Char` with `count > 1`) is interpreted as a textual
/// string field instead of an array of one-character strings.
fn is_char_buffer(info: &VariableInfo) -> bool {
    info.data_type == VariableType::Char && info.count > 1
}

/// Build the JSON Schema property for one telemetry variable.
///
/// Behavior:
/// - Character buffers are emitted as `string`
/// - Non-char variables are emitted as scalar schemas, wrapped in a fixed-size array when
///   `count > 1`
/// - The variable description is set as schema `description`
/// - iRacing-specific metadata is stored in schema extensions:
///   `x-units`, `x-iracing-var-type`, `x-count`, `x-offset`, and `x-count-as-time`
fn telemetry_property_schema(info: &VariableInfo) -> Schema {
    if is_char_buffer(info) {
        let mut schema = schema_with_type("string");
        annotate_telemetry_schema(&mut schema, info, false);
        return schema;
    }

    let mut schema = scalar_schema_for_variable_type(&info.data_type);
    annotate_telemetry_schema(&mut schema, info, true);

    if info.count > 1 {
        array_schema_for_count(schema, info.count)
    } else {
        schema
    }
}

/// Add telemetry metadata fields to a schema object.
///
/// This injects human-readable and iRacing-specific keys used by downstream schema tooling.
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
    if include_count_as_time {
        obj.insert("x-count-as-time".into(), info.count_as_time.into());
    }
    obj.insert("x-count".into(), (info.count as u64).into());
    obj.insert("x-offset".into(), (info.offset as u64).into());
}

/// Wrap an item schema in a fixed-size JSON Schema array.
///
/// The emitted array uses both `minItems` and `maxItems` set to `count` to represent
/// iRacing variables whose element count is statically known.
fn array_schema_for_count(item_schema: Schema, count: usize) -> Schema {
    let mut obj = Map::new();
    obj.insert("type".into(), "array".into());
    obj.insert("items".into(), item_schema.to_value());
    obj.insert("minItems".into(), (count as u64).into());
    obj.insert("maxItems".into(), (count as u64).into());
    Schema::from(obj)
}

/// Build the root telemetry schema from a [`VariableSchema`] definition.
///
/// Every variable name becomes an object property. The resulting root schema:
/// - has title `Telemetry`
/// - has description `Telemetry from iRacing`
/// - disallows unknown top-level properties (`additionalProperties = false`)
fn schema_for_variable_schema(schema: &VariableSchema) -> Schema {
    let mut properties = Map::new();

    for (name, info) in &schema.variables {
        properties.insert(name.clone(), telemetry_property_schema(info).to_value());
    }

    let mut root = schema_with_type("object");
    let obj = root.ensure_object();
    obj.insert("title".into(), "Telemetry".into());
    obj.insert("description".into(), "Telemetry from iRacing".into());
    obj.insert("properties".into(), Value::Object(properties));
    obj.insert("additionalProperties".into(), Value::Bool(false));

    root
}

/// Generate the baseline session JSON Schema from [`SessionInfo`] Rust types.
///
/// This requires the crate `codegen` feature and does not include runtime discovery overlays.
pub fn session_root_schema() -> Schema {
    schema_for!(SessionInfo)
}

#[cfg(feature = "schema-discovery")]
/// Generate the session JSON Schema with discovered unknown fields overlaid.
///
/// Starts from [`session_root_schema`], then augments paths returned by
/// [`SessionInfo::collect_unknown_fields`] with discovery metadata (`x-discovered`,
/// `x-example`). Existing known property type information is preserved.
///
/// Available only when both `codegen` and `schema-discovery` features are enabled.
pub fn session_root_schema_with_discovery(session: &SessionInfo) -> Schema {
    let mut schema = session_root_schema();

    for field in session.collect_unknown_fields() {
        overlay_discovered_field(&mut schema, &field.path, &field.data_type, &field.example);
    }

    schema
}

#[cfg(feature = "schema-discovery")]
/// Overlay one discovered field into an existing root schema.
///
/// The `path` tokenizes object segments and array depth markers (`[]`) to navigate or create
/// intermediate schema nodes. The leaf node receives discovered metadata via
/// [`apply_discovered_leaf`]. If traversal encounters an incompatible boolean schema node, this
/// function returns early without mutating deeper nodes.
fn overlay_discovered_field(
    root: &mut Schema,
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

    let mut current = schema_root_value_mut(root);

    for parent in parents {
        if !parent.key.is_empty() {
            let properties = ensure_object_properties(current);
            let entry = properties
                .entry(parent.key.clone())
                .or_insert_with(default_object_schema);

            if schema_object_mut(entry).is_none() {
                return;
            }
            current = entry;
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
                .or_insert_with(|| Value::Object(Map::new()))
        } else {
            properties
                .entry(leaf.key.clone())
                .or_insert_with(default_object_schema)
        };

        if leaf.array_depth == 0 {
            apply_discovered_leaf(entry, data_type, example);
            return;
        }

        if schema_object_mut(entry).is_none() {
            return;
        }

        let mut leaf_schema = entry;

        for idx in 0..leaf.array_depth {
            if idx + 1 == leaf.array_depth {
                let item_schema = ensure_array_item_schema(leaf_schema);
                apply_discovered_leaf(item_schema, data_type, example);
            } else {
                leaf_schema = ensure_array_item_object(leaf_schema);
            }
        }
    }
}

#[cfg(feature = "schema-discovery")]
#[derive(Debug)]
/// Parsed discovery path segment.
///
/// `key` is the object property name for this segment. `array_depth` is the number of array
/// levels (`[]`) attached to the segment.
struct PathToken {
    key: String,
    array_depth: usize,
}

#[cfg(feature = "schema-discovery")]
/// Parse a discovered field path into object keys and array depths.
///
/// Example:
/// - `SessionInfo.Sessions[0].Results[2].NewMetric` becomes tokens with keys
///   `SessionInfo`, `Sessions`, `Results`, `NewMetric` and array depths `0, 1, 1, 0`.
///
/// Index values are ignored; only the presence of array levels matters for schema shape.
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
/// Ensure a schema node is object-shaped and return its mutable `properties` map.
///
/// If `type` or `properties` are absent, defaults are inserted.
fn ensure_object_properties(schema_obj: &mut Value) -> &mut Map<String, Value> {
    let object = ensure_schema_object(schema_obj);

    if !object.contains_key("type") {
        object.insert("type".into(), "object".into());
    }

    let properties = object
        .entry("properties".into())
        .or_insert_with(|| Value::Object(Map::new()));

    if !properties.is_object() {
        *properties = Value::Object(Map::new());
    }

    properties
        .as_object_mut()
        .expect("properties must be an object")
}

#[cfg(feature = "schema-discovery")]
/// Ensure a schema node is an array whose item schema is an object, then return that object.
///
/// If the existing item schema is boolean, it is replaced with a default object schema before
/// returning.
fn ensure_array_item_object(schema_obj: &mut Value) -> &mut Value {
    let item = ensure_array_item_schema(schema_obj);

    match item {
        Value::Object(_) => item,
        Value::Bool(_) => {
            *item = default_object_schema();
            item
        }
        _ => {
            *item = default_object_schema();
            item
        }
    }
}

#[cfg(feature = "schema-discovery")]
/// Ensure a schema node is an array and return a mutable reference to its item schema.
///
/// This normalizes missing `items` and supports both homogeneous items (`object`/`bool`) and
/// tuple-style arrays (`items: []`) by returning the first item schema.
fn ensure_array_item_schema(schema_obj: &mut Value) -> &mut Value {
    let object = ensure_schema_object(schema_obj);

    if !object.contains_key("type") {
        object.insert("type".into(), "array".into());
    }

    let items = object
        .entry("items".into())
        .or_insert_with(default_object_schema);

    match items {
        Value::Array(vec) => {
            if vec.is_empty() {
                vec.push(default_object_schema());
            }
            vec.first_mut().expect("item inserted")
        }
        Value::Object(_) | Value::Bool(_) => items,
        _ => {
            *items = default_object_schema();
            items
        }
    }
}

#[cfg(feature = "schema-discovery")]
/// Return a mutable object view of a schema value when possible.
///
/// Returns `None` for non-object values, including boolean schemas.
fn schema_object_mut(schema: &mut Value) -> Option<&mut Map<String, Value>> {
    schema.as_object_mut()
}

#[cfg(feature = "schema-discovery")]
/// Create a default object schema node used for discovery path expansion.
fn default_object_schema() -> Value {
    let mut map = Map::new();
    map.insert("type".into(), "object".into());
    Value::Object(map)
}

#[cfg(feature = "schema-discovery")]
/// Apply discovered leaf metadata and type hints to a schema node.
///
/// Behavior:
/// - Maps [`crate::schema::session::UnknownFieldType`] to JSON `type`
/// - Sets `type` only when currently absent (does not overwrite known types)
/// - Adds discovery extensions:
///   - `x-discovered = true`
///   - `x-example = <example>`
///
/// If `schema` is not an object, this function is a no-op.
fn apply_discovered_leaf(
    schema: &mut Value,
    data_type: &crate::schema::session::UnknownFieldType,
    example: &str,
) {
    let expected_type = match data_type {
        crate::schema::session::UnknownFieldType::String => "string",
        crate::schema::session::UnknownFieldType::Number => "number",
        crate::schema::session::UnknownFieldType::Boolean => "boolean",
        crate::schema::session::UnknownFieldType::Null => "null",
        crate::schema::session::UnknownFieldType::Object => "object",
        crate::schema::session::UnknownFieldType::Array => "array",
    };

    let Some(obj) = schema_object_mut(schema) else {
        return;
    };

    if !obj.contains_key("type") {
        obj.insert("type".into(), expected_type.into());
    }

    obj.insert("x-discovered".into(), true.into());
    obj.insert("x-example".into(), example.to_string().into());
}

/// Convert a borrowed telemetry variable schema into a root JSON schema.
///
/// This is a convenience conversion that preserves the input borrow and delegates to
/// internal telemetry schema construction.
impl From<&VariableSchema> for Schema {
    fn from(v: &VariableSchema) -> Self {
        schema_for_variable_schema(v)
    }
}

/// Convert an owned telemetry variable schema into a root JSON schema.
///
/// This delegates to [`From<&VariableSchema> for Schema`] to keep conversion behavior
/// centralized.
impl From<VariableSchema> for Schema {
    fn from(v: VariableSchema) -> Self {
        Schema::from(&v)
    }
}

/// Build a schema object with a single JSON `type` value.
fn schema_with_type(instance_type: &str) -> Schema {
    let mut obj = Map::new();
    obj.insert("type".into(), instance_type.into());
    Schema::from(obj)
}

/// Return the mutable root JSON value wrapped by a [`Schema`].
#[cfg(feature = "schema-discovery")]
fn schema_root_value_mut(schema: &mut Schema) -> &mut Value {
    schema.pointer_mut("").expect("root JSON pointer is valid")
}

/// Ensure a JSON value is an object and return a mutable reference to that object.
#[cfg(feature = "schema-discovery")]
fn ensure_schema_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }

    value
        .as_object_mut()
        .expect("value should be converted to object")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_root_schema_has_expected_top_level_properties() {
        let schema = session_root_schema();
        let root = schema.as_object().expect("Session schema should be an object");
        let properties = root
            .get("properties")
            .and_then(Value::as_object)
            .expect("Session schema should expose object properties");

        assert!(properties.contains_key("WeekendInfo"));
        assert!(properties.contains_key("SessionInfo"));
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
        let root = schema.as_object().unwrap();
        let custom = root
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("CustomRoot"))
            .unwrap();

        let custom_obj = custom.as_object().expect("Expected object schema");
        assert_eq!(custom_obj.get("x-discovered"), Some(&Value::Bool(true)));
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
        let root = schema.as_object().unwrap();
        let session_info = root
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("SessionInfo"))
            .unwrap();

        let sessions = session_info
            .as_object()
            .and_then(|object| object.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("Sessions"))
            .unwrap();

        let item = sessions
            .as_object()
            .and_then(|object| object.get("items"))
            .map(|items| match items {
                Value::Array(vec) => vec.first().expect("Expected tuple item"),
                value => value,
            })
            .unwrap();

        let new_metric = item
            .as_object()
            .and_then(|object| object.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("NewMetric"))
            .unwrap();

        let new_metric_obj = new_metric.as_object().expect("Expected object leaf");
        assert_eq!(new_metric_obj.get("x-discovered"), Some(&Value::Bool(true)));
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
        let root = schema.as_object().unwrap();
        let track_name = root
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("WeekendInfo"))
            .and_then(Value::as_object)
            .and_then(|object| object.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("TrackName"))
            .unwrap();

        assert_eq!(
            track_name
                .as_object()
                .and_then(|obj| obj.get("type"))
                .and_then(Value::as_str),
            Some("string")
        );
    }
}
