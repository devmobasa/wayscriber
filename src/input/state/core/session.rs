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
}
