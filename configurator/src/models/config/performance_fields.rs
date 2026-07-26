use wayscriber::config::{
    PerformanceUnsignedChoiceFieldMetadata, PerformanceUnsignedRangeFieldMetadata,
};

use super::ConfigDraft;
use crate::models::error::FormError;

pub(super) fn validate_performance_choice(
    field: PerformanceUnsignedChoiceFieldMetadata,
    value: u32,
) -> Result<u32, FormError> {
    if field.accepts(value) {
        return Ok(value);
    }

    let choices = field
        .choices()
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    Err(FormError::new(
        field.presentation().path(),
        format!("Expected one of {choices}"),
    ))
}

pub(super) fn validate_performance_range(
    field: PerformanceUnsignedRangeFieldMetadata,
    value: u32,
) -> Result<u32, FormError> {
    if field.accepts(value) {
        return Ok(value);
    }

    Err(FormError::new(
        field.presentation().path(),
        format!("Expected {}-{}", field.min(), field.max()),
    ))
}

pub(super) fn parse_performance_range(
    field: PerformanceUnsignedRangeFieldMetadata,
    input: &str,
) -> Result<u32, FormError> {
    match input.trim().parse::<u32>() {
        Ok(value) => validate_performance_range(field, value),
        Err(error) => Err(FormError::new(
            field.presentation().path(),
            error.to_string(),
        )),
    }
}

impl ConfigDraft {
    pub(super) fn set_performance_vsync(&mut self, value: bool) {
        self.performance_enable_vsync = value;
    }

    pub(super) fn set_performance_max_fps_no_vsync(&mut self, value: String) {
        self.performance_max_fps_no_vsync = value;
    }

    pub(super) fn set_performance_ui_animation_fps(&mut self, value: String) {
        self.performance_ui_animation_fps = value;
    }

    pub(crate) fn set_performance_buffer_count(&mut self, value: u32) {
        self.performance_buffer_count = value;
    }
}

#[cfg(test)]
mod tests {
    use wayscriber::config::PERFORMANCE_FIELDS;

    use super::*;

    #[test]
    fn kind_specific_metadata_exposes_exact_performance_constraints() {
        let buffer_count = PERFORMANCE_FIELDS.buffer_count();
        let max_fps = PERFORMANCE_FIELDS.max_fps_no_vsync();
        let animation_fps = PERFORMANCE_FIELDS.ui_animation_fps();

        assert_eq!(buffer_count.choices(), &[2, 3, 4]);
        assert!(buffer_count.accepts(2));
        assert!(!buffer_count.accepts(5));
        assert_eq!(max_fps.bounds(), (0, u32::MAX));
        assert_eq!(animation_fps.bounds(), (0, 240));
        assert_eq!(
            PERFORMANCE_FIELDS.enable_vsync().presentation().path(),
            "performance.enable_vsync"
        );
    }

    #[test]
    fn named_bindings_update_each_performance_draft_field() {
        let mut draft = ConfigDraft::from_config(&wayscriber::config::Config::default());
        draft.set_performance_buffer_count(4);
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
    fn independent_performance_drafts_do_not_share_edits() {
        let mut first_config = wayscriber::config::Config::default();
        first_config.performance.buffer_count = 2;
        first_config.performance.enable_vsync = false;
        first_config.performance.max_fps_no_vsync = 60;
        first_config.performance.ui_animation_fps = 30;

        let mut second_config = wayscriber::config::Config::default();
        second_config.performance.buffer_count = 3;
        second_config.performance.enable_vsync = true;
        second_config.performance.max_fps_no_vsync = 120;
        second_config.performance.ui_animation_fps = 60;

        let mut first = ConfigDraft::from_config(&first_config);
        let second = ConfigDraft::from_config(&second_config);

        first.set_performance_buffer_count(4);
        first.set_performance_vsync(true);
        first.set_performance_max_fps_no_vsync("144".to_string());
        first.set_performance_ui_animation_fps("240".to_string());

        assert_eq!(second.performance_buffer_count, 3);
        assert!(second.performance_enable_vsync);
        assert_eq!(second.performance_max_fps_no_vsync, "120");
        assert_eq!(second.performance_ui_animation_fps, "60");

        let converted = first
            .to_config(&first_config, &crate::test_temp::path_resolver())
            .expect("valid first performance draft fixture converts");
        assert_eq!(converted.performance.buffer_count, 4);
        assert!(converted.performance.enable_vsync);
        assert_eq!(converted.performance.max_fps_no_vsync, 144);
        assert_eq!(converted.performance.ui_animation_fps, 240);
        assert_eq!(second_config.performance.buffer_count, 3);
        assert_eq!(second_config.performance.max_fps_no_vsync, 120);
    }

    #[test]
    fn conversion_rejects_values_outside_shared_performance_constraints() {
        let base = wayscriber::config::Config::default();
        let mut draft = ConfigDraft::from_config(&base);
        draft.performance_buffer_count = 5;
        draft.performance_ui_animation_fps = "241".to_string();

        let errors = draft
            .to_config(&base, &crate::test_temp::path_resolver())
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

    #[test]
    fn conversion_reports_exact_performance_paths_and_messages() {
        let base = wayscriber::config::Config::default();
        let mut draft = ConfigDraft::from_config(&base);
        draft.performance_buffer_count = 1;
        draft.performance_max_fps_no_vsync = "not-a-number".to_string();
        draft.performance_ui_animation_fps = "241".to_string();

        let errors = draft
            .to_config(&base, &crate::test_temp::path_resolver())
            .expect_err("invalid performance fixture remains actionable");
        let find = |path: &str| errors.iter().find(|error| error.field == path);

        assert_eq!(
            find("performance.buffer_count").map(|error| error.message.as_str()),
            Some("Expected one of 2, 3, 4")
        );
        assert!(
            find("performance.max_fps_no_vsync").is_some_and(|error| !error.message.is_empty())
        );
        assert_eq!(
            find("performance.ui_animation_fps").map(|error| error.message.as_str()),
            Some("Expected 0-240")
        );
    }
}
