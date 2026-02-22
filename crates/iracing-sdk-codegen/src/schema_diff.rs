use std::collections::{BTreeMap, BTreeSet};

use schemars::Schema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchemaDiffReport {
    pub added_paths: Vec<PathEntry>,
    pub removed_paths: Vec<PathEntry>,
    pub type_changed: Vec<TypeChangeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathEntry {
    pub path: String,
    pub types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TypeChangeEntry {
    pub path: String,
    pub current_types: Vec<String>,
    pub baseline_types: Vec<String>,
}

pub fn diff_schemas(current: &Schema, baseline: &Schema) -> SchemaDiffReport {
    let current_paths = collect_schema_paths(current);
    let baseline_paths = collect_schema_paths(baseline);

    let mut added_paths = Vec::new();
    let mut removed_paths = Vec::new();
    let mut type_changed = Vec::new();

    for (path, types) in &current_paths {
        match baseline_paths.get(path) {
            None => added_paths.push(PathEntry {
                path: path.clone(),
                types: to_vec(types),
            }),
            Some(baseline_types) if baseline_types != types => type_changed.push(TypeChangeEntry {
                path: path.clone(),
                current_types: to_vec(types),
                baseline_types: to_vec(baseline_types),
            }),
            Some(_) => {}
        }
    }

    for (path, types) in &baseline_paths {
        if !current_paths.contains_key(path) {
            removed_paths.push(PathEntry {
                path: path.clone(),
                types: to_vec(types),
            });
        }
    }

    added_paths.sort_by(|a, b| a.path.cmp(&b.path));
    removed_paths.sort_by(|a, b| a.path.cmp(&b.path));
    type_changed.sort_by(|a, b| a.path.cmp(&b.path));

    SchemaDiffReport {
        added_paths,
        removed_paths,
        type_changed,
    }
}

pub fn summarize_diff(report: &SchemaDiffReport) -> String {
    format!(
        "Schema diff: {} added, {} removed, {} type-changed paths",
        report.added_paths.len(),
        report.removed_paths.len(),
        report.type_changed.len()
    )
}

fn collect_schema_paths(schema: &Schema) -> BTreeMap<String, BTreeSet<String>> {
    let mut paths = BTreeMap::new();
    collect_schema_paths_from_value(schema.as_value(), "$", &mut paths);
    paths
}

fn collect_schema_object_paths(
    schema: &serde_json::Map<String, Value>,
    path: &str,
    out: &mut BTreeMap<String, BTreeSet<String>>,
) {
    let mut types = normalize_instance_types(schema.get("type"));

    if schema
        .get("properties")
        .and_then(Value::as_object)
        .is_some()
    {
        types.insert("object".to_string());
    }
    if schema.get("items").is_some() {
        types.insert("array".to_string());
    }

    if !types.is_empty() {
        out.insert(path.to_string(), types);
    }

    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (property_name, property_schema) in properties {
            let child_path = format!("{}.{}", path, property_name);
            collect_schema_paths_from_value(property_schema, &child_path, out);
        }
    }

    if let Some(items) = schema.get("items") {
        let array_path = format!("{}[]", path);
        match items {
            Value::Array(items) => {
                for item in items {
                    collect_schema_paths_from_value(item, &array_path, out);
                }
            }
            item => collect_schema_paths_from_value(item, &array_path, out),
        }
    }
}

fn collect_schema_paths_from_value(
    schema: &Value,
    path: &str,
    out: &mut BTreeMap<String, BTreeSet<String>>,
) {
    match schema {
        Value::Object(obj) => collect_schema_object_paths(obj, path, out),
        Value::Bool(value) => {
            let mut types = BTreeSet::new();
            types.insert(if *value {
                "any".to_string()
            } else {
                "never".to_string()
            });
            out.insert(path.to_string(), types);
        }
        _ => {}
    }
}

fn normalize_instance_types(types: Option<&Value>) -> BTreeSet<String> {
    let mut values = BTreeSet::new();

    match types {
        Some(Value::String(ty)) => {
            values.insert(ty.clone());
        }
        Some(Value::Array(items)) => {
            for item in items {
                if let Some(ty) = item.as_str() {
                    values.insert(ty.to_string());
                }
            }
        }
        _ => {}
    }

    values
}

fn to_vec(values: &BTreeSet<String>) -> Vec<String> {
    values.iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use schemars::schema_for;

    use super::*;

    #[derive(JsonSchema)]
    struct Baseline {
        value: i32,
    }

    #[derive(JsonSchema)]
    struct AddedField {
        value: i32,
        new_field: String,
    }

    #[derive(JsonSchema)]
    struct ChangedType {
        value: String,
    }

    #[test]
    fn diff_detects_added_paths() {
        let baseline = schema_for!(Baseline);
        let current = schema_for!(AddedField);

        let diff = diff_schemas(&current, &baseline);

        assert!(
            diff.added_paths
                .iter()
                .any(|entry| entry.path == "$.new_field")
        );
    }

    #[test]
    fn diff_detects_type_changes() {
        let baseline = schema_for!(Baseline);
        let current = schema_for!(ChangedType);

        let diff = diff_schemas(&current, &baseline);

        assert!(
            diff.type_changed
                .iter()
                .any(|entry| entry.path == "$.value")
        );
    }

    #[test]
    fn diff_ignores_metadata_only_changes() {
        let baseline = schema_for!(Baseline);
        let mut current = schema_for!(Baseline);

        current
            .ensure_object()
            .insert("title".into(), "Custom title".into());

        let diff = diff_schemas(&current, &baseline);

        assert!(diff.added_paths.is_empty());
        assert!(diff.removed_paths.is_empty());
        assert!(diff.type_changed.is_empty());
    }
}
