//! Wayland tablet/stylus protocol handling (zwp_tablet_v2).

use log::{debug, info};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_manager_v2::ZwpTabletManagerV2, zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2,
    zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2, zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2,
    zwp_tablet_pad_v2::ZwpTabletPadV2, zwp_tablet_seat_v2::ZwpTabletSeatV2,
    zwp_tablet_tool_v2::ZwpTabletToolV2, zwp_tablet_v2::ZwpTabletV2,
};

use crate::backend::wayland::state::MoveDragKind;
use crate::backend::wayland::toolbar_intent::intent_to_event;
use crate::input::MouseButton;

use super::super::state::WaylandState;

impl Dispatch<ZwpTabletManagerV2, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTabletManagerV2,
        _event: <ZwpTabletManagerV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // No events
    }
}

impl Dispatch<ZwpTabletSeatV2, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpTabletSeatV2,
        event: <ZwpTabletSeatV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::Event;
        match event {
            Event::TabletAdded { id } => {
                info!("🖊️  TABLET DEVICE DETECTED");
                state.tablets.push(id);
                if !state.tablet_found_logged {
                    state.tablet_found_logged = true;
                    info!("TABLET FOUND - Total devices: {}", state.tablets.len());
                }
            }
            Event::ToolAdded { id } => {
                info!("🖊️  TABLET TOOL DETECTED (pen/stylus)");
                state.tablet_tools.push(id);
                if !state.tablet_found_logged {
                    state.tablet_found_logged = true;
                    info!("TABLET FOUND - Total tools: {}", state.tablet_tools.len());
                }
            }
            Event::PadAdded { id } => {
                info!("🖊️  TABLET PAD DETECTED");
                state.tablet_pads.push(id);
            }
            _ => {}
        }
    }
    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::{
            EVT_PAD_ADDED_OPCODE, EVT_TABLET_ADDED_OPCODE, EVT_TOOL_ADDED_OPCODE,
        };
        match opcode {
            EVT_TABLET_ADDED_OPCODE => qhandle.make_data::<ZwpTabletV2, _>(()),
            EVT_TOOL_ADDED_OPCODE => qhandle.make_data::<ZwpTabletToolV2, _>(()),
            EVT_PAD_ADDED_OPCODE => qhandle.make_data::<ZwpTabletPadV2, _>(()),
            _ => panic!(
                "Missing tablet seat child specialization for opcode {}",
                opcode
            ),
        }
    }
}

impl Dispatch<ZwpTabletV2, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTabletV2,
        _event: <ZwpTabletV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Descriptive events are ignored.
    }
}

impl Dispatch<ZwpTabletPadV2, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpTabletPadV2,
        event: <ZwpTabletPadV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_v2::Event;
        match event {
            Event::Group { pad_group } => {
                debug!("Tablet pad group announced: {:?}", pad_group.id());
                state.tablet_pad_groups.push(pad_group);
            }
            Event::Path { path } => {
                debug!("Tablet pad path: {}", path);
            }
            Event::Buttons { buttons } => {
                debug!("Tablet pad button count: {}", buttons);
            }
            Event::Done => {
                debug!("Tablet pad description complete");
            }
            Event::Button {
                time,
                button,
                state,
            } => {
                debug!(
                    "Tablet pad button event: index {} -> {:?} @ {}",
                    button, state, time
                );
            }
            Event::Enter {
                serial,
                tablet,
                surface,
            } => {
                debug!(
                    "Tablet pad entered surface {:?} (tablet {:?}) serial {}",
                    surface.id(),
                    tablet.id(),
                    serial
                );
            }
            Event::Leave { serial, surface } => {
                debug!(
                    "Tablet pad left surface {:?} serial {}",
                    surface.id(),
                    serial
                );
            }
            Event::Removed => {
                info!("Tablet pad removed");
                state.tablet_pads.clear();
                state.tablet_pad_groups.clear();
                state.tablet_pad_rings.clear();
                state.tablet_pad_strips.clear();
            }
            _ => {}
        }
    }
    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_v2::EVT_GROUP_OPCODE;
        match opcode {
            EVT_GROUP_OPCODE => qhandle.make_data::<ZwpTabletPadGroupV2, _>(()),
            _ => panic!(
                "Missing tablet pad child specialization for opcode {}",
                opcode
            ),
        }
    }
}

