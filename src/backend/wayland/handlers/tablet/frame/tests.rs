use super::{DrawingState, WaylandState, modal_blocks_stylus_barrel_actions, stylus_barrel_action};
use crate::input::state::test_support::make_test_input_state;

#[test]
fn hard_popup_taps_through_the_pen_frame_handler_do_not_resize_the_next_stroke() {
    use crate::backend::wayland::handlers::test_support::HandlerFixture;
    use wayland_client::{Dispatch, Proxy};
    use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::{Event, ZwpTabletToolV2};
    for chrome in ["card", "popover", "color button", "color slider", "radial"] {
        let mut config = crate::config::Config::default();
        config.ui.show_onboarding_hints = chrome == "card";
        config.tablet.enabled = true;
        config.tablet.pressure_enabled = true;
        config.tablet.min_thickness = 1.0;
        config.tablet.max_thickness = 20.0;
        let mut fixture = HandlerFixture::new(config);
        let tool = ZwpTabletToolV2::inert(fixture.conn.backend().downgrade());
        let qh = fixture.queue.handle();
        let state = &mut fixture.state;
        state.tablet.on_overlay = true;
        let initial = state.input_state.style.current_thickness;
        let position = match chrome {
            "card" => {
                state
                    .onboarding_card
                    .set_layout(Some(crate::ui::OnboardingCardLayout {
                        x: 100.0,
                        y: 50.0,
                        width: 200.0,
                        height: 120.0,
                        buttons: Vec::new(),
                    }));
                (110.0, 60.0)
            }
            "popover" => {
                state.input_state.apply_toolbar_event(
                    crate::ui::toolbar::ToolbarEvent::ToggleSessionPopover(true),
                );
                (600.0, 400.0)
            }
            "radial" => {
                state.input_state.open_radial_menu(400.0, 300.0);
                state.input_state.update_radial_menu_layout(1000, 800);
                (400.0, 300.0)
            }
            _ => {
                state.input_state.open_color_picker_popup();
                state
                    .input_state
                    .update_color_picker_popup_layout(1000, 800);
                let layout = state.input_state.color_picker_popup_layout().unwrap();
                if chrome == "color slider" {
                    (layout.hue_x + 2.0, layout.hue_y + 2.0)
                } else {
                    (layout.ok_btn_x + 2.0, layout.ok_btn_y + 2.0)
                }
            }
        };
        for event in [
            Event::Motion {
                x: position.0,
                y: position.1,
            },
            Event::Pressure { pressure: 65535 },
            Event::Down { serial: 1 },
            Event::Frame { time: 1 },
        ] {
            <WaylandState as Dispatch<ZwpTabletToolV2, ()>>::event(
                state,
                &tool,
                event,
                &(),
                &fixture.conn,
                &qh,
            );
        }
        assert_eq!(
            state.input_state.style.current_thickness, initial,
            "{chrome}"
        );
        assert_eq!(state.tablet.peak_thickness, None, "{chrome}");
        for event in [Event::Up, Event::Frame { time: 2 }] {
            <WaylandState as Dispatch<ZwpTabletToolV2, ()>>::event(
                state,
                &tool,
                event,
                &(),
                &fixture.conn,
                &qh,
            );
        }
        state.input_state.close_color_picker_popup(true);
        state.input_state.close_radial_menu();
        state.onboarding_card.set_layout(None);
        for event in [
            Event::Motion { x: 700.0, y: 500.0 },
            Event::Pressure { pressure: 4096 },
            Event::Down { serial: 2 },
            Event::Frame { time: 3 },
        ] {
            <WaylandState as Dispatch<ZwpTabletToolV2, ()>>::event(
                state,
                &tool,
                event,
                &(),
                &fixture.conn,
                &qh,
            );
        }
        let DrawingState::Drawing {
            point_thicknesses, ..
        } = &state.input_state.state
        else {
            panic!("expected canvas stroke after {chrome}");
        };
        assert_eq!(
            point_thicknesses,
            &[state.input_state.style.current_thickness as f32]
        );
        assert!(state.input_state.style.current_thickness < 3.0, "{chrome}");
    }
}

