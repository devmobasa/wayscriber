use std::fs;
use std::time::{Duration, Instant};

use cairo::{Format, ImageSurface};
use smithay_client_toolkit::compositor::CompositorHandler;
use smithay_client_toolkit::seat::pointer::{
    BTN_LEFT, PointerEvent, PointerEventKind, PointerHandler,
};
use smithay_client_toolkit::seat::touch::TouchHandler;
use smithay_client_toolkit::shell::wlr_layer::{LayerShellHandler, SurfaceKind};
use wayland_client::Proxy;

use crate::backend::wayland::handlers::keyboard::KeyPressSource;
use crate::backend::wayland::handlers::test_support::HandlerFixture;
use crate::backend::wayland::session::{SessionCommand, SessionState};
use crate::backend::wayland::state::OverlaySuppression;
use crate::config::Config;
use crate::config::{
    RenderColorMappingConfig, RenderProfileConfig, RenderProfileExportMode, RenderProfilesConfig,
};
use crate::draw::{Color, Shape};
use crate::input::state::{InputTextResources, OutputFocusAction};
use crate::input::{Key, MouseButton};
use crate::render_profiles::RenderProfileSet;
use crate::session::SessionOptions;
use crate::test_temp;
use crate::ui::toolbar::ToolbarEvent;
use wayland_client::protocol::wl_pointer::WlPointer;
use wayland_client::protocol::wl_touch::WlTouch;

use super::{LightOutputOverlay, PASSIVE_BUFFER_FRAMES};

fn fixture() -> (HandlerFixture, test_temp::TempDir) {
    let mut config = Config::default();
    config.ui.keep_light_overlays_on_all_outputs = true;
    config.ui.show_onboarding_hints = false;
    config.session.persist_transparent = true;
    let mut fixture = HandlerFixture::with_layer_outputs(config, 2);
    let outputs: Vec<_> = fixture.state.protocol.output().outputs().collect();
    let first = fixture.state.surface.current_output().unwrap();
    fixture.complete_output_metadata_for(first.clone(), "A panel");
    let other = outputs.into_iter().find(|output| output != &first).unwrap();
    fixture.complete_output_metadata_for(other, "B");
    let temp = test_temp::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "light-test");
    options.persist_transparent = true;
    fixture.state.session = SessionState::new(Some(options));
    let identity = fixture.state.output_identity_for(&first);
    fixture
        .state
        .begin_session_output_transition(identity, "test load");
    (fixture, temp)
}

/// The drawing a passive view rendered when it was retained or resized,
/// before its first paint copies it into a display buffer.
fn pending_raster(retained: &LightOutputOverlay) -> &ImageSurface {
    retained
        .raster
        .as_ref()
        .expect("a retained drawing waits for its first paint")
}

fn draw(fixture: &mut HandlerFixture) {
    fixture
        .state
        .input_state
        .on_mouse_press(MouseButton::Left, 600, 400);
    fixture.state.input_state.on_mouse_motion(650, 450);
    fixture
        .state
        .input_state
        .on_mouse_release(MouseButton::Left, 650, 450);
}

fn enter_light(fixture: &mut HandlerFixture) {
    fixture
        .state
        .input_state
        .toggle_light_mode_with_resources(InputTextResources {
            ui_engine: fixture.state.render.ui_text(),
            measurer: fixture.state.render.text_measurer(),
        });
    assert!(fixture.state.input_state.light_mode_passthrough());
}

fn next(fixture: &mut HandlerFixture) {
    fixture
        .state
        .handle_output_focus_action(&fixture.queue.handle(), OutputFocusAction::Next);
    fixture.state.finish_session_command_blocking().unwrap();
    fixture.state.surface.update_dimensions(1000, 800);
    fixture.state.surface.set_configured(true);
}

