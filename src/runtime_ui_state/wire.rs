use toml::Value;

use super::{RuntimeStateObservedEnvelope, RuntimeUiFileStatus, RuntimeUiWireState};

mod v1;

pub(crate) const RUNTIME_UI_WIRE_VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodedRuntimeUiFile {
    pub(crate) status: RuntimeUiFileStatus,
    pub(crate) envelope: RuntimeStateObservedEnvelope,
    pub(crate) supported_wire: Option<RuntimeUiWireState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeUiWireError {
    message: String,
}

impl RuntimeUiWireError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

pub(crate) fn decode_runtime_ui_file(bytes: &[u8]) -> DecodedRuntimeUiFile {
    let invalid = || DecodedRuntimeUiFile {
        status: RuntimeUiFileStatus::Invalid,
        envelope: RuntimeStateObservedEnvelope::PresentWithoutReadableVersion,
        supported_wire: None,
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return invalid();
    };
    let Ok(Value::Table(mut root)) = toml::from_str::<Value>(text) else {
        return invalid();
    };
    let Some(version) = root.get("version").and_then(Value::as_integer) else {
        return invalid();
    };
    let Ok(version) = u64::try_from(version) else {
        return invalid();
    };
    if version != RUNTIME_UI_WIRE_VERSION {
        return DecodedRuntimeUiFile {
            status: RuntimeUiFileStatus::UnsupportedReadOnly {
                version: Some(version),
            },
            envelope: RuntimeStateObservedEnvelope::Version(version),
            supported_wire: None,
        };
    }
    match v1::decode(&mut root) {
        Ok(wire) => DecodedRuntimeUiFile {
            status: RuntimeUiFileStatus::Supported,
            envelope: RuntimeStateObservedEnvelope::Version(RUNTIME_UI_WIRE_VERSION),
            supported_wire: Some(wire),
        },
        Err(_) => invalid(),
    }
}

pub(crate) fn encode_runtime_ui_file(
    wire: &RuntimeUiWireState,
) -> Result<Vec<u8>, RuntimeUiWireError> {
    let value = v1::encode(wire)?;
    let mut encoded = toml::to_string_pretty(&value)
        .map_err(|error| RuntimeUiWireError::new(format!("could not encode V1: {error}")))?;
    if !encoded.ends_with('\n') {
        encoded.push('\n');
    }
    Ok(encoded.into_bytes())
}

fn preserve_value(value: &Value) -> Result<String, RuntimeUiWireError> {
    let mut wrapper = toml::map::Map::new();
    wrapper.insert("preserved".to_string(), value.clone());
    toml::to_string(&Value::Table(wrapper))
        .map_err(|error| RuntimeUiWireError::new(format!("could not preserve TOML value: {error}")))
}

fn restore_value(value: &str) -> Result<Value, RuntimeUiWireError> {
    let Value::Table(mut wrapper) = toml::from_str::<Value>(value)
        .map_err(|error| RuntimeUiWireError::new(format!("invalid passthrough value: {error}")))?
    else {
        return Err(RuntimeUiWireError::new("invalid passthrough wrapper"));
    };
    wrapper
        .remove("preserved")
        .ok_or_else(|| RuntimeUiWireError::new("passthrough wrapper omitted its value"))
}

#[cfg(test)]
mod tests;
