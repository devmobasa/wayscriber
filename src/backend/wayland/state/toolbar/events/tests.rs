use super::session::{
    SessionFileDialogMode, SessionFileDialogResult, choose_session_file_from, default_save_as_path,
    ensure_save_as_extension, forget_missing_recent_session_after_open_error, save_as_file_name,
    session_info_summary,
};
use super::*;

// Readability aliases for the tests below: each popover's dismissal rule is
// now derived from one ownership table, and these keep the assertions phrased
// per popover.
fn event_dismisses_top_overflow(event: &ToolbarEvent) -> bool {
    event_dismisses_popover(event, ToolbarPopover::TopOverflow)
}

fn event_dismisses_shape_picker(event: &ToolbarEvent) -> bool {
    event_dismisses_popover(event, ToolbarPopover::ShapePicker)
}

fn event_dismisses_precision_entry(event: &ToolbarEvent) -> bool {
    event_dismisses_popover(event, ToolbarPopover::PrecisionEntry)
}

fn event_dismisses_session_popover(event: &ToolbarEvent) -> bool {
    event_dismisses_popover(event, ToolbarPopover::Session)
}

fn event_dismisses_settings_popover(event: &ToolbarEvent) -> bool {
    event_dismisses_popover(event, ToolbarPopover::Settings)
}
use crate::backend::wayland::runtime_ui_state::{user_click_highlight_enabled, user_tool_preview};
use crate::config::{Action, StatusBarItem, ToolbarLayoutMode, ToolbarSectionFlag};
use crate::draw::{Color, FontDescriptor};
use crate::env_vars::XDG_DATA_HOME_ENV;
use crate::input::state::test_support::make_test_input_state;
use crate::input::{EraserMode, Tool};
use anyhow::anyhow;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

use super::feedback::ToolbarPinDurability;

struct EnvGuard {
    _guard: MutexGuard<'static, ()>,
    xdg_data_home: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set_xdg_data_home(path: &Path) -> Self {
        let guard = crate::test_env::lock();
        let xdg_data_home = std::env::var_os(XDG_DATA_HOME_ENV);
        unsafe {
            std::env::set_var(XDG_DATA_HOME_ENV, path);
        }
        Self {
            _guard: guard,
            xdg_data_home,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.xdg_data_home.take() {
            Some(value) => unsafe { std::env::set_var(XDG_DATA_HOME_ENV, value) },
            None => unsafe { std::env::remove_var(XDG_DATA_HOME_ENV) },
        }
    }
}

fn persistence_for(event: &ToolbarEvent) -> ToolbarPersistence {
    ToolbarEventPolicy::for_event(event).persistence
}

#[test]
fn pin_confirmations_distinguish_persistent_and_live_only_changes() {
    let cases = [
        (
            ToolbarPinChange::Top(true),
            ToolbarPinDurability::StartupPersistent,
            "Top toolbar will open at startup",
        ),
        (
            ToolbarPinChange::Top(false),
            ToolbarPinDurability::StartupPersistent,
            "Top toolbar will be hidden at startup",
        ),
        (
            ToolbarPinChange::Top(true),
            ToolbarPinDurability::LiveOnly,
            "Top toolbar pinned for this run only",
        ),
        (
            ToolbarPinChange::Top(false),
            ToolbarPinDurability::LiveOnly,
            "Top toolbar unpinned for this run only",
        ),
    ];

    for (change, durability, expected) in cases {
        assert_eq!(change.message(durability), expected);
    }
}

#[test]
fn unavailable_runtime_controller_uses_live_only_pin_confirmation() {
    assert_eq!(pin_durability(None), ToolbarPinDurability::LiveOnly);
}

#[test]
fn runtime_toolbar_events_do_not_directly_save_config() {
    let events = vec![
        ToolbarEvent::SelectTool(Tool::Line),
        ToolbarEvent::SetColor(Color {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 1.0,
        }),
        ToolbarEvent::SetQuickColor {
            color: Color {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0,
            },
            action: Some(crate::config::Action::SetColorRed),
            index: 0,
        },
        ToolbarEvent::SetThickness(8.0),
        ToolbarEvent::NudgeThickness(1.0),
        ToolbarEvent::SetMarkerOpacity(0.5),
        ToolbarEvent::NudgeMarkerOpacity(0.1),
        ToolbarEvent::SetEraserMode(EraserMode::Stroke),
        ToolbarEvent::SetFont(FontDescriptor::new(
            "Monospace".to_string(),
            "normal".to_string(),
            "italic".to_string(),
        )),
        ToolbarEvent::SetFontSize(44.0),
        ToolbarEvent::NudgeFontSize(2.0),
        ToolbarEvent::ToggleFill(true),
        ToolbarEvent::ApplyPreset(1),
        ToolbarEvent::OpenSession,
        ToolbarEvent::OpenRecentSession(std::path::PathBuf::from("/tmp/recent.wayscriber-session")),
        ToolbarEvent::SaveSessionAs,
        ToolbarEvent::SaveSessionAsConfirm(std::path::PathBuf::from(
            "/tmp/existing.wayscriber-session",
        )),
        ToolbarEvent::SaveSessionAsCancel,
        ToolbarEvent::SessionInfo,
        ToolbarEvent::ClearSession,
        // The overflow-anchored Session/Settings popovers are runtime-only
        // flyout state, like the overflow toggle itself.
        ToolbarEvent::ToggleSessionPopover(true),
        ToolbarEvent::ToggleSettingsPopover(true),
        ToolbarEvent::ScrollTopPopover(24.0),
    ];

    for event in events {
        assert_eq!(
            persistence_for(&event),
            ToolbarPersistence::Ephemeral,
            "{event:?} should not directly save config"
        );
    }
}

#[test]
fn backend_session_dispatch_finalizes_spotlight_history_and_its_deadline() {
    let mut input_state = make_test_input_state();
    let shape_id = input_state
        .boards
        .active_frame_mut()
        .add_shape(crate::draw::Shape::Spotlight {
            cx: 200,
            cy: 200,
            rx: 60,
            ry: 40,
            magnification: 2.0,
        });
    assert_eq!(
        input_state.nudge_spotlight_magnification_at(200, 200, 1),
        crate::input::state::SpotlightWheelOutcome::Adjusted
    );
    let mut deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs(1));

