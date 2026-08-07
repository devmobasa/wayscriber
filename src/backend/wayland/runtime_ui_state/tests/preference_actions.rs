use super::*;

/// The click highlight and its tool ring persist together, from the toolbar
/// and from the keyboard alike. The keyboard path applies inside `InputState`
/// before the backend sees it, so it hands its own pre-change values in as the
/// rollback rather than reading them back off the live state.
#[test]
fn click_highlight_survives_restart_from_either_path() {
    const AUTHORED: &[u8] = b"# authored config bytes stay exact\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    // The toolbar path: begin from the live state, then apply.
    let target = ToolbarRuntimeUiPersistenceTarget::ClickHighlight;
    let prepared = runtime
        .begin_toolbar_mutation(target, &input)
        .expect("click highlight permit");
    let ring = !input.highlight_tool_ring_enabled();
    assert!(input.apply_toolbar_event(ToolbarEvent::ToggleHighlightToolRing(ring)));
    runtime.finish_toolbar_mutation(prepared, true, &input);

    // The keyboard path: apply first, then persist from the snapshot taken
    // before the change.
    let previous_enabled = input.click_highlight_enabled();
    let previous_ring = input.highlight_tool_ring_enabled();
    let enabled = !previous_enabled;
    assert!(input.set_click_highlight_enabled(enabled));
    let rollback = click_highlight_values(previous_enabled, previous_ring).expect("valid rollback");
    let prepared = runtime
        .begin_toolbar_mutation_with_rollback(target, rollback)
        .expect("keyboard permit");
    let finish = runtime.finish_toolbar_mutation(prepared, true, &input);
    assert!(matches!(finish, ToolbarRuntimeFinish::KeepPreview));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);

    assert_eq!(restarted_input.click_highlight_enabled(), enabled);
    assert_eq!(restarted_input.highlight_tool_ring_enabled(), ring);
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    restarted.shutdown_blocking();
}

/// The status bar, the floating badge and the zoom chip are reachable only
/// from the keyboard, which applies inside `InputState` before the backend
/// sees the change. They still persist, through the same targets and the same
/// pre-change rollback the toolbar controls use.
#[test]
fn keyboard_only_chrome_toggles_survive_restart_without_touching_config() {
    const AUTHORED: &[u8] = b"# authored config bytes stay exact\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    // Each entry reads its own live value, flips it, and returns both -- the
    // shape the keyboard path has, where the change lands before the persist.
    type Flip = (
        ToolbarRuntimeUiPersistenceTarget,
        fn(&mut InputState) -> (bool, bool),
    );
    let flips: Vec<Flip> = vec![
        (ToolbarRuntimeUiPersistenceTarget::StatusBar, |input| {
            let previous = input.show_status_bar;
            input.show_status_bar = !previous;
            (previous, input.show_status_bar)
        }),
        (ToolbarRuntimeUiPersistenceTarget::FloatingBadge, |input| {
            let previous = input.show_floating_badge;
            input.show_floating_badge = !previous;
            (previous, input.show_floating_badge)
        }),
        (ToolbarRuntimeUiPersistenceTarget::ZoomChip, |input| {
            let previous = input.show_zoom_chip;
            input.show_zoom_chip = !previous;
            (previous, input.show_zoom_chip)
        }),
        (ToolbarRuntimeUiPersistenceTarget::InputHud, |input| {
            let previous = input.input_hud_enabled();
            input.set_input_hud_enabled(!previous);
            (previous, input.input_hud_enabled())
        }),
    ];

    let mut expected = Vec::new();
    for (target, flip) in flips {
        let seed =
            single_bool_seed_target(target).unwrap_or_else(|| panic!("{target:?} is a bool"));
        let (previous, now) = flip(&mut input);
        let rollback = RuntimeUiMutationValues::one(seed, InteractionSeedValue::Bool(previous))
            .expect("valid rollback");
        let prepared = runtime
            .begin_toolbar_mutation_with_rollback(target, rollback)
            .unwrap_or_else(|| panic!("{target:?} permit"));
        let finish = runtime.finish_toolbar_mutation(prepared, true, &input);
        assert!(
            matches!(finish, ToolbarRuntimeFinish::KeepPreview),
            "{target:?}"
        );
        expected.push(now);
    }
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);

    assert_eq!(restarted_input.show_status_bar, expected[0]);
    assert_eq!(restarted_input.show_floating_badge, expected[1]);
    assert_eq!(restarted_input.show_zoom_chip, expected[2]);
    assert_eq!(restarted_input.input_hud_enabled(), expected[3]);
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    restarted.shutdown_blocking();
}