#[test]
fn output_switch_keeps_the_source_notes_and_restores_each_outputs_own_session() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    let original = fixture
        .state
        .input_state
        .boards
        .active_frame()
        .shapes
        .clone();
    enter_light(&mut fixture);
    next(&mut fixture);
    assert!(
        fixture
            .state
            .input_state
            .boards
            .active_frame()
            .shapes
            .is_empty(),
        "B has its own empty drawing"
    );
    assert_eq!(fixture.state.light_outputs.outputs.len(), 1);
    let retained = &fixture.state.light_outputs.outputs[0];
    assert_eq!(retained.snapshot.board.frame.shapes.len(), 1);
    let mut visible = false;
    pending_raster(retained)
        .with_data(|pixels| visible = pixels.iter().any(|byte| *byte != 0))
        .unwrap();
    assert!(visible, "the retained drawing has visible pixels");
    let qh = fixture.queue.handle();
    fixture.state.render_light_outputs(&qh);
    let source = fixture.state.light_outputs.outputs[0]
        .surface
        .wl_surface()
        .unwrap()
        .id()
        .protocol_id();
    fixture.complete_surface_frame(source);
    assert!(
        !fixture.state.light_outputs.needs_render(),
        "idle notes do not repaint continuously"
    );
    fixture
        .state
        .handle_output_focus_action(&qh, OutputFocusAction::Next);
    fixture.state.surface.update_dimensions(1000, 800);
    fixture.state.surface.set_configured(true);
    assert!(fixture.state.light_output_session_pending());
    fixture.state.render_light_outputs(&qh);
    let _pending_frame = fixture.state.render(&qh).unwrap();
    assert_eq!(
        fixture.state.light_outputs.outputs.len(),
        2,
        "a blank editor frame during the session load cannot replace A's drawing"
    );
    fixture.state.finish_session_command_blocking().unwrap();
    fixture.state.render_light_outputs(&qh);
    assert_eq!(
        fixture.state.light_outputs.outputs.len(),
        2,
        "A's passive drawing must remain until the arriving editor commits its replacement frame"
    );
    let _outcome = fixture.state.render(&qh).unwrap();
    assert_eq!(
        fixture.state.input_state.boards.active_frame().shapes.len(),
        original.len(),
        "returning to A restores its drawing"
    );
    assert_eq!(
        fixture.state.light_outputs.outputs.len(),
        1,
        "only B remains passive; A is not duplicated"
    );
    assert!(
        fixture.state.light_outputs.outputs[0]
            .snapshot
            .board
            .frame
            .shapes
            .is_empty(),
        "B never acquired A's notes"
    );
}

#[test]
fn passive_surface_releases_keyboard_and_has_an_empty_pointer_input_region() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    enter_light(&mut fixture);
    let surface = fixture
        .state
        .surface
        .wl_surface()
        .unwrap()
        .id()
        .protocol_id();
    let layer = fixture.state.surface.layer_surface_mut().unwrap().clone();
    let layer_id = match layer.kind() {
        SurfaceKind::Wlr(layer) => layer.id().protocol_id(),
        _ => panic!("test layer uses wlr-layer-shell"),
    };
    next(&mut fixture);
    let requests = fixture.requests_until_commit(surface);
    assert!(
        requests
            .iter()
            .any(|(object, opcode, payload)| *object == layer_id
                && *opcode == 4
                && payload == &0u32.to_ne_bytes()),
        "the passive layer explicitly releases the keyboard"
    );
    let region = requests
        .iter()
        .find(|(object, opcode, _)| *object == surface && *opcode == 5)
        .map(|(_, _, payload)| u32::from_ne_bytes(payload[..4].try_into().unwrap()))
        .expect("passive input region was sent");
    assert_ne!(region, 0, "null would take every pointer event");
    assert!(
        !requests
            .iter()
            .any(|(object, opcode, _)| *object == region && *opcode == 1),
        "the region contains no rectangles, so desktop clicks pass through"
    );
}

