use log::debug;
use smithay_client_toolkit::seat::pointer::{AxisScroll, PointerEvent};
use std::time::Instant;
use wayland_client::protocol::wl_pointer;

use super::*;
use crate::input::Tool;
use crate::input::state::{InputState, SpotlightWheelClaim, SpotlightWheelOutcome};

/// Quiet period after which a discrete wheel burst over a loupe is finished.
///
/// Long enough that a pause mid-scroll does not split one adjustment in two,
/// short enough that a later visit is separately undoable.
pub(super) const SPOTLIGHT_WHEEL_IDLE: std::time::Duration = std::time::Duration::from_millis(600);

fn scroll_direction(vertical: AxisScroll) -> i32 {
    if vertical.value120 != 0 {
        vertical.value120.signum()
    } else if vertical.discrete != 0 {
        vertical.discrete
    } else if vertical.absolute.abs() > 0.1 {
        if vertical.absolute > 0.0 { 1 } else { -1 }
    } else {
        0
    }
}

fn finalize_spotlight_wheel_if_axis_stopped(
    input_state: &mut InputState,
    spotlight_wheel_idle_deadline: &mut Option<Instant>,
    stop: bool,
) {
    if stop {
        input_state.flush_spotlight_magnification_gesture();
        *spotlight_wheel_idle_deadline = None;
    }
}

/// Applies the Spotlight-owned part of an axis frame.
///
/// Stop finalization is deliberately owned by the outer axis handler after
/// routing: SCTK may aggregate a final movement and stop in one frame.
fn try_handle_spotlight_axis(
    input_state: &mut InputState,
    spotlight_wheel_idle_deadline: &mut Option<Instant>,
    canvas_position: (i32, i32),
    vertical: AxisScroll,
    source: Option<wl_pointer::AxisSource>,
    now: Instant,
) -> bool {
    let direction = scroll_direction(vertical);
    if direction == 0 {
        return false;
    }
    let (canvas_x, canvas_y) = canvas_position;
    let claim = input_state.claim_spotlight_wheel_axis_at(
        canvas_x,
        canvas_y,
        vertical.value120,
        vertical.discrete,
        vertical.absolute,
    );
    match claim {
        SpotlightWheelClaim::NotOverLoupe => return false,
        SpotlightWheelClaim::Locked => {
            debug!("Spotlight wheel at ({canvas_x}, {canvas_y}): locked");
        }
        SpotlightWheelClaim::Adjustable(steps) => {
            if steps != 0 {
                let outcome =
                    input_state.nudge_spotlight_magnification_at(canvas_x, canvas_y, steps);
                debug_assert_ne!(outcome, SpotlightWheelOutcome::NotOverLoupe);
                debug!("Spotlight wheel at ({canvas_x}, {canvas_y}): {outcome:?}");
            }
        }
    }
    *spotlight_wheel_idle_deadline = if input_state.has_pending_spotlight_wheel_axis_sequence()
        && !matches!(source, Some(wl_pointer::AxisSource::Finger))
    {
        Some(now + SPOTLIGHT_WHEEL_IDLE)
    } else {
        None
    };
    true
}

impl WaylandState {
    pub(super) fn handle_pointer_axis(
        &mut self,
        event: &PointerEvent,
        on_toolbar: bool,
        vertical: AxisScroll,
        source: Option<wl_pointer::AxisSource>,
    ) {
        let stopped = vertical.stop;
        self.handle_pointer_axis_inner(event, on_toolbar, vertical, source);
        finalize_spotlight_wheel_if_axis_stopped(
            &mut self.input_state,
            &mut self.spotlight_wheel_idle_deadline,
            stopped,
        );
    }

