use super::super::KeybindingsConfig;
use super::BindingInserter;
use crate::config::Action;

impl KeybindingsConfig {
    pub(super) fn insert_zoom_bindings(
        &self,
        inserter: &mut BindingInserter,
    ) -> Result<(), String> {
        inserter.insert_all(&self.zoom.toggle_frozen_mode, Action::ToggleFrozenMode)?;
        inserter.insert_all(&self.zoom.zoom_in, Action::ZoomIn)?;
        inserter.insert_all(&self.zoom.zoom_out, Action::ZoomOut)?;
        inserter.insert_all(&self.zoom.reset_zoom, Action::ResetZoom)?;
        inserter.insert_all(&self.zoom.toggle_zoom_lock, Action::ToggleZoomLock)?;
        inserter.insert_all(&self.zoom.refresh_zoom_capture, Action::RefreshZoomCapture)?;
        Ok(())
    }
}