/// A cancelled or rejected preview hands back a rollback snapshot, and the
/// live UI has to follow it. Every durable chrome preference is covered:
/// a target the rollback ignored would leave the screen showing a value the
/// controller already rejected, and the next start disagreeing with it.
#[test]
fn a_rollback_restores_every_durable_chrome_preference() {
    use crate::config::{ToolbarLayoutMode, ToolbarSectionFlag};

    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut positions = ToolbarPositionSnapshot { top: (0.0, 0.0) };

    // Move every preference away from its configured value, then roll back to
    // exactly the values the run started with.
    /// One boolean preference: its seed target, how to read it, how to flip it.
    type BoolPreference = (
        InteractionSeedTarget,
        fn(&InputState) -> bool,
        fn(&mut InputState),
    );
    let bools: Vec<BoolPreference> = vec![
        (
            InteractionSeedTarget::StatusBar,
            |i| i.show_status_bar,
            |i| i.show_status_bar = !i.show_status_bar,
        ),
        (
            InteractionSeedTarget::StatusBoardBadge,
            |i| i.show_status_board_badge,
            |i| i.show_status_board_badge = !i.show_status_board_badge,
        ),
        (
            InteractionSeedTarget::StatusPageBadge,
            |i| i.show_status_page_badge,
            |i| i.show_status_page_badge = !i.show_status_page_badge,
        ),
        (
            InteractionSeedTarget::FloatingBadgeAlways,
            |i| i.show_floating_badge_always,
            |i| i.show_floating_badge_always = !i.show_floating_badge_always,
        ),
        (
            InteractionSeedTarget::FloatingBadge,
            |i| i.show_floating_badge,
            |i| i.show_floating_badge = !i.show_floating_badge,
        ),
        (
            InteractionSeedTarget::ZoomChip,
            |i| i.show_zoom_chip,
            |i| i.show_zoom_chip = !i.show_zoom_chip,
        ),
        (
            InteractionSeedTarget::ToolbarIcons,
            |i| i.toolbar_use_icons,
            |i| i.toolbar_use_icons = !i.toolbar_use_icons,
        ),
        (
            InteractionSeedTarget::ToolbarMoreColors,
            |i| i.show_more_colors,
            |i| i.show_more_colors = !i.show_more_colors,
        ),
        (
            InteractionSeedTarget::ToolbarContextAwareUi,
            |i| i.context_aware_ui,
            |i| i.context_aware_ui = !i.context_aware_ui,
        ),
        (
            InteractionSeedTarget::ToolbarPresetToasts,
            |i| i.show_preset_toasts,
            |i| i.show_preset_toasts = !i.show_preset_toasts,
        ),
        (
            InteractionSeedTarget::ToolbarToolPreview,
            |i| i.show_tool_preview,
            |i| i.show_tool_preview = !i.show_tool_preview,
        ),
        (
            InteractionSeedTarget::ToolbarDelaySliders,
            |i| i.show_delay_sliders,
            |i| i.show_delay_sliders = !i.show_delay_sliders,
        ),
        (
            InteractionSeedTarget::HistoryCustomSection,
            |i| i.custom_section_enabled,
            |i| i.custom_section_enabled = !i.custom_section_enabled,
        ),
        (
            InteractionSeedTarget::InputHud,
            |i| i.input_hud_enabled(),
            |i| {
                let enabled = !i.input_hud_enabled();
                i.set_input_hud_enabled(enabled);
            },
        ),
        (
            InteractionSeedTarget::ClickHighlight,
            |i| i.click_highlight_enabled(),
            |i| {
                let enabled = !i.click_highlight_enabled();
                i.set_click_highlight_enabled(enabled);
            },
        ),
        (
            InteractionSeedTarget::ClickHighlightToolRing,
            |i| i.highlight_tool_ring_enabled(),
            |i| {
                let enabled = !i.highlight_tool_ring_enabled();
                i.set_highlight_tool_ring_enabled(enabled);
            },
        ),
    ];

    let mut values = std::collections::BTreeMap::new();
    for (target, read, _) in &bools {
        values.insert(target.clone(), InteractionSeedValue::Bool(read(&input)));
    }
    let original_layout = input.toolbar_layout_mode;
    values.insert(
        InteractionSeedTarget::ToolbarLayoutMode,
        InteractionSeedValue::LayoutMode(original_layout),
    );
    values.insert(
        InteractionSeedTarget::SectionVisibility(ToolbarSectionFlag::Presets),
        InteractionSeedValue::Visibility(crate::config::ToolbarItemVisibilitySetting::Default),
    );

    for (_, _, flip) in &bools {
        flip(&mut input);
    }
    input.apply_toolbar_event(ToolbarEvent::SetToolbarLayoutMode(
        ToolbarLayoutMode::Advanced,
    ));
    input.apply_toolbar_event(section_toggle(ToolbarSectionFlag::Presets, false));

    apply_toolbar_runtime_rollback(
        &mut input,
        &mut positions,
        &PreviewRollbackSnapshot {
            values,
            derive_toolbar_visibility_from_pins: false,
        },
    );

    let restarted = input_from_config(&config);
    for (target, read, _) in &bools {
        assert_eq!(
            read(&input),
            read(&restarted),
            "{target:?} was not rolled back"
        );
    }
    assert_eq!(input.toolbar_layout_mode, original_layout);
    assert_eq!(
        crate::config::item_visibility_setting(
            &input.resolved_toolbar_items,
            ToolbarSectionFlag::Presets.item_id()
        ),
        crate::config::ToolbarItemVisibilitySetting::Default,
        "the section override was not rolled back"
    );
}
