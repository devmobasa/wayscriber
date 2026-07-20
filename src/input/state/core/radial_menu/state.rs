use std::time::{Duration, Instant};

use super::layout::CENTER_RADIUS;
use super::size_ring::size_ring_value_for_angle;
use super::{RADIAL_PAINT_DELAY, RadialMenuState, RadialRingSwatch, RadialSegmentId};
use super::{
    RadialSliceKind, compass_slice, slice_parent, sub_ring_child_count, sub_ring_children,
};
use crate::input::events::MouseButton;
use crate::input::state::InputState;

impl InputState {
    /// Whether the radial menu is currently visible.
    pub fn is_radial_menu_open(&self) -> bool {
        matches!(self.radial_menu_state, RadialMenuState::Open { .. })
    }

    fn open_radial_menu_internal(&mut self, x: f64, y: f64, track_usage: bool) {
        // Mutual exclusion with other popups
        if self.show_help {
            self.toggle_help_overlay();
        }
        if self.is_context_menu_open() {
            self.close_context_menu();
        }
        if self.is_color_picker_popup_open() {
            self.close_color_picker_popup(true);
        }
        if self.is_properties_panel_open() {
            self.close_properties_panel();
        }
        if self.command_palette_open {
            self.command_palette_open = false;
        }

        self.radial_menu_state = RadialMenuState::Open {
            center_x: x,
            center_y: y,
            hover: None,
            expanded_sub_ring: None,
            opened_at: Instant::now(),
            painted: false,
            flick_armed: false,
            size_dragging: false,
        };
        if track_usage {
            self.pending_onboarding_usage.used_radial_menu = true;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Open the radial menu centered on the given surface coordinates.
    pub fn open_radial_menu(&mut self, x: f64, y: f64) {
        self.open_radial_menu_internal(x, y, true);
    }

    /// Close the radial menu.
    pub fn close_radial_menu(&mut self) {
        if self.is_radial_menu_open() {
            self.radial_menu_state = RadialMenuState::Hidden;
            self.radial_menu_layout = None;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Toggle the radial menu open/closed at the given position.
    pub fn toggle_radial_menu(&mut self, x: f64, y: f64) {
        if self.is_radial_menu_open() {
            self.close_radial_menu();
        } else {
            self.open_radial_menu(x, y);
        }
    }

    // ── paint-delay gate ──

    /// Wakeup budget for the event loop: time left until the open menu is
    /// due to paint. `None` once painted (or while hidden), and `None` past
    /// the deadline once the painting redraw is already requested — the
    /// render path (or its pending frame callback) takes it from there, and
    /// a zero timeout would spin the dispatch loop until it lands.
    pub fn radial_menu_paint_timeout(&self, now: Instant) -> Option<Duration> {
        match &self.radial_menu_state {
            RadialMenuState::Open {
                opened_at,
                painted: false,
                ..
            } => {
                let deadline = *opened_at + RADIAL_PAINT_DELAY;
                if now >= deadline && self.needs_redraw {
                    return None;
                }
                Some(deadline.saturating_duration_since(now))
            }
            _ => None,
        }
    }

    /// Event-loop pump: when the paint deadline of a still-unpainted open
    /// menu has passed, request the redraw that will paint it. Returns true
    /// when a redraw was requested.
    pub fn tick_radial_menu_paint(&mut self, now: Instant) -> bool {
        match &self.radial_menu_state {
            RadialMenuState::Open {
                opened_at,
                painted: false,
                ..
            } if now >= *opened_at + RADIAL_PAINT_DELAY => {
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                true
            }
            _ => false,
        }
    }

    /// Render-site gate: true when the menu may paint this frame. Marks the
    /// menu as painted on the first frame at/after the deadline; layout and
    /// hit-testing stay live regardless.
    pub fn radial_menu_mark_painted_if_due(&mut self, now: Instant) -> bool {
        if let RadialMenuState::Open {
            opened_at,
            ref mut painted,
            ..
        } = self.radial_menu_state
        {
            if *painted {
                return true;
            }
            if now >= opened_at + RADIAL_PAINT_DELAY {
                *painted = true;
                return true;
            }
        }
        false
    }

    /// Whether the open menu has been painted at least once.
    pub fn radial_menu_has_painted(&self) -> bool {
        matches!(
            self.radial_menu_state,
            RadialMenuState::Open { painted: true, .. }
        )
    }

    // ── hover / hit-testing ──

    /// Update the hovered segment based on pointer position.
    pub fn update_radial_menu_hover(&mut self, x: f64, y: f64) {
        let color_count = self.radial_ring_swatch_count();
        if let RadialMenuState::Open {
            ref mut hover,
            ref mut expanded_sub_ring,
            ..
        } = self.radial_menu_state
            && let Some(layout) = &self.radial_menu_layout
        {
            let segment =
                super::hit_test::hit_test_radial(layout, *expanded_sub_ring, color_count, x, y);
            let old_hover = *hover;
            let old_expanded_sub_ring = *expanded_sub_ring;
            *hover = segment;

            // Expand/collapse sub-ring based on hovered segment
            match segment {
                // Hovering a parent slice expands its sub-ring
                Some(RadialSegmentId::Tool(idx)) if slice_parent(idx).is_some() => {
                    *expanded_sub_ring = Some(idx);
                }
                // Keep sub-ring expanded while hovering its children
                Some(RadialSegmentId::SubTool(_, _)) => {}
                // Keep sub-ring expanded when cursor is in/near the sub-ring
                // band (None from gap or outside parent angle range)
                None if expanded_sub_ring.is_some() => {
                    let dx = x - layout.center_x;
                    let dy = y - layout.center_y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    // Only collapse if cursor left the sub-ring distance band
                    if dist < layout.tool_inner || dist > layout.sub_outer {
                        *expanded_sub_ring = None;
                    }
                }
                // Collapse when hovering a different tool, color, or outer segment
                Some(RadialSegmentId::Tool(_))
                | Some(RadialSegmentId::Color(_))
                | Some(RadialSegmentId::SizeRing)
                | Some(RadialSegmentId::Center) => {
                    *expanded_sub_ring = None;
                }
                None => {}
            }

            if old_hover != *hover || old_expanded_sub_ring != *expanded_sub_ring {
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
        }
    }

    /// Select the currently hovered segment and close the menu.
    pub fn radial_menu_select_hovered(&mut self) {
        let hover = match &self.radial_menu_state {
            RadialMenuState::Open { hover, .. } => *hover,
            _ => return,
        };

        match hover {
            Some(RadialSegmentId::Tool(idx)) if sub_ring_child_count(idx) > 0 => {
                // Parent with children — expand sub-ring, don't close
                self.radial_menu_expand_sub_ring(idx);
                return;
            }
            Some(RadialSegmentId::Tool(idx)) => {
                self.dispatch_tool_segment(idx);
            }
            Some(RadialSegmentId::SubTool(parent, child)) => {
                self.dispatch_sub_tool_segment(parent, child);
            }
            Some(RadialSegmentId::Color(idx)) => {
                self.dispatch_color_segment(idx);
            }
            Some(RadialSegmentId::SizeRing) => {
                // The gauge is a drag/scroll surface, never a commit target.
                return;
            }
            Some(RadialSegmentId::Center) | None => {
                // Dismiss only
            }
        }

        self.close_radial_menu();
    }

    /// Adjust thickness via scroll wheel while the menu is open.
    pub fn radial_menu_adjust_thickness(&mut self, delta: f64) -> bool {
        if !self.nudge_thickness_for_active_tool(delta) {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    // ── flick commit ──

    /// Record a pointer-motion sample while the menu is open: once the
    /// pointer travels a deadzone radius away from the open position the
    /// flick is armed and the next toggle-button release commits (or, back
    /// inside the deadzone, cancels).
    pub(crate) fn radial_menu_sample_flick(&mut self, x: f64, y: f64) {
        if let RadialMenuState::Open {
            center_x,
            center_y,
            ref mut flick_armed,
            size_dragging: false,
            ..
        } = self.radial_menu_state
        {
            // Arm on pointer travel from the raw open position, never from
            // the clamped layout center: near a screen edge that center sits
            // far from the press point, which would let a pixel of
            // press-jitter arm a stationary toggle click.
            let dx = x - center_x;
            let dy = y - center_y;
            if (dx * dx + dy * dy).sqrt() > CENTER_RADIUS {
                *flick_armed = true;
            }
        }
    }

    /// Handle a pointer-button release while the menu is open. Returns false
    /// when the menu is closed (release not consumed).
    ///
    /// Left releases end a size-ring drag. A toggle-button release commits an
    /// armed flick: by pointer direction alone before the menu has painted
    /// (a flick through a parent slice opens its sub-ring and keeps the menu
    /// open — no blind sub-ring commits), or by the hovered sub-ring child /
    /// color swatch once the menu is visible. Releasing back inside the
    /// center deadzone cancels; an unarmed release keeps the menu open
    /// (click-to-open browsing).
    pub(crate) fn radial_menu_handle_release(
        &mut self,
        button: MouseButton,
        x: f64,
        y: f64,
    ) -> bool {
        if !self.is_radial_menu_open() {
            return false;
        }
        if button == MouseButton::Left {
            self.radial_menu_end_size_drag();
            return true;
        }
        if !self.is_radial_menu_toggle_button(button) {
            return true;
        }
        let (flick_armed, painted, expanded_sub_ring) = match &self.radial_menu_state {
            RadialMenuState::Open {
                flick_armed,
                painted,
                expanded_sub_ring,
                ..
            } => (*flick_armed, *painted, *expanded_sub_ring),
            RadialMenuState::Hidden => return false,
        };
        if !flick_armed {
            return true;
        }
        let Some((cx, cy, deadzone)) = self.radial_menu_flick_geometry() else {
            return true;
        };
        let dx = x - cx;
        let dy = y - cy;
        if (dx * dx + dy * dy).sqrt() <= deadzone {
            // Pulled back into the deadzone: cancel.
            self.close_radial_menu();
            return true;
        }

        // Sighted releases (menu painted) may commit exactly what the
        // pointer is on when that is a sub-ring child or a color swatch.
        if painted && let Some(layout) = self.radial_menu_layout {
            match super::hit_test::hit_test_radial(
                &layout,
                expanded_sub_ring,
                self.radial_ring_swatch_count(),
                x,
                y,
            ) {
                Some(RadialSegmentId::SubTool(parent, child)) => {
                    self.dispatch_sub_tool_segment(parent, child);
                    self.close_radial_menu();
                    return true;
                }
                Some(RadialSegmentId::Color(idx)) => {
                    self.dispatch_color_segment(idx);
                    self.close_radial_menu();
                    return true;
                }
                // The gauge is a drag/scroll surface, never a commit target.
                Some(RadialSegmentId::SizeRing) => return true,
                Some(RadialSegmentId::Tool(_)) | Some(RadialSegmentId::Center) | None => {}
            }
        }

        // Direction-only commit (the blind-flick path, also the fallback for
        // sighted releases outside the sub-ring/color bands).
        let idx = super::hit_test::primary_segment_for_point(cx, cy, x, y);
        if slice_parent(idx).is_some() {
            self.radial_menu_expand_sub_ring(idx);
            return true;
        }
        self.dispatch_tool_segment(idx);
        self.close_radial_menu();
        true
    }

    // ── size-ring drag ──

    /// Whether the current hover is the size ring (a press there starts a
    /// drag instead of selecting).
    pub(crate) fn radial_menu_hover_is_size_ring(&self) -> bool {
        matches!(
            self.radial_menu_state,
            RadialMenuState::Open {
                hover: Some(RadialSegmentId::SizeRing),
                ..
            }
        )
    }

    /// Whether a size-ring drag is capturing pointer motion.
    pub fn radial_menu_is_size_dragging(&self) -> bool {
        matches!(
            self.radial_menu_state,
            RadialMenuState::Open {
                size_dragging: true,
                ..
            }
        )
    }

    /// Begin a size-ring drag at the given pointer position (applies the
    /// value immediately).
    pub(crate) fn radial_menu_begin_size_drag(&mut self, x: f64, y: f64) {
        if let RadialMenuState::Open {
            ref mut size_dragging,
            ..
        } = self.radial_menu_state
        {
            *size_dragging = true;
        }
        self.radial_menu_drag_size_to(x, y);
    }

    /// Drag-capture update: map the pointer angle to a thickness and apply
    /// it to the active tool, regardless of pointer distance.
    pub(crate) fn radial_menu_drag_size_to(&mut self, x: f64, y: f64) {
        if !self.radial_menu_is_size_dragging() {
            return;
        }
        let Some((cx, cy, _)) = self.radial_menu_center_geometry() else {
            return;
        };
        let angle = (y - cy).atan2(x - cx);
        let value = size_ring_value_for_angle(angle);
        let _ = self.set_thickness_for_active_tool(value);
    }

    /// End a size-ring drag (the menu stays open).
    pub(crate) fn radial_menu_end_size_drag(&mut self) {
        if let RadialMenuState::Open {
            ref mut size_dragging,
            ..
        } = self.radial_menu_state
        {
            *size_dragging = false;
        }
    }

    // ── color ring composition ──

    /// The color-ring swatches: the quick palette's radial entries first,
    /// then the session's recent colors (minus exact quick-palette
    /// duplicates) appended as a visually separated arc.
    pub(crate) fn radial_ring_swatches(&self) -> Vec<RadialRingSwatch> {
        let mut swatches: Vec<RadialRingSwatch> = self
            .quick_colors
            .radial_rendered_entries()
            .iter()
            .map(|entry| RadialRingSwatch {
                color: entry.color,
                recent: false,
            })
            .collect();
        for color in &self.recent_colors {
            if swatches
                .iter()
                .any(|swatch| !swatch.recent && swatch.color == *color)
            {
                continue;
            }
            swatches.push(RadialRingSwatch {
                color: *color,
                recent: true,
            });
        }
        swatches
    }

    /// Number of color-ring segments (quick palette + displayed recents).
    pub(crate) fn radial_ring_swatch_count(&self) -> usize {
        let quick = self.quick_colors.radial_rendered_entries();
        quick.len()
            + self
                .recent_colors
                .iter()
                .filter(|color| !quick.iter().any(|entry| entry.color == **color))
                .count()
    }

    // ── dispatch helpers ──
    //
    // All slice dispatch flows through `handle_action`, so the radial can
    // never behave differently from the same action fired by a keybinding
    // or the toolbar.

    fn dispatch_tool_segment(&mut self, idx: u8) {
        if let Some(slice) = compass_slice(idx)
            && let RadialSliceKind::Action(action) = slice.kind
        {
            self.handle_action(action);
        }
    }

    fn dispatch_sub_tool_segment(&mut self, parent: u8, child: u8) {
        if let Some(action) = sub_ring_children(parent).get(child as usize) {
            self.handle_action(*action);
        }
    }

    fn dispatch_color_segment(&mut self, idx: u8) {
        if let Some(swatch) = self.radial_ring_swatches().get(idx as usize) {
            self.apply_color_from_ui(swatch.color);
        }
    }

    /// Expand a parent slice's sub-ring and keep the menu open.
    fn radial_menu_expand_sub_ring(&mut self, idx: u8) {
        if let RadialMenuState::Open {
            ref mut expanded_sub_ring,
            ..
        } = self.radial_menu_state
        {
            *expanded_sub_ring = Some(idx);
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Anchor point and deadzone radius for flick commit geometry. A blind
    /// (pre-paint) flick resolves against the raw open position — the point
    /// under the pointer at press time — because near a screen edge the
    /// clamped layout center the user has never seen can sit far away and
    /// would misread the flick direction. Sighted (painted) releases resolve
    /// against the visible menu center.
    fn radial_menu_flick_geometry(&self) -> Option<(f64, f64, f64)> {
        match &self.radial_menu_state {
            RadialMenuState::Open {
                center_x,
                center_y,
                painted: false,
                ..
            } => Some((*center_x, *center_y, CENTER_RADIUS)),
            RadialMenuState::Open { .. } => self.radial_menu_center_geometry(),
            RadialMenuState::Hidden => None,
        }
    }

    /// Menu center and deadzone radius: the clamped layout when available
    /// (normal path — layout is computed on the first render pass after
    /// opening), else the raw open position with the fixed center radius.
    fn radial_menu_center_geometry(&self) -> Option<(f64, f64, f64)> {
        if let Some(layout) = &self.radial_menu_layout {
            return Some((layout.center_x, layout.center_y, layout.center_radius));
        }
        match &self.radial_menu_state {
            RadialMenuState::Open {
                center_x, center_y, ..
            } => Some((*center_x, *center_y, CENTER_RADIUS)),
            RadialMenuState::Hidden => None,
        }
    }
}
