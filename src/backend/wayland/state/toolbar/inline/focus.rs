use super::*;
use crate::input::Key;

impl WaylandState {
    pub(in crate::backend::wayland) fn inline_toolbar_focus_hover(&self) -> Option<(f64, f64)> {
        self.toolbar_chrome.inline_focus_hover()
    }

    pub(in crate::backend::wayland) fn inline_toolbar_focus_next(&mut self, reverse: bool) -> bool {
        if !self.toolbar_chrome.inline_focus_next(reverse) {
            return false;
        }
        self.mark_inline_toolbar_full_damage();
        true
    }

    pub(in crate::backend::wayland) fn inline_toolbar_focused_event(&self) -> Option<ToolbarEvent> {
        self.toolbar_chrome.inline_focused_event()
    }

    pub(in crate::backend::wayland) fn clear_toolbar_focus(&mut self) {
        self.toolbar_chrome.set_focus_active(false);
        self.toolbar.clear_focus();
        let had_inline_focus = self.toolbar_chrome.clear_inline_focus();
        if self.toolbar_chrome.inline_toolbars() && had_inline_focus {
            self.mark_inline_toolbar_full_damage();
        }
    }

    /// Whether the pointer currently hovers the toolbar in the active
    /// placement, which is what seeds keyboard focus on the first Tab.
    pub(in crate::backend::wayland) fn toolbar_hovered(&self) -> bool {
        if self.toolbar_chrome.inline_toolbars() {
            self.toolbar_chrome.inline_hover().is_some()
        } else {
            self.toolbar.is_hovered()
        }
    }

    pub(in crate::backend::wayland) fn handle_toolbar_key(
        &mut self,
        key: Key,
        conn: Option<&wayland_client::Connection>,
        qh: Option<&wayland_client::QueueHandle<Self>>,
    ) -> bool {
        if matches!(key, Key::Escape) && self.input_state.toolbar_top_menu().is_flyout() {
            self.input_state.close_top_toolbar_menus();
            if self.toolbar_chrome.inline_toolbars() {
                self.mark_inline_toolbar_full_damage();
            } else {
                self.toolbar.mark_dirty();
            }
            return true;
        }
        if !should_route_toolbar_key(
            key,
            self.toolbar.is_visible(),
            matches!(self.input_state.state, DrawingState::TextInput { .. }),
            self.input_state.command_palette.open,
        ) {
            return false;
        }
        let is_tab = matches!(key, Key::Tab);
        let is_activate = matches!(key, Key::Return | Key::Space);

        if !self.toolbar_chrome.focus_active() {
            if !self.toolbar_hovered() {
                return false;
            }
            self.toolbar_chrome.set_focus_active(true);
        }

        if !self.toolbar.is_top_visible() {
            self.clear_toolbar_focus();
            return false;
        }

        if is_tab {
            let reverse = self.input_state.modifiers.shift;
            if self.toolbar_chrome.inline_toolbars() {
                self.inline_toolbar_focus_next(reverse);
            } else {
                self.toolbar.focus_next(reverse);
            }
            return true;
        }

        if is_activate {
            let event = if self.toolbar_chrome.inline_toolbars() {
                self.inline_toolbar_focused_event()
            } else {
                self.toolbar.focused_event()
            };
            if let Some(event) = event {
                self.handle_toolbar_event(event, conn, qh);
            }
            return true;
        }

        false
    }
}

fn should_route_toolbar_key(
    key: Key,
    toolbar_visible: bool,
    in_text_input: bool,
    command_palette_open: bool,
) -> bool {
    if !toolbar_visible || in_text_input || command_palette_open {
        return false;
    }
    matches!(key, Key::Tab | Key::Return | Key::Space)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_toolbar_key_rejects_when_command_palette_open() {
        assert!(!should_route_toolbar_key(Key::Return, true, false, true));
        assert!(!should_route_toolbar_key(Key::Space, true, false, true));
        assert!(!should_route_toolbar_key(Key::Tab, true, false, true));
    }

    #[test]
    fn route_toolbar_key_rejects_when_toolbar_hidden_or_text_input_active() {
        assert!(!should_route_toolbar_key(Key::Return, false, false, false));
        assert!(!should_route_toolbar_key(Key::Space, false, false, false));
        assert!(!should_route_toolbar_key(Key::Tab, false, false, false));

        assert!(!should_route_toolbar_key(Key::Return, true, true, false));
        assert!(!should_route_toolbar_key(Key::Space, true, true, false));
        assert!(!should_route_toolbar_key(Key::Tab, true, true, false));
    }

    #[test]
    fn route_toolbar_key_allows_tab_and_activate_when_not_blocked() {
        assert!(should_route_toolbar_key(Key::Return, true, false, false));
        assert!(should_route_toolbar_key(Key::Space, true, false, false));
        assert!(should_route_toolbar_key(Key::Tab, true, false, false));
        assert!(!should_route_toolbar_key(Key::Down, true, false, false));
    }
}
