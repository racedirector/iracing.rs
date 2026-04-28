#[cfg(feature = "codegen")]
use serde_json::{Map, Value};

#[cfg(feature = "codegen")]
pub(crate) fn named_schema_values(values: &[(&'static str, i64)]) -> Value {
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
