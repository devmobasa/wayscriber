use super::Config;
use schemars::schema_for;
use serde_json::Value;

impl Config {
    /// Generates a JSON Schema describing the full configuration surface.
    ///
    /// # Panics
    ///
    /// Panics if the generated schema cannot be converted to a JSON value.
    /// The schema is already JSON, so this only happens on a `schemars` bug.
    #[allow(dead_code)]
    #[must_use]
    pub fn json_schema() -> Value {
        serde_json::to_value(schema_for!(Config))
            .expect("serializing configuration schema should succeed")
    }
}