    assert_eq!(
        handle_toolbar_event_preflight(
            &mut input_state,
            &mut deadline,
            &ToolbarEvent::ClearSession,
            false,
        ),
        ToolbarEventPreflight::Continue
    );

    assert!(deadline.is_none());
    input_state.handle_action(Action::Undo);
    let magnification = match input_state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("spotlight")
        .shape
    {
        crate::draw::Shape::Spotlight { magnification, .. } => magnification,
        ref other => panic!("expected a spotlight, got {other:?}"),
    };
    assert_eq!(
        magnification, 2.0,
        "the session persistence barrier must see the gesture's undo entry"
    );
}

#[test]
fn toolbar_runtime_preferences_have_exact_runtime_state_targets() {
    use crate::config::{
        ToolbarItemOrderGroup, ToolbarItemVisibilitySetting, toolbar_item_ids as ids,
    };
    use ToolbarRuntimeUiPersistenceTarget as Runtime;

    let events = vec![
        (ToolbarEvent::PinTopToolbar(true), Runtime::TopPinned),
        (ToolbarEvent::SetTopMinimized(true), Runtime::TopMinimized),
        (ToolbarEvent::CloseTopToolbar, Runtime::TopMinimized),
        (
            ToolbarEvent::SetToolbarItemHidden(ids::TOP_TOOL_PEN, true),
            Runtime::ItemVisibility {
                id: ids::TOP_TOOL_PEN,
                setting: ToolbarItemVisibilitySetting::Hidden,
            },
        ),
        (
            ToolbarEvent::MoveToolbarItem {
                group: ToolbarItemOrderGroup::TopTools,
                id: ids::TOP_TOOL_PEN,
                delta: 1,
            },
            Runtime::ItemOrder(ToolbarItemOrderGroup::TopTools),
        ),
        (
            ToolbarEvent::ResetToolbarItemOrder(ToolbarItemOrderGroup::TopTools),
            Runtime::ItemOrder(ToolbarItemOrderGroup::TopTools),
        ),
        (
            ToolbarEvent::ResetToolbarItemHiddenOverrides,
            Runtime::ResetItemVisibility,
        ),
    ];

    for (event, target) in events {
        assert_eq!(
            persistence_for(&event),
            ToolbarPersistence::RuntimeUi(target),
            "{event:?} should save only its runtime UI target"
        );
    }

    assert_eq!(
        persistence_for(&ToolbarEvent::DragToolbarItemOver {
            group: ToolbarItemOrderGroup::TopTools,
            target_index: 2,
        }),
        ToolbarPersistence::Ephemeral
    );
}

/// Status-bar content is chrome the user arranges from the overlay, so - like
/// the toolbars themselves - it survives a restart as a runtime override
/// layered over the configured value.
#[test]
fn status_bar_content_persists_as_runtime_ui_state() {
    use ToolbarRuntimeUiPersistenceTarget as Runtime;

    assert_eq!(
        persistence_for(&ToolbarEvent::SetStatusBarInteractive(false)),
        ToolbarPersistence::RuntimeUi(Runtime::StatusBarInteractive),
    );

    for item in StatusBarItem::ALL {
        assert_eq!(
            persistence_for(&ToolbarEvent::SetStatusBarItemVisible(item, false)),
            ToolbarPersistence::RuntimeUi(Runtime::StatusBarItem(item)),
            "{item:?} must survive a restart"
        );
    }
}