fn pen_fixture(
    pressure_enabled: bool,
) -> (
    crate::backend::wayland::handlers::test_support::HandlerFixture,
    wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::ZwpTabletToolV2,
) {
    use wayland_client::Proxy;
    let mut config = crate::config::Config::default();
    config.ui.show_onboarding_hints = false;
    config.tablet.enabled = true;
    config.tablet.pressure_enabled = pressure_enabled;
    config.tablet.min_thickness = 1.0;
    config.tablet.max_thickness = 20.0;
    let mut fixture = crate::backend::wayland::handlers::test_support::HandlerFixture::new(config);
    fixture.state.tablet.on_overlay = true;
    let tool =
        wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::ZwpTabletToolV2::inert(
            fixture.conn.backend().downgrade(),
        );
    (fixture, tool)
}

fn send_pen(
    fixture: &mut crate::backend::wayland::handlers::test_support::HandlerFixture,
    tool: &wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::ZwpTabletToolV2,
    events: impl IntoIterator<
        Item = wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::Event,
    >,
) {
    use wayland_client::Dispatch;
    use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::ZwpTabletToolV2;
    let qh = fixture.queue.handle();
    for event in events {
        <WaylandState as Dispatch<ZwpTabletToolV2, ()>>::event(
            &mut fixture.state,
            tool,
            event,
            &(),
            &fixture.conn,
            &qh,
        );
    }
}

#[test]
fn a_pen_drag_on_the_size_ring_keeps_its_value_after_the_pen_lifts() {
    use crate::input::state::size_ring_angle_for_value;
    use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::Event;
    let ring = |value: f64| {
        let angle = size_ring_angle_for_value(value);
        (400.0 + 183.0 * angle.cos(), 300.0 + 183.0 * angle.sin())
    };
    for pressure_enabled in [true, false] {
        let (mut fixture, tool) = pen_fixture(pressure_enabled);
        fixture.state.input_state.open_radial_menu(400.0, 300.0);
        fixture
            .state
            .input_state
            .update_radial_menu_layout(1000, 800);
        let (start, end) = (ring(30.0), ring(12.0));
        let pressure = || Event::Pressure { pressure: 40000 };
        send_pen(
            &mut fixture,
            &tool,
            [
                Event::Motion {
                    x: start.0,
                    y: start.1,
                },
                pressure(),
                Event::Down { serial: 1 },
                Event::Frame { time: 1 },
            ],
        );
        assert!(fixture.state.input_state.radial_menu_is_size_dragging());
        send_pen(
            &mut fixture,
            &tool,
            [
                Event::Motion { x: end.0, y: end.1 },
                pressure(),
                Event::Frame { time: 2 },
            ],
        );
        let dragged = fixture.state.input_state.style.current_thickness;
        assert!(
            (dragged - 12.0).abs() < 0.5,
            "pressure {pressure_enabled}: {dragged}"
        );
        send_pen(&mut fixture, &tool, [Event::Up, Event::Frame { time: 3 }]);
        assert_eq!(
            fixture.state.input_state.style.current_thickness, dragged,
            "pressure {pressure_enabled}"
        );
    }
}

#[test]
fn a_pen_tap_on_a_toast_acts_on_the_toast_instead_of_drawing() {
    use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::Event;
    let (mut fixture, tool) = pen_fixture(true);
    let initial = fixture.state.input_state.style.current_thickness;
    fixture.state.input_state.push_toast(
        crate::input::state::ToastPriority::Info,
        "test",
        crate::input::state::Toast::info("Saved"),
    );
    fixture
        .state
        .input_state
        .set_toast_geometry(Some((550.0, 350.0, 200.0, 80.0)), [None, None]);

    send_pen(
        &mut fixture,
        &tool,
        [
            Event::Motion { x: 600.0, y: 400.0 },
            Event::Pressure { pressure: 65535 },
            Event::Down { serial: 1 },
            Event::Frame { time: 1 },
        ],
    );
    assert!(!matches!(
        fixture.state.input_state.state,
        DrawingState::Drawing { .. }
    ));
    send_pen(&mut fixture, &tool, [Event::Up, Event::Frame { time: 2 }]);

    assert!(
        fixture
            .state
            .input_state
            .boards
            .active_frame()
            .shapes
            .is_empty()
    );
    assert_eq!(fixture.state.input_state.style.current_thickness, initial);
    assert_eq!(fixture.state.tablet.toast_press, None);
}