    fn handle_pointer_axis_inner(
        &mut self,
        event: &PointerEvent,
        on_toolbar: bool,
        vertical: AxisScroll,
        source: Option<wl_pointer::AxisSource>,
    ) {
        let scroll_direction = scroll_direction(vertical);
        // Report the physical wheel tick to the input HUD before any surface
        // claims it. Positive axis values scroll the content down, so a
        // negative direction is the "scroll up" the user performed.
        if scroll_direction != 0 {
            self.input_state
                .note_input_hud_scroll(scroll_direction < 0, self.input_state.modifiers);
        }
        // Handle radial menu scroll-to-thickness
        if self.input_state.is_radial_menu_open() {
            if scroll_direction != 0 {
                let delta = if scroll_direction > 0 { -1.0 } else { 1.0 };
                self.adjust_active_tool_thickness(delta, true);
            }
            return;
        }

        // Handle command palette scrolling (display-row space; selection is
        // kept inside the window, skipping group headers).
        if self.input_state.command_palette_open {
            if scroll_direction != 0 {
                self.input_state
                    .command_palette_wheel_scroll(scroll_direction);
            }
            return;
        }

        if self.input_state.show_help {
            if scroll_direction != 0 {
                let delta = if scroll_direction > 0 { 1.0 } else { -1.0 };
                let scroll_step = 48.0;
                let max_scroll = self.input_state.help_overlay_scroll_max;
                let mut next = self.input_state.help_overlay_scroll + delta * scroll_step;
                if max_scroll > 0.0 {
                    next = next.clamp(0.0, max_scroll);
                } else {
                    next = next.max(0.0);
                }
                if (next - self.input_state.help_overlay_scroll).abs() > f64::EPSILON {
                    self.input_state.help_overlay_scroll = next;
                    self.input_state.dirty_tracker.mark_full();
                    self.input_state.needs_redraw = true;
                }
            }
            return;
        }
        if try_handle_board_picker_page_panel_axis(
            &mut self.input_state,
            event.position,
            scroll_direction,
        ) {
            return;
        }
        if on_toolbar || self.pointer_over_toolbar() {
            if scroll_direction != 0 && self.wheel_over_top_toolbar(&event.surface, event.position)
            {
                // With a Canvas/Session/Settings popover open, the wheel scrolls
                // its capped viewport; otherwise a top-strip wheel stays a
                // no-op (it never falls through to thickness/zoom).
                self.scroll_top_popover_by_wheel(scroll_direction);
            }
            return;
        }
        if self.input_state.modifiers.ctrl && self.input_state.modifiers.alt {
            if scroll_direction != 0 {
                let zoom_in = scroll_direction < 0;
                self.handle_zoom_scroll(zoom_in, event.position.0, event.position.1);
            }
            return;
        }

        // A wheel over a loupe adjusts that loupe, before the wheel's usual
        // meaning applies. It is the cheapest route to the property: no
        // selection, no toolbar trip, and the magnification follows the ticks
        // live. Off a loupe, nothing here claims the event.
        let canvas_position = self.input_state.canvas_pointer_position();
        if try_handle_spotlight_axis(
            &mut self.input_state,
            &mut self.spotlight_wheel_idle_deadline,
            canvas_position,
            vertical,
            source,
            Instant::now(),
        ) {
            return;
        }

        match scroll_direction.cmp(&0) {
            std::cmp::Ordering::Greater if self.input_state.modifiers.shift => {
                self.input_state.adjust_font_size(-2.0);
                debug!(
                    "Font size decreased: {:.1}px",
                    self.input_state.current_font_size
                );
            }
            std::cmp::Ordering::Less if self.input_state.modifiers.shift => {
                self.input_state.adjust_font_size(2.0);
                debug!(
                    "Font size increased: {:.1}px",
                    self.input_state.current_font_size
                );
            }
            std::cmp::Ordering::Greater | std::cmp::Ordering::Less => {
                let delta = if scroll_direction > 0 { -1.0 } else { 1.0 };
                self.adjust_active_tool_thickness(delta, false);
            }
            std::cmp::Ordering::Equal => {}
        }
    }

    fn adjust_active_tool_thickness(&mut self, delta: f64, radial_menu_path: bool) {
        let eraser_active = self.input_state.active_tool() == Tool::Eraser;
        #[cfg(feature = "tablet-input")]
        let prev_thickness = self.input_state.current_thickness;

        let changed = if radial_menu_path {
            self.input_state.radial_menu_adjust_thickness(delta)
        } else if self.input_state.nudge_thickness_for_active_tool(delta) {
            self.input_state.needs_redraw = true;
            true
        } else {
            false
        };

        if changed {
            if eraser_active {
                debug!(
                    "Eraser size adjusted: {:.0}px",
                    self.input_state.eraser_size
                );
            } else {
                debug!(
                    "Thickness adjusted: {:.0}px",
                    self.input_state.current_thickness
                );
            }
        }

        #[cfg(feature = "tablet-input")]
        if !eraser_active
            && (self.input_state.current_thickness - prev_thickness).abs() > f64::EPSILON
        {
            self.stylus_base_thickness = Some(self.input_state.current_thickness);
            if self.stylus_tip_down {
                self.stylus_pressure_thickness = Some(self.input_state.current_thickness);
                self.record_stylus_peak(self.input_state.current_thickness);
            } else {
                self.stylus_pressure_thickness = None;
                self.stylus_peak_thickness = None;
            }
        }
    }
}

