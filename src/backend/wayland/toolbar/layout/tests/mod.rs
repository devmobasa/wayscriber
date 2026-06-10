use super::super::events::HitKind;
use super::*;
use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
use crate::draw::{Color, FontDescriptor};
use crate::input::{ClickHighlightSettings, EraserMode, InputState, ToolbarDrawerTab};
use crate::ui::toolbar::model::{
    ToolbarActivation, ToolbarSessionModel, ToolbarSettingsModel, ToolbarSliderSpec,
};
use crate::ui::toolbar::{
    SessionRecentSnapshot, ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot,
};

mod collapsible;

fn create_test_input_state() -> InputState {
    let keybindings = KeybindingsConfig::default();
    let action_map = keybindings.build_action_map().unwrap();
    let action_bindings = keybindings.build_action_bindings().unwrap();

    let mut state = InputState::with_defaults(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        3.0,
        12.0,
        EraserMode::Brush,
        0.32,
        false,
        32.0,
        FontDescriptor {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        },
        false,
        20.0,
        30.0,
        false,
        true,
        BoardsConfig::default(),
        action_map,
        usize::MAX,
        ClickHighlightSettings::disabled(),
        0,
        0,
        false,
        0,
        0,
        5,
        5,
        PresenterModeConfig::default(),
    );
    state.set_action_bindings(action_bindings);
    state
}

fn snapshot_from_state(state: &InputState) -> ToolbarSnapshot {
    ToolbarSnapshot::from_input_with_bindings(state, ToolbarBindingHints::default())
}

fn event_name(event: &ToolbarEvent) -> String {
    format!("{event:?}")
}

fn activation_event_name(activation: &ToolbarActivation) -> String {
    event_name(&activation.compatibility_event())
}

#[test]
fn top_size_respects_icon_mode() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    let snapshot = snapshot_from_state(&state);
    assert_eq!(top_size(&snapshot), (965, 72));

    state.toolbar_use_icons = false;
    let snapshot = snapshot_from_state(&state);
    assert_eq!(top_size(&snapshot), (1110, 60));
}

#[test]
fn build_top_hits_includes_toggle_and_pin() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    let snapshot = snapshot_from_state(&state);
    let mut hits = Vec::new();
    let (w, h) = top_size(&snapshot);
    build_top_hits(w as f64, h as f64, &snapshot, &mut hits);

    assert!(
        hits.iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::ToggleIconMode(false)))
    );
    assert!(hits.iter().any(|hit| matches!(
        hit.event,
        ToolbarEvent::SelectTool(crate::input::Tool::StepMarker)
    )));
    assert!(
        hits.iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::PinTopToolbar(_)))
    );
    assert!(
        hits.iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::CloseTopToolbar))
    );
}

#[test]
fn top_size_keeps_toggle_and_window_controls_separate() {
    let mut state = create_test_input_state();

    for use_icons in [true, false] {
        state.toolbar_use_icons = use_icons;
        let snapshot = snapshot_from_state(&state);
        let (w, h) = top_size(&snapshot);
        let mut hits = Vec::new();
        build_top_hits(w as f64, h as f64, &snapshot, &mut hits);

        let toggle = hits
            .iter()
            .find(|hit| matches!(hit.event, ToolbarEvent::ToggleIconMode(_)))
            .expect("icon/text toggle hit");
        let pin = hits
            .iter()
            .find(|hit| matches!(hit.event, ToolbarEvent::PinTopToolbar(_)))
            .expect("pin hit");
        let close = hits
            .iter()
            .find(|hit| matches!(hit.event, ToolbarEvent::CloseTopToolbar))
            .expect("close hit");

        assert!(
            toggle.rect.0 + toggle.rect.2 + ToolbarLayoutSpec::TOP_GAP <= pin.rect.0,
            "icon/text toggle should not overlap the pin button"
        );
        assert!(
            pin.rect.0 + pin.rect.2 <= close.rect.0,
            "pin and close buttons should not overlap"
        );
        assert!(
            close.rect.0 + close.rect.2 <= w as f64,
            "close button should fit inside the top toolbar"
        );
    }
}

#[test]
fn build_side_hits_color_picker_height_tracks_palette_mode() {
    let mut state = create_test_input_state();
    state.show_more_colors = false;
    let snapshot = snapshot_from_state(&state);
    let mut hits = Vec::new();
    build_side_hits(260.0, 400.0, &snapshot, &mut hits);
    let picker_height = hits.iter().find_map(|hit| {
        if let HitKind::PickColor { h, .. } = hit.kind {
            Some(h)
        } else {
            None
        }
    });
    assert_eq!(picker_height, Some(24.0));

    state.show_more_colors = true;
    let snapshot = snapshot_from_state(&state);
    let mut hits = Vec::new();
    build_side_hits(260.0, 400.0, &snapshot, &mut hits);
    let picker_height = hits.iter().find_map(|hit| {
        if let HitKind::PickColor { h, .. } = hit.kind {
            Some(h)
        } else {
            None
        }
    });
    assert_eq!(picker_height, Some(54.0));
}

