use super::*;

impl WaylandState {
    pub(super) fn clear_inline_toolbar_hits(&mut self) {
        self.data.inline_top_hits.clear();
        self.data.inline_side_hits.clear();
        self.data.inline_top_rect = None;
        self.data.inline_side_rect = None;
        self.data.inline_top_hover = None;
        self.data.inline_side_hover = None;
    }

    pub(in crate::backend::wayland) fn render_inline_toolbars(
        &mut self,
        ctx: &cairo::Context,
        snapshot: &ToolbarSnapshot,
    ) {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            self.clear_inline_toolbar_hits();
            return;
        }

        self.clear_inline_toolbar_hits();
        self.clamp_toolbar_offsets(snapshot);

        let top_size = top_size(snapshot);
        let side_size = side_size(snapshot);

        // Position inline toolbars with padding and keep top bar to the right of the side bar.
        let side_offset = (
            Self::INLINE_SIDE_X + self.data.toolbar_side_offset_x,
            Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset,
        );

        let top_base_x = self.inline_top_base_x(snapshot);
        let top_offset = (
            top_base_x + self.data.toolbar_top_offset,
            self.inline_top_base_y() + self.data.toolbar_top_offset_y,
        );

        // Top toolbar
        let top_hover_local = self
            .data
            .inline_top_hover
            .map(|(x, y)| (x - top_offset.0, y - top_offset.1));
        let _ = ctx.save();
        ctx.translate(top_offset.0, top_offset.1);
        if let Err(err) = render_top_strip(
            ctx,
            top_size.0 as f64,
            top_size.1 as f64,
            snapshot,
            &mut self.data.inline_top_hits,
            top_hover_local,
        ) {
            log::warn!("Failed to render inline top toolbar: {}", err);
        }
        let _ = ctx.restore();
        for hit in &mut self.data.inline_top_hits {
            hit.rect.0 += top_offset.0;
            hit.rect.1 += top_offset.1;
        }
        self.data.inline_top_rect = Some((
            top_offset.0,
            top_offset.1,
            top_size.0 as f64,
            top_size.1 as f64,
        ));

        // Side toolbar
        let side_hover_local = self
            .data
            .inline_side_hover
            .map(|(x, y)| (x - side_offset.0, y - side_offset.1));
        let _ = ctx.save();
        ctx.translate(side_offset.0, side_offset.1);
        if let Err(err) = render_side_palette(
            ctx,
            side_size.0 as f64,
            side_size.1 as f64,
            snapshot,
            &mut self.data.inline_side_hits,
            side_hover_local,
        ) {
            log::warn!("Failed to render inline side toolbar: {}", err);
        }
        let _ = ctx.restore();
        for hit in &mut self.data.inline_side_hits {
            hit.rect.0 += side_offset.0;
            hit.rect.1 += side_offset.1;
        }
        self.data.inline_side_rect = Some((
            side_offset.0,
            side_offset.1,
            side_size.0 as f64,
            side_size.1 as f64,
        ));
    }

    fn inline_toolbar_hit_at(
        &self,
        position: (f64, f64),
    ) -> Option<(crate::backend::wayland::toolbar_intent::ToolbarIntent, bool)> {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return None;
        }
        self.data
            .inline_top_hits
            .iter()
            .chain(self.data.inline_side_hits.iter())
            .find_map(|hit| intent_for_hit(hit, position.0, position.1))
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
        self.data
            .inline_top_hits
            .iter()
            .chain(self.data.inline_side_hits.iter())
            .find_map(|hit| drag_intent_for_hit(hit, position.0, position.1))
    }

    /// Generate a drag intent from the active toolbar move drag state.
    /// This bypasses hit testing to allow dragging to continue when the mouse
    /// moves outside the original drag handle region.
    pub(in crate::backend::wayland) fn move_drag_intent(
        &self,
        x: f64,
        y: f64,
    ) -> Option<crate::backend::wayland::toolbar_intent::ToolbarIntent> {
        use crate::backend::wayland::toolbar_intent::ToolbarIntent;
        use crate::ui::toolbar::ToolbarEvent;

        match self.data.toolbar_move_drag {
            Some(MoveDrag {
                kind: MoveDragKind::Top,
                ..
            }) => Some(ToolbarIntent(ToolbarEvent::MoveTopToolbar { x, y })),
            Some(MoveDrag {
                kind: MoveDragKind::Side,
                ..
            }) => Some(ToolbarIntent(ToolbarEvent::MoveSideToolbar { x, y })),
            None => None,
        }
    }

    /// Returns true if we're currently in a toolbar move drag operation.
    pub(in crate::backend::wayland) fn is_move_dragging(&self) -> bool {
        self.data.toolbar_move_drag.is_some()
    }

    pub(in crate::backend::wayland) fn active_move_drag_kind(&self) -> Option<MoveDragKind> {
        self.data.active_drag_kind
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

        let mut over_toolbar = false;

        if let Some((x, y, w, h)) = self.data.inline_top_rect
            && geometry::point_in_rect(position.0, position.1, x, y, w, h)
        {
            over_toolbar = true;
            self.data.inline_top_hover = Some(position);
        }

        if let Some((x, y, w, h)) = self.data.inline_side_rect
            && geometry::point_in_rect(position.0, position.1, x, y, w, h)
        {
            over_toolbar = true;
            self.data.inline_side_hover = Some(position);
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
        self.data.inline_top_hover = None;
        self.data.inline_side_hover = None;
        self.set_pointer_over_toolbar(false);
        // Don't clear drag state if we're in a move drag - the drag continues outside
        if !self.is_move_dragging() {
            self.set_toolbar_dragging(false);
            self.end_toolbar_move_drag();
        }
        if had_hover {
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
