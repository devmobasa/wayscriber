use super::*;

/// Status-bar content is chrome the user arranges from the overlay's settings
/// popover. It used to apply to the current run only, so every toggle came
/// back on the next launch; it now survives a restart the way the toolbars do,
/// as a runtime override layered over the configured value, with `config.toml`
/// still untouched.
#[test]
fn status_bar_content_survives_restart_without_touching_config() {
    use crate::config::StatusBarItem;

    const AUTHORED: &[u8] = b"# authored config bytes stay exact\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    assert!(
        input.ui_visibility.status_bar_interactive,
        "shipped default"
    );
    assert!(input.status_bar_item_visible(StatusBarItem::Size));

    let target = ToolbarRuntimeUiPersistenceTarget::StatusBarInteractive;
    let prepared = runtime
        .begin_toolbar_mutation(target, &input)
        .expect("interactivity permit");
    input.ui_visibility.status_bar_interactive = false;
    let finish = runtime.finish_toolbar_mutation(prepared, true, &input);
    assert!(matches!(finish, ToolbarRuntimeFinish::KeepPreview));

    let target = ToolbarRuntimeUiPersistenceTarget::StatusBarItem(StatusBarItem::Size);
    let prepared = runtime
        .begin_toolbar_mutation(target, &input)
        .expect("item permit");
    input.set_status_bar_item_visible(StatusBarItem::Size, false);
    let finish = runtime.finish_toolbar_mutation(prepared, true, &input);
    assert!(matches!(finish, ToolbarRuntimeFinish::KeepPreview));

    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);

    assert!(
        !restarted_input.ui_visibility.status_bar_interactive,
        "clickable-segments stays off across a restart"
    );
    assert!(
        !restarted_input.status_bar_item_visible(StatusBarItem::Size),
        "a hidden segment stays hidden across a restart"
    );
    assert!(
        restarted_input.status_bar_item_visible(StatusBarItem::Tool),
        "segments the user did not touch keep their configured value"
    );
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    restarted.shutdown_blocking();
}

/// The toolbar and status-bar preference toggles the overlay exposes are
/// chrome the user arranges, and they used to reset on every launch. Each now
/// survives a restart as a runtime override, with `config.toml` untouched.
#[test]
fn toolbar_preference_toggles_survive_restart_without_touching_config() {
    const AUTHORED: &[u8] = b"# authored config bytes stay exact\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    // Flip each away from its configured value.
    type Flip = (ToolbarRuntimeUiPersistenceTarget, fn(&mut InputState));
    let flips: Vec<Flip> = vec![
        (ToolbarRuntimeUiPersistenceTarget::StatusBar, |input| {
            input.ui_visibility.show_status_bar = !input.ui_visibility.show_status_bar;
        }),
        (ToolbarRuntimeUiPersistenceTarget::ToolbarIcons, |input| {
            input.toolbar_use_icons = !input.toolbar_use_icons;
        }),
        (
            ToolbarRuntimeUiPersistenceTarget::ToolbarContextAwareUi,
            |input| input.ui_visibility.context_aware_ui = !input.ui_visibility.context_aware_ui,
        ),
        (
            ToolbarRuntimeUiPersistenceTarget::ToolbarDelaySliders,
            |input| {
                input.ui_visibility.show_delay_sliders = !input.ui_visibility.show_delay_sliders
            },
        ),
        (
            ToolbarRuntimeUiPersistenceTarget::HistoryCustomSection,
            |input| {
                let enabled = !input.history_limits.custom_section_enabled();
                input.history_limits.set_custom_section_enabled(enabled);
            },
        ),
        (
            ToolbarRuntimeUiPersistenceTarget::FloatingBadgeAlways,
            |input| {
                input.ui_visibility.show_floating_badge_always =
                    !input.ui_visibility.show_floating_badge_always
            },
        ),
    ];

    for (target, flip) in &flips {
        let prepared = runtime
            .begin_toolbar_mutation(*target, &input)
            .unwrap_or_else(|| panic!("{target:?} permit"));
        flip(&mut input);
        let finish = runtime.finish_toolbar_mutation(prepared, true, &input);
        assert!(
            matches!(finish, ToolbarRuntimeFinish::KeepPreview),
            "{target:?}"
        );
    }
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);

    let expected_status_bar = input.ui_visibility.show_status_bar;
    let expected_icons = input.toolbar_use_icons;
    let expected_context = input.ui_visibility.context_aware_ui;
    let expected_sliders = input.ui_visibility.show_delay_sliders;
    let expected_custom = input.history_limits.custom_section_enabled();
    let expected_badge = input.ui_visibility.show_floating_badge_always;
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);

    assert_eq!(
        restarted_input.ui_visibility.show_status_bar,
        expected_status_bar
    );
    assert_eq!(restarted_input.toolbar_use_icons, expected_icons);
    assert_eq!(
        restarted_input.ui_visibility.context_aware_ui,
        expected_context
    );
    assert_eq!(
        restarted_input.ui_visibility.show_delay_sliders,
        expected_sliders
    );
    assert_eq!(
        restarted_input.history_limits.custom_section_enabled(),
        expected_custom
    );
    assert_eq!(
        restarted_input.ui_visibility.show_floating_badge_always,
        expected_badge
    );
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    restarted.shutdown_blocking();
}

