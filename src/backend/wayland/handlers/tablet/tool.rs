use log::{debug, info};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::ZwpTabletToolV2;

use crate::backend::wayland::toolbar_intent::intent_to_event;
use crate::{
    input::{DrawingState, EraserMode, Tool},
    util::Rect,
};

use crate::backend::wayland::state::{RegionReviewPress, WaylandState};
use crate::input::state::RegionInputSource;

const STYLUS_CURSOR_DAMAGE_RADIUS: i32 = 64;

/// Whether a pressure sample must be dropped rather than queued for the next
/// tablet frame.
///
/// A committed pressure sample runs `set_pressure_thickness_for_active_tool`,
/// so a sample that should not have reached the canvas resizes the drawing tool
/// — the one thing OCR promises not to touch. Three independent reasons to drop
/// one, none of which subsumes the others:
///
/// * the pen is not over the overlay, so the sample is not ours to apply;
/// * the contact was disowned when a screen modal took over, and is only still
///   arriving because clearing our flags does not lift the pen;
/// * a screen modal is on screen and owns the pen outright.
///
/// The modal term follows the *active* boundary, which keeps this an exact twin
/// of the tip and motion guards: a stroke begun while a capture is still pending
/// is a real stroke and keeps its pressure. The retired term is what stops that
/// allowance from also readmitting the contact the modal already took over.
fn drop_stylus_pressure(
    on_overlay: bool,
    contact_retired: bool,
    input_state: &crate::input::InputState,
) -> bool {
    !on_overlay || contact_retired || input_state.screen_modal_is_active()
}
impl WaylandState {
    fn stylus_hover_cursor_pos(&self) -> Option<(f64, f64)> {
        self.stylus_hover_cursor_position()
    }

    fn stylus_hover_needs_full_damage(
        &self,
        previous: Option<(f64, f64)>,
        next: Option<(f64, f64)>,
    ) -> bool {
        if previous.is_none() && next.is_none() {
            return false;
        }

        self.stylus_hover_eraser_needs_full_damage() || self.stylus_hover_ui_needs_full_damage()
    }

    fn stylus_hover_eraser_needs_full_damage(&self) -> bool {
        self.input_state.eraser_mode == EraserMode::Stroke
            && self.input_state.active_tool() == Tool::Eraser
            && matches!(self.input_state.state, DrawingState::Idle)
    }

    fn stylus_hover_ui_needs_full_damage(&self) -> bool {
        self.input_state.is_radial_menu_open()
            || self.input_state.is_color_picker_popup_open()
            || self.input_state.is_board_picker_open()
            || self.input_state.is_properties_panel_open()
            || self.input_state.is_context_menu_open()
            || (self.inline_toolbars_active() && self.toolbar.is_visible())
    }

    pub(in crate::backend::wayland) fn mark_stylus_hover_cursor_dirty(
        &mut self,
        previous: Option<(f64, f64)>,
        next: Option<(f64, f64)>,
    ) {
        if previous.is_none() && next.is_none() {
            return;
        }

        if self.stylus_hover_needs_full_damage(previous, next) {
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            return;
        }

        let width = self.surface.width().min(i32::MAX as u32) as i32;
        let height = self.surface.height().min(i32::MAX as u32) as i32;
        for pos in [previous, next].into_iter().flatten() {
            if let Some(rect) = stylus_cursor_damage_rect(pos, width, height) {
                self.input_state.dirty_tracker.mark_rect(rect);
            } else if width <= 0 || height <= 0 {
                self.input_state.dirty_tracker.mark_full();
            }
        }
        self.input_state.needs_redraw = true;
    }
}

fn stylus_cursor_damage_rect(pos: (f64, f64), width: i32, height: i32) -> Option<Rect> {
    if width <= 0 || height <= 0 {
        return None;
    }

    let x = pos.0.round() as i32;
    let y = pos.1.round() as i32;
    let min_x = x
        .saturating_sub(STYLUS_CURSOR_DAMAGE_RADIUS)
        .clamp(0, width);
    let min_y = y
        .saturating_sub(STYLUS_CURSOR_DAMAGE_RADIUS)
        .clamp(0, height);
    let max_x = x
        .saturating_add(STYLUS_CURSOR_DAMAGE_RADIUS)
        .clamp(0, width);
    let max_y = y
        .saturating_add(STYLUS_CURSOR_DAMAGE_RADIUS)
        .clamp(0, height);
    Rect::from_min_max(min_x, min_y, max_x, max_y)
}

