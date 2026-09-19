#[cfg(feature = "codegen")]
pub(crate) fn named_schema_values(values: &[(&str, i64)]) -> serde_json::Value {
    serde_json::Value::Array(
        values
            .iter()
            .map(|(name, value)| serde_json::json!({ "name": name, "value": value }))
            .collect(),
    )
}