/// A section row carries a whole section's visibility, not one item's
/// override, so it persists under its own target rather than as an
/// `ItemVisibility` entry -- the seed registry deliberately grows no
/// `ItemVisibility` seed for a section id. Every other item keeps its item
/// override.
#[test]
fn named_section_visibility_persists_under_its_own_target() {
    use crate::config::{ToolbarItemVisibilitySetting, toolbar_item_ids as ids};

    for flag in ToolbarSectionFlag::ALL {
        for event in [
            ToolbarEvent::SetToolbarItemHidden(flag.item_id(), true),
            section_toggle_event(flag, false),
        ] {
            assert_eq!(
                persistence_for(&event),
                ToolbarPersistence::RuntimeUi(ToolbarRuntimeUiPersistenceTarget::NamedSection(
                    flag
                )),
                "{flag:?} must survive a restart"
            );
        }
    }

    let individual = ToolbarEvent::SetToolbarItemHidden(ids::TOP_TOOL_PEN, true);
    assert_eq!(
        persistence_for(&individual),
        ToolbarPersistence::RuntimeUi(ToolbarRuntimeUiPersistenceTarget::ItemVisibility {
            id: ids::TOP_TOOL_PEN,
            setting: ToolbarItemVisibilitySetting::Hidden,
        })
    );
}

/// Presenter mode owns the click highlight and the tool preview while it
/// runs, so what persists is the user's own value -- which is what the mode
/// restores on exit -- not the mode's.
#[test]
fn presenter_mode_persists_the_users_values_not_its_own() {
    let mut input_state = make_test_input_state();
    input_state.presenter_mode_config.enable_click_highlight = true;
    input_state.presenter_mode_config.hide_tool_preview = true;
    input_state.ui_visibility.show_tool_preview = true;
    input_state.toggle_presenter_mode();
    assert!(input_state.presenter_mode);
    assert!(input_state.click_highlight_enabled());

    assert!(
        !user_click_highlight_enabled(&input_state),
        "presenter mode must not persist its own forced value"
    );
    assert!(
        user_tool_preview(&input_state),
        "the tool preview presenter hid is still the user's own"
    );

    // Leaving presenter mode restores the user's values, and a later toggle
    // is the user's own again.
    input_state.toggle_presenter_mode();
    assert!(!input_state.presenter_mode);
    assert!(input_state.toggle_click_highlight());
    assert!(user_click_highlight_enabled(&input_state));
}

/// The ring is the user's either way: presenter mode never forces it, so it
/// persists its runtime value even while the mode holds the enabled flag.
#[test]
fn presenter_mode_still_follows_the_highlight_ring_preference() {
    let mut input_state = make_test_input_state();
    input_state.presenter_mode_config.enable_click_highlight = true;
    input_state.toggle_presenter_mode();
    assert!(input_state.set_highlight_tool_ring_enabled(true));

    assert!(input_state.highlight_tool_ring_enabled());
    assert!(!user_click_highlight_enabled(&input_state));
}

/// No overlay control writes `config.toml`, and none writes the effective
/// config either. Both matter: the file is an authored input, and the
/// effective config is the baseline every durable override is measured
/// against, so a control that moved it would hand reconciliation a seed
/// already equal to its own override and lose it at the next seed refresh.
#[test]
fn overlay_preference_toggles_leave_both_configs_untouched() {
    crate::config::test_helpers::with_temp_config_home(|home| {
        let path = home.join("wayscriber").join("config.toml");
        fs::create_dir_all(path.parent().expect("config dir")).expect("config dir");
        fs::write(
            &path,
            "# authored by hand\n[ui.toolbar]\nuse_icons = true\nlayout_mode = \"regular\"\n",
        )
        .expect("test config should be written");
        let snapshot = crate::config::test_helpers::ConfigFileSnapshot::capture(&path);

        let config = crate::config::Config::default();
        let mut input_state = make_test_input_state();
        for event in [
            ToolbarEvent::ToggleIconMode(false),
            ToolbarEvent::ToggleMoreColors(true),
            ToolbarEvent::ToggleContextAwareUi(false),
            ToolbarEvent::TogglePresetToasts(false),
            ToolbarEvent::ToggleIdleFade(false),
            ToolbarEvent::ToggleDelaySliders(true),
            ToolbarEvent::ToggleCustomSection(true),
            ToolbarEvent::ToggleInputHud(true),
            ToolbarEvent::ToggleStatusBar(false),
            ToolbarEvent::SetStatusBarInteractive(false),
            ToolbarEvent::ToggleStatusBoardBadge(false),
            ToolbarEvent::ToggleStatusPageBadge(false),
            ToolbarEvent::ToggleFloatingBadgeAlways(true),
            ToolbarEvent::SetToolbarLayoutMode(ToolbarLayoutMode::Advanced),
            ToolbarEvent::TogglePresets(false),
            ToolbarEvent::ToggleAllHighlight(true),
        ] {
            input_state.apply_toolbar_event(event);
        }

        let untouched = crate::config::Config::default();
        assert_eq!(config.ui.toolbar.use_icons, untouched.ui.toolbar.use_icons);
        assert_eq!(
            config.ui.toolbar.show_more_colors,
            untouched.ui.toolbar.show_more_colors
        );
        assert_eq!(
            config.ui.toolbar.context_aware_ui,
            untouched.ui.toolbar.context_aware_ui
        );
        assert_eq!(
            config.ui.toolbar.layout_mode,
            untouched.ui.toolbar.layout_mode
        );
        assert_eq!(
            config.ui.toolbar.show_presets,
            untouched.ui.toolbar.show_presets
        );
        assert_eq!(config.ui.toolbar.items, untouched.ui.toolbar.items);
        assert_eq!(config.ui.show_status_bar, untouched.ui.show_status_bar);
        assert_eq!(
            config.ui.status_bar_interactive,
            untouched.ui.status_bar_interactive
        );
        assert_eq!(
            config.ui.show_status_board_badge,
            untouched.ui.show_status_board_badge
        );
        assert_eq!(config.ui.input_hud.enabled, untouched.ui.input_hud.enabled);
        assert_eq!(
            config.ui.click_highlight.enabled,
            untouched.ui.click_highlight.enabled
        );
        assert_eq!(
            config.history.custom_section_enabled,
            untouched.history.custom_section_enabled
        );
        snapshot.assert_unchanged("flipping every overlay preference toggle");
    });
}

