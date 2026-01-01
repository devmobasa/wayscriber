use super::super::KeybindingsConfig;
use super::BindingInserter;
use crate::config::Action;

impl KeybindingsConfig {
    pub(super) fn insert_ui_bindings(&self, inserter: &mut BindingInserter) -> Result<(), String> {
        inserter.insert_all(&self.toggle_help, Action::ToggleHelp)?;
        inserter.insert_all(&self.toggle_status_bar, Action::ToggleStatusBar)?;
        inserter.insert_all(&self.toggle_click_highlight, Action::ToggleClickHighlight)?;
        inserter.insert_all(&self.toggle_toolbar, Action::ToggleToolbar)?;
        inserter.insert_all(&self.toggle_fill, Action::ToggleFill)?;
        inserter.insert_all(
            &self.toggle_selection_properties,
            Action::ToggleSelectionProperties,
        )?;
        inserter.insert_all(&self.open_context_menu, Action::OpenContextMenu)?;
        inserter.insert_all(&self.open_configurator, Action::OpenConfigurator)?;
        Ok(())
    }
}
