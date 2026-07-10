use super::session::{
    SessionFileDialogMode, SessionFileDialogResult, choose_session_file_from, default_save_as_path,
    ensure_save_as_extension, forget_missing_recent_session_after_open_error, save_as_file_name,
    session_info_summary,
};
use super::*;
use crate::config::ToolbarLayoutMode;
use crate::draw::{Color, FontDescriptor};
use crate::env_vars::XDG_DATA_HOME_ENV;
use crate::input::state::test_support::make_test_input_state;
use crate::input::{EraserMode, Tool};
use crate::ui::toolbar::ToolbarSideSection;
use anyhow::anyhow;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

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
fn runtime_toolbar_events_do_not_directly_save_config() {
    let events = vec![
        ToolbarEvent::SelectTool(Tool::Line),
        ToolbarEvent::SetColor(Color {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 1.0,
        }),
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
    ];

    for event in events {
        assert_eq!(
            persistence_for(&event),
            ToolbarPersistence::RuntimeOnly,
            "{event:?} should not directly save config"
        );
    }
}

#[test]
fn toolbar_preference_events_save_toolbar_config() {
    let events = vec![
        ToolbarEvent::PinTopToolbar(true),
        ToolbarEvent::PinSideToolbar(true),
        ToolbarEvent::ToggleIconMode(true),
        ToolbarEvent::ToggleMoreColors(true),
        ToolbarEvent::ToggleActionsSection(true),
        ToolbarEvent::ToggleActionsAdvanced(true),
        ToolbarEvent::ToggleZoomActions(true),
        ToolbarEvent::TogglePagesSection(true),
        ToolbarEvent::ToggleBoardsSection(true),
        ToolbarEvent::TogglePresets(true),
        ToolbarEvent::ToggleStepSection(true),
        ToolbarEvent::ToggleTextControls(true),
        ToolbarEvent::ToggleContextAwareUi(true),
        ToolbarEvent::TogglePresetToasts(true),
        ToolbarEvent::ToggleToolPreview(true),
        ToolbarEvent::ToggleDelaySliders(true),
        ToolbarEvent::SetToolbarLayoutMode(ToolbarLayoutMode::Advanced),
        ToolbarEvent::SetSidePane(crate::ui::toolbar::SidePane::Canvas),
        ToolbarEvent::ToggleSideSectionCollapsed(ToolbarSideSection::Session, true),
    ];

    for event in events {
        assert_eq!(
            persistence_for(&event),
            ToolbarPersistence::Persist(ToolbarPersistenceTarget::Toolbar),
            "{event:?} should save toolbar config"
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
            ToolbarPersistence::Persist(ToolbarPersistenceTarget::Ui(target)),
            "{event:?} should save only its UI config field"
        );
    }

    assert_eq!(
        persistence_for(&ToolbarEvent::ToggleCustomSection(true)),
        ToolbarPersistence::Persist(ToolbarPersistenceTarget::History)
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
fn click_highlight_toolbar_events_are_explicit_config_exceptions() {
    let events = vec![
        ToolbarEvent::ToggleAllHighlight(true),
        ToolbarEvent::SelectTool(Tool::Highlight),
        ToolbarEvent::ToggleHighlightToolRing(true),
    ];

    for event in events {
        assert_eq!(
            persistence_for(&event),
            ToolbarPersistence::Persist(ToolbarPersistenceTarget::ClickHighlight),
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