#[test]
fn a_missing_passive_capture_callback_times_out_and_restores_notes() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    enter_light(&mut fixture);
    next(&mut fixture);
    let qh = fixture.queue.handle();
    assert!(
        fixture
            .state
            .enter_overlay_suppression(OverlaySuppression::DesktopBackdrop)
    );
    fixture.state.render_light_outputs(&qh);
    let _outcome = fixture.state.render(&qh).unwrap();
    fixture.complete_main_frame();
    assert!(
        fixture.state.suppression.suppressed(),
        "a main callback alone cannot start acquisition"
    );
    assert!(!fixture.state.light_outputs.ready_for_capture());
    let deadline = fixture
        .state
        .overlay_capture_barrier_timeout(Instant::now(), Duration::ZERO);
    assert!(
        deadline.is_some(),
        "the remaining monitor still has a bounded wait"
    );
    fixture
        .state
        .poll_overlay_capture_barrier_timeout(Instant::now() + Duration::from_secs(2));
    assert!(
        !fixture.state.suppression.suppressed(),
        "a missing passive callback cancels capture and restores the editor"
    );
    fixture.state.render_light_outputs(&qh);
    assert!(
        !fixture.state.light_outputs.outputs[0].hidden,
        "notes return after capture cancellation"
    );
}

#[test]
fn a_closed_passive_layer_does_not_close_the_editor_and_late_events_are_ignored() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    enter_light(&mut fixture);
    let retired = fixture.state.surface.layer_surface_mut().unwrap().clone();
    next(&mut fixture);
    let qh = fixture.queue.handle();
    fixture.state.closed(&fixture.conn, &qh, &retired);
    assert!(fixture.state.light_outputs.outputs.is_empty());
    assert!(
        !fixture.state.input_state.should_exit,
        "only the disconnected passive view closes"
    );
    fixture.state.closed(&fixture.conn, &qh, &retired);
    assert!(
        !fixture.state.input_state.should_exit,
        "late events from a retired surface cannot close the current editor"
    );
}

#[test]
fn activation_resumes_light_mode_without_exiting_and_editing_toggle_closes_everything() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    enter_light(&mut fixture);
    fixture
        .state
        .toggle_light_output_editor(&fixture.queue.handle());
    assert!(!fixture.state.input_state.should_exit);
    assert!(
        !fixture.state.input_state.light_mode_active(),
        "the arriving output opens the normal editor"
    );
    assert_eq!(fixture.state.light_outputs.outputs.len(), 1);
    fixture
        .state
        .toggle_light_output_editor(&fixture.queue.handle());
    assert!(
        fixture.state.input_state.should_exit,
        "a second toggle while editing closes the whole overlay runtime"
    );
}

#[test]
fn failed_session_save_keeps_the_source_drawing_and_its_passive_view() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    enter_light(&mut fixture);
    let source_path = fixture.state.session_options().unwrap().session_file_path();
    fs::create_dir_all(&source_path).unwrap();
    fixture
        .state
        .handle_output_focus_action(&fixture.queue.handle(), OutputFocusAction::Next);
    let _result = fixture.state.finish_session_command_blocking();
    assert!(
        source_path.is_dir(),
        "failed session saving preserves the existing directory"
    );
    assert!(
        fixture.state.session.pending_output_transition().is_some(),
        "failed persistence defers the output transition"
    );
    assert_eq!(
        fixture.state.input_state.boards.active_frame().shapes.len(),
        1,
        "failed persistence cannot erase the editor's notes"
    );
    assert_eq!(
        fixture.state.light_outputs.outputs.len(),
        1,
        "the original monitor keeps its display snapshot"
    );
}

