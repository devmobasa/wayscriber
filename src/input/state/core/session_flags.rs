//! Session persistence flags, preflight options, and path state.

use crate::session::SessionOptions;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(in crate::input::state) struct SessionFlags {
    dirty: bool,
    preflight_options: Option<SessionOptions>,
    pending_save_as_overwrite: Option<PathBuf>,
    last_capture_path: Option<PathBuf>,
}

impl SessionFlags {
    pub(in crate::input::state) fn new() -> Self {
        Self {
            dirty: false,
            preflight_options: None,
            pending_save_as_overwrite: None,
            last_capture_path: None,
        }
    }

    pub(in crate::input::state) const fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub(in crate::input::state) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub(in crate::input::state) fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub(in crate::input::state) fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    pub(in crate::input::state) fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub(in crate::input::state) const fn preflight_options(&self) -> Option<&SessionOptions> {
        self.preflight_options.as_ref()
    }

    pub(in crate::input::state) fn replace_preflight_options(
        &mut self,
        options: Option<SessionOptions>,
    ) {
        self.preflight_options = options;
    }

    pub(in crate::input::state) fn pending_save_as_overwrite(&self) -> Option<&Path> {
        self.pending_save_as_overwrite.as_deref()
    }

    pub(in crate::input::state) fn set_pending_save_as_overwrite(&mut self, path: PathBuf) {
        self.pending_save_as_overwrite = Some(path);
    }

    pub(in crate::input::state) fn take_pending_save_as_overwrite(&mut self) -> Option<PathBuf> {
        self.pending_save_as_overwrite.take()
    }

    pub(in crate::input::state) fn last_capture_path(&self) -> Option<&Path> {
        self.last_capture_path.as_deref()
    }

    pub(in crate::input::state) fn set_last_capture_path(&mut self, path: Option<PathBuf>) {
        self.last_capture_path = path;
    }
}

#[cfg(test)]
mod tests {
    use super::SessionFlags;
    use crate::session::SessionOptions;
    use std::path::PathBuf;

    fn options(name: &str) -> SessionOptions {
        SessionOptions::new(PathBuf::from("/tmp"), name)
    }

    #[test]
    fn last_capture_path_can_be_replaced_and_cleared() {
        let mut flags = SessionFlags::new();
        let path = PathBuf::from("capture.png");

        flags.set_last_capture_path(Some(path.clone()));
        assert_eq!(flags.last_capture_path(), Some(path.as_path()));

        flags.set_last_capture_path(None);
        assert!(flags.last_capture_path().is_none());
    }

    #[test]
    fn dirty_flag_can_be_marked_and_cleared() {
        let mut flags = SessionFlags::new();

        flags.mark_dirty();
        assert!(flags.is_dirty());

        flags.clear_dirty();
        assert!(!flags.is_dirty());
    }

    #[test]
    fn pending_save_as_overwrite_round_trips_independently() {
        let mut flags = SessionFlags::new();
        let path = PathBuf::from("session.wayscriber");

        flags.set_pending_save_as_overwrite(path.clone());
        flags.mark_dirty();

        assert_eq!(flags.pending_save_as_overwrite(), Some(path.as_path()));
        assert_eq!(flags.take_pending_save_as_overwrite(), Some(path));
        assert!(flags.is_dirty());
        assert!(flags.pending_save_as_overwrite().is_none());
    }

    #[test]
    fn preflight_options_can_be_replaced_without_changing_other_flags() {
        let mut flags = SessionFlags::new();
        flags.mark_dirty();
        let mut replacement = options("replacement");
        replacement.persist_history = false;

        flags.replace_preflight_options(Some(replacement.clone()));

        let stored = flags.preflight_options().expect("stored options");
        assert_eq!(stored.display_id, replacement.display_id);
        assert!(!stored.persist_history);
        assert!(flags.is_dirty());

        flags.replace_preflight_options(None);
        assert!(flags.preflight_options().is_none());
    }
}
