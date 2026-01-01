use super::Config;
use schemars::schema_for;
use serde_json::Value;

impl Config {
    /// Generates a JSON Schema describing the full configuration surface.
    #[allow(dead_code)]
    pub fn json_schema() -> Value {
        serde_json::to_value(schema_for!(Config))
            .expect("serializing configuration schema should succeed")
    }
}
