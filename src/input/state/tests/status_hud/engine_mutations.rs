use super::*;
use crate::ui_text::UiTextEngine;

fn seeded(engine: &UiTextEngine) -> InputState {
    let mut input = create_test_input_state();
    input.compositor_capabilities.layer_shell = true;
    input.presenter_mode_config_mut_for_test().hide_toolbars = true;
    input.presenter_mode_config_mut_for_test().toolbar_mode =
        crate::config::PresenterToolbarMode::Micro;
    input.update_status_hud_layout_for_pointer_with_engine(
        engine,
        StatusPosition::BottomLeft,
        &StatusBarStyle::default(),
        1280,
        720,
        true,
    );
    input
}

#[test]
fn explicit_mutations_rebuild_immediately_and_preserve_no_op_results() {
    let engine = UiTextEngine::default();
    let mut input = seeded(&engine);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Help);
    input.on_mouse_motion(x, y);
    assert_eq!(input.status_hud.hover, Some(StatusHudSegmentKind::Help));
    let before = format!("{:?}", input.status_hud_layout());
    input.needs_redraw = false;
    assert!(input.set_status_bar_item_visible_with_engine(&engine, StatusBarItem::Help, false));
    assert_eq!(input.status_hud.hover, None);
    assert!(
        !input
            .status_hud_layout()
            .unwrap()
            .segments
            .iter()
            .any(|s| s.kind == StatusHudSegmentKind::Help)
    );
    assert_ne!(format!("{:?}", input.status_hud_layout()), before);
    assert!(input.needs_redraw);
    input.needs_redraw = false;
    assert!(!input.set_status_bar_item_visible_with_engine(&engine, StatusBarItem::Help, false));
    assert!(!input.needs_redraw);

    assert!(input.set_toolbar_visible_with_engine(&engine, false));
    assert!(
        input
            .status_hud_layout()
            .unwrap()
            .segments
            .iter()
            .any(|s| s.kind == StatusHudSegmentKind::Toolbar)
    );
    input.needs_redraw = false;
    assert!(!input.set_toolbar_visible_with_engine(&engine, false));
    assert!(!input.needs_redraw);

    let mut unpainted = create_test_input_state();
    assert!(unpainted.set_status_bar_item_visible_with_engine(&engine, StatusBarItem::Help, false));
    assert!(
        unpainted.status_hud_layout().is_none(),
        "mutations do not invent frame dimensions"
    );
}

fn assert_same_chrome(actual: &InputState, expected: &InputState) {
    assert_eq!(actual.toolbar_visible(), expected.toolbar_visible());
    assert_eq!(actual.top_display_state(), expected.top_display_state());
    assert_eq!(
        actual.ui_visibility.show_status_bar,
        expected.ui_visibility.show_status_bar
    );
    assert_eq!(actual.focus_mode_active(), expected.focus_mode_active());
    assert_eq!(
        actual.presenter_mode_active(),
        expected.presenter_mode_active()
    );
    assert_eq!(actual.light_mode_active(), expected.light_mode_active());
    assert_eq!(
        actual.light_mode_drawing_active(),
        expected.light_mode_drawing_active()
    );
    assert_eq!(actual.tour.is_active(), expected.tour.is_active());
    assert_eq!(actual.status_hud.hover, expected.status_hud.hover);
    assert_eq!(
        format!("{:?}", actual.status_hud_layout()),
        format!("{:?}", expected.status_hud_layout())
    );
}

