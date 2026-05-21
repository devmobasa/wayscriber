use super::base::InputState;

impl InputState {
    /// Marks session data as dirty for autosave tracking.
    pub(crate) fn mark_session_dirty(&mut self) {
        self.session_dirty = true;
    }

    /// Returns true if session data was marked dirty since the last check.
    #[allow(dead_code)]
    pub(crate) fn take_session_dirty(&mut self) -> bool {
        if self.session_dirty {
            self.session_dirty = false;
            true
        } else {
            false
        }
    }

    /// Clears session dirtiness after loading persisted state into memory.
    #[allow(dead_code)]
    pub(crate) fn clear_session_dirty(&mut self) {
        self.session_dirty = false;
    }

    /// Returns whether session data is dirty without clearing the dirty flag.
    #[allow(dead_code)]
    pub(crate) fn is_session_dirty(&self) -> bool {
        self.session_dirty
    }
}
