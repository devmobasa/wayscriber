use log::{debug, info};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::ZwpTabletToolV2;

use crate::backend::wayland::toolbar_intent::intent_to_event;
use crate::{
    input::{DrawingState, EraserMode, Tool},
    util::Rect,
};

use crate::backend::wayland::state::WaylandState;
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
                let tool_id = _proxy.id();
                let tool_type = state.stylus_tool_types.get(&tool_id).copied();
                debug!(
                    "Tablet proximity in: tool {:?}, type: {:?}",
                    tool_id, tool_type
                );
                let on_overlay = state
                    .surface
                    .wl_surface()
                    .is_some_and(|s| s.id() == surface.id());
                let on_toolbar = state.toolbar.is_toolbar_surface(&surface);
                state.stylus_surface = Some(surface.clone());
                state.stylus_on_overlay = on_overlay;
                state.stylus_on_toolbar = on_toolbar;
                state.finish_toolbar_item_drag(false);
                state.set_toolbar_dragging(false);
                state.cancel_toolbar_move_drag();
                state.stylus_tip_down = false;
                state.stylus_base_thickness = Some(state.input_state.current_thickness);
                state.stylus_pressure_thickness = None;
                state.stylus_last_pos = None;
                state.pending_stylus_frame = Default::default();

                // Auto-switch to eraser if physical tool is eraser (and config enables it)
                if state.config.tablet.auto_eraser_switch
                    && let Some(tool_type) = tool_type
                    && tool_type.is_eraser()
                {
                    // Only auto-switch if not already on eraser
                    if state.input_state.active_tool() != Tool::Eraser {
                        // Save the current tool override before switching
                        state.stylus_pre_eraser_tool_override = state.input_state.tool_override();
                        state.input_state.set_tool_override(Some(Tool::Eraser));
                        state.stylus_auto_switched_to_eraser = true;
                        info!(
                            "Auto-switched to eraser (physical eraser detected), saved previous: {:?}",
                            state.stylus_pre_eraser_tool_override
                        );
                    }
                }

                if on_overlay {
                    info!("✏️  Stylus ENTERED overlay surface");
                } else if state.toolbar.is_toolbar_surface(&surface) {
                    debug!("Stylus entered toolbar surface");
                } else {
                    debug!("Tablet proximity in on non-overlay surface");
                }
            }
            Event::ProximityOut => {
                let tool_id = _proxy.id();
                let tool_type = state.stylus_tool_types.get(&tool_id).copied();
                debug!(
                    "Tablet proximity out: tool {:?}, type: {:?}",
                    tool_id, tool_type
                );
                state.commit_pending_stylus_frame();
                // The tip is gone and no Up is coming, so a region drag *this
                // pen* started must end here rather than keeping the selector —
                // and any OCR-owned freeze — alive until the user cancels by
                // hand. A region the mouse or a finger is dragging is not ours
                // to withdraw.
                state.cancel_region_selection_from(RegionInputSource::Stylus);
                // The pen is gone, so the tip-up the latch was waiting for is
                // never coming; leaving it armed would swallow a later contact.
                state.take_retired_stylus_contact();
                let hover_cursor_pos = state.stylus_hover_cursor_pos();
                state.stylus_tip_down = false;
                state.stylus_on_overlay = false;
                state.stylus_on_toolbar = false;
                state.finish_toolbar_item_drag(false);
                state.set_toolbar_dragging(false);
                state.cancel_toolbar_move_drag();
                if let Some(surf) = state.stylus_surface.take()
                    && state.toolbar.is_toolbar_surface(&surf)
                {
                    state.toolbar.pointer_leave(&surf);
                    state.toolbar.mark_dirty();
                    state.input_state.needs_redraw = true;
                }
                state.stylus_pressure_thickness = None;
                state.stylus_last_pos = None;
                state.mark_stylus_hover_cursor_dirty(hover_cursor_pos, None);

                // Restore previous tool if we auto-switched to eraser
                if state.stylus_auto_switched_to_eraser {
                    let restored_tool = state.stylus_pre_eraser_tool_override;
                    state.input_state.set_tool_override(restored_tool);
                    state.stylus_auto_switched_to_eraser = false;
                    state.stylus_pre_eraser_tool_override = None;
                    info!(
                        "Restored previous tool after eraser proximity out: {:?}",
                        restored_tool
                    );
                }

                // Note: We keep the tool type in the map - tools persist across proximity events
            }
            Event::Down { .. } => {
                // A contact a screen modal disowned is not the user pressing —
                // it is the pen that was already down, still reported until it
                // lifts. Swallow it whole rather than declining one branch:
                // falling through queues the tip-down, and the frame commit
                // would start a region (or a stroke) from it one hop later.
                //
                // Protocol-unreachable today, since the latch only clears on a
                // tip-up or proximity-out and a press must follow one of those.
                // It is enforced rather than argued because the surrounding
                // pressure, motion, and release guards all rely on it.
                if state.stylus_contact_retired {
                    return;
                }
                if state.input_state.region_is_active() {
                    if state.stylus_on_toolbar {
                        state.cancel_ocr_for_toolbar_interaction();
                    } else if state.stylus_on_overlay {
                        let (x, y) = state.current_or_pending_stylus_position();
                        state.begin_region_selection(RegionInputSource::Stylus, x, y);
                        return;
                    }
                }
                if state.input_state.eyedropper_is_active() {
                    if state.stylus_on_toolbar {
                        state.cancel_eyedropper();
                    } else if state.stylus_on_overlay {
                        let (x, y) = state.current_or_pending_stylus_position();
                        state.sample_eyedropper(x, y);
                        return;
                    }
                }
                let inline_active = state.inline_toolbars_active() && state.toolbar.is_visible();
                if inline_active {
                    let (sx, sy) = state.current_or_pending_stylus_position();
                    if state.inline_toolbar_press((sx, sy), Some(conn), Some(qh)) {
                        state.stylus_on_toolbar = true;
                        state.set_toolbar_dragging(state.toolbar_dragging());
                        return;
                    }
                }
                if state.stylus_on_toolbar {
                    let (sx, sy) = state.current_or_pending_stylus_position();
                    state.set_current_mouse(sx as i32, sy as i32);
                    if let Some(surface) = state.stylus_surface.as_ref()
                        && let Some((intent, drag)) = state.toolbar.pointer_press(surface, (sx, sy))
                    {
                        state.set_toolbar_dragging(drag);
                        let evt = intent_to_event(intent, state.toolbar.last_snapshot());
                        state.handle_toolbar_event(evt, Some(conn), Some(qh));
                        state.toolbar.mark_dirty();
                        state.input_state.needs_redraw = true;
                        state.refresh_keyboard_interactivity();
                    }
                    return;
                }
                if !state.stylus_on_overlay {
                    return;
                }
                state.queue_stylus_down();
            }
            Event::Up => {
                // Read before the modal branches so the latch is consumed by
                // the tip-up that actually ends the disowned contact, whatever
                // else this Up does.
                let retired_contact = state.take_retired_stylus_contact();
                if state.input_state.region_is_active() {
                    if state.stylus_on_overlay {
                        let (x, y) = state.current_or_pending_stylus_position();
                        // A no-op unless this pen owns the region, so a retired
                        // contact lifting cannot submit one the mouse or a
                        // finger is still drawing.
                        state.finish_region_selection(RegionInputSource::Stylus, x, y);
                    } else {
                        // The tip left the overlay before lifting; there is no
                        // region of ours to submit, so withdraw only our own.
                        state.cancel_region_selection_from(RegionInputSource::Stylus);
                    }
                    return;
                }
                let inline_active = state.inline_toolbars_active() && state.toolbar.is_visible();
                if inline_active && state.stylus_on_toolbar {
                    let (mx, my) = state.current_mouse();
                    state.inline_toolbar_release((mx as f64, my as f64));
                    state.stylus_on_toolbar = false;
                    state.set_toolbar_dragging(false);
                    state.end_toolbar_move_drag();
                    return;
                }
                if state.stylus_on_toolbar {
                    state.finish_toolbar_item_drag(true);
                    state.set_toolbar_dragging(false);
                    state.end_toolbar_move_drag();
                    return;
                }
                if !state.stylus_on_overlay || retired_contact {
                    return;
                }
                state.queue_stylus_up();
            }
            Event::Motion { x, y } => {
                if state.input_state.region_is_active() && state.stylus_on_overlay {
                    state.stylus_last_pos = Some((x, y));
                    state.set_current_mouse(x.round() as i32, y.round() as i32);
                    // Dropped unless this pen owns the region: a pen hovering
                    // over the overlay, or one whose contact was retired, must
                    // not drag somebody else's selection around.
                    state.update_region_selection(RegionInputSource::Stylus, x, y);
                    return;
                }
                if state.input_state.eyedropper_is_active() && state.stylus_on_overlay {
                    state.stylus_last_pos = Some((x, y));
                    state.set_current_mouse(x.round() as i32, y.round() as i32);
                    state.update_eyedropper_hover(x, y);
                    return;
                }
                if state.is_move_dragging()
                    && let Some(kind) = state.active_move_drag_kind()
                {
                    // On toolbar surface: coords are toolbar-local, need conversion
                    // On main surface: coords are already screen-relative
                    if state.stylus_on_toolbar {
                        state.handle_toolbar_move(kind, (x, y));
                    } else {
                        state.handle_toolbar_move_screen(kind, (x, y));
                    }
                    state.toolbar.mark_dirty();
                    state.input_state.needs_redraw = true;
                    state.set_current_mouse(x as i32, y as i32);
                    return;
                }
                let previous_hover_cursor_pos = state.stylus_hover_cursor_pos();
                let inline_active = state.inline_toolbars_active() && state.toolbar.is_visible();
                if state.stylus_on_toolbar {
                    let xf = x;
                    let yf = y;
                    state.stylus_last_pos = Some((xf, yf));
                    if let Some(surface) = state.stylus_surface.as_ref() {
                        let evt = state.toolbar.pointer_motion(surface, (xf, yf));
                        if state.toolbar_dragging() {
                            // Use move_drag_intent if pointer_motion didn't return an intent
                            // This allows dragging to continue when stylus moves outside hit region
                            let intent = evt.or_else(|| state.move_drag_intent(xf, yf));
                            if let Some(intent) = intent {
                                let evt = intent_to_event(intent, state.toolbar.last_snapshot());
                                state.handle_toolbar_event(evt, Some(conn), Some(qh));
                            }
                        } else {
                            state.toolbar.mark_dirty();
                        }
                        state.input_state.needs_redraw = true;
                        state.refresh_keyboard_interactivity();
                    }
                    state.set_current_mouse(x as i32, y as i32);
                    return;
                }
                if inline_active {
                    // Toolbar hit-testing is immediate, even though drawing samples are
                    // committed on tablet frames. Keep the stylus cache current so a
                    // following Down uses the same toolbar-local position.
                    state.stylus_last_pos = Some((x, y));
                    if state.inline_toolbar_motion((x, y)) {
                        state.commit_pending_stylus_frame();
                        // Flushing pending overlay state can restore an older drawing
                        // position; toolbar Down handling needs the latest hover point.
                        state.stylus_last_pos = Some((x, y));
                        state.stylus_on_toolbar = true;
                        state.mark_stylus_hover_cursor_dirty(previous_hover_cursor_pos, None);
                        return;
                    } else {
                        state.stylus_on_toolbar = false;
                    }
                }
                if !state.stylus_on_overlay {
                    return;
                }
                state.queue_stylus_motion(x, y);
            }
            Event::Pressure { pressure } => {
                if drop_stylus_pressure(
                    state.stylus_on_overlay,
                    state.stylus_contact_retired,
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
                state.stylus_tool_types.insert(tool_id, physical_type);

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
