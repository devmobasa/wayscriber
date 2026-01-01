use super::super::KeybindingsConfig;
use super::BindingInserter;
use crate::config::Action;

impl KeybindingsConfig {
    pub(super) fn insert_preset_bindings(
        &self,
        inserter: &mut BindingInserter,
    ) -> Result<(), String> {
        inserter.insert_all(&self.apply_preset_1, Action::ApplyPreset1)?;
        inserter.insert_all(&self.apply_preset_2, Action::ApplyPreset2)?;
        inserter.insert_all(&self.apply_preset_3, Action::ApplyPreset3)?;
        inserter.insert_all(&self.apply_preset_4, Action::ApplyPreset4)?;
        inserter.insert_all(&self.apply_preset_5, Action::ApplyPreset5)?;
        inserter.insert_all(&self.save_preset_1, Action::SavePreset1)?;
        inserter.insert_all(&self.save_preset_2, Action::SavePreset2)?;
        inserter.insert_all(&self.save_preset_3, Action::SavePreset3)?;
        inserter.insert_all(&self.save_preset_4, Action::SavePreset4)?;
        inserter.insert_all(&self.save_preset_5, Action::SavePreset5)?;
        inserter.insert_all(&self.clear_preset_1, Action::ClearPreset1)?;
        inserter.insert_all(&self.clear_preset_2, Action::ClearPreset2)?;
        inserter.insert_all(&self.clear_preset_3, Action::ClearPreset3)?;
        inserter.insert_all(&self.clear_preset_4, Action::ClearPreset4)?;
        inserter.insert_all(&self.clear_preset_5, Action::ClearPreset5)?;
        Ok(())
    }
}
