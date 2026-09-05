use super::*;
use crate::config::{StatusBarItem, StatusBarStyle, StatusPosition};
use crate::ui::StatusHudSegmentKind;
use crate::ui_text::UiTextEngine;

#[test]
fn preference_reapply_and_rollback_refresh_text_geometry_before_another_frame() {
    let temp = crate::test_temp::tempdir().unwrap();
    let mut config = Config::default();
    config
        .ui
        .set_status_bar_item_visible(StatusBarItem::Help, true);
    let engine = UiTextEngine::default();
    let mut input = input_from_config(&config);
    let mut runtime = controller_only_runtime(&config, &temp.path().join("runtime-ui.toml"));
    runtime.apply_startup_state(&engine, &crate::draw::TextMeasurer::default(), &mut input);
    assert!(
        input.status_hud_layout().is_none(),
        "startup has no previous frame dimensions"
    );
    input.update_status_hud_layout_for_pointer_with_resources(
        crate::input::state::InputTextResources {
            measurer: &crate::draw::TextMeasurer::default(),
            ui_engine: &engine,
        },
        StatusPosition::BottomLeft,
        &StatusBarStyle::default(),
        1280,
        720,
        true,
    );
    let help = input
        .status_hud_layout()
        .unwrap()
        .segments
        .iter()
        .find(|s| s.kind == StatusHudSegmentKind::Help)
        .unwrap();
    let (x, y) = (
        (help.x + help.width / 2.0).round() as i32,
        (help.y + help.height / 2.0).round() as i32,
    );
    input.on_mouse_motion(x, y);
    assert_eq!(input.status_hud.hover(), Some(StatusHudSegmentKind::Help));
    let mut positions = ToolbarPositionSnapshot { top: (12.0, 24.0) };
    config
        .ui
        .set_status_bar_item_visible(StatusBarItem::Help, false);
    assert!(
        runtime
            .refresh_config_seeds(
                &engine,
                &crate::draw::TextMeasurer::default(),
                &config,
                &mut input,
                &mut positions
            )
            .applied
    );
    assert!(!input.status_bar_item_visible(StatusBarItem::Help));
    assert_eq!(input.status_hud.hover(), None);
    assert!(
        !input
            .status_hud_layout()
            .unwrap()
            .segments
            .iter()
            .any(|s| s.kind == StatusHudSegmentKind::Help)
    );
    let hidden_geometry = format!("{:?}", input.status_hud_layout());
    let rollback = PreviewRollbackSnapshot {
        values: BTreeMap::from([(
            InteractionSeedTarget::StatusBarItem(StatusBarItem::Help),
            InteractionSeedValue::Bool(true),
        )]),
        derive_toolbar_visibility_from_pins: false,
    };
    apply_toolbar_runtime_rollback(
        &engine,
        &crate::draw::TextMeasurer::default(),
        &mut input,
        &mut positions,
        &rollback,
    );
    assert!(input.status_bar_item_visible(StatusBarItem::Help));
    assert!(
        input
            .status_hud_layout()
            .unwrap()
            .segments
            .iter()
            .any(|s| s.kind == StatusHudSegmentKind::Help)
    );
    assert_ne!(format!("{:?}", input.status_hud_layout()), hidden_geometry);
    runtime.apply_live_state(
        &engine,
        &crate::draw::TextMeasurer::default(),
        &mut input,
        &mut positions,
    );
    assert!(!input.status_bar_item_visible(StatusBarItem::Help));
    assert_eq!(format!("{:?}", input.status_hud_layout()), hidden_geometry);
}
