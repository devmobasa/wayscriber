use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn inline_toolbars_render_active(&self) -> bool {
        self.toolbar_chrome.inline_toolbars()
            || self.toolbar_drag.preview_active()
            || self.toolbar_drag.gtk_preview_kind().is_some()
    }
}
