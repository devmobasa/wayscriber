use std::time::Instant;

use super::*;

impl WaylandState {
    fn inline_toolbar_hit_at(
        &self,
        position: (f64, f64),
    ) -> Option<(crate::backend::wayland::toolbar_intent::ToolbarIntent, bool)> {
        if !self.toolbar_chrome.inline_toolbars() || !self.toolbar.is_visible() {
            return None;
        }
        if !self.toolbar.is_top_visible() || !self.toolbar_chrome.inline_contains(position) {
            return None;
        }
        self.toolbar_chrome.inline_primary_hit_at(position)
    }

    /// The quick-color slot an inline-toolbar secondary press targets, read
    /// from the same hit regions as the primary path.
    fn inline_quick_color_slot_at(&self, position: (f64, f64)) -> Option<usize> {
        if !self.toolbar_chrome.inline_toolbars() || !self.toolbar.is_visible() {
            return None;
        }
        if !self.toolbar.is_top_visible() || !self.toolbar_chrome.inline_contains(position) {
            return None;
        }
        self.toolbar_chrome.inline_quick_color_slot_at(position)
    }

    /// Secondary press on an inline-toolbar swatch: opens the picker bound to
    /// that palette slot, mirroring the layer-shell surfaces' gesture.
    pub(in crate::backend::wayland) fn inline_toolbar_secondary_press(
        &mut self,
        position: (f64, f64),
        conn: Option<&wayland_client::Connection>,
        qh: Option<&wayland_client::QueueHandle<Self>>,
    ) -> bool {
        let Some(index) = self.inline_quick_color_slot_at(position) else {
            return false;
        };
        self.handle_toolbar_event(ToolbarEvent::EditQuickColor { index }, conn, qh);
        self.toolbar_chrome.set_pointer_over_toolbar(true);
        true
    }

    fn inline_toolbar_drag_at(
        &self,
        position: (f64, f64),
    ) -> Option<crate::backend::wayland::toolbar_intent::ToolbarIntent> {
        if !self.toolbar_chrome.inline_toolbars() || !self.toolbar.is_visible() {
            return None;
        }
        // If we have an active move drag, generate intent directly from it
        // This allows dragging to continue even when mouse is outside the hit region
        if let Some(intent) = self.move_drag_intent(position.0, position.1) {
            return Some(intent);
        }
        if !self.toolbar.is_top_visible() || !self.toolbar_chrome.inline_contains(position) {
            return None;
        }
        self.toolbar_chrome.inline_drag_hit_at(position)
    }

    pub(in crate::backend::wayland) fn inline_toolbar_motion(
        &mut self,
        position: (f64, f64),
    ) -> bool {
        if !self.toolbar_chrome.inline_toolbars() || !self.toolbar.is_visible() {
            return false;
        }

        self.pointer
            .set_position((position.0 as i32, position.1 as i32));
        let (mx, my) = self.pointer.position();
        self.input_state.update_pointer_position(mx, my);

        let top_hover = (self.toolbar.is_top_visible()
            && self.toolbar_chrome.inline_contains(position))
        .then_some(position);
        let hover_change = self
            .toolbar_chrome
            .set_inline_hover(top_hover, Instant::now());
        let mut over_toolbar = top_hover.is_some();

        if self.toolbar_dragging()
            && let Some(intent) = self.inline_toolbar_drag_at(position)
        {
            let evt = intent_to_event(intent, self.toolbar.last_snapshot());
            self.handle_toolbar_event(evt, None, None);
            over_toolbar = true;
        } else if self.toolbar_dragging() {
            if let Some(kind) = self.active_move_drag_kind() {
                self.handle_toolbar_move(kind, position);
            }
            over_toolbar = true;
        }

        if hover_change.target_changed {
            // The inline toolbar and annotations share the main surface's SHM
            // swapchain. Refresh every slot when hover visuals change so a
            // compositor cannot resurface a buffer containing older toolbar or
            // annotation pixels during rapid pointer motion.
            self.mark_inline_toolbar_full_damage();
        } else if hover_change.position_changed {
            // Same hit region, new pointer position - which still changes what
            // is painted: hit regions are inflated to MIN_HIT_TARGET, so
            // moving from that inflated margin onto the control itself keeps
            // the same target while flipping the hover highlight, which paints
            // against the visual rect. Keep the repaint scoped to the strip:
            // setting needs_redraw with no damage rect would repaint the whole
            // surface through the empty-damage fallback on every motion event.
            self.mark_inline_toolbar_rect_damage();
        }

        if over_toolbar {
            self.toolbar_chrome.set_pointer_over_toolbar(true);
        } else if !self.toolbar_dragging() {
            self.toolbar_chrome.set_pointer_over_toolbar(false);
            if self.toolbar_chrome.focus_active() {
                self.toolbar_chrome.set_focus_active(false);
                self.toolbar_chrome.clear_inline_focus();
                self.mark_inline_toolbar_full_damage();
            }
        }

        over_toolbar
    }