#[test]
fn explicit_mode_cycles_match_legacy_without_an_intervening_frame() {
    let engine = UiTextEngine::default();
    let measurer = crate::draw::TextMeasurer::default();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &engine,
    };
    let mut explicit = seeded(&engine);
    let mut legacy = seeded(&UiTextEngine::default());
    // Enter/leave Focus, then make Light's drawing-entry path leave Focus.
    explicit.toggle_focus_mode_with_resources(resources);
    legacy.toggle_focus_mode_with_resources(resources);
    assert_same_chrome(&explicit, &legacy);
    assert!(explicit.focus_mode_active());
    assert!(explicit.set_light_mode_drawing_with_resources(resources, true));
    assert!(legacy.set_light_mode_drawing(true));
    assert_same_chrome(&explicit, &legacy);
    assert!(explicit.light_mode_active());
    assert!(!explicit.focus_mode_active());
    assert_eq!(
        explicit.toggle_light_mode_drawing_with_resources(resources),
        legacy.toggle_light_mode_drawing()
    );
    assert_same_chrome(&explicit, &legacy);
    assert_eq!(
        explicit.toggle_light_mode_with_resources(resources),
        legacy.toggle_light_mode_with_resources(resources)
    );
    assert_same_chrome(&explicit, &legacy);
    assert!(!explicit.light_mode_active());
    // Presenter and Light replace each other's numeric visibility snapshots.
    assert_eq!(
        explicit.toggle_presenter_mode_with_resources(resources),
        legacy.toggle_presenter_mode_with_resources(resources)
    );
    assert_same_chrome(&explicit, &legacy);
    assert!(explicit.presenter_mode_active());
    assert_eq!(
        explicit.toggle_light_mode_with_resources(resources),
        legacy.toggle_light_mode_with_resources(resources)
    );
    assert_same_chrome(&explicit, &legacy);
    assert!(!explicit.presenter_mode_active());
    explicit.toggle_focus_mode_with_resources(resources);
    legacy.toggle_focus_mode_with_resources(resources);
    assert_same_chrome(&explicit, &legacy);
    explicit.start_tour_replay_with_resources(resources);
    legacy.start_tour_replay();
    assert_same_chrome(&explicit, &legacy);
    assert!(explicit.tour.is_active());
    assert!(!explicit.focus_mode_active());
}

#[test]
fn explicit_focus_rescue_and_display_cycle_refresh_saved_geometry() {
    let engine = UiTextEngine::default();
    let measurer = crate::draw::TextMeasurer::default();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &engine,
    };
    let mut input = seeded(&engine);
    input.set_toolbar_visible_with_engine(&engine, false);
    input.ui_visibility.show_status_bar = false;
    input.ui_visibility.show_floating_badge = false;
    input.ui_visibility.show_zoom_chip = false;
    input.refresh_status_hud_layout_with_engine(&engine);
    assert!(!input.focus_mode_active());
    input.toggle_focus_mode_with_resources(resources);
    assert!(input.toolbar_visible(), "Focus rescues fully hidden chrome");
    assert!(input.ui_visibility.show_status_bar);
    assert!(!input.focus_mode_active());
    // Rescue restores status visibility after its toolbar refresh. Do not add a new refresh.
    assert!(input.status_hud_layout().is_none());
    input.set_top_display_mode_with_engine(&engine, crate::config::TopDisplayMode::Full);
    assert!(input.status_hud_layout().is_some());
    assert_eq!(
        input.cycle_top_toolbar_display_with_engine(&engine),
        crate::config::TopDisplayMode::Micro
    );
    input.derive_toolbar_visibility_from_pins_with_engine(&engine);
    assert!(input.status_hud_layout().is_some());
}

#[test]
fn explicit_action_and_toolbar_routes_refresh_status_geometry_before_another_frame() {
    let engine = UiTextEngine::default();
    let measurer = crate::draw::TextMeasurer::default();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &engine,
    };
    let mut input = seeded(&engine);
    input.set_toolbar_visible_with_engine(&engine, true);
    input.handle_action_with_resources(resources, crate::domain::Action::ToggleToolbar);
    assert!(!input.toolbar_visible());
    assert!(
        input
            .status_hud_layout()
            .unwrap()
            .segments
            .iter()
            .any(|segment| segment.kind == StatusHudSegmentKind::Toolbar)
    );

    let (x, y) = segment_center(&input, StatusHudSegmentKind::Help);
    input.on_mouse_motion(x, y);
    assert_eq!(input.status_hud.hover, Some(StatusHudSegmentKind::Help));
    assert!(input.apply_toolbar_event_with_resources(
        resources,
        crate::ui::toolbar::ToolbarEvent::SetStatusBarItemVisible(StatusBarItem::Help, false)
    ));
    assert_eq!(input.status_hud.hover, None);
    assert!(
        !input
            .status_hud_layout()
            .unwrap()
            .segments
            .iter()
            .any(|segment| segment.kind == StatusHudSegmentKind::Help)
    );
}