#[test]
fn unsupported_and_disabled_modes_keep_single_output_behavior() {
    let (mut fixture, _temp) = fixture();
    for (enabled, per_output, persist) in [
        (false, true, true),
        (true, false, true),
        (true, true, false),
    ] {
        fixture.state.config.ui.keep_light_overlays_on_all_outputs = enabled;
        fixture.state.config.session.per_output = per_output;
        fixture.state.config.session.persist_transparent = persist;
        assert!(!fixture.state.light_output_overlays_enabled());
    }
    fixture.state.config.session.persist_transparent = true;
    fixture.state.session = SessionState::new(None);
    assert!(
        !fixture.state.light_output_overlays_enabled(),
        "disabled persistence cannot advertise retained output sessions"
    );
    let mut config = Config::default();
    config.ui.keep_light_overlays_on_all_outputs = true;
    let xdg = HandlerFixture::new(config);
    assert!(
        !xdg.state.light_output_overlays_enabled(),
        "XDG fallback never advertises passive output support"
    );
}

#[test]
fn freeze_acquisition_starts_only_after_both_monitors_hide_and_notes_restore_afterward() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    enter_light(&mut fixture);
    next(&mut fixture);
    let qh = fixture.queue.handle();
    fixture.state.refresh_freeze_zoom_geometry();
    fixture.state.frozen.start_capture().unwrap();
    assert!(
        fixture
            .state
            .enter_overlay_suppression(OverlaySuppression::Frozen)
    );
    fixture.state.render_light_outputs(&qh);
    let _outcome = fixture.state.render(&qh).unwrap();
    fixture.complete_main_frame();
    assert!(
        fixture.state.frozen.preflight_pending(),
        "one confirmed frame cannot start acquisition"
    );
    let source = fixture.state.light_outputs.outputs[0]
        .surface
        .wl_surface()
        .unwrap()
        .id()
        .protocol_id();
    fixture.complete_surface_frame(source);
    assert!(
        !fixture.state.frozen.preflight_pending(),
        "the final monitor unlocks acquisition"
    );
    fixture.assert_direct_capture_started();
    fixture
        .state
        .exit_overlay_suppression(OverlaySuppression::Frozen);
    fixture.state.render_light_outputs(&qh);
    assert!(
        !fixture.state.light_outputs.outputs[0].hidden,
        "the passive drawing is restored after capture"
    );
}

#[test]
fn retained_notes_rerasterize_at_the_outputs_new_scale_and_large_allocations_are_refused() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    enter_light(&mut fixture);
    next(&mut fixture);
    let surface = fixture.state.light_outputs.outputs[0]
        .surface
        .wl_surface()
        .unwrap()
        .clone();
    assert!(fixture.state.light_outputs.scale_surface(&surface, 2));
    let retained = &fixture.state.light_outputs.outputs[0];
    let raster = pending_raster(retained);
    assert_eq!((raster.width(), raster.height()), (2000, 1600));
    assert_eq!(
        retained.snapshot.board.frame.shapes.len(),
        1,
        "rerasterizing preserves the vector content"
    );
    fixture.state.surface.update_dimensions(u32::MAX, u32::MAX);
    let before = fixture.state.surface.wl_surface().unwrap().clone();
    assert!(
        fixture.state.retain_light_output().is_err(),
        "an oversized output is refused before allocating"
    );
    assert!(
        fixture.state.surface.is_surface(&before),
        "the refused move preserves the editor's surface"
    );
}

#[test]
fn rapid_moves_cannot_retain_the_previous_sessions_notes_on_the_destination() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    enter_light(&mut fixture);
    fixture
        .state
        .handle_output_focus_action(&fixture.queue.handle(), OutputFocusAction::Next);
    fixture.state.surface.update_dimensions(1000, 800);
    fixture.state.surface.set_configured(true);
    let current = fixture.state.surface.wl_surface().unwrap().clone();
    assert!(
        !fixture.state.light_output_session_ready(),
        "B cannot paint A's pending session"
    );
    assert!(
        fixture.state.retain_light_output().is_err(),
        "another move waits for the source session transaction"
    );
    assert!(fixture.state.surface.is_surface(&current));
    assert_eq!(
        fixture.state.light_outputs.outputs.len(),
        1,
        "only A owns a passive snapshot"
    );
    fixture.state.finish_session_command_blocking().unwrap();
    assert!(
        fixture.state.light_output_session_ready(),
        "B is ready after its own session loads"
    );
    assert!(
        fixture
            .state
            .input_state
            .boards
            .active_frame()
            .shapes
            .is_empty()
    );
}