/// The rebind gesture arms the capture modal for the control's own action, so
/// the next chord the user presses rebinds that control and not the last row
/// the palette happened to select.
#[test]
fn the_toolbar_rebind_gesture_opens_capture_for_the_controls_action() {
    let mut input_state = make_test_input_state();

    for (event, expected) in [
        (ToolbarEvent::SelectTool(Tool::Pen), Action::SelectPenTool),
        (ToolbarEvent::Undo, Action::Undo),
        (
            ToolbarEvent::ClearCanvas { instant: false },
            Action::ClearCanvas,
        ),
    ] {
        let action = crate::ui::toolbar::model::action_for_event(&event)
            .unwrap_or_else(|| panic!("{event:?} should name an action"));
        assert_eq!(action, expected, "{event:?} names the wrong action");

        let mut deadline = None;
        assert_eq!(
            handle_toolbar_event_preflight(&mut input_state, &mut deadline, &event, true),
            ToolbarEventPreflight::RebindCaptured
        );
        assert_eq!(
            input_state.keybinding_capture_action(),
            Some(expected),
            "{event:?} should arm capture for its own action"
        );
        assert!(
            input_state.take_pending_keybinding_edits().is_empty(),
            "arming capture must not queue an edit on its own"
        );
        input_state.on_key_press(crate::input::Key::Escape);
    }
}

#[test]
fn toolbar_rebind_capture_finalizes_a_held_arrow_bend_before_its_early_return() {
    let mut input_state = make_test_input_state();
    let shape_id = input_state
        .boards
        .active_frame_mut()
        .add_shape(crate::draw::Shape::Arrow {
            x1: 0,
            y1: 100,
            x2: 400,
            y2: 100,
            color: input_state.style.current_color,
            thick: 4.0,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            head_at_end: true,
            style: crate::draw::ArrowStyle::Curved,
            bend: 0.0,
            label: None,
        });
    input_state.set_selection(vec![shape_id]);
    input_state.state = crate::input::state::DrawingState::BendingArrow {
        shape_id,
        snapshot: crate::draw::frame::ShapeSnapshot {
            shape: input_state
                .boards
                .active_frame()
                .shape(shape_id)
                .expect("arrow")
                .shape
                .clone(),
            locked: false,
        },
    };
    assert!(input_state.drag_arrow_bend_to(200, 20, false));
    let mut deadline = None;
    let event = ToolbarEvent::Undo;

    assert_eq!(
        handle_toolbar_event_preflight(&mut input_state, &mut deadline, &event, true),
        ToolbarEventPreflight::RebindCaptured
    );
    assert_eq!(input_state.keybinding_capture_action(), Some(Action::Undo));
    assert!(
        matches!(input_state.state, crate::input::state::DrawingState::Idle),
        "rebind capture returned while the bend gesture was still active"
    );

    // Escape belongs to the newly-opened capture modal, and the later pointer
    // release must not revive or recommit the already-finalized bend.
    input_state.on_key_press(crate::input::Key::Escape);
    assert!(input_state.keybinding_capture_action().is_none());
    input_state.on_mouse_release(crate::input::MouseButton::Left, 200, 20);
    input_state.handle_action(Action::Undo);
    match input_state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("arrow")
        .shape
    {
        crate::draw::Shape::Arrow { bend, .. } => assert_eq!(bend, 0.0),
        ref other => panic!("expected an arrow, got {other:?}"),
    }
}