fn try_handle_board_picker_page_panel_axis(
    input_state: &mut InputState,
    position: (f64, f64),
    scroll_direction: i32,
) -> bool {
    if !input_state.is_board_picker_open() || scroll_direction == 0 {
        return false;
    }
    let x = position.0.round() as i32;
    let y = position.1.round() as i32;
    if !input_state.board_picker_page_panel_content_at(x, y) {
        return false;
    }
    let delta = if scroll_direction > 0 { 1 } else { -1 };
    let _ = input_state.board_picker_scroll_page_panel_rows(delta);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Action;
    use crate::draw::{Frame, Shape};
    use crate::input::state::{BoardPickerFocus, test_support::make_test_input_state};
    use std::time::Duration;

    fn update_picker_layout(input_state: &mut InputState) {
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, 1280, 720).expect("image surface");
        let ctx = cairo::Context::new(&surface).expect("cairo context");
        input_state.update_board_picker_layout(&ctx, 1280, 720);
    }

    fn set_board_page_count(input_state: &mut InputState, board_index: usize, page_count: usize) {
        let pages = input_state.boards.board_states_mut()[board_index]
            .pages
            .pages_mut();
        pages.clear();
        pages.extend((0..page_count.max(1)).map(|_| Frame::new()));
    }

    #[test]
    fn board_picker_page_panel_axis_consumes_before_thickness_changes() {
        let mut input_state = make_test_input_state();
        input_state.open_board_picker();
        let board_index = input_state
            .board_picker_page_panel_board_index()
            .expect("page panel board index");
        set_board_page_count(&mut input_state, board_index, 80);
        update_picker_layout(&mut input_state);

        let layout = *input_state.board_picker_layout().expect("layout");
        let position = (layout.page_viewport_x + 1.0, layout.page_viewport_y + 1.0);
        let thickness = input_state.current_thickness;
        input_state.board_picker_set_focus(BoardPickerFocus::PagePanel);

        assert!(try_handle_board_picker_page_panel_axis(
            &mut input_state,
            position,
            1,
        ));
        assert_eq!(input_state.current_thickness, thickness);
        update_picker_layout(&mut input_state);

        let layout = *input_state.board_picker_layout().expect("layout");
        assert_eq!(layout.page_scroll_row, 1);
    }

    #[test]
    fn value120_keeps_shared_axis_routing_in_direction_space() {
        assert_eq!(
            scroll_direction(AxisScroll {
                value120: -240,
                ..AxisScroll::default()
            }),
            -1
        );
    }

    #[test]
    fn a_final_axis_delta_and_stop_complete_one_spotlight_gesture() {
        let mut input_state = make_test_input_state();
        let shape_id = input_state
            .boards
            .active_frame_mut()
            .add_shape(Shape::Spotlight {
                cx: 200,
                cy: 200,
                rx: 60,
                ry: 40,
                magnification: 2.0,
            });
        let mut deadline = None;
        let now = Instant::now();

        assert!(try_handle_spotlight_axis(
            &mut input_state,
            &mut deadline,
            (200, 200),
            AxisScroll {
                absolute: -1.0,
                discrete: 0,
                stop: false,
                ..AxisScroll::default()
            },
            Some(wl_pointer::AxisSource::Wheel),
            now,
        ));
        assert!(deadline.is_some());
        assert!(try_handle_spotlight_axis(
            &mut input_state,
            &mut deadline,
            (200, 200),
            AxisScroll {
                absolute: -1.0,
                discrete: 0,
                stop: true,
                ..AxisScroll::default()
            },
            Some(wl_pointer::AxisSource::Wheel),
            now + Duration::from_millis(10),
        ));
        finalize_spotlight_wheel_if_axis_stopped(&mut input_state, &mut deadline, true);
        assert!(
            deadline.is_none(),
            "axis stop owns the final deadline clear"
        );

        input_state.handle_action(Action::Undo);
        let magnification = match input_state
            .boards
            .active_frame()
            .shape(shape_id)
            .expect("spotlight")
            .shape
        {
            Shape::Spotlight { magnification, .. } => magnification,
            ref other => panic!("expected a spotlight, got {other:?}"),
        };
        assert_eq!(
            magnification, 2.0,
            "the final movement must be part of the gesture completed by stop"
        );
    }

    #[test]
    fn a_coalesced_value120_frame_applies_every_logical_step() {
        let mut input_state = make_test_input_state();
        let shape_id = input_state
            .boards
            .active_frame_mut()
            .add_shape(Shape::Spotlight {
                cx: 200,
                cy: 200,
                rx: 60,
                ry: 40,
                magnification: 2.0,
            });
        let mut deadline = None;

        assert!(try_handle_spotlight_axis(
            &mut input_state,
            &mut deadline,
            (200, 200),
            AxisScroll {
                value120: -240,
                stop: true,
                ..AxisScroll::default()
            },
            Some(wl_pointer::AxisSource::Wheel),
            Instant::now(),
        ));
        finalize_spotlight_wheel_if_axis_stopped(&mut input_state, &mut deadline, true);

        let Shape::Spotlight { magnification, .. } = input_state
            .boards
            .active_frame()
            .shape(shape_id)
            .expect("spotlight")
            .shape
        else {
            panic!("expected a spotlight");
        };
        assert_eq!(magnification, 2.5);

        input_state.handle_action(Action::Undo);
        let Shape::Spotlight { magnification, .. } = input_state
            .boards
            .active_frame()
            .shape(shape_id)
            .expect("spotlight")
            .shape
        else {
            panic!("expected a spotlight");
        };
        assert_eq!(magnification, 2.0);
    }

    #[test]
    fn partial_value120_frames_accumulate_before_applying_a_logical_step() {
        let mut input_state = make_test_input_state();
        let shape_id = input_state
            .boards
            .active_frame_mut()
            .add_shape(Shape::Spotlight {
                cx: 200,
                cy: 200,
                rx: 60,
                ry: 40,
                magnification: 2.0,
            });
        let mut deadline = None;
        let now = Instant::now();
        let partial_tick = AxisScroll {
            value120: -60,
            ..AxisScroll::default()
        };

        assert!(try_handle_spotlight_axis(
            &mut input_state,
            &mut deadline,
            (200, 200),
            partial_tick,
            Some(wl_pointer::AxisSource::Wheel),
            now,
        ));
        assert!(deadline.is_some(), "the partial unit must remain live");
        let Shape::Spotlight { magnification, .. } = input_state
            .boards
            .active_frame()
            .shape(shape_id)
            .expect("spotlight")
            .shape
        else {
            panic!("expected a spotlight");
        };
        assert_eq!(magnification, 2.0, "a partial unit must not move the loupe");

        assert!(try_handle_spotlight_axis(
            &mut input_state,
            &mut deadline,
            (200, 200),
            AxisScroll {
                stop: true,
                ..partial_tick
            },
            Some(wl_pointer::AxisSource::Wheel),
            now + Duration::from_millis(10),
        ));
        finalize_spotlight_wheel_if_axis_stopped(&mut input_state, &mut deadline, true);
        let Shape::Spotlight { magnification, .. } = input_state
            .boards
            .active_frame()
            .shape(shape_id)
            .expect("spotlight")
            .shape
        else {
            panic!("expected a spotlight");
        };
        assert_eq!(magnification, 2.25);

        input_state.handle_action(Action::Undo);
        let Shape::Spotlight { magnification, .. } = input_state
            .boards
            .active_frame()
            .shape(shape_id)
            .expect("spotlight")
            .shape
        else {
            panic!("expected a spotlight");
        };
        assert_eq!(magnification, 2.0);
    }

    #[test]
    fn a_finger_axis_pause_longer_than_the_wheel_timeout_stays_one_gesture() {
        let mut input_state = make_test_input_state();
        let shape_id = input_state
            .boards
            .active_frame_mut()
            .add_shape(Shape::Spotlight {
                cx: 200,
                cy: 200,
                rx: 60,
                ry: 40,
                magnification: 2.0,
            });
        let mut deadline = None;
        let now = Instant::now();
        let finger_delta = AxisScroll {
            absolute: -1.0,
            discrete: 0,
            stop: false,
            ..AxisScroll::default()
        };

        assert!(try_handle_spotlight_axis(
            &mut input_state,
            &mut deadline,
            (200, 200),
            finger_delta,
            Some(wl_pointer::AxisSource::Finger),
            now,
        ));
        assert!(deadline.is_none(), "axis_stop owns finger completion");

        assert!(try_handle_spotlight_axis(
            &mut input_state,
            &mut deadline,
            (200, 200),
            finger_delta,
            Some(wl_pointer::AxisSource::Finger),
            now + SPOTLIGHT_WHEEL_IDLE + Duration::from_millis(1),
        ));
        finalize_spotlight_wheel_if_axis_stopped(&mut input_state, &mut deadline, true);

        input_state.handle_action(Action::Undo);
        let Shape::Spotlight { magnification, .. } = input_state
            .boards
            .active_frame()
            .shape(shape_id)
            .expect("spotlight")
            .shape
        else {
            panic!("expected a spotlight");
        };
        assert_eq!(magnification, 2.0);
    }
}