#[test]
fn passive_notes_use_the_live_canvas_colors_independently_of_png_export_settings() {
    for apply_to_canvas in [false, true] {
        for export in [
            RenderProfileExportMode::Off,
            RenderProfileExportMode::Profile,
        ] {
            let (mut fixture, _temp) = fixture();
            fixture
                .state
                .input_state
                .set_render_profiles(RenderProfileSet::from_config(&RenderProfilesConfig {
                    active: Some("live".into()),
                    apply_to_canvas,
                    export,
                    export_profile: Some("png".into()),
                    profiles: vec![
                        RenderProfileConfig {
                            id: "live".into(),
                            name: "Live".into(),
                            mappings: vec![RenderColorMappingConfig {
                                from: "ff0000".into(),
                                to: "00ff00".into(),
                            }],
                        },
                        RenderProfileConfig {
                            id: "png".into(),
                            name: "PNG".into(),
                            mappings: vec![RenderColorMappingConfig {
                                from: "ff0000".into(),
                                to: "0000ff".into(),
                            }],
                        },
                    ],
                    ..RenderProfilesConfig::default()
                }));
            fixture
                .state
                .input_state
                .boards
                .active_frame_mut()
                .add_shape(Shape::Line {
                    x1: 100,
                    y1: 100,
                    x2: 200,
                    y2: 100,
                    color: Color::new(1.0, 0.0, 0.0, 1.0),
                    thick: 12.0,
                });
            enter_light(&mut fixture);
            next(&mut fixture);

            let raster = pending_raster(&fixture.state.light_outputs.outputs[0]);
            let mut pixel = 0;
            raster
                .with_data(|data| {
                    let offset = 100 * raster.stride() as usize + 150 * 4;
                    pixel = u32::from_ne_bytes(data[offset..offset + 4].try_into().unwrap());
                })
                .unwrap();
            assert_eq!(
                pixel,
                if apply_to_canvas {
                    0xff00ff00
                } else {
                    0xffff0000
                },
                "the passive display preserves live canvas colors with export={export:?}"
            );
        }
    }
}

#[test]
fn an_unavailable_spotlight_magnifier_keeps_its_normal_opening_on_a_passive_monitor() {
    let (mut fixture, _temp) = fixture();
    let id = fixture
        .state
        .input_state
        .boards
        .active_frame_mut()
        .add_shape(Shape::Spotlight {
            cx: 200,
            cy: 200,
            rx: 60,
            ry: 60,
            magnification: 2.0,
        });
    enter_light(&mut fixture);
    let original = fixture.state.surface.wl_surface().unwrap().clone();
    next(&mut fixture);

    assert!(
        !fixture.state.surface.is_surface(&original),
        "the unavailable magnifier cannot block a monitor move"
    );
    let raster = pending_raster(&fixture.state.light_outputs.outputs[0]);
    raster
        .with_data(|data| {
            let alpha_at = |x: usize, y: usize| {
                let offset = y * raster.stride() as usize + x * 4;
                u32::from_ne_bytes(data[offset..offset + 4].try_into().unwrap()) >> 24
            };
            assert_eq!(alpha_at(200, 200), 0, "the opening remains transparent");
            assert!(alpha_at(400, 400) > 0, "the area outside remains dimmed");
        })
        .unwrap();

    next(&mut fixture);
    assert!(
        matches!(
            fixture
                .state
                .input_state
                .boards
                .active_frame()
                .shape(id)
                .unwrap()
                .shape,
            Shape::Spotlight {
                magnification: 2.0,
                ..
            }
        ),
        "display fallback must preserve the saved magnification"
    );
}