impl WaylandState {
    fn handle_stylus_proximity_in(
        &mut self,
        proxy: &ZwpTabletToolV2,
        surface: wayland_client::protocol::wl_surface::WlSurface,
    ) {
        let tool_id = proxy.id();
        let tool_type = self.tablet.tool_types.get(&tool_id).copied();
        debug!(
            "Tablet proximity in: tool {:?}, type: {:?}",
            tool_id, tool_type
        );
        let on_overlay = self
            .surface
            .wl_surface()
            .is_some_and(|candidate| candidate.id() == surface.id());
        let on_toolbar = self.toolbar.is_toolbar_surface(&surface);
        self.tablet.surface = Some(surface);
        self.tablet.on_overlay = on_overlay;
        self.tablet.on_toolbar = on_toolbar;
        self.finish_toolbar_item_drag(false);
        self.set_toolbar_dragging(false);
        self.cancel_toolbar_move_drag();
        self.tablet.tip_down = false;
        self.tablet.base_thickness = Some(self.input_state.current_thickness);
        self.tablet.pressure_thickness = None;
        self.tablet.last_pos = None;
        self.tablet.pending_frame = Default::default();
        self.auto_switch_physical_eraser(tool_type);

        if on_overlay {
            info!("✏️  Stylus ENTERED overlay surface");
        } else if on_toolbar {
            debug!("Stylus entered toolbar surface");
        } else {
            debug!("Tablet proximity in on non-overlay surface");
        }
    }

    fn auto_switch_physical_eraser(
        &mut self,
        tool_type: Option<crate::backend::wayland::TabletToolType>,
    ) {
        if !self.config.tablet.auto_eraser_switch
            || !tool_type.is_some_and(|tool_type| tool_type.is_eraser())
            || self.input_state.active_tool() == Tool::Eraser
        {
            return;
        }
        self.tablet.pre_eraser_tool_override = self.input_state.tool_override();
        self.input_state.set_tool_override(Some(Tool::Eraser));
        self.tablet.auto_switched_to_eraser = true;
        info!(
            "Auto-switched to eraser (physical eraser detected), saved previous: {:?}",
            self.tablet.pre_eraser_tool_override
        );
    }

    fn handle_stylus_proximity_out(&mut self, proxy: &ZwpTabletToolV2) {
        let tool_id = proxy.id();
        let tool_type = self.tablet.tool_types.get(&tool_id).copied();
        debug!(
            "Tablet proximity out: tool {:?}, type: {:?}",
            tool_id, tool_type
        );
        self.commit_pending_stylus_frame();
        self.cancel_region_selection_from(RegionInputSource::Stylus);
        self.take_retired_stylus_contact();
        let hover_cursor_pos = self.stylus_hover_cursor_pos();
        self.tablet.tip_down = false;
        self.tablet.on_overlay = false;
        self.tablet.on_toolbar = false;
        self.finish_toolbar_item_drag(false);
        self.set_toolbar_dragging(false);
        self.cancel_toolbar_move_drag();
        if let Some(surface) = self.tablet.surface.take()
            && self.toolbar.is_toolbar_surface(&surface)
        {
            self.toolbar.pointer_leave(&surface);
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
        }
        self.tablet.pressure_thickness = None;
        self.tablet.last_pos = None;
        self.mark_stylus_hover_cursor_dirty(hover_cursor_pos, None);
        self.restore_tool_after_physical_eraser();
    }

    fn restore_tool_after_physical_eraser(&mut self) {
        if !self.tablet.auto_switched_to_eraser {
            return;
        }
        let restored_tool = self.tablet.pre_eraser_tool_override;
        self.input_state.set_tool_override(restored_tool);
        self.tablet.auto_switched_to_eraser = false;
        self.tablet.pre_eraser_tool_override = None;
        info!(
            "Restored previous tool after eraser proximity out: {:?}",
            restored_tool
        );
    }

