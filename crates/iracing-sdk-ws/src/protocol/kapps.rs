use std::{collections::BTreeMap, sync::Arc};

use iracing_sdk::{DynamicFrame, TelemetryValue};
use serde::{Deserialize, Deserializer, de};
use serde_json::{Number, Value};

#[derive(Debug, Clone, Copy)]
struct Fps(u8);

impl Fps {
    fn new(value: u8) -> Option<Self> {
        (1..=60).contains(&value).then_some(Self(value))
    }

    const fn get(self) -> u8 {
        self.0
    }
}

impl<'de> Deserialize<'de> for Fps {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        Self::new(value).ok_or_else(|| de::Error::custom("fps must be between 1 and 60"))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KappsRequest {
    fps: Fps,
    read_ibt: bool,
    request_params: Vec<String>,
    request_params_once: Vec<String>,
}

impl KappsRequest {
    pub(crate) fn read_ibt(&self) -> bool {
        self.read_ibt
    }
}

pub(crate) struct Subscription {
    fps: u8,
    recurring: Vec<String>,
    once: Vec<String>,
}

impl Subscription {
    pub(crate) fn from_request(request: KappsRequest) -> Self {
        Self {
            fps: request.fps.get(),
            recurring: request.request_params,
            once: request.request_params_once,
        }
    }

    pub(crate) fn fps(&self) -> u8 {
        self.fps
    }

    pub(crate) fn response(
        &mut self,
        frame: Option<&Arc<DynamicFrame>>,
        session: Option<&Arc<Value>>,
    ) -> Option<BTreeMap<String, Value>> {
        if frame.is_none() && session.is_none() {
            return None;
        }

        let mut response = BTreeMap::new();

        for name in &self.recurring {
            response.insert(
                name.clone(),
                resolve(name, frame, session).unwrap_or(Value::Null),
            );
        }

        let sources_ready = frame.is_some() && session.is_some();
        let mut pending = Vec::new();
        for name in std::mem::take(&mut self.once) {
            match resolve(&name, frame, session) {
                Some(value) => {
                    response.insert(name, value);
                }
                None if sources_ready => {
                    response.insert(name, Value::Null);
                }
                None => pending.push(name),
            }
        }
        self.once = pending;

        (!response.is_empty()).then_some(response)
    }
}

fn resolve(
    name: &str,
    frame: Option<&Arc<DynamicFrame>>,
    session: Option<&Arc<Value>>,
) -> Option<Value> {
    if let Some(value) = frame.and_then(|frame| frame.value(name).ok().flatten()) {
        return Some(telemetry_json(value));
    }

    session.and_then(|session| session_path(session, name).cloned())
}

fn telemetry_json(value: TelemetryValue) -> Value {
    match value {
        TelemetryValue::Char(value) => Value::from(value),
        TelemetryValue::Int8(value) => Value::from(value),
        TelemetryValue::UInt8(value) => Value::from(value),
        TelemetryValue::Int16(value) => Value::from(value),
        TelemetryValue::UInt16(value) => Value::from(value),
        TelemetryValue::Int32(value) => Value::from(value),
        TelemetryValue::UInt32(value) => Value::from(value),
        TelemetryValue::Float32(value) => Number::from_f64(value as f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        TelemetryValue::Float64(value) => Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        TelemetryValue::Bool(value) => Value::from(value),
        TelemetryValue::BitField(value) => Value::from(value.value()),
        TelemetryValue::Array(values) => {
            Value::Array(values.into_iter().map(telemetry_json).collect())
        }
    }
}

fn session_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;

    for segment in path.split('.') {
        let key_end = segment.find('[').unwrap_or(segment.len());
        let key = &segment[..key_end];
        if !key.is_empty() {
            current = current.get(key)?;
        }

        let mut rest = &segment[key_end..];
        while !rest.is_empty() {
            if !rest.starts_with('[') {
                return None;
            }
            let index_end = rest.find(']')?;
            let index = rest[1..index_end].parse::<usize>().ok()?;
            current = current.get(index)?;
            rest = &rest[index_end + 1..];
        }
    }

    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_validates_fps() {
        let valid = r#"{"fps":30,"readIbt":false,"requestParams":[],"requestParamsOnce":[]}"#;
        assert!(serde_json::from_str::<KappsRequest>(valid).is_ok());

        for fps in [0, 61] {
            let invalid = format!(
                r#"{{"fps":{fps},"readIbt":false,"requestParams":[],"requestParamsOnce":[]}}"#
            );
            assert!(serde_json::from_str::<KappsRequest>(&invalid).is_err());
        }
    }

    #[test]
    fn session_path_supports_objects_and_arrays() {
        let session = serde_json::json!({
            "DriverInfo": { "Drivers": [{ "UserName": "Driver" }] }
        });

        assert_eq!(
            session_path(&session, "DriverInfo.Drivers[0].UserName"),
            Some(&Value::String("Driver".to_owned()))
        );
    }
}
