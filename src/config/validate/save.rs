use super::{Config, ConfigValidationReport};

/// Why an authored configuration cannot be saved without losing user input.
#[derive(Debug)]
pub enum SaveValidationError {
    /// Validation would change these values on their way to disk. Never
    /// empty: an unchanged configuration is accepted.
    CorrectedValues(Vec<CorrectedValue>),
    Representation(toml::ser::Error),
}

/// One persisted value that validation would change on save.
#[derive(Debug, Clone, PartialEq)]
pub struct CorrectedValue {
    /// Dotted config path with list indices, e.g. `drawing.default_thickness`
    /// or `boards.items[1].id`: the same spelling the editor uses for its
    /// field errors.
    pub path: String,
    /// What the save was asked to write. `None` when validation would add
    /// the value.
    pub authored: Option<toml::Value>,
    /// What validation would write instead. `None` when it would remove the
    /// value.
    pub corrected: Option<toml::Value>,
}

impl CorrectedValue {
    /// The change without the path, for a caller that shows the path itself.
    pub fn summary(&self) -> String {
        match (&self.authored, &self.corrected) {
            (Some(authored), Some(corrected)) => format!(
                "{} would be saved as {}",
                describe_value(authored),
                describe_value(corrected)
            ),
            (Some(authored), None) => {
                format!("{} would be removed on save", describe_value(authored))
            }
            (None, Some(corrected)) => {
                format!("{} would be added on save", describe_value(corrected))
            }
            (None, None) => "would change on save".to_string(),
        }
    }
}

impl std::fmt::Display for CorrectedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.summary())
    }
}

/// Scalars read best inline; a list or table the save would reshape is
/// summarized by size rather than dumped whole into a status line.
fn describe_value(value: &toml::Value) -> String {
    match value {
        toml::Value::Array(items) => match items.len() {
            1 => "1 entry".to_string(),
            count => format!("{count} entries"),
        },
        toml::Value::Table(_) => "a table".to_string(),
        scalar => scalar.to_string(),
    }
}

impl std::fmt::Display for SaveValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CorrectedValues(values) => {
                f.write_str("Some values would be changed on save. Fix them before saving:")?;
                for value in values {
                    write!(f, "\n{value}")?;
                }
                Ok(())
            }
            Self::Representation(error) => write!(
                f,
                "Configuration could not be represented for validation: {error}"
            ),
        }
    }
}
impl std::error::Error for SaveValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Representation(error) => Some(error),
            Self::CorrectedValues(_) => None,
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

        let mut corrected = Vec::new();
        collect_corrections(
            &mut String::new(),
            Some(&before),
            Some(&after),
            &mut corrected,
        );
        if !corrected.is_empty() {
            return Err(SaveValidationError::CorrectedValues(corrected));
        }
        Ok((self, report))
    }
}

/// Records the smallest paths at which `before` and `after` differ.
///
/// Tables are compared key by key and equal-length lists index by index, so a
/// clamp names its own field. A list whose length changed is reported whole:
/// its indices no longer line up, and naming one of them would point at the
/// wrong entry.
fn collect_corrections(
    path: &mut String,
    before: Option<&toml::Value>,
    after: Option<&toml::Value>,
    corrected: &mut Vec<CorrectedValue>,
) {
    if before == after {
        return;
    }

    match (before, after) {
        (Some(toml::Value::Table(before)), Some(toml::Value::Table(after))) => {
            let added = after.keys().filter(|key| !before.contains_key(*key));
            for key in before.keys().chain(added) {
                let len = path.len();
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(key);
                collect_corrections(path, before.get(key), after.get(key), corrected);
                path.truncate(len);
            }
        }
        (Some(toml::Value::Array(before)), Some(toml::Value::Array(after)))
            if before.len() == after.len() =>
        {
            for (index, (before, after)) in before.iter().zip(after).enumerate() {
                let len = path.len();
                path.push_str(&format!("[{index}]"));
                collect_corrections(path, Some(before), Some(after), corrected);
                path.truncate(len);
            }
        }
        (before, after) => corrected.push(CorrectedValue {
            path: path.clone(),
            authored: before.cloned(),
            corrected: after.cloned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BoardsConfig;

    fn corrections(config: Config) -> Vec<CorrectedValue> {
        match config.validate_for_save() {
            Err(SaveValidationError::CorrectedValues(values)) => values,
            other => panic!("expected corrected values, got {other:?}"),
        }
    }

    #[test]
    fn authored_out_of_range_values_are_rejected() {
        let mut config = Config::default();
        config.drawing.default_thickness = 999.0;

        assert_eq!(
            corrections(config),
            vec![CorrectedValue {
                path: "drawing.default_thickness".to_string(),
                authored: Some(toml::Value::Float(999.0)),
                corrected: Some(toml::Value::Float(50.0)),
            }]
        );
    }

    /// A normalization inside a list names the entry and the key, so an
    /// editor can point at the one field the user has to change.
    #[test]
    fn a_corrected_board_id_is_named_by_its_index() {
        let mut config = Config::default();
        let boards = config.boards.get_or_insert_with(BoardsConfig::default);
        boards.items[1].id = "Math".to_string();
        boards.default_board = boards.items[0].id.clone();

        let corrected = corrections(config);

        assert_eq!(corrected.len(), 1, "{corrected:?}");
        assert_eq!(corrected[0].path, "boards.items[1].id");
        assert_eq!(
            corrected[0].to_string(),
            "boards.items[1].id: \"Math\" would be saved as \"math\""
        );
    }

    /// A list the save would lengthen or shorten is reported as a whole,
    /// because its indices no longer describe the same entries.
    #[test]
    fn a_list_that_changes_length_is_reported_whole() {
        let mut config = Config::default();
        let boards = config.boards.get_or_insert_with(BoardsConfig::default);
        boards.max_count = 1;
        boards.items.truncate(2);

        let corrected = corrections(config);

        assert_eq!(corrected.len(), 1, "{corrected:?}");
        assert_eq!(
            corrected[0].to_string(),
            "boards.items: 2 entries would be saved as 1 entry"
        );
    }

    #[test]
    fn the_error_message_lists_every_corrected_path() {
        let mut config = Config::default();
        config.drawing.default_thickness = 999.0;
        config.arrow.length = 1.0;
        let Err(error) = config.validate_for_save() else {
            panic!("out-of-range values must be rejected");
        };

        let message = error.to_string();

        assert!(message.contains("drawing.default_thickness: 999.0 would be saved as 50.0"));
        assert!(message.contains("arrow.length: 1.0 would be saved as 5.0"));
    }

    #[test]
    fn unchanged_values_are_accepted() {
        assert!(Config::default().validate_for_save().is_ok());
    }
}