#[test]
fn arriving_editor_input_waits_for_its_own_output_session_and_escape_still_exits() {
    for source in ["pointer", "touch", "keyboard", "toolbar", "escape"] {
        let (mut fixture, _temp) = fixture();
        fixture.state.input_state.ui_visibility.show_tool_preview = true;
        draw(&mut fixture);
        enter_light(&mut fixture);
        let previous = fixture.state.surface.current_output().unwrap();
        let previous_surface = fixture.state.surface.wl_surface().unwrap().clone();
        fixture
            .state
            .toggle_light_output_editor(&fixture.queue.handle());
        let target = fixture
            .state
            .protocol
            .output()
            .outputs()
            .find(|output| output != &previous)
            .unwrap();
        let surface = fixture.state.surface.wl_surface().unwrap().clone();
        let qh = fixture.queue.handle();
        fixture
            .state
            .surface_enter(&fixture.conn, &qh, &surface, &target);
        fixture.state.surface.update_dimensions(1000, 800);
        fixture.state.surface.set_configured(true);
        assert!(!fixture.state.light_output_session_ready());

        let pointer = WlPointer::inert(fixture.conn.backend().downgrade());
        if source == "pointer" {
            fixture.state.pointer_frame(
                &fixture.conn,
                &qh,
                &pointer,
                &[PointerEvent {
                    surface: previous_surface,
                    position: (600.0, 400.0),
                    kind: PointerEventKind::Enter { serial: 1 },
                }],
            );
            assert!(
                !fixture.state.has_cursor_focus(),
                "a late entry on the passive surface cannot focus the new editor"
            );
            fixture.state.pointer_frame(
                &fixture.conn,
                &qh,
                &pointer,
                &[PointerEvent {
                    surface: surface.clone(),
                    position: (600.0, 400.0),
                    kind: PointerEventKind::Enter { serial: 2 },
                }],
            );
        }
        let events = [
            PointerEvent {
                surface: surface.clone(),
                position: (600.0, 400.0),
                kind: PointerEventKind::Press {
                    time: 1,
                    button: BTN_LEFT,
                    serial: 1,
                },
            },
            PointerEvent {
                surface: surface.clone(),
                position: (650.0, 450.0),
                kind: PointerEventKind::Motion { time: 2 },
            },
            PointerEvent {
                surface,
                position: (650.0, 450.0),
                kind: PointerEventKind::Release {
                    time: 3,
                    button: BTN_LEFT,
                    serial: 2,
                },
            },
        ];
        match source {
            "pointer" => fixture
                .state
                .pointer_frame(&fixture.conn, &qh, &pointer, &events),
            "touch" => {
                let touch = WlTouch::inert(fixture.conn.backend().downgrade());
                fixture.state.down(
                    &fixture.conn,
                    &qh,
                    &touch,
                    1,
                    1,
                    events[0].surface.clone(),
                    0,
                    (600.0, 400.0),
                );
                fixture
                    .state
                    .motion(&fixture.conn, &qh, &touch, 2, 0, (650.0, 450.0));
                fixture.state.up(&fixture.conn, &qh, &touch, 2, 3, 0);
            }
            "keyboard" => {
                fixture.state.input_state.modifiers.ctrl = true;
                fixture.state.dispatch_key_press(
                    Key::Char('z'),
                    KeyPressSource::GtkToolbar,
                    &fixture.conn,
                    &qh,
                );
                fixture.state.input_state.modifiers.ctrl = false;
            }
            "toolbar" => fixture.state.handle_toolbar_event(
                ToolbarEvent::ClearCanvas { instant: true },
                Some(&fixture.conn),
                Some(&qh),
            ),
            "escape" => fixture.state.dispatch_key_press(
                Key::Escape,
                KeyPressSource::GtkToolbar,
                &fixture.conn,
                &qh,
            ),
            _ => unreachable!(),
        }
        assert_eq!(
            fixture.state.input_state.boards.active_frame().shapes.len(),
            1,
            "{source} on B cannot edit A's session while B is loading"
        );
        assert_eq!(fixture.state.input_state.should_exit, source == "escape");
        fixture.state.finish_session_command_blocking().unwrap();
        assert!(fixture.state.light_output_session_ready());
        if source == "pointer" {
            assert!(
                fixture.state.mouse_tool_preview_eligible(),
                "the arriving editor must retain pointer focus and its preview without requiring another surface entry"
            );
            assert_eq!(
                fixture.state.pointer.position(),
                (650, 450),
                "motion during loading must keep the visible preview at the current cursor position"
            );
        }
        assert!(
            fixture
                .state
                .input_state
                .boards
                .active_frame()
                .shapes
                .is_empty()
        );

        if source != "escape" {
            fixture
                .state
                .pointer_frame(&fixture.conn, &qh, &pointer, &events);
            assert_eq!(
                fixture.state.input_state.boards.active_frame().shapes.len(),
                1,
                "the destination accepts drawing after its session loads"
            );
        }
    }
}