/// The configurator route the palette's Ctrl+Shift+E uses: the same actions
/// resolve to the configurator section that holds their shortcut.
#[test]
fn the_toolbar_controls_actions_resolve_to_their_keybindings_section() {
    use crate::configurator_destination::keybindings_destination_for_action;

    for (event, expected) in [
        (
            ToolbarEvent::SelectTool(Tool::Pen),
            "keybindings/tools?search=select pen tool",
        ),
        (ToolbarEvent::Undo, "keybindings/history?search=undo"),
        (
            ToolbarEvent::ClearCanvas { instant: false },
            "keybindings/drawing?search=clear canvas",
        ),
    ] {
        let action = crate::ui::toolbar::model::action_for_event(&event)
            .unwrap_or_else(|| panic!("{event:?} should name an action"));
        assert_eq!(
            keybindings_destination_for_action(action)
                .map(|destination| destination.as_arg())
                .as_deref(),
            Some(expected),
            "{event:?} would open the wrong screen"
        );
    }
}

#[test]
fn command_palette_and_shortcut_capture_block_shared_toolbar_events() {
    let mut input_state = make_test_input_state();
    assert!(!toolbar_event_blocked_by_modal(&input_state));

    input_state.toggle_command_palette();
    assert!(toolbar_event_blocked_by_modal(&input_state));

    input_state.toggle_command_palette();
    assert!(input_state.begin_keybinding_capture(Action::Undo));
    assert!(toolbar_event_blocked_by_modal(&input_state));
}

fn failing_session_file_chooser(
    _mode: SessionFileDialogMode,
    _current_path: Option<&Path>,
) -> Result<Option<SessionFileDialogResult>> {
    Err(anyhow!("zenity failed"))
}

fn missing_session_file_chooser(
    _mode: SessionFileDialogMode,
    _current_path: Option<&Path>,
) -> Result<Option<SessionFileDialogResult>> {
    Ok(None)
}

fn selecting_session_file_chooser(
    _mode: SessionFileDialogMode,
    _current_path: Option<&Path>,
) -> Result<Option<SessionFileDialogResult>> {
    Ok(Some(SessionFileDialogResult::Selected(PathBuf::from(
        "/tmp/selected.wayscriber-session",
    ))))
}

#[test]
fn session_file_chooser_falls_back_after_backend_error() {
    let selected = choose_session_file_from(
        SessionFileDialogMode::Open,
        None,
        &[failing_session_file_chooser, selecting_session_file_chooser],
    )
    .expect("fallback chooser should succeed");

    assert_eq!(
        selected,
        Some(PathBuf::from("/tmp/selected.wayscriber-session"))
    );
}

#[test]
fn session_file_chooser_reports_errors_after_all_backends_fail() {
    let err = choose_session_file_from(
        SessionFileDialogMode::Open,
        None,
        &[failing_session_file_chooser, missing_session_file_chooser],
    )
    .expect_err("all chooser failures should be reported");

    assert!(format!("{err:#}").contains("zenity failed"));
}

#[test]
fn default_session_save_as_path_uses_visible_dir_and_session_extension() {
    let path = default_save_as_path(Some(Path::new("/tmp/lecture.wayscriber-session")));

    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("lecture-copy.wayscriber-session")
    );
}

#[test]
fn save_as_file_name_normalizes_extensionless_auto_session_names() {
    assert_eq!(
        save_as_file_name(Some(Path::new(
            "session-wayland_1-DP_3_ASUSTek_COMPUTER_INC_PC32UCDP"
        ))),
        "session-wayland_1-DP_3_ASUSTek_COMPUTER_INC_PC32UCDP-copy.wayscriber-session"
    );
}

#[test]
fn save_as_file_name_replaces_existing_extension_with_session_extension() {
    assert_eq!(
        save_as_file_name(Some(Path::new("lecture.session"))),
        "lecture-copy.wayscriber-session"
    );
}

#[test]
fn save_as_dialog_selection_adds_session_extension_when_missing() {
    assert_eq!(
        ensure_save_as_extension(PathBuf::from("/tmp/lecture-copy")),
        PathBuf::from("/tmp/lecture-copy.wayscriber-session")
    );
}

#[test]
fn save_as_dialog_selection_keeps_explicit_extension() {
    assert_eq!(
        ensure_save_as_extension(PathBuf::from("/tmp/lecture.session")),
        PathBuf::from("/tmp/lecture.session")
    );
}

#[test]
fn missing_recent_open_error_forgets_catalog_entry() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let missing = temp.path().join("missing.wayscriber-session");
    crate::session::catalog::upsert_session_event(
        &missing,
        crate::session::catalog::CatalogEvent::Opened,
    )
    .expect("catalog stale recent");

    let err = crate::session::validate_named_session_file_for_open(&missing)
        .expect_err("missing session should fail open validation");

    assert!(forget_missing_recent_session_after_open_error(
        &missing, &err
    ));
    assert!(
        crate::session::catalog::recent_sessions()
            .expect("recent sessions")
            .is_empty(),
        "missing Open Recent target should be removed from catalog"
    );
    assert!(
        !missing.exists(),
        "forgetting a stale recent must not create or delete session artifacts"
    );
}

