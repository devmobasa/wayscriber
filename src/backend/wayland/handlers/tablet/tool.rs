use log::{debug, info};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::ZwpTabletToolV2;

use crate::backend::wayland::toolbar_intent::intent_to_event;
use crate::input::{MouseButton, Tool};

use crate::backend::wayland::state::WaylandState;

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
                state.set_toolbar_dragging(false);
                state.stylus_tip_down = false;
                state.stylus_base_thickness = Some(state.input_state.current_thickness);
                state.stylus_pressure_thickness = None;
                state.stylus_last_pos = None;

                // Auto-switch to eraser if physical tool is eraser (and config enables it)
                if state.config.tablet.auto_eraser_switch {
                    if let Some(tool_type) = tool_type {
                        if tool_type.is_eraser() {
                            // Only auto-switch if not already on eraser
                            if state.input_state.active_tool() != Tool::Eraser {
                                // Save the current tool override before switching
                                state.stylus_pre_eraser_tool_override =
                                    state.input_state.tool_override();
                                state.input_state.set_tool_override(Some(Tool::Eraser));
                                state.stylus_auto_switched_to_eraser = true;
                                info!(
                                    "Auto-switched to eraser (physical eraser detected), saved previous: {:?}",
                                    state.stylus_pre_eraser_tool_override
                                );
                            }
                        }
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
                state.stylus_tip_down = false;
                state.stylus_on_overlay = false;
                state.stylus_on_toolbar = false;
                state.set_toolbar_dragging(false);
                state.end_toolbar_move_drag();
                if let Some(surf) = state.stylus_surface.take()
                    && state.toolbar.is_toolbar_surface(&surf)
                {
                    state.toolbar.pointer_leave(&surf);
                    state.toolbar.mark_dirty();
                    state.input_state.needs_redraw = true;
                }
                state.stylus_pressure_thickness = None;
                state.stylus_last_pos = None;

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
                    if let Some(surface) = state.stylus_surface.as_ref()
                        && let Some((intent, drag)) = state.toolbar.pointer_press(surface, (sx, sy))
                    {
                        state.set_toolbar_dragging(drag);
                        let evt = intent_to_event(intent, state.toolbar.last_snapshot());
                        state.handle_toolbar_event(evt);
                        state.toolbar.mark_dirty();
                        state.input_state.needs_redraw = true;
                        state.refresh_keyboard_interactivity();
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
                let (wx, wy) = state.zoomed_world_coords(
                    state.current_mouse().0 as f64,
                    state.current_mouse().1 as f64,
                );
                state.input_state.on_mouse_press(MouseButton::Left, wx, wy);
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
                let (wx, wy) = state.zoomed_world_coords(
                    state.current_mouse().0 as f64,
                    state.current_mouse().1 as f64,
                );
                state
                    .input_state
                    .on_mouse_release(MouseButton::Left, wx, wy);
                state.input_state.needs_redraw = true;
            }
            Event::Motion { x, y } => {
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
                let (wx, wy) = state.zoomed_world_coords(
                    state.current_mouse().0 as f64,
                    state.current_mouse().1 as f64,
                );
                state.input_state.on_mouse_motion(wx, wy);
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
                if pressure > 0 {
                    use crate::input::tablet::apply_pressure_to_state;
                    apply_pressure_to_state(p01, &mut state.input_state, state.tablet_settings);
                    state.stylus_pressure_thickness = Some(state.input_state.current_thickness);
                    state.record_stylus_peak(state.input_state.current_thickness);
                } else {
                    // Ignore zero-pressure while tip is down to avoid flickers
                    debug!("Stylus pressure reported 0; deferring to peak/base");
                }
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
            Event::Frame { .. } => {
                debug!("Tablet frame event");
            }
            other => {
                debug!("Unhandled tablet tool event: {:?}", other);
            }
        }
    }
}