/// A durable preference override is measured against a seed derived from the
/// authored config, so a seed refresh — a session load, a board change, an
/// output change — must leave it standing. If the toggle also moved the
/// effective config, the refreshed seed would arrive already equal to the
/// override and reconciliation would prune it as redundant, quietly undoing
/// the persistence at the next unrelated refresh.
#[test]
fn a_seed_refresh_does_not_prune_persisted_preference_overrides() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    let target = ToolbarRuntimeUiPersistenceTarget::ToolbarIcons;
    let prepared = runtime
        .begin_toolbar_mutation(target, &input)
        .expect("icon toggle permit");
    input.toolbar_use_icons = !input.toolbar_use_icons;
    let flipped = input.toolbar_use_icons;
    runtime.finish_toolbar_mutation(prepared, true, &input);
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());

    // Whatever else the run does, the seed baseline stays authored.
    let mut positions = ToolbarPositionSnapshot { top: (0.0, 0.0) };
    runtime.refresh_config_seeds(&config, &mut input, &mut positions);
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);
    assert_eq!(
        restarted_input.toolbar_use_icons, flipped,
        "the icon override must outlive an unrelated seed refresh"
    );
    restarted.shutdown_blocking();
}

/// Hiding a toolbar section is a durable choice, and it has to come back
/// without `config.toml` moving. A section the user never touched keeps
/// following the layout mode instead of being pinned by the restore.
#[test]
fn section_visibility_survives_restart_without_touching_config() {
    use crate::config::{ToolbarSectionFlag, resolve_section_visibility};

    const AUTHORED: &[u8] = b"# authored config bytes stay exact\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    let live = |input: &InputState, flag| {
        resolve_section_visibility(
            input.toolbar_layout_mode,
            &input.toolbar_mode_overrides,
            &input.resolved_toolbar_items,
        )
        .get(flag)
    };

    let toggled = ToolbarSectionFlag::Presets;
    let untouched = ToolbarSectionFlag::Boards;
    let untouched_before = live(&input, untouched);

    let target = ToolbarRuntimeUiPersistenceTarget::NamedSection(toggled);
    let prepared = runtime
        .begin_toolbar_mutation(target, &input)
        .expect("section toggle permit");
    let flipped = !live(&input, toggled);
    assert!(input.apply_toolbar_event(section_toggle(toggled, flipped)));
    let finish = runtime.finish_toolbar_mutation(prepared, true, &input);
    assert!(matches!(finish, ToolbarRuntimeFinish::KeepPreview));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);

    assert_eq!(
        live(&restarted_input, toggled),
        flipped,
        "the toggled section must come back hidden"
    );
    assert_eq!(live(&restarted_input, untouched), untouched_before);
    assert!(
        !restarted_input
            .toolbar_items
            .resolved()
            .hidden
            .contains(&untouched.item_id())
            && !restarted_input
                .toolbar_items
                .resolved()
                .shown
                .contains(&untouched.item_id()),
        "an untouched section keeps following the layout mode"
    );
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    restarted.shutdown_blocking();
}

/// Switching the toolbar layout preset is a durable choice. It restores
/// before the section settings do, so a section the user pinned keeps its own
/// visibility while every untouched section follows the restored preset.
#[test]
fn toolbar_layout_mode_survives_restart_without_touching_config() {
    use crate::config::{ToolbarLayoutMode, ToolbarSectionFlag, resolve_section_visibility};

    const AUTHORED: &[u8] = b"# authored config bytes stay exact\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let mut config = Config::default();
    config.ui.toolbar.layout_mode = ToolbarLayoutMode::Regular;
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    let pinned = ToolbarSectionFlag::Presets;
    let prepared = runtime
        .begin_toolbar_mutation(
            ToolbarRuntimeUiPersistenceTarget::NamedSection(pinned),
            &input,
        )
        .expect("section permit");
    assert!(input.apply_toolbar_event(section_toggle(pinned, false)));
    runtime.finish_toolbar_mutation(prepared, true, &input);

    let prepared = runtime
        .begin_toolbar_mutation(ToolbarRuntimeUiPersistenceTarget::LayoutMode, &input)
        .expect("layout mode permit");
    assert!(
        input.apply_toolbar_event(ToolbarEvent::SetToolbarLayoutMode(
            ToolbarLayoutMode::Advanced
        ))
    );
    let finish = runtime.finish_toolbar_mutation(prepared, true, &input);
    assert!(matches!(finish, ToolbarRuntimeFinish::KeepPreview));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);

    assert_eq!(
        restarted_input.toolbar_layout_mode,
        ToolbarLayoutMode::Advanced
    );
    let sections = resolve_section_visibility(
        restarted_input.toolbar_layout_mode,
        &restarted_input.toolbar_mode_overrides,
        &restarted_input.resolved_toolbar_items,
    );
    assert!(!sections.get(pinned), "the pinned section stays hidden");
    let baseline = ToolbarLayoutMode::Advanced;
    for flag in ToolbarSectionFlag::ALL {
        if flag == pinned {
            continue;
        }
        assert_eq!(
            sections.get(flag),
            flag.baseline(baseline, &restarted_input.toolbar_mode_overrides),
            "{flag:?} must follow the restored preset"
        );
    }
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    restarted.shutdown_blocking();
}