    fn handle_stylus_down(&mut self, conn: &Connection, qh: &QueueHandle<Self>) {
        if self.tablet.contact_retired {
            return;
        }
        self.input_state.dismiss_ocr_scan_result();
        if self.handle_stylus_region_down() || self.handle_stylus_eyedropper_down() {
            return;
        }
        if self.inline_toolbars_active()
            && self.toolbar.is_visible()
            && self.handle_inline_stylus_down(conn, qh)
        {
            return;
        }
        if self.handle_toolbar_stylus_down(conn, qh) || !self.tablet.on_overlay {
            return;
        }
        self.queue_stylus_down();
    }

    fn handle_stylus_region_down(&mut self) -> bool {
        if !self.input_state.region_is_active() {
            return false;
        }
        if self.tablet.on_toolbar {
            self.cancel_region_for_toolbar_interaction();
            return false;
        }
        if !self.tablet.on_overlay {
            return false;
        }
        let (x, y) = self.current_or_pending_stylus_position();
        match self.consume_region_review_press(RegionInputSource::Stylus, (x, y)) {
            RegionReviewPress::NotReview | RegionReviewPress::Fallthrough => {
                self.begin_region_selection(RegionInputSource::Stylus, x, y);
            }
            RegionReviewPress::Consumed { .. } => {
                self.retire_stylus_contact();
            }
        }
        true
    }

    fn handle_stylus_eyedropper_down(&mut self) -> bool {
        if !self.input_state.eyedropper_is_active() {
            return false;
        }
        if self.tablet.on_toolbar {
            self.cancel_eyedropper();
            return false;
        }
        if !self.tablet.on_overlay {
            return false;
        }
        let (x, y) = self.current_or_pending_stylus_position();
        self.sample_eyedropper(x, y);
        true
    }

    fn handle_inline_stylus_down(&mut self, conn: &Connection, qh: &QueueHandle<Self>) -> bool {
        let position = self.current_or_pending_stylus_position();
        if !self.inline_toolbar_press(position, Some(conn), Some(qh)) {
            return false;
        }
        self.tablet.on_toolbar = true;
        self.set_toolbar_dragging(self.toolbar_dragging());
        true
    }

    fn handle_toolbar_stylus_down(&mut self, conn: &Connection, qh: &QueueHandle<Self>) -> bool {
        if !self.tablet.on_toolbar {
            return false;
        }
        let (x, y) = self.current_or_pending_stylus_position();
        self.set_current_mouse(x as i32, y as i32);
        if let Some(surface) = self.tablet.surface.as_ref()
            && let Some((intent, drag)) = self.toolbar.pointer_press(surface, (x, y))
        {
            self.set_toolbar_dragging(drag);
            let event = intent_to_event(intent, self.toolbar.last_snapshot());
            self.handle_toolbar_event(event, Some(conn), Some(qh));
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            self.refresh_keyboard_interactivity();
        }
        true
    }

    fn handle_stylus_up(&mut self) {
        let retired_contact = self.take_retired_stylus_contact();
        if self.input_state.region_is_active() {
            if self.tablet.on_overlay {
                let (x, y) = self.current_or_pending_stylus_position();
                self.finish_region_selection(RegionInputSource::Stylus, x, y);
            } else {
                self.cancel_region_selection_from(RegionInputSource::Stylus);
            }
            return;
        }
        let inline_active = self.inline_toolbars_active() && self.toolbar.is_visible();
        if inline_active && self.tablet.on_toolbar {
            let (x, y) = self.current_mouse();
            self.inline_toolbar_release((x as f64, y as f64));
            self.tablet.on_toolbar = false;
            self.set_toolbar_dragging(false);
            self.end_toolbar_move_drag();
            return;
        }
        if self.tablet.on_toolbar {
            self.finish_toolbar_item_drag(true);
            self.set_toolbar_dragging(false);
            self.end_toolbar_move_drag();
            return;
        }
        if self.tablet.on_overlay && !retired_contact {
            self.queue_stylus_up();
        }
    }

