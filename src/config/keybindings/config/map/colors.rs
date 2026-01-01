use super::super::KeybindingsConfig;
use super::BindingInserter;
use crate::config::Action;

impl KeybindingsConfig {
    pub(super) fn insert_color_bindings(
        &self,
        inserter: &mut BindingInserter,
    ) -> Result<(), String> {
        inserter.insert_all(&self.set_color_red, Action::SetColorRed)?;
        inserter.insert_all(&self.set_color_green, Action::SetColorGreen)?;
        inserter.insert_all(&self.set_color_blue, Action::SetColorBlue)?;
        inserter.insert_all(&self.set_color_yellow, Action::SetColorYellow)?;
        inserter.insert_all(&self.set_color_orange, Action::SetColorOrange)?;
        inserter.insert_all(&self.set_color_pink, Action::SetColorPink)?;
        inserter.insert_all(&self.set_color_white, Action::SetColorWhite)?;
        inserter.insert_all(&self.set_color_black, Action::SetColorBlack)?;
        Ok(())
    }
}