#[test]
fn side_header_static_hits_match_render_time_header_hits() {
    let mut state = create_test_input_state();
    state.toolbar_drawer_open = true;
    let snapshot = snapshot_from_state(&state);
    let (w, h) = side_size(&snapshot);

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let mut render_hits = Vec::new();
    crate::backend::wayland::toolbar::render_side_palette(
        &ctx,
        w as f64,
        h as f64,
        &snapshot,
        &mut render_hits,
        None,
        None,
    )
    .unwrap();

    let mut static_hits = Vec::new();
    build_side_hits(w as f64, h as f64, &snapshot, &mut static_hits);

    let header_len = 9;
    let render_header: Vec<_> = render_hits
        .iter()
        .take(header_len)
        .map(|hit| (event_name(&hit.event), format!("{:?}", hit.kind)))
        .collect();
    let static_header: Vec<_> = static_hits
        .iter()
        .take(header_len)
        .map(|hit| (event_name(&hit.event), format!("{:?}", hit.kind)))
        .collect();

    assert_eq!(static_header, render_header);
    assert_eq!(render_header[0].1, format!("{:?}", HitKind::DragMoveSide));
    assert!(
        render_header
            .iter()
            .any(|(event, _)| event == "ToggleBoardPicker")
    );
}

#[test]
fn side_settings_static_hits_include_model_controls() {
    let mut state = create_test_input_state();
    state.toolbar_drawer_open = true;
    state.toolbar_drawer_tab = ToolbarDrawerTab::Session;
    state.show_settings_section = true;
    state.toolbar_layout_mode = crate::config::ToolbarLayoutMode::Regular;
    let snapshot = snapshot_from_state(&state);
    let model = ToolbarSettingsModel::from_snapshot(&snapshot).expect("settings model");
    let expected: Vec<_> = model
        .toggles()
        .iter()
        .map(|toggle| activation_event_name(&toggle.activation))
        .chain(
            model
                .buttons()
                .iter()
                .map(|button| event_name(&button.event)),
        )
        .collect();

    let (w, h) = side_size(&snapshot);
    let mut hits = Vec::new();
    build_side_hits(w as f64, h as f64, &snapshot, &mut hits);
    let hit_events: Vec<_> = hits.iter().map(|hit| event_name(&hit.event)).collect();

    for expected_event in &expected {
        assert!(
            hit_events.contains(expected_event),
            "missing settings hit {expected_event}"
        );
    }
    assert!(hit_events.contains(&"ToggleContextAwareUi(false)".to_string()));
}

#[test]
fn side_session_static_hits_include_model_controls_and_recents() {
    let mut state = create_test_input_state();
    state.toolbar_drawer_open = true;
    state.toolbar_drawer_tab = ToolbarDrawerTab::Session;
    let mut snapshot = snapshot_from_state(&state);
    snapshot.active_session_path =
        Some(std::path::PathBuf::from("/tmp/current.wayscriber-session"));
    snapshot.active_session_name = Some("current.wayscriber-session".to_string());
    snapshot.recent_sessions = vec![SessionRecentSnapshot {
        display_name: "recent.wayscriber-session".to_string(),
        path: std::path::PathBuf::from("/tmp/recent.wayscriber-session"),
    }];
    let model = ToolbarSessionModel::from_snapshot(&snapshot).expect("session model");

    let (w, h) = side_size(&snapshot);
    let mut hits = Vec::new();
    build_side_hits(w as f64, h as f64, &snapshot, &mut hits);
    let hit_events: Vec<_> = hits.iter().map(|hit| event_name(&hit.event)).collect();

    for button in &model.buttons {
        assert!(
            hit_events.contains(&event_name(&button.event)),
            "missing session button hit {:?}",
            button.event
        );
    }
    assert!(hit_events.iter().any(|event| {
        event.contains("OpenRecentSession") && event.contains("recent.wayscriber-session")
    }));
}

#[test]
fn side_session_overwrite_confirmation_hits_replace_action_buttons() {
    let mut state = create_test_input_state();
    state.toolbar_drawer_open = true;
    state.toolbar_drawer_tab = ToolbarDrawerTab::App;
    let mut snapshot = snapshot_from_state(&state);
    let target = std::path::PathBuf::from("/tmp/existing.wayscriber-session");
    snapshot.active_session_path =
        Some(std::path::PathBuf::from("/tmp/current.wayscriber-session"));
    snapshot.active_session_name = Some("current.wayscriber-session".to_string());
    snapshot.pending_save_as_overwrite_path = Some(target.clone());

    let (w, h) = side_size(&snapshot);
    let mut static_hits = Vec::new();
    build_side_hits(w as f64, h as f64, &snapshot, &mut static_hits);
    assert_session_overwrite_confirmation_hits(&static_hits, &target);

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let mut rendered_hits = Vec::new();
    crate::backend::wayland::toolbar::render_side_palette(
        &ctx,
        w as f64,
        h as f64,
        &snapshot,
        &mut rendered_hits,
        None,
        None,
    )
    .unwrap();
    assert_session_overwrite_confirmation_hits(&rendered_hits, &target);
}