    fn handle_stylus_motion(&mut self, conn: &Connection, qh: &QueueHandle<Self>, x: f64, y: f64) {
        if self.handle_modal_stylus_motion(x, y) || self.handle_stylus_move_drag(x, y) {
            return;
        }
        let previous_hover = self.stylus_hover_cursor_pos();
        if self.handle_toolbar_stylus_motion(conn, qh, x, y) {
            return;
        }
        if self.inline_toolbars_active() && self.toolbar.is_visible() {
            self.tablet.last_pos = Some((x, y));
            if self.inline_toolbar_motion((x, y)) {
                self.commit_pending_stylus_frame();
                self.tablet.last_pos = Some((x, y));
                self.tablet.on_toolbar = true;
                self.mark_stylus_hover_cursor_dirty(previous_hover, None);
                return;
            }
            self.tablet.on_toolbar = false;
        }
        if self.tablet.on_overlay {
            self.queue_stylus_motion(x, y);
        }
    }

    fn handle_modal_stylus_motion(&mut self, x: f64, y: f64) -> bool {
        if self.input_state.region_is_active() && self.tablet.on_overlay {
            self.tablet.last_pos = Some((x, y));
            self.set_current_mouse(x.round() as i32, y.round() as i32);
            self.update_region_selection(RegionInputSource::Stylus, x, y);
            return true;
        }
        if self.input_state.eyedropper_is_active() && self.tablet.on_overlay {
            self.tablet.last_pos = Some((x, y));
            self.set_current_mouse(x.round() as i32, y.round() as i32);
            self.update_eyedropper_hover(x, y);
            return true;
        }
        false
    }

    fn handle_stylus_move_drag(&mut self, x: f64, y: f64) -> bool {
        if !self.is_move_dragging() {
            return false;
        }
        let Some(kind) = self.active_move_drag_kind() else {
            return false;
        };
        if self.tablet.on_toolbar {
            self.handle_toolbar_move(kind, (x, y));
        } else {
            self.handle_toolbar_move_screen(kind, (x, y));
        }
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
        self.set_current_mouse(x as i32, y as i32);
        true
    }

    fn handle_toolbar_stylus_motion(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        x: f64,
        y: f64,
    ) -> bool {
        if !self.tablet.on_toolbar {
            return false;
        }
        self.tablet.last_pos = Some((x, y));
        if let Some(surface) = self.tablet.surface.as_ref() {
            let event = self.toolbar.pointer_motion(surface, (x, y));
            if self.toolbar_dragging() {
                let intent = event.or_else(|| self.move_drag_intent(x, y));
                if let Some(intent) = intent {
                    let event = intent_to_event(intent, self.toolbar.last_snapshot());
                    self.handle_toolbar_event(event, Some(conn), Some(qh));
                }
            } else {
                self.toolbar.mark_dirty();
            }
            self.input_state.needs_redraw = true;
            self.refresh_keyboard_interactivity();
        }
        self.set_current_mouse(x as i32, y as i32);
        true
    }
}