#[test]
fn missing_recent_parent_open_error_forgets_catalog_entry() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let deleted_parent = temp.path().join("deleted-parent");
    fs::create_dir(&deleted_parent).expect("deleted parent");
    let missing = deleted_parent.join("missing.wayscriber-session");
    crate::session::catalog::upsert_session_event(
        &missing,
        crate::session::catalog::CatalogEvent::Opened,
    )
    .expect("catalog stale recent");
    fs::remove_dir(&deleted_parent).expect("remove stale parent");

    let err = crate::session::validate_named_session_file_for_open(&missing)
        .expect_err("missing parent should fail open validation");

    assert!(forget_missing_recent_session_after_open_error(
        &missing, &err
    ));
    assert!(
        crate::session::catalog::recent_sessions()
            .expect("recent sessions")
            .is_empty(),
        "Open Recent target with a removed parent should be removed from catalog"
    );
}

#[test]
fn non_missing_recent_open_error_keeps_catalog_entry() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let directory = temp.path().join("directory.wayscriber-session");
    fs::create_dir(&directory).expect("directory-shaped session target");
    crate::session::catalog::upsert_session_event(
        &directory,
        crate::session::catalog::CatalogEvent::Opened,
    )
    .expect("catalog non-regular recent");

    let err = crate::session::validate_named_session_file_for_open(&directory)
        .expect_err("directory session should fail open validation");

    assert!(!forget_missing_recent_session_after_open_error(
        &directory, &err
    ));
    let recents = crate::session::catalog::recent_sessions().expect("recent sessions");
    assert_eq!(recents.len(), 1);
    assert_eq!(PathBuf::from(&recents[0].path), directory);
}

fn inspection_for_summary(path: &str) -> crate::session::SessionInspection {
    crate::session::SessionInspection {
        session_path: PathBuf::from(path),
        exists: true,
        size_bytes: Some(14_600),
        modified: None,
        backup_path: PathBuf::from(format!("{path}.bak")),
        backup_exists: false,
        backup_size_bytes: None,
        active_identity: None,
        per_output: false,
        persist_transparent: true,
        persist_whiteboard: true,
        persist_blackboard: true,
        persist_history: true,
        restore_tool_state: true,
        history_limit: None,
        frame_counts: Some(crate::session::FrameCounts {
            transparent: 3,
            whiteboard: 2,
            blackboard: 1,
        }),
        history_counts: None,
        history_present: true,
        tool_state_present: true,
        compressed: true,
        file_version: Some(1),
    }
}

#[test]
fn session_info_summary_reports_saved_counts() {
    let inspection = inspection_for_summary("/tmp/lecture.wayscriber-session");

    assert_eq!(
        session_info_summary(&inspection),
        "Session lecture.wayscriber-session: 14.3 KiB, shapes T/W/B 3/2/1, history"
    );
}

#[test]
fn session_info_summary_reports_missing_session() {
    let mut inspection = inspection_for_summary("/tmp/missing.wayscriber-session");
    inspection.exists = false;
    inspection.size_bytes = None;
    inspection.frame_counts = None;
    inspection.history_present = false;

    assert_eq!(
        session_info_summary(&inspection),
        "Session missing.wayscriber-session: no saved file yet"
    );
}

#[test]
fn session_info_summary_reports_backup_without_primary() {
    let mut inspection = inspection_for_summary("/tmp/recovered.wayscriber-session");
    inspection.exists = false;
    inspection.size_bytes = None;
    inspection.backup_exists = true;
    inspection.backup_size_bytes = Some(4096);

    assert_eq!(
        session_info_summary(&inspection),
        "Session recovered.wayscriber-session: no primary file, backup 4.0 KiB"
    );
}

#[test]
fn shape_picker_survives_its_own_inline_options() {
    // The Shapes popover hosts the Fill checkbox and the polygon-sides stepper,
    // so using them must not dismiss the popover...
    assert!(!event_dismisses_shape_picker(&ToolbarEvent::ToggleFill(
        true
    )));
    assert!(!event_dismisses_shape_picker(
        &ToolbarEvent::NudgePolygonSides(1)
    ));
    assert!(!event_dismisses_shape_picker(
        &ToolbarEvent::ToggleShapePicker(false)
    ));
    // ...while selecting a shape or any other action still closes it.
    assert!(event_dismisses_shape_picker(&ToolbarEvent::SelectTool(
        Tool::Line
    )));
    assert!(event_dismisses_shape_picker(&ToolbarEvent::Undo));
}

