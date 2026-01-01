use super::super::KeybindingsConfig;
use super::BindingInserter;
use crate::config::Action;

impl KeybindingsConfig {
    pub(super) fn insert_capture_bindings(
        &self,
        inserter: &mut BindingInserter,
    ) -> Result<(), String> {
        inserter.insert_all(&self.capture_full_screen, Action::CaptureFullScreen)?;
        inserter.insert_all(&self.capture_active_window, Action::CaptureActiveWindow)?;
        inserter.insert_all(&self.capture_selection, Action::CaptureSelection)?;
        inserter.insert_all(&self.capture_clipboard_full, Action::CaptureClipboardFull)?;
        inserter.insert_all(&self.capture_file_full, Action::CaptureFileFull)?;
        inserter.insert_all(
            &self.capture_clipboard_selection,
            Action::CaptureClipboardSelection,
        )?;
        inserter.insert_all(&self.capture_file_selection, Action::CaptureFileSelection)?;
        inserter.insert_all(
            &self.capture_clipboard_region,
            Action::CaptureClipboardRegion,
        )?;
        inserter.insert_all(&self.capture_file_region, Action::CaptureFileRegion)?;
        inserter.insert_all(&self.open_capture_folder, Action::OpenCaptureFolder)?;
        Ok(())
    }
}
