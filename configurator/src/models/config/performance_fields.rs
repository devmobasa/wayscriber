use wayscriber::config::{PerformanceFieldId, ScalarConstraint, performance_field_metadata};

use super::ConfigDraft;
use crate::models::error::FormError;

pub(super) fn validate_performance_u32(
    id: PerformanceFieldId,
    value: u32,
    errors: &mut Vec<FormError>,
) -> Option<u32> {
    let metadata = performance_field_metadata(id);
    if metadata.constraint.accepts_u32(value) {
        return Some(value);
    }

    let expected = match metadata.constraint {
        ScalarConstraint::Unsigned { min, max } => format!("Expected {min}-{max}"),
        ScalarConstraint::UnsignedChoice(values) => format!(
            "Expected one of {}",
            values
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ScalarConstraint::Boolean => "Expected a boolean".to_string(),
    };
    errors.push(FormError::new(metadata.path, expected));
    None
}

pub(super) fn parse_performance_u32(
    id: PerformanceFieldId,
    input: &str,
    errors: &mut Vec<FormError>,
) -> Option<u32> {
    let metadata = performance_field_metadata(id);
    match input.trim().parse::<u32>() {
        Ok(value) => validate_performance_u32(id, value, errors),
        Err(error) => {
            errors.push(FormError::new(metadata.path, error.to_string()));
            None
        }
    }
}

impl ConfigDraft {
    pub(super) fn set_performance_bool(&mut self, id: PerformanceFieldId, value: bool) {
        match id {
            PerformanceFieldId::EnableVsync => self.performance_enable_vsync = value,
            PerformanceFieldId::BufferCount
            | PerformanceFieldId::MaxFpsNoVsync
            | PerformanceFieldId::UiAnimationFps => {
                unreachable!("non-boolean Performance field routed through boolean setter")
            }
        }
    }

    pub(super) fn set_performance_text(&mut self, id: PerformanceFieldId, value: String) {
        match id {
            PerformanceFieldId::MaxFpsNoVsync => self.performance_max_fps_no_vsync = value,
            PerformanceFieldId::UiAnimationFps => self.performance_ui_animation_fps = value,
            PerformanceFieldId::BufferCount | PerformanceFieldId::EnableVsync => {
                unreachable!("non-text Performance field routed through text setter")
            }
        }
    }

    pub(crate) fn set_performance_choice(&mut self, id: PerformanceFieldId, value: u32) {
        match id {
            PerformanceFieldId::BufferCount => self.performance_buffer_count = value,
            PerformanceFieldId::EnableVsync
            | PerformanceFieldId::MaxFpsNoVsync
            | PerformanceFieldId::UiAnimationFps => {
                unreachable!("non-choice Performance field routed through choice setter")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use wayscriber::config::{PERFORMANCE_FIELD_METADATA, PerformanceFieldId, ScalarConstraint};

    use super::*;

    #[test]
    fn every_performance_descriptor_routes_through_a_typed_binding() {
        let mut draft = ConfigDraft::from_config(&wayscriber::config::Config::default());
        for metadata in PERFORMANCE_FIELD_METADATA {
            match metadata.constraint {
                ScalarConstraint::Boolean => {
                    draft.set_performance_bool(metadata.id, true);
                }
                ScalarConstraint::Unsigned { min, .. } => {
                    draft.set_performance_text(metadata.id, min.to_string());
                }
                ScalarConstraint::UnsignedChoice(values) => {
                    draft.set_performance_choice(metadata.id, values[0]);
                }
            }
        }
    }

    #[test]
    fn typed_bindings_update_each_performance_draft_field() {
        let mut draft = ConfigDraft::from_config(&wayscriber::config::Config::default());
        draft.set_performance_choice(PerformanceFieldId::BufferCount, 4);
        draft.set_toggle(crate::models::ToggleField::PerformanceVsync, true);
        draft.set_text(
            crate::models::TextField::PerformanceMaxFpsNoVsync,
            "144".to_string(),
        );
        draft.set_text(
            crate::models::TextField::PerformanceUiAnimationFps,
            "60".to_string(),
        );

        assert_eq!(draft.performance_buffer_count, 4);
        assert!(draft.performance_enable_vsync);
        assert_eq!(draft.performance_max_fps_no_vsync, "144");
        assert_eq!(draft.performance_ui_animation_fps, "60");
    }

    #[test]
    fn conversion_rejects_values_outside_shared_performance_constraints() {
        let base = wayscriber::config::Config::default();
        let mut draft = ConfigDraft::from_config(&base);
        draft.performance_buffer_count = 5;
        draft.performance_ui_animation_fps = "241".to_string();

        let errors = draft
            .to_config(&base)
            .expect_err("out-of-range Performance values must remain actionable");
        assert!(
            errors
                .iter()
                .any(|error| error.field == "performance.buffer_count")
        );
        assert!(
            errors
                .iter()
                .any(|error| error.field == "performance.ui_animation_fps")
        );
    }
}
