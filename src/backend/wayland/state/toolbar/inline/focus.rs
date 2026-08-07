use super::*;
use crate::backend::wayland::toolbar::hit::{
    focus_hover_point, focused_event, next_focus_index, resolve_focus_index,
};
use crate::input::Key;

impl WaylandState {
    fn inline_focus_index(&self) -> Option<usize> {
        self.data.inline_top_focus_index
    }

    fn inline_focus_id(&self) -> Option<&str> {
        self.data.inline_top_focus_id.as_deref()
    }

    pub(in crate::backend::wayland) fn inline_toolbar_focus_hover(&self) -> Option<(f64, f64)> {
        let hits = &self.data.inline_top_hits;
        focus_hover_point(
            hits,
            resolve_focus_index(hits, self.inline_focus_index(), self.inline_focus_id()),
        )
    }

    pub(in crate::backend::wayland) fn inline_toolbar_focus_next(&mut self, reverse: bool) -> bool {
        let hits = &self.data.inline_top_hits;
        let current = resolve_focus_index(hits, self.inline_focus_index(), self.inline_focus_id());
        let next = next_focus_index(hits, current, reverse);
        if next != current {
            let id = next.and_then(|index| hits[index].focus_id.clone());
            self.data.inline_top_focus_index = next;
            self.data.inline_top_focus_id = id;
            self.mark_inline_toolbar_full_damage();
            return true;
        }
        false
    }

    pub(in crate::backend::wayland) fn inline_toolbar_focused_event(&self) -> Option<ToolbarEvent> {
        let hits = &self.data.inline_top_hits;
        focused_event(
            hits,
            resolve_focus_index(hits, self.inline_focus_index(), self.inline_focus_id()),
        )
    }

    pub(in crate::backend::wayland) fn toolbar_focus_active(&self) -> bool {
        self.data.toolbar_focus_active
    }

    pub(in crate::backend::wayland) fn set_toolbar_focus_active(&mut self, active: bool) {
        self.data.toolbar_focus_active = active;
    }

    pub(in crate::backend::wayland) fn clear_toolbar_focus(&mut self) {
        self.data.toolbar_focus_active = false;
        self.toolbar.clear_focus();
        let had_inline_focus =
            self.data.inline_top_focus_index.is_some() || self.data.inline_top_focus_id.is_some();
        self.clear_inline_toolbar_focus();
        if self.inline_toolbars_active() && had_inline_focus {
            self.mark_inline_toolbar_full_damage();
        }
    }

    /// Whether the pointer currently hovers the toolbar in the active
    /// placement, which is what seeds keyboard focus on the first Tab.
    pub(in crate::backend::wayland) fn toolbar_hovered(&self) -> bool {
        if self.inline_toolbars_active() {
            self.data.inline_top_hover.is_some()
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
        if matches!(key, Key::Escape)
            && (self.input_state.toolbar_shapes_expanded
                || self.input_state.toolbar_top_overflow_open)
        {
            self.input_state
                .apply_toolbar_event(ToolbarEvent::ToggleShapePicker(false));
            self.input_state
                .apply_toolbar_event(ToolbarEvent::ToggleTopOverflow(false));
            if self.inline_toolbars_active() {
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
            self.input_state.command_palette_open,
        ) {
            return false;
        }
        let is_tab = matches!(key, Key::Tab);
        let is_activate = matches!(key, Key::Return | Key::Space);

        if !self.toolbar_focus_active() {
            if !self.toolbar_hovered() {
                return false;
            }
            self.data.toolbar_focus_active = true;
        }

        if !self.toolbar.is_top_visible() {
            self.clear_toolbar_focus();
            return false;
        }

        if is_tab {
            let reverse = self.input_state.modifiers.shift;
            if self.inline_toolbars_active() {
                self.inline_toolbar_focus_next(reverse);
            } else {
                self.toolbar.focus_next(reverse);
            }
            return true;
        }

        if is_activate {
            let event = if self.inline_toolbars_active() {
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
