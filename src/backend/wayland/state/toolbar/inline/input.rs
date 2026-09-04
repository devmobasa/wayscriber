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
        if !self.toolbar.is_top_visible() || !point_in_surface(self.data.inline_top_rect, position)
        {
            return None;
        }
        self.data
            .inline_top_hits
            .iter()
            .find_map(|hit| intent_for_hit(hit, position.0, position.1))
    }

    /// The quick-color slot an inline-toolbar secondary press targets, read
    /// from the same hit regions as the primary path.
    fn inline_quick_color_slot_at(&self, position: (f64, f64)) -> Option<usize> {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return None;
        }
        if !self.toolbar.is_top_visible() || !point_in_surface(self.data.inline_top_rect, position)
        {
            return None;
        }
        self.data
            .inline_top_hits
            .iter()
            .find_map(|hit| quick_color_slot_for_hit(hit, position.0, position.1))
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
        self.set_pointer_over_toolbar(true);
        true
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
        if !self.toolbar.is_top_visible() || !point_in_surface(self.data.inline_top_rect, position)
        {
            return None;
        }
        self.data
            .inline_top_hits
            .iter()
            .find_map(|hit| drag_intent_for_hit(hit, position.0, position.1))
    }

    pub(in crate::backend::wayland) fn inline_toolbar_motion(
        &mut self,
        position: (f64, f64),
    ) -> bool {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return false;
        }

        self.pointer
            .set_position((position.0 as i32, position.1 as i32));
        let (mx, my) = self.pointer.position();
        self.input_state.update_pointer_position(mx, my);

        let was_top_hover = self.data.inline_top_hover;
        let was_top_hit = was_top_hover.and_then(|(x, y)| {
            self.data
                .inline_top_hits
                .iter()
                .position(|hit| hit.contains(x, y))
        });

        self.data.inline_top_hover = None;

        let top_visible = self.toolbar.is_top_visible();
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

        let top_hit = self.data.inline_top_hover.and_then(|(x, y)| {
            self.data
                .inline_top_hits
                .iter()
                .position(|hit| hit.contains(x, y))
        });
        let top_target_changed = inline_hover_target_changed(
            was_top_hover,
            was_top_hit,
            self.data.inline_top_hover,
            top_hit,
        );
        if top_target_changed {
            self.data.inline_top_tooltip_pending = inline_tooltip_pending(
                self.data.inline_top_hover_start,
                hit_has_tooltip(&self.data.inline_top_hits, top_hit),
            );
        }
        if top_target_changed {
            // The inline toolbar and annotations share the main surface's SHM
            // swapchain. Refresh every slot when hover visuals change so a
            // compositor cannot resurface a buffer containing older toolbar or
            // annotation pixels during rapid pointer motion.
            self.mark_inline_toolbar_full_damage();
        } else if was_top_hover != self.data.inline_top_hover {
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
            self.set_pointer_over_toolbar(true);
        } else if !self.toolbar_dragging() {
            self.set_pointer_over_toolbar(false);
            if self.data.toolbar_focus_active {
                self.data.toolbar_focus_active = false;
                self.clear_inline_toolbar_focus();
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
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
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
            self.set_pointer_over_toolbar(true);
            return true;
        }
        false
    }

    pub(in crate::backend::wayland) fn inline_toolbar_leave(&mut self) {
        if !self.inline_toolbars_active() {
            return;
        }
        let had_hover = self.data.inline_top_hover.is_some();
        let had_focus =
            self.data.inline_top_focus_index.is_some() || self.data.inline_top_focus_id.is_some();
        self.data.inline_top_hover = None;
        self.data.inline_top_hover_start = None;
        self.data.inline_top_tooltip_pending = false;
        self.data.toolbar_focus_active = false;
        self.clear_inline_toolbar_focus();
        self.set_pointer_over_toolbar(false);
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
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return false;
        }
        if self.pointer_over_toolbar() || self.toolbar_dragging() {
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
                    self.pointer_over_toolbar()
                )
            });
            self.finish_toolbar_item_drag(true);
            self.set_toolbar_dragging(false);
            self.set_pointer_over_toolbar(false);
            self.end_toolbar_move_drag();
            return true;
        }
        false
    }
}

fn point_in_surface(rect: Option<(f64, f64, f64, f64)>, position: (f64, f64)) -> bool {
    rect.is_some_and(|(x, y, w, h)| geometry::point_in_rect(position.0, position.1, x, y, w, h))
}

fn inline_hover_target_changed(
    previous_hover: Option<(f64, f64)>,
    previous_hit: Option<usize>,
    hover: Option<(f64, f64)>,
    hit: Option<usize>,
) -> bool {
    previous_hover.is_some() != hover.is_some() || previous_hit != hit
}

fn hit_has_tooltip(
    hits: &[crate::backend::wayland::toolbar::hit::HitRegion],
    hit: Option<usize>,
) -> bool {
    hit.and_then(|index| hits.get(index))
        .is_some_and(|hit| hit.tooltip.is_some())
}

fn inline_tooltip_pending(hover_start: Option<Instant>, hit_has_tooltip: bool) -> bool {
    hit_has_tooltip
        && hover_start.is_some_and(|start| {
            start.elapsed() < crate::backend::wayland::toolbar::render::TOOLTIP_DELAY
        })
}

#[cfg(test)]
mod tests {
    use super::{inline_hover_target_changed, inline_tooltip_pending, point_in_surface};
    use std::time::Instant;

    #[test]
    fn inline_hover_damage_tracks_control_transitions_not_pointer_pixels() {
        assert!(!inline_hover_target_changed(
            Some((10.0, 10.0)),
            Some(2),
            Some((11.0, 10.0)),
            Some(2),
        ));
        assert!(inline_hover_target_changed(
            Some((10.0, 10.0)),
            Some(2),
            Some((20.0, 10.0)),
            Some(3),
        ));
        assert!(inline_hover_target_changed(
            None,
            None,
            Some((10.0, 10.0)),
            None,
        ));
        assert!(inline_hover_target_changed(
            Some((10.0, 10.0)),
            None,
            None,
            None,
        ));
    }

    #[test]
    fn inline_tooltip_is_pending_only_during_the_delay() {
        let recent = Instant::now();
        assert!(inline_tooltip_pending(Some(recent), true));
        assert!(!inline_tooltip_pending(Some(recent), false));
        assert!(!inline_tooltip_pending(None, true));
    }

    #[test]
    fn inline_surface_gate_rejects_clicks_beyond_all_edges() {
        let rect = Some((10.0, 20.0, 100.0, 50.0));
        assert!(!point_in_surface(rect, (9.9, 45.0)));
        assert!(!point_in_surface(rect, (110.1, 45.0)));
        assert!(!point_in_surface(rect, (60.0, 19.9)));
        assert!(!point_in_surface(rect, (60.0, 70.1)));
        assert!(point_in_surface(rect, (10.0, 20.0)));
        assert!(point_in_surface(rect, (110.0, 70.0)));
    }
}
