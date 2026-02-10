use super::*;
use crate::backend::wayland::toolbar::ToolbarFocusTarget;
use crate::backend::wayland::toolbar::hit::{focus_hover_point, focused_event, next_focus_index};
use crate::input::Key;

impl WaylandState {
    pub(in crate::backend::wayland) fn inline_toolbar_focus_hover(
        &self,
        target: ToolbarFocusTarget,
    ) -> Option<(f64, f64)> {
        let hits = match target {
            ToolbarFocusTarget::Top => &self.data.inline_top_hits,
            ToolbarFocusTarget::Side => &self.data.inline_side_hits,
        };
        focus_hover_point(hits, self.inline_focus_index(target))
    }

    pub(in crate::backend::wayland) fn inline_toolbar_focus_next(
        &mut self,
        target: ToolbarFocusTarget,
        reverse: bool,
    ) -> bool {
        let hits = match target {
            ToolbarFocusTarget::Top => &self.data.inline_top_hits,
            ToolbarFocusTarget::Side => &self.data.inline_side_hits,
        };
        let current = self.inline_focus_index(target);
        let next = next_focus_index(hits, current, reverse);
        if next != current {
            *self.inline_focus_index_mut(target) = next;
            self.input_state.needs_redraw = true;
            return true;
        }
        false
    }

    pub(in crate::backend::wayland) fn inline_toolbar_focused_event(
        &self,
        target: ToolbarFocusTarget,
    ) -> Option<ToolbarEvent> {
        let hits = match target {
            ToolbarFocusTarget::Top => &self.data.inline_top_hits,
            ToolbarFocusTarget::Side => &self.data.inline_side_hits,
        };
        focused_event(hits, self.inline_focus_index(target))
    }

    pub(in crate::backend::wayland) fn inline_toolbar_focus_target_from_hover(
        &self,
    ) -> Option<ToolbarFocusTarget> {
        if self.data.inline_top_hover.is_some() {
            Some(ToolbarFocusTarget::Top)
        } else if self.data.inline_side_hover.is_some() {
            Some(ToolbarFocusTarget::Side)
        } else {
            None
        }
    }

    pub(in crate::backend::wayland) fn toolbar_focus_target(&self) -> Option<ToolbarFocusTarget> {
        self.data.toolbar_focus_target
    }

    pub(in crate::backend::wayland) fn set_toolbar_focus_target(
        &mut self,
        target: Option<ToolbarFocusTarget>,
    ) {
        self.data.toolbar_focus_target = target;
    }

    pub(in crate::backend::wayland) fn clear_toolbar_focus(&mut self) {
        self.data.toolbar_focus_target = None;
        self.toolbar.clear_focus();
        let had_inline_focus = self.data.inline_top_focus_index.is_some()
            || self.data.inline_side_focus_index.is_some();
        self.clear_inline_toolbar_focus();
        if self.inline_toolbars_active() && had_inline_focus {
            self.input_state.needs_redraw = true;
        }
    }

    pub(in crate::backend::wayland) fn toolbar_focus_target_from_hover(
        &self,
    ) -> Option<ToolbarFocusTarget> {
        if self.inline_toolbars_active() {
            self.inline_toolbar_focus_target_from_hover()
        } else {
            self.toolbar.hovered_target()
        }
    }

    pub(in crate::backend::wayland) fn handle_toolbar_key(&mut self, key: Key) -> bool {
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

        let mut target = self.toolbar_focus_target();
        if target.is_none() {
            target = self.toolbar_focus_target_from_hover();
            if target.is_none() {
                return false;
            }
            self.data.toolbar_focus_target = target;
        }

        let target = match target {
            Some(target) => target,
            None => return false,
        };
        if matches!(target, ToolbarFocusTarget::Top) && !self.toolbar.is_top_visible() {
            self.clear_toolbar_focus();
            return false;
        }
        if matches!(target, ToolbarFocusTarget::Side) && !self.toolbar.is_side_visible() {
            self.clear_toolbar_focus();
            return false;
        }

        if is_tab {
            let reverse = self.input_state.modifiers.shift;
            if self.inline_toolbars_active() {
                self.inline_toolbar_focus_next(target, reverse);
            } else {
                self.toolbar.focus_next(target, reverse);
            }
            return true;
        }

        if is_activate {
            let event = if self.inline_toolbars_active() {
                self.inline_toolbar_focused_event(target)
            } else {
                self.toolbar.focused_event(target)
            };
            if let Some(event) = event {
                self.handle_toolbar_event(event);
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