#[test]
fn an_existing_pointer_contact_can_finish_while_the_destination_session_waits() {
    let (mut fixture, _temp) = fixture();
    let qh = fixture.queue.handle();
    let pointer = WlPointer::inert(fixture.conn.backend().downgrade());
    let source = fixture.state.surface.wl_surface().unwrap().clone();
    fixture.state.pointer_frame(
        &fixture.conn,
        &qh,
        &pointer,
        &[
            PointerEvent {
                surface: source.clone(),
                position: (600.0, 400.0),
                kind: PointerEventKind::Press {
                    time: 1,
                    button: BTN_LEFT,
                    serial: 1,
                },
            },
            PointerEvent {
                surface: source,
                position: (650.0, 450.0),
                kind: PointerEventKind::Motion { time: 2 },
            },
        ],
    );
    fixture
        .state
        .handle_output_focus_action(&qh, OutputFocusAction::Next);
    assert!(fixture.state.light_output_session_pending());
    let destination = fixture.state.surface.wl_surface().unwrap().clone();
    fixture.state.pointer_frame(
        &fixture.conn,
        &qh,
        &pointer,
        &[PointerEvent {
            surface: destination,
            position: (650.0, 450.0),
            kind: PointerEventKind::Release {
                time: 3,
                button: BTN_LEFT,
                serial: 2,
            },
        }],
    );
    assert!(
        !fixture.state.input_state.has_active_pointer_interaction(),
        "release cannot be swallowed by the session input guard"
    );
    fixture
        .state
        .retry_pending_output_transition_if_due(Instant::now() + Duration::from_secs(2))
        .unwrap();
    fixture.state.finish_session_command_blocking().unwrap();
    assert!(
        fixture.state.light_output_session_ready(),
        "the finished contact no longer holds off the destination session"
    );
    assert!(
        fixture
            .state
            .input_state
            .boards
            .active_frame()
            .shapes
            .is_empty()
    );
    next(&mut fixture);
    assert_eq!(
        fixture.state.input_state.boards.active_frame().shapes.len(),
        1,
        "the source kept its completed stroke"
    );
}

#[test]
fn a_session_inspection_keeps_the_current_editor_usable_but_defers_surface_retention() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    fixture
        .state
        .start_session_command(SessionCommand::Inspect)
        .unwrap();
    assert!(
        fixture.state.session_transaction.is_some(),
        "inspection is awaiting its real worker receipt"
    );
    assert!(fixture.state.session.pending_output_transition().is_none());
    let qh = fixture.queue.handle();
    fixture
        .state
        .handle_toolbar_event(ToolbarEvent::Undo, Some(&fixture.conn), Some(&qh));
    assert!(
        fixture
            .state
            .input_state
            .boards
            .active_frame()
            .shapes
            .is_empty(),
        "an inspection of the visible output cannot disable its Undo control"
    );

    enter_light(&mut fixture);
    assert!(
        fixture.state.retain_light_output().is_err(),
        "surface retention must still wait for an outstanding session command"
    );
    fixture.state.finish_session_command_blocking().unwrap();
    assert!(fixture.state.light_output_session_ready());
}