fn assert_session_overwrite_confirmation_hits(
    hits: &[crate::backend::wayland::toolbar::hit::HitRegion],
    target: &std::path::Path,
) {
    assert!(hits.iter().any(|hit| matches!(
        &hit.event,
        ToolbarEvent::SaveSessionAsConfirm(path) if path == target
    )));
    assert!(
        hits.iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::SaveSessionAsCancel))
    );
    assert!(
        !hits
            .iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::SaveSessionAs)),
        "pending overwrite prompt should replace the Save As action grid"
    );
}

#[test]
fn font_size_nudge_hits_use_slider_spec_step() {
    let mut state = create_test_input_state();
    state.show_text_controls = true;
    state.current_font_size = 32.0;
    let snapshot = snapshot_from_state(&state);
    let (w, h) = side_size(&snapshot);

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let mut hits = Vec::new();
    crate::backend::wayland::toolbar::render_side_palette(
        &ctx, w as f64, h as f64, &snapshot, &mut hits, None, None,
    )
    .unwrap();

    let step = ToolbarSliderSpec::FONT_SIZE.step.expect("font size step");
    let expected_minus = snapshot.font_size - step;
    let expected_plus = snapshot.font_size + step;

    assert!(hits.iter().any(|hit| matches!(
        hit.event,
        ToolbarEvent::SetFontSize(value) if (value - expected_minus).abs() < f64::EPSILON
    )));
    assert!(hits.iter().any(|hit| matches!(
        hit.event,
        ToolbarEvent::SetFontSize(value) if (value - expected_plus).abs() < f64::EPSILON
    )));
}

#[test]
fn top_size_scales_with_toolbar_scale() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    state.toolbar_scale = 1.0;
    let snapshot = snapshot_from_state(&state);
    let base_size = top_size(&snapshot);

    // Scale 1.5x should increase size proportionally
    state.toolbar_scale = 1.5;
    let snapshot = snapshot_from_state(&state);
    let scaled_size = top_size(&snapshot);
    assert_eq!(
        scaled_size.0,
        (base_size.0 as f64 * 1.5).ceil() as u32,
        "Width should scale by 1.5x"
    );
    assert_eq!(
        scaled_size.1,
        (base_size.1 as f64 * 1.5).ceil() as u32,
        "Height should scale by 1.5x"
    );

    // Scale 0.75x should decrease size
    state.toolbar_scale = 0.75;
    let snapshot = snapshot_from_state(&state);
    let small_size = top_size(&snapshot);
    assert!(
        small_size.0 < base_size.0,
        "Scaled down width should be smaller"
    );
    assert!(
        small_size.1 < base_size.1,
        "Scaled down height should be smaller"
    );
}

#[test]
fn scale_size_handles_non_finite_values() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    state.toolbar_scale = 1.0;
    let snapshot = snapshot_from_state(&state);
    let base_size = top_size(&snapshot);

    // NaN should fall back to 1.0
    state.toolbar_scale = f64::NAN;
    let snapshot = snapshot_from_state(&state);
    let nan_size = top_size(&snapshot);
    assert_eq!(nan_size, base_size, "NaN scale should fall back to 1.0");

    // Infinity should fall back to 1.0
    state.toolbar_scale = f64::INFINITY;
    let snapshot = snapshot_from_state(&state);
    let inf_size = top_size(&snapshot);
    assert_eq!(
        inf_size, base_size,
        "Infinity scale should fall back to 1.0"
    );

    // Negative infinity should fall back to 1.0
    state.toolbar_scale = f64::NEG_INFINITY;
    let snapshot = snapshot_from_state(&state);
    let neg_inf_size = top_size(&snapshot);
    assert_eq!(
        neg_inf_size, base_size,
        "Neg infinity scale should fall back to 1.0"
    );
}

#[test]
fn scale_size_clamps_extreme_values() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;

    // Test upper bound clamping (max 3.0)
    state.toolbar_scale = 10.0;
    let snapshot = snapshot_from_state(&state);
    let huge_size = top_size(&snapshot);

    state.toolbar_scale = 3.0;
    let snapshot = snapshot_from_state(&state);
    let max_size = top_size(&snapshot);
    assert_eq!(huge_size, max_size, "Scale > 3.0 should clamp to 3.0");

    // Test lower bound clamping (min 0.5)
    state.toolbar_scale = 0.1;
    let snapshot = snapshot_from_state(&state);
    let tiny_size = top_size(&snapshot);

    state.toolbar_scale = 0.5;
    let snapshot = snapshot_from_state(&state);
    let min_size = top_size(&snapshot);
    assert_eq!(tiny_size, min_size, "Scale < 0.5 should clamp to 0.5");
}