impl Dispatch<ZwpTabletToolV2, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpTabletToolV2,
        event: <ZwpTabletToolV2 as Proxy>::Event,
        _data: &(),
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::Event;
        match event {
            Event::ProximityIn { surface, .. } => {
                state.handle_stylus_proximity_in(_proxy, surface);
            }
            Event::ProximityOut => {
                state.handle_stylus_proximity_out(_proxy);
            }
            Event::Down { .. } => {
                state.handle_stylus_down(conn, qh);
            }
            Event::Up => {
                state.handle_stylus_up();
            }
            Event::Motion { x, y } => {
                state.handle_stylus_motion(conn, qh, x, y);
            }
            Event::Pressure { pressure } => {
                if drop_stylus_pressure(
                    state.tablet.on_overlay,
                    state.tablet.contact_retired,
                    &state.input_state,
                ) {
                    return;
                }
                state.queue_stylus_pressure(pressure);
            }
            Event::Type { tool_type } => {
                use crate::backend::wayland::TabletToolType;
                let physical_type = TabletToolType::from(tool_type);
                let tool_id = _proxy.id();
                debug!("Tablet tool type: {:?} -> {:?}", tool_id, physical_type);
                state.tablet.tool_types.insert(tool_id, physical_type);

                // Note: We don't switch tools here - this event comes during initial
                // tool setup, before proximity_in. The actual switch happens in proximity_in.
            }
            Event::Button {
                button,
                state: button_state,
                ..
            } => {
                use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::ButtonState;
                let pressed = button_state == wayland_client::WEnum::Value(ButtonState::Pressed);
                debug!(
                    "Tablet tool button: {} {}",
                    button,
                    if pressed { "pressed" } else { "released" }
                );
                if pressed {
                    state.queue_stylus_button_press(button);
                }
            }
            Event::Frame { .. } => {
                state.commit_pending_stylus_frame();
                debug!("Tablet frame event");
            }
            other => {
                debug!("Unhandled tablet tool event: {:?}", other);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The modal term stops exactly where the tip and motion guards stop: once
    /// the selector is on screen. While a capture is still pending the canvas
    /// keeps the pen, so a stroke drawn then must keep its pressure — otherwise
    /// a capture that fails would leave a silently flat stroke behind.
    #[test]
    fn stylus_pressure_stops_at_the_selector_not_at_the_request() {
        use crate::input::state::test_support::make_test_input_state;
        use crate::input::state::{EyedropperCaptureSource, RegionPurposeTag, ScreenCaptureSource};

        let fresh_contact = |state: &_| drop_stylus_pressure(true, false, state);

        let mut state = make_test_input_state();
        assert!(!fresh_contact(&state));

        state.set_region_pending_capture(RegionPurposeTag::Ocr, 1, ScreenCaptureSource::Frozen);
        assert!(
            !fresh_contact(&state),
            "a stroke drawn while the capture is pending is still a real stroke"
        );
        state.activate_region(RegionPurposeTag::Ocr, 1);
        assert!(fresh_contact(&state));
        state.start_region_selection(RegionInputSource::Stylus, (10.0, 10.0));
        assert!(fresh_contact(&state));
        state.cancel_region_ui_only();
        assert!(!fresh_contact(&state));

        state.set_eyedropper_pending_capture(EyedropperCaptureSource::Frozen);
        assert!(!fresh_contact(&state));
        state.activate_eyedropper(Some(1));
        assert!(fresh_contact(&state));
        state.cancel_eyedropper();
        assert!(!fresh_contact(&state));
    }

    /// A contact disowned by a modal keeps arriving until the pen lifts, and the
    /// pending-capture allowance above must not readmit it: the canvas holds the
    /// pen again during the wait, but this contact is not the user drawing.
    #[test]
    fn a_retired_contact_never_reaches_the_tool_whatever_the_modal_is_doing() {
        use crate::input::state::test_support::make_test_input_state;
        use crate::input::state::{RegionPurposeTag, ScreenCaptureSource};

        let mut state = make_test_input_state();

        // The window the previous test allows, and the one that matters here.
        state.set_region_pending_capture(RegionPurposeTag::Ocr, 1, ScreenCaptureSource::Frozen);
        assert!(!drop_stylus_pressure(true, false, &state));
        assert!(drop_stylus_pressure(true, true, &state));

        // And it outlives the modal: cancelling before activation leaves the pen
        // physically down with no modal to blame.
        state.cancel_region_ui_only();
        assert!(!drop_stylus_pressure(true, false, &state));
        assert!(drop_stylus_pressure(true, true, &state));

        // Off the overlay nothing is ours either way.
        assert!(drop_stylus_pressure(false, false, &state));
    }

    #[test]
    fn stylus_cursor_damage_rect_covers_cursor_area() {
        let rect = stylus_cursor_damage_rect((100.2, 80.7), 400, 300).expect("rect");

        assert_eq!(
            rect,
            Rect::new(
                100 - STYLUS_CURSOR_DAMAGE_RADIUS,
                81 - STYLUS_CURSOR_DAMAGE_RADIUS,
                STYLUS_CURSOR_DAMAGE_RADIUS * 2,
                STYLUS_CURSOR_DAMAGE_RADIUS * 2,
            )
            .unwrap()
        );
    }

    #[test]
    fn stylus_cursor_damage_rect_clamps_to_surface() {
        let rect = stylus_cursor_damage_rect((4.0, 3.0), 400, 300).expect("rect");

        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.width, 68);
        assert_eq!(rect.height, 67);
    }

    #[test]
    fn stylus_cursor_damage_rect_ignores_empty_surface() {
        assert_eq!(stylus_cursor_damage_rect((10.0, 10.0), 0, 300), None);
        assert_eq!(stylus_cursor_damage_rect((10.0, 10.0), 400, 0), None);
    }
}