#[test]
fn two_buffers_per_view_leave_room_for_two_hidpi_monitors() {
    // A 4K monitor at a fractional scale of 1.5 renders 2560x1440 at scale 2.
    let one_view = 5120 * 2880 * 4 * PASSIVE_BUFFER_FRAMES;
    assert!(
        super::check_budget(one_view, 2560, 1440, 2).is_ok(),
        "the second high-density monitor keeps its drawing"
    );
    assert!(
        super::check_budget(2 * one_view, 2560, 1440, 2).is_err(),
        "a third view would exceed the shared budget"
    );
}

#[test]
fn a_painted_view_releases_its_drawing_and_renders_it_again_after_a_capture() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    enter_light(&mut fixture);
    next(&mut fixture);
    let qh = fixture.queue.handle();

    fixture.state.render_light_outputs(&qh);
    assert!(
        fixture.state.light_outputs.outputs[0].raster.is_none(),
        "the display buffer is the only copy kept on screen"
    );

    fixture.state.light_outputs.set_suppressed(true);
    fixture.state.light_outputs.set_suppressed(false);
    fixture.state.render_light_outputs(&qh);

    let retained = &fixture.state.light_outputs.outputs[0];
    assert!(
        !retained.hidden && !retained.dirty,
        "the drawing is rendered again from its snapshot into the second buffer"
    );
    assert!(retained.raster.is_none());
}

#[test]
fn a_repeated_configure_keeps_the_painted_drawing() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    enter_light(&mut fixture);
    next(&mut fixture);
    let qh = fixture.queue.handle();
    fixture.state.render_light_outputs(&qh);
    let layer = fixture.state.light_outputs.outputs[0]
        .surface
        .layer_surface_mut()
        .unwrap()
        .clone();

    assert!(
        fixture
            .state
            .light_outputs
            .configure_layer(&layer, 1000, 800)
    );
    let retained = &fixture.state.light_outputs.outputs[0];
    assert!(
        retained.raster.is_none() && !retained.dirty,
        "an unchanged size neither renders nor repaints"
    );

    assert!(
        fixture
            .state
            .light_outputs
            .configure_layer(&layer, 1200, 800)
    );
    let retained = &fixture.state.light_outputs.outputs[0];
    assert_eq!(pending_raster(retained).width(), 1200);
    assert!(retained.dirty, "a new size repaints");
}

#[test]
fn a_view_that_cannot_be_painted_closes_without_stopping_the_editor() {
    let (mut fixture, _temp) = fixture();
    draw(&mut fixture);
    enter_light(&mut fixture);
    next(&mut fixture);
    let qh = fixture.queue.handle();
    fixture.state.light_outputs.outputs[0].raster =
        Some(ImageSurface::create(Format::ARgb32, 1, 1).unwrap());

    fixture.state.render_light_outputs(&qh);

    assert!(
        fixture.state.light_outputs.outputs.is_empty(),
        "only the failing view closes"
    );
    assert!(!fixture.state.input_state.should_exit);
    assert_eq!(
        fixture.state.input_state.test_active_toast_message(),
        Some("Could not keep showing the drawings on another monitor.")
    );
}

#[test]
fn a_daemon_toggle_closes_an_overlay_that_cannot_keep_drawings() {
    let (mut fixture, _temp) = fixture();
    enter_light(&mut fixture);
    fixture.state.config.ui.keep_light_overlays_on_all_outputs = false;

    fixture
        .state
        .toggle_light_output_editor(&fixture.queue.handle());

    assert!(
        fixture.state.input_state.should_exit,
        "the toggle falls back to hiding instead of doing nothing"
    );
}
