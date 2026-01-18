use super::super::KeybindingsConfig;
use super::BindingInserter;
use crate::config::Action;

impl KeybindingsConfig {
    pub(super) fn insert_ui_bindings(&self, inserter: &mut BindingInserter) -> Result<(), String> {
        inserter.insert_all(&self.ui.toggle_help, Action::ToggleHelp)?;
        inserter.insert_all(&self.ui.toggle_quick_help, Action::ToggleQuickHelp)?;
        inserter.insert_all(&self.ui.toggle_status_bar, Action::ToggleStatusBar)?;
        inserter.insert_all(
            &self.ui.toggle_click_highlight,
            Action::ToggleClickHighlight,
        )?;
        inserter.insert_all(&self.ui.toggle_toolbar, Action::ToggleToolbar)?;
        inserter.insert_all(&self.ui.toggle_presenter_mode, Action::TogglePresenterMode)?;
        inserter.insert_all(&self.ui.toggle_fill, Action::ToggleFill)?;
        inserter.insert_all(
            &self.ui.toggle_selection_properties,
            Action::ToggleSelectionProperties,
        )?;
        inserter.insert_all(&self.ui.open_context_menu, Action::OpenContextMenu)?;
        inserter.insert_all(&self.ui.open_configurator, Action::OpenConfigurator)?;
        inserter.insert_all(
            &self.ui.toggle_command_palette,
            Action::ToggleCommandPalette,
        )?;
        Ok(())
    }
}
