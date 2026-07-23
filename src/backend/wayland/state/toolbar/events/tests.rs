use super::session::{
    SessionFileDialogMode, SessionFileDialogResult, choose_session_file_from, default_save_as_path,
    ensure_save_as_extension, forget_missing_recent_session_after_open_error, save_as_file_name,
    session_info_summary,
};
use super::*;
use crate::config::{ToolbarLayoutMode, ToolbarSectionFlag};
use crate::draw::{Color, FontDescriptor};
use crate::env_vars::XDG_DATA_HOME_ENV;
use crate::input::state::test_support::make_test_input_state;
use crate::input::{EraserMode, Tool};
use crate::ui::toolbar::ToolbarSideSection;
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
            ToolbarPinChange::Side(true),
            ToolbarPinDurability::StartupPersistent,
            "Side toolbar will open at startup",
        ),
        (
            ToolbarPinChange::Side(false),
            ToolbarPinDurability::StartupPersistent,
            "Side toolbar will be hidden at startup",
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
        (
            ToolbarPinChange::Side(true),
            ToolbarPinDurability::LiveOnly,
            "Side toolbar pinned for this run only",
        ),
        (
            ToolbarPinChange::Side(false),
            ToolbarPinDurability::LiveOnly,
            "Side toolbar unpinned for this run only",
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
        ToolbarEvent::ScrollSidePane(24.0),
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
fn toolbar_runtime_preferences_have_exact_runtime_state_targets() {
    use crate::config::{
        ToolbarItemOrderGroup, ToolbarItemVisibilitySetting, toolbar_item_ids as ids,
    };
    use ToolbarRuntimeUiPersistenceTarget as Runtime;

    let events = vec![
        (ToolbarEvent::PinTopToolbar(true), Runtime::TopPinned),
        (ToolbarEvent::PinSideToolbar(true), Runtime::SidePinned),
        (
            ToolbarEvent::SetSidePane(crate::ui::toolbar::SidePane::Canvas),
            Runtime::SidePane,
        ),
        (
            ToolbarEvent::ToggleSideSectionCollapsed(ToolbarSideSection::Session, true),
            Runtime::CollapsedSection(ToolbarSideSection::Session),
        ),
        (ToolbarEvent::SetTopMinimized(true), Runtime::TopMinimized),
        (ToolbarEvent::SetSideMinimized(true), Runtime::SideMinimized),
        (ToolbarEvent::CloseTopToolbar, Runtime::TopMinimized),
        (ToolbarEvent::CloseSideToolbar, Runtime::SideMinimized),
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

#[test]
fn authored_toolbar_preferences_have_exact_config_targets() {
    use crate::config::TopDisplayMode;
    use ToolbarConfigPersistenceTarget::*;

    let events = [
        (ToolbarEvent::ToggleIconMode(true), Icons),
        (ToolbarEvent::ToggleMoreColors(true), MoreColors),
        (ToolbarEvent::ToggleContextAwareUi(true), ContextAwareUi),
        (ToolbarEvent::TogglePresetToasts(true), PresetToasts),
        (ToolbarEvent::ToggleToolPreview(true), ToolPreview),
        (ToolbarEvent::ToggleDelaySliders(true), DelaySliders),
        (
            ToolbarEvent::SetToolbarLayoutMode(ToolbarLayoutMode::Advanced),
            LayoutMode,
        ),
        (
            ToolbarEvent::ToggleActionsSection(false),
            SectionVisibility(ToolbarSectionFlag::Actions),
        ),
        (
            ToolbarEvent::ToggleActionsAdvanced(false),
            SectionVisibility(ToolbarSectionFlag::ActionsAdvanced),
        ),
        (
            ToolbarEvent::ToggleZoomActions(false),
            SectionVisibility(ToolbarSectionFlag::ZoomActions),
        ),
        (
            ToolbarEvent::TogglePagesSection(false),
            SectionVisibility(ToolbarSectionFlag::Pages),
        ),
        (
            ToolbarEvent::ToggleBoardsSection(false),
            SectionVisibility(ToolbarSectionFlag::Boards),
        ),
        (
            ToolbarEvent::TogglePresets(false),
            SectionVisibility(ToolbarSectionFlag::Presets),
        ),
        (
            ToolbarEvent::ToggleStepSection(false),
            SectionVisibility(ToolbarSectionFlag::StepSection),
        ),
        (
            ToolbarEvent::ToggleTextControls(false),
            SectionVisibility(ToolbarSectionFlag::TextControls),
        ),
        (
            ToolbarEvent::SetToolbarItemHidden(ToolbarSectionFlag::Actions.item_id(), true),
            SectionVisibility(ToolbarSectionFlag::Actions),
        ),
        (
            ToolbarEvent::SetTopDisplayMode(TopDisplayMode::Micro),
            TopDisplayMode,
        ),
    ];

    for (event, target) in events {
        assert_eq!(
            persistence_for(&event),
            ToolbarPersistence::Config(ToolbarPersistenceTarget::Toolbar(target)),
            "{event:?} should save only its authored config target"
        );
    }
}

#[test]
fn named_section_visibility_is_authored_config_not_runtime_state() {
    let events = [
        ToolbarEvent::ToggleActionsSection(false),
        ToolbarEvent::ToggleActionsAdvanced(false),
        ToolbarEvent::ToggleZoomActions(false),
        ToolbarEvent::TogglePagesSection(false),
        ToolbarEvent::ToggleBoardsSection(false),
        ToolbarEvent::TogglePresets(false),
        ToolbarEvent::ToggleStepSection(false),
        ToolbarEvent::ToggleTextControls(false),
        ToolbarEvent::SetToolbarItemHidden(ToolbarSectionFlag::Actions.item_id(), true),
    ];

    for event in events {
        assert!(
            matches!(
                persistence_for(&event),
                ToolbarPersistence::Config(ToolbarPersistenceTarget::Toolbar(_))
            ),
            "{event:?} must update its authored section setting"
        );
    }
}

#[test]
fn ui_and_history_preference_events_save_their_own_config_targets() {
    let ui_events = [
        (
            ToolbarEvent::ToggleStatusBar(true),
            ToolbarUiPersistenceTarget::StatusBar,
        ),
        (
            ToolbarEvent::ToggleStatusBoardBadge(true),
            ToolbarUiPersistenceTarget::StatusBoardBadge,
        ),
        (
            ToolbarEvent::ToggleStatusPageBadge(true),
            ToolbarUiPersistenceTarget::StatusPageBadge,
        ),
        (
            ToolbarEvent::ToggleFloatingBadgeAlways(true),
            ToolbarUiPersistenceTarget::FloatingBadgeAlways,
        ),
    ];

    for (event, target) in ui_events {
        assert_eq!(
            persistence_for(&event),
            ToolbarPersistence::Config(ToolbarPersistenceTarget::Ui(target)),
            "{event:?} should save only its UI config field"
        );
    }

    assert_eq!(
        persistence_for(&ToolbarEvent::ToggleCustomSection(true)),
        ToolbarPersistence::Config(ToolbarPersistenceTarget::History)
    );
}

#[test]
fn toolbar_ui_config_target_save_leaves_sibling_fields_unchanged() {
    let mut config = crate::config::Config::default();
    config.ui.show_status_bar = true;
    config.ui.show_status_board_badge = false;
    config.ui.show_status_page_badge = true;
    config.ui.show_floating_badge_always = false;

    let mut input_state = make_test_input_state();
    input_state.show_status_bar = false;
    input_state.show_status_board_badge = true;
    input_state.show_status_page_badge = false;
    input_state.show_floating_badge_always = true;

    apply_toolbar_ui_config_target(
        &mut config,
        &input_state,
        ToolbarUiPersistenceTarget::StatusBoardBadge,
    );

    assert!(config.ui.show_status_bar);
    assert!(config.ui.show_status_board_badge);
    assert!(config.ui.show_status_page_badge);
    assert!(!config.ui.show_floating_badge_always);
}

#[test]
fn toolbar_position_target_includes_the_side_drags_reconciled_top_offset() {
    let mut config = crate::config::Config::default();
    config.ui.toolbar.top_offset = 1.0;
    config.ui.toolbar.top_offset_y = 2.0;
    config.ui.toolbar.side_offset_x = 3.0;
    config.ui.toolbar.side_offset = 4.0;
    let input_state = make_test_input_state();
    let positions = ToolbarPositions {
        top_x: 10.0,
        top_y: 20.0,
        side_x: 30.0,
        side_y: 40.0,
    };

    apply_toolbar_config_target(
        &mut config,
        &input_state,
        positions,
        ToolbarConfigPersistenceTarget::TopPosition,
    );
    assert_eq!(config.ui.toolbar.top_offset, 10.0);
    assert_eq!(config.ui.toolbar.top_offset_y, 20.0);
    assert_eq!(config.ui.toolbar.side_offset_x, 3.0);
    assert_eq!(config.ui.toolbar.side_offset, 4.0);

    config.ui.toolbar.top_offset = 1.0;
    config.ui.toolbar.top_offset_y = 2.0;
    apply_toolbar_config_target(
        &mut config,
        &input_state,
        positions,
        ToolbarConfigPersistenceTarget::SidePosition,
    );
    assert_eq!(config.ui.toolbar.top_offset, 10.0);
    assert_eq!(config.ui.toolbar.top_offset_y, 2.0);
    assert_eq!(config.ui.toolbar.side_offset_x, 30.0);
    assert_eq!(config.ui.toolbar.side_offset, 40.0);
}

#[test]
fn toolbar_layout_target_updates_only_layout_and_derived_compatibility_mirrors() {
    let mut config = crate::config::Config::default();
    config.ui.toolbar.layout_mode = ToolbarLayoutMode::Simple;
    config.ui.toolbar.top_pinned = true;
    config.ui.toolbar.items.hidden = vec!["future-hidden".to_string()];
    let original_items = config.ui.toolbar.items.clone();

    let mut input_state = make_test_input_state();
    input_state.toolbar_layout_mode = ToolbarLayoutMode::Advanced;
    input_state.toolbar_top_pinned = false;
    input_state.show_presets = false;

    apply_toolbar_config_target(
        &mut config,
        &input_state,
        ToolbarPositions::default(),
        ToolbarConfigPersistenceTarget::LayoutMode,
    );

    assert_eq!(config.ui.toolbar.layout_mode, ToolbarLayoutMode::Advanced);
    assert!(!config.ui.toolbar.show_presets);
    assert!(config.ui.toolbar.top_pinned);
    assert_eq!(config.ui.toolbar.items, original_items);
}

#[test]
fn section_visibility_target_updates_only_its_canonical_override_and_mirror() {
    let mut config = crate::config::Config::default();
    config.ui.toolbar.show_actions_section = true;
    config.ui.toolbar.show_presets = false;
    config
        .ui
        .toolbar
        .items
        .set_hidden(ToolbarSectionFlag::Presets.item_id(), true);
    let presets_before = config.ui.toolbar.items.clone();

    let mut input_state = make_test_input_state();
    input_state.toolbar_items = config.ui.toolbar.items.clone();
    input_state.resolved_toolbar_items = input_state.toolbar_items.resolved();
    assert!(input_state.set_toolbar_item_hidden(ToolbarSectionFlag::Actions.item_id(), true,));

    apply_toolbar_config_target(
        &mut config,
        &input_state,
        ToolbarPositions::default(),
        ToolbarConfigPersistenceTarget::SectionVisibility(ToolbarSectionFlag::Actions),
    );

    assert!(!config.ui.toolbar.show_actions_section);
    assert!(!config.ui.toolbar.show_presets);
    assert!(
        config
            .ui
            .toolbar
            .items
            .resolved()
            .hidden
            .contains(&ToolbarSectionFlag::Actions.item_id())
    );
    assert!(
        config
            .ui
            .toolbar
            .items
            .resolved()
            .hidden
            .contains(&ToolbarSectionFlag::Presets.item_id())
    );
    assert_eq!(
        &config.ui.toolbar.items.hidden[..presets_before.hidden.len()],
        presets_before.hidden.as_slice(),
        "all existing hidden overrides keep their positions and values"
    );
    assert_eq!(config.ui.toolbar.items.shown, presets_before.shown);
}

#[test]
fn command_palette_and_shortcut_capture_block_shared_toolbar_events() {
    let mut input_state = make_test_input_state();
    assert!(!toolbar_event_blocked_by_modal(&input_state));

    input_state.toggle_command_palette();
    assert!(toolbar_event_blocked_by_modal(&input_state));

    input_state.toggle_command_palette();
    assert!(input_state.begin_keybinding_capture(crate::config::Action::Undo));
    assert!(toolbar_event_blocked_by_modal(&input_state));
}

#[test]
fn click_highlight_toolbar_events_are_explicit_config_exceptions() {
    let events = vec![
        ToolbarEvent::ToggleAllHighlight(true),
        ToolbarEvent::SelectTool(Tool::Highlight),
        ToolbarEvent::ToggleHighlightToolRing(true),
    ];

    for event in events {
        assert_eq!(
            persistence_for(&event),
            ToolbarPersistence::Config(ToolbarPersistenceTarget::ClickHighlight),
            "{event:?} should save click-highlight config"
        );
    }
}

#[test]
fn drawer_hint_pre_apply_effect_is_conditionally_recorded_below_max() {
    let mut state = OnboardingState {
        drawer_hint_count: crate::onboarding::DRAWER_HINT_MAX - 1,
        drawer_hint_shown: false,
        ..OnboardingState::default()
    };

    assert!(record_drawer_hint_shown(&mut state));
    assert_eq!(state.drawer_hint_count, crate::onboarding::DRAWER_HINT_MAX);
    assert!(state.drawer_hint_shown);
}

#[test]
fn drawer_hint_pre_apply_effect_is_ignored_at_max() {
    let mut state = OnboardingState {
        drawer_hint_count: crate::onboarding::DRAWER_HINT_MAX,
        drawer_hint_shown: true,
        ..OnboardingState::default()
    };

    assert!(!record_drawer_hint_shown(&mut state));
    assert_eq!(state.drawer_hint_count, crate::onboarding::DRAWER_HINT_MAX);
    assert!(state.drawer_hint_shown);
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
fn tool_preview_config_preserves_presenter_mode_restore_value() {
    assert!(persisted_tool_preview_value(false, Some(true)));
    assert!(!persisted_tool_preview_value(false, Some(false)));
    assert!(persisted_tool_preview_value(true, None));
    assert!(!persisted_tool_preview_value(false, None));
}

#[test]
fn toolbar_display_persist_during_presenter_micro_writes_the_pre_presenter_mode() {
    use crate::config::{PresenterToolbarMode, TopDisplayMode};

    let mut state = make_test_input_state();
    state.presenter_mode_config.hide_toolbars = true;
    state.presenter_mode_config.toolbar_mode = PresenterToolbarMode::Micro;
    state.toolbar_top_minimized = true;
    state.toolbar_top_display_mode = TopDisplayMode::Full;

    state.toggle_presenter_mode();
    assert_eq!(state.toolbar_top_display_mode, TopDisplayMode::Micro);
    assert!(
        !state.toolbar_top_minimized,
        "micro mapping clears minimized"
    );

    // A targeted top-display save during presenter mode keeps the saved
    // pre-presenter display mode, not the presenter mapping. Minimized state
    // is runtime-owned and is not part of this authored-config save.
    let restore = state.presenter_restore.as_ref().expect("presenter restore");
    assert_eq!(
        persisted_top_display_mode_value(
            state.toolbar_top_display_mode,
            restore.toolbar_top_display_mode
        ),
        TopDisplayMode::Full
    );

    // Exit restores the live values and drops the restore slots, so a
    // post-exit persist writes live state again.
    state.toggle_presenter_mode();
    assert!(state.presenter_restore.is_none());
    assert!(state.toolbar_top_minimized);
    assert_eq!(state.toolbar_top_display_mode, TopDisplayMode::Full);
    state.set_top_display_mode(TopDisplayMode::Micro);
    assert_eq!(
        persisted_top_display_mode_value(state.toolbar_top_display_mode, None),
        TopDisplayMode::Micro
    );
    // The hidden step stays runtime-only even through the presenter path.
    assert_eq!(
        persisted_top_display_mode_value(TopDisplayMode::Hidden, None),
        TopDisplayMode::Full
    );
    assert_eq!(
        persisted_top_display_mode_value(TopDisplayMode::Micro, Some(TopDisplayMode::Hidden)),
        TopDisplayMode::Full
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
