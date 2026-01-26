use std::time::Instant;

use super::*;

impl WaylandState {
    fn inline_toolbar_hit_at(
        &self,
        position: (f64, f64),
    ) -> Option<(crate::backend::wayland::toolbar_intent::ToolbarIntent, bool)> {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return None;
        }
        if self.toolbar.is_top_visible()
            && let Some(intent) = self
                .data
                .inline_top_hits
                .iter()
                .find_map(|hit| intent_for_hit(hit, position.0, position.1))
        {
            return Some(intent);
        }
        if self.toolbar.is_side_visible() {
            return self
                .data
                .inline_side_hits
                .iter()
                .find_map(|hit| intent_for_hit(hit, position.0, position.1));
        }
        None
    }

    fn inline_toolbar_drag_at(
        &self,
        position: (f64, f64),
    ) -> Option<crate::backend::wayland::toolbar_intent::ToolbarIntent> {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return None;
        }
        // If we have an active move drag, generate intent directly from it
        // This allows dragging to continue even when mouse is outside the hit region
        if let Some(intent) = self.move_drag_intent(position.0, position.1) {
            return Some(intent);
        }
        if self.toolbar.is_top_visible()
            && let Some(intent) = self
                .data
                .inline_top_hits
                .iter()
                .find_map(|hit| drag_intent_for_hit(hit, position.0, position.1))
        {
            return Some(intent);
        }
        if self.toolbar.is_side_visible() {
            return self
                .data
                .inline_side_hits
                .iter()
                .find_map(|hit| drag_intent_for_hit(hit, position.0, position.1));
        }
        None
    }

    pub(in crate::backend::wayland) fn inline_toolbar_motion(
        &mut self,
        position: (f64, f64),
    ) -> bool {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return false;
        }

        self.set_current_mouse(position.0 as i32, position.1 as i32);
        let (mx, my) = self.current_mouse();
        self.input_state.update_pointer_position(mx, my);

        let was_top_hover = self.data.inline_top_hover;
        let was_side_hover = self.data.inline_side_hover;

        self.data.inline_top_hover = None;
        self.data.inline_side_hover = None;

        let top_visible = self.toolbar.is_top_visible();
        let side_visible = self.toolbar.is_side_visible();
        let mut over_toolbar = false;

        if top_visible
            && let Some((x, y, w, h)) = self.data.inline_top_rect
            && geometry::point_in_rect(position.0, position.1, x, y, w, h)
        {
            over_toolbar = true;
            if was_top_hover.is_none() {
                self.data.inline_top_hover_start = Some(Instant::now());
            }
            self.data.inline_top_hover = Some(position);
        } else {
            self.data.inline_top_hover_start = None;
        }

        if side_visible
            && let Some((x, y, w, h)) = self.data.inline_side_rect
            && geometry::point_in_rect(position.0, position.1, x, y, w, h)
        {
            over_toolbar = true;
            if was_side_hover.is_none() {
                self.data.inline_side_hover_start = Some(Instant::now());
            }
            self.data.inline_side_hover = Some(position);
        } else {
            self.data.inline_side_hover_start = None;
        }

        if self.toolbar_dragging()
            && let Some(intent) = self.inline_toolbar_drag_at(position)
        {
            let evt = intent_to_event(intent, self.toolbar.last_snapshot());
            self.handle_toolbar_event(evt);
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            over_toolbar = true;
        } else if self.toolbar_dragging() {
            if let Some(kind) = self.active_move_drag_kind() {
                self.handle_toolbar_move(kind, position);
            }
            over_toolbar = true;
        }

        if was_top_hover != self.data.inline_top_hover
            || was_side_hover != self.data.inline_side_hover
        {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
        }

        if over_toolbar {
            self.set_pointer_over_toolbar(true);
        } else if !self.toolbar_dragging() {
            self.set_pointer_over_toolbar(false);
            if self.data.toolbar_focus_target.is_some() {
                self.data.toolbar_focus_target = None;
                self.clear_inline_toolbar_focus();
                self.toolbar.mark_dirty();
                self.input_state.needs_redraw = true;
            }
        }

        over_toolbar
    }

    pub(in crate::backend::wayland) fn inline_toolbar_press(
        &mut self,
        position: (f64, f64),
    ) -> bool {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return false;
        }
        if let Some((intent, drag)) = self.inline_toolbar_hit_at(position) {
            self.set_toolbar_dragging(drag);
            let evt = intent_to_event(intent, self.toolbar.last_snapshot());
            self.handle_toolbar_event(evt);
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            self.set_pointer_over_toolbar(true);
            return true;
        }
        false
    }

    pub(in crate::backend::wayland) fn inline_toolbar_leave(&mut self) {
        if !self.inline_toolbars_active() {
            return;
        }
        let had_hover =
            self.data.inline_top_hover.is_some() || self.data.inline_side_hover.is_some();
        let had_focus = self.data.inline_top_focus_index.is_some()
            || self.data.inline_side_focus_index.is_some();
        self.data.inline_top_hover = None;
        self.data.inline_side_hover = None;
        self.data.inline_top_hover_start = None;
        self.data.inline_side_hover_start = None;
        self.data.toolbar_focus_target = None;
        self.clear_inline_toolbar_focus();
        self.set_pointer_over_toolbar(false);
        // Don't clear drag state if we're in a move drag - the drag continues outside
        if !self.is_move_dragging() {
            self.set_toolbar_dragging(false);
            self.end_toolbar_move_drag();
        }
        if had_hover || had_focus {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
        }
    }

    pub(in crate::backend::wayland) fn inline_toolbar_release(
        &mut self,
        position: (f64, f64),
    ) -> bool {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return false;
        }
        if self.pointer_over_toolbar() || self.toolbar_dragging() {
            if self.toolbar_dragging()
                && let Some(intent) = self.inline_toolbar_drag_at(position)
            {
                let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                self.handle_toolbar_event(evt);
                self.toolbar.mark_dirty();
                self.input_state.needs_redraw = true;
            }
            self.set_toolbar_dragging(false);
            self.set_pointer_over_toolbar(false);
            self.end_toolbar_move_drag();
            return true;
        }
        false
    }
}