#[test]
fn top_overflow_menu_closes_on_any_non_toggle_event() {
    // The overflow menu owns none of the inline options, so even a Fill or
    // polygon-sides event fired while it is open (e.g. via keybinding) dismisses it.
    assert!(event_dismisses_top_overflow(&ToolbarEvent::ToggleFill(
        true
    )));
    assert!(event_dismisses_top_overflow(
        &ToolbarEvent::NudgePolygonSides(1)
    ));
    assert!(event_dismisses_precision_entry(&ToolbarEvent::SelectTool(
        crate::input::Tool::Pen
    )));
    assert!(event_dismisses_precision_entry(&ToolbarEvent::Undo));
    assert!(!event_dismisses_precision_entry(
        &ToolbarEvent::OpenPrecisionEntry(crate::ui::toolbar::PrecisionEntryTarget::Thickness)
    ));
    assert!(!event_dismisses_precision_entry(
        &ToolbarEvent::CommitPrecisionEntry {
            target: crate::ui::toolbar::PrecisionEntryTarget::Thickness,
            value: 4.0,
        }
    ));
    assert!(!event_dismisses_precision_entry(
        &ToolbarEvent::CancelPrecisionEntry
    ));
    assert!(event_dismisses_top_overflow(&ToolbarEvent::SelectTool(
        Tool::Line
    )));
    // Its own toggle spares it.
    assert!(!event_dismisses_top_overflow(
        &ToolbarEvent::ToggleTopOverflow(false)
    ));
    assert!(!event_dismisses_top_overflow(
        &ToolbarEvent::ToggleShapePicker(true)
    ));
    // The Session/Settings entries close the menu they live in.
    assert!(event_dismisses_top_overflow(
        &ToolbarEvent::ToggleSessionPopover(true)
    ));
    assert!(event_dismisses_top_overflow(
        &ToolbarEvent::ToggleSettingsPopover(true)
    ));
}

#[test]
fn session_popover_survives_its_own_controls_and_dismisses_on_everything_else() {
    // Every event the Session popover's controls emit keeps it open...
    for spared in [
        ToolbarEvent::OpenSession,
        ToolbarEvent::OpenRecentSession(PathBuf::from("/tmp/recent.wayscriber-session")),
        ToolbarEvent::SaveSessionAs,
        ToolbarEvent::SaveSessionAsConfirm(PathBuf::from("/tmp/existing.wayscriber-session")),
        ToolbarEvent::SaveSessionAsCancel,
        ToolbarEvent::SessionInfo,
        ToolbarEvent::ClearSession,
        ToolbarEvent::OpenConfigurator,
        ToolbarEvent::ScrollTopPopover(12.0),
        // Mutual exclusion is the apply layer's job, not a dismissal.
        ToolbarEvent::ToggleSessionPopover(true),
        ToolbarEvent::ToggleSettingsPopover(true),
    ] {
        assert!(
            !event_dismisses_session_popover(&spared),
            "{spared:?} must keep the Session popover open"
        );
    }
    // ...while unrelated toolbar interactions dismiss it like a flyout.
    for dismissing in [
        ToolbarEvent::SelectTool(Tool::Line),
        ToolbarEvent::Undo,
        ToolbarEvent::ToggleFill(true),
        ToolbarEvent::ToggleIconMode(true),
        ToolbarEvent::ToggleTopOverflow(true),
    ] {
        assert!(
            event_dismisses_session_popover(&dismissing),
            "{dismissing:?} must dismiss the Session popover"
        );
    }
}

