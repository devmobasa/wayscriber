use super::super::fields::{OverrideOption, ToolbarLayoutModeOption, ToolbarOverrideField};
use wayscriber::config::{ToolbarModeOverride, ToolbarModeOverrides};

#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarModeOverrideDraft {
    pub show_presets: OverrideOption,
    pub show_actions_section: OverrideOption,
    pub show_actions_advanced: OverrideOption,
    pub show_zoom_actions: OverrideOption,
    pub show_pages_section: OverrideOption,
    pub show_step_section: OverrideOption,
    pub show_text_controls: OverrideOption,
    pub show_settings_section: OverrideOption,
}

impl ToolbarModeOverrideDraft {
    pub(super) fn from_override(override_cfg: &ToolbarModeOverride) -> Self {
        Self {
            show_presets: OverrideOption::from_option(override_cfg.show_presets),
            show_actions_section: OverrideOption::from_option(override_cfg.show_actions_section),
            show_actions_advanced: OverrideOption::from_option(override_cfg.show_actions_advanced),
            show_zoom_actions: OverrideOption::from_option(override_cfg.show_zoom_actions),
            show_pages_section: OverrideOption::from_option(override_cfg.show_pages_section),
            show_step_section: OverrideOption::from_option(override_cfg.show_step_section),
            show_text_controls: OverrideOption::from_option(override_cfg.show_text_controls),
            show_settings_section: OverrideOption::from_option(override_cfg.show_settings_section),
        }
    }

    pub(super) fn to_override(&self) -> ToolbarModeOverride {
        ToolbarModeOverride {
            show_actions_section: self.show_actions_section.to_option(),
            show_actions_advanced: self.show_actions_advanced.to_option(),
            show_zoom_actions: self.show_zoom_actions.to_option(),
            show_pages_section: self.show_pages_section.to_option(),
            show_presets: self.show_presets.to_option(),
            show_step_section: self.show_step_section.to_option(),
            show_text_controls: self.show_text_controls.to_option(),
            show_settings_section: self.show_settings_section.to_option(),
        }
    }

    pub(super) fn set(&mut self, field: ToolbarOverrideField, value: OverrideOption) {
        match field {
            ToolbarOverrideField::ShowPresets => self.show_presets = value,
            ToolbarOverrideField::ShowActionsSection => self.show_actions_section = value,
            ToolbarOverrideField::ShowActionsAdvanced => self.show_actions_advanced = value,
            ToolbarOverrideField::ShowZoomActions => self.show_zoom_actions = value,
            ToolbarOverrideField::ShowPagesSection => self.show_pages_section = value,
            ToolbarOverrideField::ShowStepSection => self.show_step_section = value,
            ToolbarOverrideField::ShowTextControls => self.show_text_controls = value,
            ToolbarOverrideField::ShowSettingsSection => self.show_settings_section = value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarModeOverridesDraft {
    pub simple: ToolbarModeOverrideDraft,
    pub regular: ToolbarModeOverrideDraft,
    pub advanced: ToolbarModeOverrideDraft,
}

impl ToolbarModeOverridesDraft {
    pub(super) fn from_config(config: &ToolbarModeOverrides) -> Self {
        Self {
            simple: ToolbarModeOverrideDraft::from_override(&config.simple),
            regular: ToolbarModeOverrideDraft::from_override(&config.regular),
            advanced: ToolbarModeOverrideDraft::from_override(&config.advanced),
        }
    }

    pub(super) fn to_config(&self) -> ToolbarModeOverrides {
        ToolbarModeOverrides {
            simple: self.simple.to_override(),
            regular: self.regular.to_override(),
            advanced: self.advanced.to_override(),
        }
    }

    pub fn for_mode(&self, mode: ToolbarLayoutModeOption) -> &ToolbarModeOverrideDraft {
        match mode {
            ToolbarLayoutModeOption::Simple => &self.simple,
            ToolbarLayoutModeOption::Regular => &self.regular,
            ToolbarLayoutModeOption::Advanced => &self.advanced,
        }
    }

    pub(super) fn for_mode_mut(
        &mut self,
        mode: ToolbarLayoutModeOption,
    ) -> &mut ToolbarModeOverrideDraft {
        match mode {
            ToolbarLayoutModeOption::Simple => &mut self.simple,
            ToolbarLayoutModeOption::Regular => &mut self.regular,
            ToolbarLayoutModeOption::Advanced => &mut self.advanced,
        }
    }
}