impl Dispatch<ZwpTabletPadGroupV2, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpTabletPadGroupV2,
        event: <ZwpTabletPadGroupV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_group_v2::Event;
        match event {
            Event::Buttons { buttons } => {
                debug!("Tablet pad group buttons: {:?}", buttons);
            }
            Event::Ring { ring } => {
                debug!("Tablet pad ring announced: {:?}", ring.id());
                state.tablet_pad_rings.push(ring);
            }
            Event::Strip { strip } => {
                debug!("Tablet pad strip announced: {:?}", strip.id());
                state.tablet_pad_strips.push(strip);
            }
            Event::Modes { modes } => {
                debug!("Tablet pad group modes: {}", modes);
            }
            Event::Done => {
                debug!("Tablet pad group description complete");
            }
            Event::ModeSwitch { time, serial, mode } => {
                debug!(
                    "Tablet pad group mode switch -> mode {} (serial {}, time {})",
                    mode, serial, time
                );
            }
            _ => {}
        }
    }
    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_group_v2::{
            EVT_RING_OPCODE, EVT_STRIP_OPCODE,
        };
        match opcode {
            EVT_RING_OPCODE => qhandle.make_data::<ZwpTabletPadRingV2, _>(()),
            EVT_STRIP_OPCODE => qhandle.make_data::<ZwpTabletPadStripV2, _>(()),
            _ => panic!(
                "Missing tablet pad group child specialization for opcode {}",
                opcode
            ),
        }
    }
}