#[test]
fn settings_popover_survives_its_own_controls_and_dismisses_on_everything_else() {
    // The Settings popover hosts the full Settings-pane control set.
    for spared in [
        ToolbarEvent::SetToolbarLayoutMode(ToolbarLayoutMode::Simple),
        ToolbarEvent::ToggleContextAwareUi(true),
        ToolbarEvent::ToggleIconMode(true),
        ToolbarEvent::ToggleTextControls(true),
        ToolbarEvent::ToggleStatusBar(true),
        ToolbarEvent::ToggleStatusBoardBadge(true),
        ToolbarEvent::ToggleStatusPageBadge(true),
        ToolbarEvent::ToggleFloatingBadgeAlways(true),
        ToolbarEvent::TogglePresetToasts(true),
        ToolbarEvent::ToggleIdleFade(true),
        ToolbarEvent::ToggleInputHud(true),
        ToolbarEvent::TogglePresets(true),
        ToolbarEvent::ToggleActionsSection(true),
        ToolbarEvent::ToggleZoomActions(true),
        ToolbarEvent::ToggleActionsAdvanced(true),
        ToolbarEvent::ToggleBoardsSection(true),
        ToolbarEvent::TogglePagesSection(true),
        ToolbarEvent::ToggleStepSection(true),
        ToolbarEvent::SetToolbarItemCustomizationOpen(true),
        ToolbarEvent::SetToolbarItemCustomizationGroup(Some(
            crate::ui::toolbar::ToolbarItemCustomizeGroup::TopTools,
        )),
        ToolbarEvent::SetToolbarItemHidden(crate::config::toolbar_item_ids::TOP_TOOL_PEN, true),
        ToolbarEvent::MoveToolbarItem {
            group: crate::config::ToolbarItemOrderGroup::TopTools,
            id: crate::config::toolbar_item_ids::TOP_TOOL_PEN,
            delta: 1,
        },
        ToolbarEvent::StartToolbarItemDrag {
            group: crate::config::ToolbarItemOrderGroup::TopTools,
            id: crate::config::toolbar_item_ids::TOP_TOOL_PEN,
        },
        ToolbarEvent::DragToolbarItemOver {
            group: crate::config::ToolbarItemOrderGroup::TopTools,
            target_index: 1,
        },
        ToolbarEvent::ResetToolbarItemOrder(crate::config::ToolbarItemOrderGroup::TopTools),
        ToolbarEvent::ResetToolbarItemHiddenOverrides,
        ToolbarEvent::OpenCommandPalette,
        ToolbarEvent::OpenConfigurator,
        ToolbarEvent::OpenConfigFile,
        ToolbarEvent::RequestRuntimeUiReset,
        ToolbarEvent::ConfirmUnsupportedRuntimeUiReset,
        ToolbarEvent::CancelUnsupportedRuntimeUiReset,
        ToolbarEvent::RetryRuntimeUiPersistence,
        ToolbarEvent::DiscardPendingRuntimeUiAndAdoptDisk,
        ToolbarEvent::RequestPreserveInvalidRuntimeUiReset,
        ToolbarEvent::ConfirmPreserveInvalidRuntimeUiReset,
        ToolbarEvent::CancelPreserveInvalidRuntimeUiReset,
        ToolbarEvent::CancelRuntimeUiRecovery,
        ToolbarEvent::ScrollTopPopover(12.0),
        ToolbarEvent::ToggleSessionPopover(true),
        ToolbarEvent::ToggleSettingsPopover(true),
    ] {
        assert!(
            !event_dismisses_settings_popover(&spared),
            "{spared:?} must keep the Settings popover open"
        );
    }
    for dismissing in [
        ToolbarEvent::SelectTool(Tool::Line),
        ToolbarEvent::Undo,
        ToolbarEvent::OpenSession,
        ToolbarEvent::ToggleTopOverflow(true),
        ToolbarEvent::ToggleShapePicker(true),
    ] {
        assert!(
            event_dismisses_settings_popover(&dismissing),
            "{dismissing:?} must dismiss the Settings popover"
        );
    }
}

/// The toolbar toggle event for one named section.
fn section_toggle_event(flag: ToolbarSectionFlag, show: bool) -> ToolbarEvent {
    match flag {
        ToolbarSectionFlag::Actions => ToolbarEvent::ToggleActionsSection(show),
        ToolbarSectionFlag::ActionsAdvanced => ToolbarEvent::ToggleActionsAdvanced(show),
        ToolbarSectionFlag::ZoomActions => ToolbarEvent::ToggleZoomActions(show),
        ToolbarSectionFlag::Pages => ToolbarEvent::TogglePagesSection(show),
        ToolbarSectionFlag::Boards => ToolbarEvent::ToggleBoardsSection(show),
        ToolbarSectionFlag::Presets => ToolbarEvent::TogglePresets(show),
        ToolbarSectionFlag::StepSection => ToolbarEvent::ToggleStepSection(show),
        ToolbarSectionFlag::TextControls => ToolbarEvent::ToggleTextControls(show),
    }
}

#[test]
fn backend_session_dispatch_finalizes_a_held_arrow_bend() {
    // Session routes return before `apply_toolbar_event`, and an open or clear
    // replaces the frame the gesture's snapshot belongs to. Shape ids restart
    // per frame, so a bend flushed after that would attach to an unrelated
    // shape on the new page.
    let mut input_state = make_test_input_state();
    let shape_id = input_state
        .boards
        .active_frame_mut()
        .add_shape(crate::draw::Shape::Arrow {
            x1: 0,
            y1: 100,
            x2: 400,
            y2: 100,
            color: input_state.style.current_color,
            thick: 4.0,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            head_at_end: true,
            style: crate::draw::ArrowStyle::Curved,
            bend: 0.0,
            label: None,
        });
    input_state.set_selection(vec![shape_id]);
    input_state.state = crate::input::state::DrawingState::BendingArrow {
        shape_id,
        snapshot: crate::draw::frame::ShapeSnapshot {
            shape: input_state
                .boards
                .active_frame()
                .shape(shape_id)
                .expect("arrow")
                .shape
                .clone(),
            locked: false,
        },
    };
    assert!(input_state.drag_arrow_bend_to(200, 20, false));
    let mut deadline = None;

    assert_eq!(
        handle_toolbar_event_preflight(
            &mut input_state,
            &mut deadline,
            &ToolbarEvent::ClearSession,
            false,
        ),
        ToolbarEventPreflight::Continue
    );

    assert!(
        matches!(input_state.state, crate::input::state::DrawingState::Idle),
        "the backend barrier left the bend gesture running"
    );
    // Committed rather than discarded, so the undo stack can take it back.
    input_state.handle_action(Action::Undo);
    match input_state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("arrow")
        .shape
    {
        crate::draw::Shape::Arrow { bend, .. } => assert_eq!(bend, 0.0),
        ref other => panic!("expected an arrow, got {other:?}"),
    }
}
