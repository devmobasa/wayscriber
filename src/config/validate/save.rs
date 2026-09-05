use super::{Config, ConfigValidationReport};

/// Why an authored configuration cannot be saved without losing user input.
#[derive(Debug)]
pub enum SaveValidationError {
    CorrectedValues,
    Representation(toml::ser::Error),
}

impl std::fmt::Display for SaveValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CorrectedValues => f.write_str("Some values are outside their allowed ranges and would be changed on save. Fix them before saving."),
            Self::Representation(error) => write!(f, "Configuration could not be represented for validation: {error}"),
        }
    }
}
impl std::error::Error for SaveValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Representation(error) => Some(error),
            Self::CorrectedValues => None,
        }
    }
}
impl From<toml::ser::Error> for SaveValidationError {
    fn from(error: toml::ser::Error) -> Self {
        Self::Representation(error)
    }
}

impl Config {
    /// Validate an authored save, rejecting corrections outside keybindings.
    /// Keybinding arbitration is intentional and returned for user feedback.
    pub fn validate_for_save(
        mut self,
    ) -> Result<(Self, ConfigValidationReport), SaveValidationError> {
        let mut before = toml::Value::try_from(&self)?;
        let report = self.validate_and_clamp();
        let mut after = toml::Value::try_from(&self)?;
        // Compare persisted typed values, independent of diagnostic formatting
        // and document-only keybinding authorship metadata.
        for value in [&mut before, &mut after] {
            if let Some(table) = value.as_table_mut() {
                table.remove("keybindings");
            }
        }
        if before != after {
            return Err(SaveValidationError::CorrectedValues);
        }
        Ok((self, report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_out_of_range_values_are_rejected() {
        let mut config = Config::default();
        config.drawing.default_thickness = 999.0;
        assert!(matches!(
            config.validate_for_save(),
            Err(SaveValidationError::CorrectedValues)
        ));
    }
    #[test]
    fn unchanged_values_are_accepted() {
        assert!(Config::default().validate_for_save().is_ok());
    }
}