    pub(in crate::backend::wayland) fn inline_toolbar_press(
        &mut self,
        position: (f64, f64),
        conn: Option<&wayland_client::Connection>,
        qh: Option<&wayland_client::QueueHandle<Self>>,
    ) -> bool {
        if !self.toolbar_chrome.inline_toolbars() || !self.toolbar.is_visible() {
            return false;
        }
        if let Some((intent, drag)) = self.inline_toolbar_hit_at(position) {
            if drag {
                drag_log(|| {
                    format!(
                        "inline press: drag_start pos=({:.3}, {:.3})",
                        position.0, position.1
                    )
                });
            }
            self.set_toolbar_dragging(drag);
            let evt = intent_to_event(intent, self.toolbar.last_snapshot());
            self.handle_toolbar_event(evt, conn, qh);
            self.toolbar_chrome.set_pointer_over_toolbar(true);
            return true;
        }
        false
    }

    pub(in crate::backend::wayland) fn inline_toolbar_leave(&mut self) {
        if !self.toolbar_chrome.inline_toolbars() {
            return;
        }
        let had_hover = self.toolbar_chrome.clear_inline_hover();
        let had_focus = self.toolbar_chrome.clear_inline_focus();
        self.toolbar_chrome.set_focus_active(false);
        self.toolbar_chrome.set_pointer_over_toolbar(false);
        // Don't clear drag state if we're in a move drag - the drag continues outside
        if !self.is_move_dragging() {
            self.finish_toolbar_item_drag(false);
            self.set_toolbar_dragging(false);
            self.cancel_toolbar_move_drag();
        }
        if had_hover || had_focus {
            self.mark_inline_toolbar_full_damage();
        }
    }

    pub(in crate::backend::wayland) fn inline_toolbar_release(
        &mut self,
        position: (f64, f64),
    ) -> bool {
        if !self.toolbar_chrome.inline_toolbars() || !self.toolbar.is_visible() {
            return false;
        }
        if self.toolbar_chrome.pointer_over_toolbar() || self.toolbar_dragging() {
            if self.toolbar_dragging()
                && !self.pointer_lock_active()
                && let Some(intent) = self.inline_toolbar_drag_at(position)
            {
                let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                self.handle_toolbar_event(evt, None, None);
            }
            drag_log(|| {
                format!(
                    "inline release: pos=({:.3}, {:.3}), drag_active={}, pointer_over_toolbar={}",
                    position.0,
                    position.1,
                    self.toolbar_dragging(),
                    self.toolbar_chrome.pointer_over_toolbar()
                )
            });
            self.finish_toolbar_item_drag(true);
            self.set_toolbar_dragging(false);
            self.toolbar_chrome.set_pointer_over_toolbar(false);
            self.end_toolbar_move_drag();
            return true;
        }
        false
    }
}