#[test]
fn a_pen_tap_that_closes_a_menu_keeps_a_thickness_set_since_the_last_stroke() {
    use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::Event;
    let (mut fixture, tool) = pen_fixture(true);
    send_pen(
        &mut fixture,
        &tool,
        [
            Event::Motion { x: 700.0, y: 500.0 },
            Event::Pressure { pressure: 4096 },
            Event::Down { serial: 1 },
            Event::Frame { time: 1 },
            Event::Up,
            Event::Frame { time: 2 },
        ],
    );
    assert!(fixture.state.input_state.style.current_thickness < 3.0);
    // A mouse or keyboard change after the stroke, which the pen never saw.
    let measurer = crate::draw::TextMeasurer::default();
    let _ = fixture
        .state
        .input_state
        .set_thickness_for_active_tool_with(&measurer, 9.0);
    fixture
        .state
        .input_state
        .apply_toolbar_event(crate::ui::toolbar::ToolbarEvent::ToggleSessionPopover(true));
    send_pen(
        &mut fixture,
        &tool,
        [
            Event::Motion { x: 600.0, y: 400.0 },
            Event::Pressure { pressure: 65535 },
            Event::Down { serial: 2 },
            Event::Frame { time: 3 },
            Event::Up,
            Event::Frame { time: 4 },
        ],
    );
    assert_eq!(fixture.state.input_state.style.current_thickness, 9.0);
}

#[test]
fn help_blocks_stylus_barrel_actions() {
    let mut state = make_test_input_state();
    assert!(!modal_blocks_stylus_barrel_actions(&state));

    state.toggle_help_overlay();
    assert!(modal_blocks_stylus_barrel_actions(&state));
}

/// Both screen-region modals swallow pointer and keyboard input, so a
/// barrel button must not run its bound action on the canvas behind them —
/// including while they are still waiting on a capture.
#[test]
fn screen_region_modals_block_stylus_barrel_actions() {
    use crate::input::state::{EyedropperCaptureSource, RegionPurposeTag, ScreenCaptureSource};

    let mut state = make_test_input_state();
    assert!(!modal_blocks_stylus_barrel_actions(&state));

    state.set_region_pending_capture(RegionPurposeTag::Ocr, 1, ScreenCaptureSource::Frozen);
    assert!(modal_blocks_stylus_barrel_actions(&state));
    state.activate_region_with(
        &crate::draw::TextMeasurer::default(),
        RegionPurposeTag::Ocr,
        1,
    );
    assert!(modal_blocks_stylus_barrel_actions(&state));
    state.cancel_region_ui_only();
    assert!(!modal_blocks_stylus_barrel_actions(&state));

    state.set_eyedropper_pending_capture(EyedropperCaptureSource::Frozen);
    assert!(modal_blocks_stylus_barrel_actions(&state));
    state.activate_eyedropper_with(&crate::draw::TextMeasurer::default(), Some(1));
    assert!(modal_blocks_stylus_barrel_actions(&state));
    state.cancel_eyedropper();
    assert!(!modal_blocks_stylus_barrel_actions(&state));
}

#[test]
fn canonical_stylus_shortcut_wins_over_legacy_tablet_binding() {
    use crate::config::keybindings::linux;
    use crate::config::{Action, KeybindingsConfig};

    let mut keybindings = KeybindingsConfig::default();
    keybindings.core.undo = vec!["StylusPrimary".to_string()];
    let action_map = keybindings.build_action_map().expect("map");
    let action_bindings = keybindings.build_action_bindings().expect("bindings");
    let mut state = crate::input::state::test_support::make_test_input_state();
    state.set_keybinding_maps(action_map, action_bindings);

    let mut tablet = crate::config::TabletInputConfig::default();
    tablet.stylus_button.action = Some(Action::ToggleRadialMenu);

    assert_eq!(
        stylus_barrel_action(&state, linux::BTN_STYLUS, &tablet),
        Some(Action::Undo)
    );
    tablet.stylus_button2.action = Some(Action::Redo);
    assert_eq!(
        stylus_barrel_action(&state, linux::BTN_STYLUS2, &tablet),
        Some(Action::Redo)
    );
}

#[test]
fn unbound_stylus_falls_back_to_legacy_tablet_action() {
    use crate::config::Action;
    use crate::config::keybindings::linux;

    let state = crate::input::state::test_support::make_test_input_state();
    let mut tablet = crate::config::TabletInputConfig::default();
    tablet.stylus_button.action = Some(Action::ToggleRadialMenu);
    tablet.stylus_button2.action = None;

    assert_eq!(
        stylus_barrel_action(&state, linux::BTN_STYLUS, &tablet),
        Some(Action::ToggleRadialMenu)
    );
    assert_eq!(
        stylus_barrel_action(&state, linux::BTN_STYLUS2, &tablet),
        None
    );
    assert_eq!(stylus_barrel_action(&state, 0, &tablet), None);
}