impl Dispatch<ZwpTabletPadRingV2, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTabletPadRingV2,
        event: <ZwpTabletPadRingV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_ring_v2::Event;
        match event {
            Event::Source { source } => {
                debug!("Tablet pad ring source: {:?}", source);
            }
            Event::Angle { degrees } => {
                debug!("Tablet pad ring angle: {:?}", degrees);
            }
            Event::Stop => {
                debug!("Tablet pad ring interaction stopped");
            }
            Event::Frame { time } => {
                debug!("Tablet pad ring frame @ {}", time);
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwpTabletPadStripV2, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTabletPadStripV2,
        event: <ZwpTabletPadStripV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_strip_v2::Event;
        match event {
            Event::Source { source } => {
                debug!("Tablet pad strip source: {:?}", source);
            }
            Event::Position { position } => {
                debug!("Tablet pad strip position: {}", position);
            }
            Event::Stop => {
                debug!("Tablet pad strip interaction stopped");
            }
            Event::Frame { time } => {
                debug!("Tablet pad strip frame @ {}", time);
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwpTabletToolV2, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpTabletToolV2,
        event: <ZwpTabletToolV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::Event;
        match event {
            Event::ProximityIn { surface, .. } => {
                let on_overlay = state
                    .surface
                    .wl_surface()
                    .is_some_and(|s| s.id() == surface.id());
                let on_toolbar = state.toolbar.is_toolbar_surface(&surface);
                state.stylus_surface = Some(surface.clone());
                state.stylus_on_overlay = on_overlay;
                state.stylus_on_toolbar = on_toolbar;
                state.set_toolbar_dragging(false);
                state.stylus_tip_down = false;
                state.stylus_base_thickness = Some(state.input_state.current_thickness);
                state.stylus_pressure_thickness = None;
                state.stylus_last_pos = None;
                if on_overlay {
                    info!("✏️  Stylus ENTERED overlay surface");
                } else if state.toolbar.is_toolbar_surface(&surface) {
                    debug!("Stylus entered toolbar surface");
                } else {
                    debug!("Tablet proximity in on non-overlay surface");
                }
            }
            Event::ProximityOut => {
                info!("✏️  Stylus LEFT surface");
                state.stylus_tip_down = false;
                state.stylus_on_overlay = false;
                state.stylus_on_toolbar = false;
                state.set_toolbar_dragging(false);
                state.end_toolbar_move_drag();
                if let Some(surf) = state.stylus_surface.take() {
                    if state.toolbar.is_toolbar_surface(&surf) {
                        state.toolbar.pointer_leave(&surf);
                        state.toolbar.mark_dirty();
                        state.input_state.needs_redraw = true;
                    }
                }
                state.stylus_pressure_thickness = None;
                state.stylus_last_pos = None;
            }
            Event::Down { .. } => {
                let inline_active = state.inline_toolbars_active() && state.toolbar.is_visible();
                if inline_active {
                    let (sx, sy) = state.stylus_last_pos.unwrap_or_else(|| {
                        let (mx, my) = state.current_mouse();
                        (mx as f64, my as f64)
                    });
                    if state.inline_toolbar_press((sx, sy)) {
                        state.stylus_on_toolbar = true;
                        state.set_toolbar_dragging(state.toolbar_dragging());
                        return;
                    }
                }
                if state.stylus_on_toolbar {
                    let (sx, sy) = state.stylus_last_pos.unwrap_or_else(|| {
                        let (mx, my) = state.current_mouse();
                        (mx as f64, my as f64)
                    });
                    state.set_current_mouse(sx as i32, sy as i32);
                    if let Some(surface) = state.stylus_surface.as_ref() {
                        if let Some((intent, drag)) = state.toolbar.pointer_press(surface, (sx, sy))
                        {
                            state.set_toolbar_dragging(drag);
                            let evt = intent_to_event(intent, state.toolbar.last_snapshot());
                            state.handle_toolbar_event(evt);
                            state.toolbar.mark_dirty();
                            state.input_state.needs_redraw = true;
                            state.refresh_keyboard_interactivity();
                        }
                    }
                    return;
                }
                if !state.stylus_on_overlay {
                    return;
                }
                state.stylus_tip_down = true;
                state.stylus_base_thickness = Some(state.input_state.current_thickness);
                state.stylus_pressure_thickness = Some(state.input_state.current_thickness);
                state.record_stylus_peak(state.input_state.current_thickness);
                info!(
                    "✏️  Stylus DOWN at ({}, {})",
                    state.current_mouse().0,
                    state.current_mouse().1
                );
                state.input_state.on_mouse_press(
                    MouseButton::Left,
                    state.current_mouse().0,
                    state.current_mouse().1,
                );
                state.input_state.needs_redraw = true;
            }
            Event::Up => {
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
                    state.set_toolbar_dragging(false);
                    state.end_toolbar_move_drag();
                    return;
                }
                if !state.stylus_on_overlay {
                    return;
                }
                state.stylus_tip_down = false;
                let final_thick = state
                    .stylus_peak_thickness
                    .or(state.stylus_pressure_thickness)
                    .or(state.stylus_base_thickness);
                if let Some(thick) = final_thick {
                    // Keep the pressure-adjusted (peak) thickness for subsequent strokes
                    state.input_state.current_thickness = thick;
                    state.stylus_base_thickness = Some(thick);
                }
                state.stylus_pressure_thickness = None;
                state.stylus_peak_thickness = None;
                info!(
                    "✏️  Stylus UP at ({}, {})",
                    state.current_mouse().0,
                    state.current_mouse().1
                );
                state.input_state.on_mouse_release(
                    MouseButton::Left,
                    state.current_mouse().0,
                    state.current_mouse().1,
                );
                state.input_state.needs_redraw = true;
            }
            Event::Motion { x, y } => {
                if state.is_move_dragging() {
                    if let Some(kind) = state.active_move_drag_kind() {
                        let coord = match kind {
                            MoveDragKind::Top => x,
                            MoveDragKind::Side => y,
                        };
                        state.handle_toolbar_move(kind, coord);
                        state.toolbar.mark_dirty();
                        state.input_state.needs_redraw = true;
                        state.set_current_mouse(x as i32, y as i32);
                        return;
                    }
                }
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
                                state.handle_toolbar_event(evt);
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
                    state.stylus_last_pos = Some((x, y));
                    if state.inline_toolbar_motion((x, y)) {
                        state.stylus_on_toolbar = true;
                        return;
                    } else {
                        state.stylus_on_toolbar = false;
                    }
                }
                if !state.stylus_on_overlay {
                    return;
                }
                state.set_current_mouse(x as i32, y as i32);
                let xf = x;
                let yf = y;
                state.stylus_last_pos = Some((xf, yf));
                let (mx, my) = state.current_mouse();
                state.input_state.on_mouse_motion(mx, my);
                if state.stylus_tip_down {
                    state.stylus_pressure_thickness = Some(state.input_state.current_thickness);
                    state.record_stylus_peak(state.input_state.current_thickness);
                }
            }
            Event::Pressure { pressure } => {
                if !state.stylus_on_overlay {
                    return;
                }
                let p01 = (pressure as f64) / 65535.0;
                debug!("Stylus pressure: {} (raw: {}/65535)", p01, pressure);
                if pressure > 0 {
                    use crate::input::tablet::apply_pressure_to_state;
                    apply_pressure_to_state(p01, &mut state.input_state, state.tablet_settings);
                    // Keep thickness monotonic during a stroke to avoid dips near lift.
                    let current = state.input_state.current_thickness;
                    let peak = state.stylus_peak_thickness.unwrap_or(current);
                    if current < peak {
                        state.input_state.current_thickness = peak;
                    }
                    state.stylus_pressure_thickness = Some(state.input_state.current_thickness);
                    state.record_stylus_peak(state.input_state.current_thickness);
                } else {
                    // Ignore zero-pressure while tip is down to avoid flickers
                    debug!("Stylus pressure reported 0; deferring to peak/base");
                }
            }
            Event::Frame { .. } => {
                debug!("Tablet frame event");
            }
            other => {
                debug!("Unhandled tablet tool event: {:?}", other);
            }
        }
    }
}
