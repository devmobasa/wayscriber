//! Session persistence bookkeeping for per-output snapshots.
//!
//! Tracks the current session options and whether a snapshot has been loaded
//! so WaylandState can coordinate persistence without storing extra fields.

use anyhow::{Result, anyhow};

use crate::input::InputState;
use crate::session::{self as stored_session, LoadSnapshotOutcome, SessionOptions};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Tracks session persistence state and bookkeeping for per-output snapshots.
pub struct SessionState {
    options: Option<SessionOptions>,
    loaded: bool,
    loaded_board_data: bool,
    dirty: bool,
    dirty_since: Option<Instant>,
    last_dirty_at: Option<Instant>,
    last_save_at: Option<Instant>,
    autosave_retry_at: Option<Instant>,
    autosave_deferred_until: Option<Instant>,
    notified_failure: bool,
    notified_near_limit_paths: HashSet<PathBuf>,
    notified_trimmed_history: bool,
    notified_visible_only: bool,
    protected_session_paths: HashSet<PathBuf>,
    notified_expanded_load_paths: HashSet<PathBuf>,
}

impl SessionState {
    /// Creates a new session state wrapper using the supplied options.
    pub fn new(options: Option<SessionOptions>) -> Self {
        Self {
            options,
            loaded: false,
            loaded_board_data: false,
            dirty: false,
            dirty_since: None,
            last_dirty_at: None,
            last_save_at: None,
            autosave_retry_at: None,
            autosave_deferred_until: None,
            notified_failure: false,
            notified_near_limit_paths: HashSet::new(),
            notified_trimmed_history: false,
            notified_visible_only: false,
            protected_session_paths: HashSet::new(),
            notified_expanded_load_paths: HashSet::new(),
        }
    }

    /// Returns immutable access to the session options, if present.
    pub fn options(&self) -> Option<&SessionOptions> {
        self.options.as_ref()
    }

    /// Returns mutable access to the session options, if present.
    pub fn options_mut(&mut self) -> Option<&mut SessionOptions> {
        self.options.as_mut()
    }

    /// Returns true if a session snapshot has already been loaded this run.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Marks the session as loaded and records whether board data is now on disk.
    pub fn mark_loaded(&mut self, loaded_board_data: bool) {
        self.loaded = true;
        self.loaded_board_data = loaded_board_data;
    }

    pub fn has_loaded_board_data(&self) -> bool {
        self.loaded_board_data
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn record_input_dirty(&mut self, now: Instant, input_dirty: bool) {
        if !input_dirty {
            return;
        }
        if !self.dirty {
            self.dirty_since = Some(now);
        }
        self.dirty = true;
        self.last_dirty_at = Some(now);
    }

    pub fn mark_saved(&mut self, now: Instant, saved_board_data: bool) {
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.last_save_at = Some(now);
        self.autosave_retry_at = None;
        self.autosave_deferred_until = None;
        self.notified_failure = false;
        self.loaded_board_data = saved_board_data;
    }

    pub fn mark_clean_after_load(&mut self) {
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.autosave_retry_at = None;
        self.autosave_deferred_until = None;
    }

    fn commit_runtime_open(&mut self, options: SessionOptions, loaded_board_data: bool) {
        self.options = Some(options);
        self.loaded = true;
        self.loaded_board_data = loaded_board_data;
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.last_save_at = None;
        self.autosave_retry_at = None;
        self.autosave_deferred_until = None;
        self.notified_failure = false;
    }

    pub fn mark_autosave_failure(&mut self, now: Instant, backoff: Duration) -> bool {
        self.autosave_retry_at = Some(now + backoff);
        if self.notified_failure {
            false
        } else {
            self.notified_failure = true;
            true
        }
    }

    pub fn defer_autosave(&mut self, now: Instant, delay: Duration) {
        let until = now + delay;
        self.autosave_deferred_until = Some(match self.autosave_deferred_until {
            Some(current) => current.max(until),
            None => until,
        });
    }

    pub fn mark_near_limit_notified(&mut self, path: &Path) -> bool {
        self.notified_near_limit_paths.insert(path.to_path_buf())
    }

    pub fn mark_trimmed_history_notified(&mut self) -> bool {
        if self.notified_trimmed_history {
            false
        } else {
            self.notified_trimmed_history = true;
            true
        }
    }

    pub fn mark_visible_only_notified(&mut self) -> bool {
        if self.notified_visible_only {
            false
        } else {
            self.notified_visible_only = true;
            true
        }
    }

    pub fn protect_session_path(&mut self, path: PathBuf) {
        self.protected_session_paths.insert(path);
    }

    pub fn mark_expanded_load_notified(&mut self, path: &Path) -> bool {
        self.notified_expanded_load_paths.insert(path.to_path_buf())
    }

    pub fn should_skip_save_for_protected_path(&self, path: &Path, input_dirty: bool) -> bool {
        self.protected_session_paths.contains(path) && !self.dirty && !input_dirty
    }

    pub fn autosave_due(&self, now: Instant, options: &SessionOptions) -> bool {
        if !autosave_active(options) || !self.dirty {
            return false;
        }
        if let Some(retry_at) = self.autosave_retry_at
            && now < retry_at
        {
            return false;
        }
        if let Some(deferred_until) = self.autosave_deferred_until
            && now < deferred_until
        {
            return false;
        }
        let Some(last_dirty_at) = self.last_dirty_at else {
            return false;
        };
        let debounce_due = now >= last_dirty_at + options.autosave_idle;
        let dirty_since = self.dirty_since.unwrap_or(last_dirty_at);
        let base = match self.last_save_at {
            Some(last_save) if last_save > dirty_since => last_save,
            Some(_) | None => dirty_since,
        };
        let periodic_due = now >= base + options.autosave_interval;
        debounce_due || periodic_due
    }

    pub fn autosave_timeout(&self, now: Instant, options: &SessionOptions) -> Option<Duration> {
        if !autosave_active(options) || !self.dirty {
            return None;
        }
        let last_dirty_at = self.last_dirty_at?;
        let debounce_due = last_dirty_at + options.autosave_idle;
        let dirty_since = self.dirty_since.unwrap_or(last_dirty_at);
        let base = match self.last_save_at {
            Some(last_save) if last_save > dirty_since => last_save,
            Some(_) | None => dirty_since,
        };
        let periodic_due = base + options.autosave_interval;
        let next_due = if debounce_due <= periodic_due {
            debounce_due
        } else {
            periodic_due
        };
        let mut next_time = next_due;
        if let Some(retry_at) = self.autosave_retry_at {
            next_time = next_time.max(retry_at);
        }
        if let Some(deferred_until) = self.autosave_deferred_until {
            next_time = next_time.max(deferred_until);
        }
        Some(next_time.saturating_duration_since(now))
    }
}

fn autosave_active(options: &SessionOptions) -> bool {
    options.autosave_enabled
        && (options.any_enabled() || options.restore_tool_state || options.persist_history)
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct RuntimeOpenSessionReport {
    pub previous_path: PathBuf,
    pub opened_path: PathBuf,
    pub saved_current: bool,
    pub loaded_board_data: bool,
}

#[allow(dead_code)]
pub(in crate::backend::wayland) fn open_named_session_runtime(
    input_state: &mut InputState,
    session_state: &mut SessionState,
    target_path: &Path,
    now: Instant,
) -> Result<RuntimeOpenSessionReport> {
    let current_options = session_state
        .options()
        .cloned()
        .ok_or_else(|| anyhow!("cannot open session without active session options"))?;
    let previous_path = current_options.session_file_path();

    let saved_current = save_current_session_before_runtime_open(
        input_state,
        session_state,
        &current_options,
        now,
    )?;

    let mut candidate_options = current_options;
    candidate_options.set_named_file_target(target_path.to_path_buf());
    candidate_options.force_resume_persistence();

    let outcome = stored_session::load_named_session_candidate(&candidate_options)?;
    let candidate_snapshot = match outcome {
        LoadSnapshotOutcome::Loaded(snapshot)
        | LoadSnapshotOutcome::LoadedFromBackup(snapshot)
        | LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => *snapshot,
        LoadSnapshotOutcome::Empty => {
            return Err(anyhow!(
                "named session file contains no usable session data: {}",
                candidate_options.session_file_path().display()
            ));
        }
        LoadSnapshotOutcome::NonRegularArtifact { path } => {
            return Err(anyhow!(
                "named session file is not a regular file: {}",
                path.display()
            ));
        }
        LoadSnapshotOutcome::ExpandedTooLarge {
            path,
            max_expanded_size,
        } => {
            return Err(anyhow!(
                "named session file expands beyond the {} byte safety limit: {}",
                max_expanded_size,
                path.display()
            ));
        }
    };

    let loaded_board_data = candidate_snapshot.has_board_data();
    stored_session::apply_snapshot_replacing_boards(
        input_state,
        candidate_snapshot,
        &candidate_options,
    )?;
    input_state.set_session_preflight_options(Some(candidate_options.clone()));
    input_state.clear_session_dirty();
    let opened_path = candidate_options.session_file_path();
    session_state.commit_runtime_open(candidate_options.clone(), loaded_board_data);
    stored_session::catalog::record_named_session_opened(&candidate_options);

    Ok(RuntimeOpenSessionReport {
        previous_path,
        opened_path,
        saved_current,
        loaded_board_data,
    })
}

fn save_current_session_before_runtime_open(
    input_state: &mut InputState,
    session_state: &mut SessionState,
    options: &SessionOptions,
    now: Instant,
) -> Result<bool> {
    if !input_state.is_session_dirty() && !session_state.is_dirty() {
        return Ok(false);
    }

    let snapshot = input_state.with_active_interaction_canceled_for_capture(|input_state| {
        stored_session::snapshot_from_input(input_state, options)
    });
    if should_skip_unloaded_contentless_save(
        session_state.has_loaded_board_data(),
        session_state.is_dirty(),
        input_state.is_session_dirty(),
        snapshot
            .as_ref()
            .is_some_and(stored_session::SessionSnapshot::has_board_data),
        has_session_artifact(options),
    ) {
        return Ok(false);
    }

    let saved_board_data = snapshot
        .as_ref()
        .is_some_and(stored_session::SessionSnapshot::has_board_data);
    let report = if let Some(snapshot) = snapshot {
        stored_session::save_snapshot_with_report_and_clear_boundary(
            &snapshot,
            options,
            session_state.has_loaded_board_data(),
        )?
    } else if persistence_enabled(options) {
        let empty_snapshot = stored_session::SessionSnapshot {
            active_board_id: input_state.board_id().to_string(),
            boards: Vec::new(),
            tool_state: None,
        };
        stored_session::save_snapshot_with_report_and_clear_boundary(
            &empty_snapshot,
            options,
            session_state.has_loaded_board_data(),
        )?
    } else {
        return Err(anyhow!(
            "current session has unsaved changes but persistence is disabled"
        ));
    };

    if report.is_none() {
        return Err(anyhow!(
            "current session had unsaved changes but no session file was written"
        ));
    }

    let _ = input_state.take_session_dirty();
    session_state.mark_saved(now, saved_board_data);
    Ok(true)
}

fn persistence_enabled(options: &SessionOptions) -> bool {
    options.any_enabled() || options.restore_tool_state || options.persist_history
}

pub(super) fn has_session_artifact(options: &SessionOptions) -> bool {
    options.session_file_path().exists()
        || options.backup_file_path().exists()
        || options.backup_recovery_marker_file_path().exists()
        || options.clear_marker_file_path().exists()
        || options.recovery_recoverable_marker_file_path().exists()
        || has_recovery_artifact(options)
}

fn has_recovery_artifact(options: &SessionOptions) -> bool {
    let recovery_path = options.recovery_file_path();
    if recovery_path.exists() {
        return true;
    }
    let Some(parent) = recovery_path.parent() else {
        return false;
    };
    let Some(recovery_name) = recovery_path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let preserved_prefix = format!("{recovery_name}.");
    let Ok(entries) = std::fs::read_dir(parent) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let path = entry.path();
        path.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&preserved_prefix))
    })
}

pub(super) fn should_skip_unloaded_contentless_save(
    loaded_board_data: bool,
    session_dirty: bool,
    input_dirty: bool,
    has_board_data: bool,
    session_artifact_exists: bool,
) -> bool {
    !has_board_data
        && !loaded_board_data
        && !session_dirty
        && !input_dirty
        && session_artifact_exists
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Action, BoardsConfig, KeyBinding, PresenterModeConfig};
    use crate::draw::{
        Color, EraserKind, FontDescriptor, Frame, PageDeleteOutcome, REGULAR_POLYGON_DEFAULT_SIDES,
        Shape, ShapeId,
    };
    use crate::input::{
        BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD, ClickHighlightSettings, DrawingState,
        EraserMode, Tool,
    };
    use crate::util::Rect;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::MutexGuard;

    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};

    struct EnvGuard {
        _guard: MutexGuard<'static, ()>,
        catalog_hooks: Option<std::ffi::OsString>,
        xdg_data_home: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set_xdg_data_home(path: &Path) -> Self {
            let guard = crate::test_env::lock();
            let catalog_hooks = std::env::var_os("WAYSCRIBER_ENABLE_CATALOG_HOOKS_IN_TESTS");
            let xdg_data_home = std::env::var_os("XDG_DATA_HOME");
            unsafe {
                std::env::set_var("WAYSCRIBER_ENABLE_CATALOG_HOOKS_IN_TESTS", path);
                std::env::set_var("XDG_DATA_HOME", path);
            }
            Self {
                _guard: guard,
                catalog_hooks,
                xdg_data_home,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.catalog_hooks.take() {
                Some(value) => unsafe {
                    std::env::set_var("WAYSCRIBER_ENABLE_CATALOG_HOOKS_IN_TESTS", value)
                },
                None => unsafe { std::env::remove_var("WAYSCRIBER_ENABLE_CATALOG_HOOKS_IN_TESTS") },
            }
            match self.xdg_data_home.take() {
                Some(value) => unsafe { std::env::set_var("XDG_DATA_HOME", value) },
                None => unsafe { std::env::remove_var("XDG_DATA_HOME") },
            }
        }
    }

    #[cfg(unix)]
    struct ReadonlyDirGuard {
        path: PathBuf,
        original_mode: u32,
    }

    #[cfg(unix)]
    impl Drop for ReadonlyDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(
                &self.path,
                std::fs::Permissions::from_mode(self.original_mode),
            );
        }
    }

    #[cfg(unix)]
    fn readonly_dir_guard(path: &Path) -> ReadonlyDirGuard {
        let original_mode = std::fs::metadata(path)
            .expect("parent metadata")
            .permissions()
            .mode();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o555))
            .expect("make parent read-only");
        ReadonlyDirGuard {
            path: path.to_path_buf(),
            original_mode,
        }
    }

    #[cfg(unix)]
    fn can_create_probe(parent: &Path) -> bool {
        let probe = parent.join(format!(
            ".wayscriber-runtime-open-manual-access-test-{}",
            std::process::id()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
        {
            Ok(file) => {
                drop(file);
                let _ = std::fs::remove_file(&probe);
                true
            }
            Err(_) => false,
        }
    }

    fn test_input_state() -> InputState {
        let mut action_map = HashMap::new();
        action_map.insert(KeyBinding::parse("Escape").unwrap(), Action::Exit);
        InputState::with_defaults(
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
            FontDescriptor::default(),
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
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        )
    }

    fn add_line(input: &mut InputState, x2: i32) -> ShapeId {
        input.boards.active_frame_mut().add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2,
            y2: 10,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            },
            thick: 2.0,
        })
    }

    fn board_shape_count(input: &InputState, id: &str) -> usize {
        input
            .boards
            .board_states()
            .iter()
            .find(|board| board.spec.id == id)
            .expect("board")
            .pages
            .active_frame()
            .shapes
            .len()
    }

    fn sample_snapshot() -> stored_session::SessionSnapshot {
        snapshot_for_board("transparent", 42)
    }

    fn snapshot_for_board(id: &str, x2: i32) -> stored_session::SessionSnapshot {
        stored_session::SessionSnapshot {
            active_board_id: id.to_string(),
            boards: vec![board_snapshot(id, x2)],
            tool_state: None,
        }
    }

    fn board_snapshot(id: &str, x2: i32) -> stored_session::BoardSnapshot {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2,
            y2: 10,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        });
        stored_session::BoardSnapshot {
            id: id.to_string(),
            pages: stored_session::BoardPagesSnapshot {
                pages: vec![frame],
                active: 0,
            },
        }
    }

    fn sample_tool_state() -> stored_session::ToolStateSnapshot {
        stored_session::ToolStateSnapshot {
            current_color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            current_thickness: 3.0,
            eraser_size: 12.0,
            eraser_kind: EraserKind::Circle,
            eraser_mode: EraserMode::Brush,
            marker_opacity: Some(0.32),
            fill_enabled: Some(false),
            tool_override: None,
            current_font_size: 24.0,
            font_descriptor: Some(FontDescriptor::default()),
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: Some(false),
            arrow_label_enabled: Some(false),
            polygon_sides: REGULAR_POLYGON_DEFAULT_SIDES,
            board_previous_color: None,
            show_status_bar: true,
            tool_settings: None,
        }
    }

    fn named_options(base: &Path, name: &str) -> SessionOptions {
        let mut options = SessionOptions::new(base.join("configured"), name);
        options.persist_transparent = true;
        options.set_named_file_target(base.join(format!("{name}.wayscriber-session")));
        options
    }

    fn assert_no_candidate_sidecars(options: &SessionOptions) {
        for path in [
            options.lock_file_path(),
            options.backup_file_path(),
            options.backup_recovery_marker_file_path(),
            options.recovery_file_path(),
            options.recovery_recoverable_marker_file_path(),
            options.clear_marker_file_path(),
        ] {
            assert!(
                !path.exists(),
                "failed runtime open must not create candidate sidecar {}",
                path.display()
            );
        }
    }

    #[test]
    fn runtime_open_success_commits_target_and_catalog_after_apply() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current");
        let candidate_options = named_options(temp.path(), "candidate");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        let mut session_state = SessionState::new(Some(current_options.clone()));
        let report = open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");

        assert_eq!(report.previous_path, current_options.session_file_path());
        assert_eq!(report.opened_path, candidate_path);
        assert!(!report.saved_current);
        assert!(report.loaded_board_data);
        assert_eq!(
            session_state
                .options()
                .map(SessionOptions::session_file_path),
            Some(candidate_options.session_file_path())
        );
        assert!(session_state.is_loaded());
        assert!(!session_state.is_dirty());
        assert!(!input.is_session_dirty());
        assert_eq!(input.boards.active_frame().shapes.len(), 1);

        let recent = stored_session::catalog::recent_sessions().expect("recent sessions");
        let candidate_entry = recent
            .iter()
            .find(|entry| entry.path == candidate_options.session_file_path().display().to_string())
            .expect("candidate catalog entry");
        assert!(
            candidate_entry.last_opened_at_millis.is_some(),
            "runtime open should record catalog opened only after commit"
        );
    }

    #[test]
    fn runtime_open_marks_full_damage_after_replacing_boards() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-damage");
        let candidate_options = named_options(temp.path(), "candidate-damage");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        input.update_screen_dimensions(800, 600);
        input
            .dirty_tracker
            .mark_rect(Rect::new(10, 10, 20, 20).expect("small dirty rect"));
        let mut session_state = SessionState::new(Some(current_options));

        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");

        assert_eq!(
            input.take_dirty_regions(),
            vec![Rect::new(0, 0, 800, 600).expect("full dirty rect")]
        );
    }

    #[test]
    fn runtime_open_uses_recoverable_backup_without_mutating_candidate_artifacts() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let current_options = named_options(temp.path(), "current-before-backup-open");
        let source_options = named_options(temp.path(), "source-contentful-backup");
        stored_session::save_snapshot(&sample_snapshot(), &source_options)
            .expect("save source contentful session");
        let backup_bytes =
            std::fs::read(source_options.session_file_path()).expect("source session bytes");

        let candidate_options = named_options(temp.path(), "candidate-recoverable-backup");
        let contentless = stored_session::SessionSnapshot {
            active_board_id: "transparent".to_string(),
            boards: Vec::new(),
            tool_state: Some(sample_tool_state()),
        };
        stored_session::save_snapshot(&contentless, &candidate_options)
            .expect("save contentless candidate primary");
        std::fs::remove_file(candidate_options.lock_file_path()).expect("remove save-created lock");
        std::fs::write(candidate_options.backup_file_path(), &backup_bytes)
            .expect("seed recoverable backup");
        std::fs::write(
            candidate_options.backup_recovery_marker_file_path(),
            b"recoverable",
        )
        .expect("seed recoverable backup marker");
        let primary_before =
            std::fs::read(candidate_options.session_file_path()).expect("candidate primary bytes");

        let mut input = test_input_state();
        let mut session_state = SessionState::new(Some(current_options));
        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_options.session_file_path(),
            Instant::now(),
        )
        .expect("runtime open");

        assert_eq!(input.boards.active_frame().shapes.len(), 1);
        assert_eq!(
            std::fs::read(candidate_options.session_file_path())
                .expect("candidate primary unchanged"),
            primary_before
        );
        assert_eq!(
            std::fs::read(candidate_options.backup_file_path()).expect("backup unchanged"),
            backup_bytes
        );
        assert!(
            candidate_options
                .backup_recovery_marker_file_path()
                .exists()
        );
        assert!(
            !candidate_options.lock_file_path().exists(),
            "runtime open must not create a missing candidate lock"
        );
    }

    #[test]
    fn runtime_open_replaces_boards_missing_from_candidate_snapshot() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-with-whiteboard");
        let candidate_options = named_options(temp.path(), "candidate-transparent-only");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        input.switch_board_force(BOARD_ID_WHITEBOARD);
        add_line(&mut input, 77);
        assert_eq!(board_shape_count(&input, BOARD_ID_WHITEBOARD), 1);
        let mut session_state = SessionState::new(Some(current_options));

        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");

        assert_eq!(board_shape_count(&input, BOARD_ID_WHITEBOARD), 0);
        assert_eq!(input.board_id(), "transparent");
        assert_eq!(input.boards.active_frame().shapes.len(), 1);
    }

    #[test]
    fn runtime_open_resyncs_canvas_pointer_after_same_active_board_view_offset() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-pointer-cache");
        let mut candidate_options = named_options(temp.path(), "candidate-pointer-cache");
        candidate_options.persist_whiteboard = true;

        let mut candidate_snapshot = snapshot_for_board(BOARD_ID_WHITEBOARD, 77);
        assert!(candidate_snapshot.boards[0].pages.pages[0].set_view_offset(100, 50));
        stored_session::save_snapshot(&candidate_snapshot, &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        input.switch_board_force(BOARD_ID_WHITEBOARD);
        input.update_pointer_position(30, 40);
        assert_eq!(input.canvas_pointer_position(), (30, 40));
        let mut session_state = SessionState::new(Some(current_options));

        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");

        assert_eq!(input.board_id(), BOARD_ID_WHITEBOARD);
        assert_eq!(input.boards.active_frame().view_offset(), (100, 50));
        assert_eq!(input.canvas_pointer_position(), (130, 90));
    }

    #[test]
    fn runtime_open_releases_old_board_slots_for_candidate_boards() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-full-board-slots");
        let candidate_options = named_options(temp.path(), "candidate-custom-board");
        let target_board = "session-target-board";
        stored_session::save_snapshot(&snapshot_for_board(target_board, 99), &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        while input.boards.board_count() < input.boards.max_count() {
            assert!(input.boards.create_board());
        }
        assert_eq!(input.boards.board_count(), input.boards.max_count());
        assert!(!input.boards.has_board(target_board));
        let mut session_state = SessionState::new(Some(current_options));

        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");

        assert!(input.boards.has_board(target_board));
        assert_eq!(input.board_id(), target_board);
        assert_eq!(board_shape_count(&input, target_board), 1);
    }

    #[test]
    fn runtime_open_apply_capacity_failure_keeps_current_session_active() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-apply-failure");
        let candidate_options = named_options(temp.path(), "candidate-too-many-boards");

        let input = test_input_state();
        let max_boards = input.boards.max_count();
        let oversized_snapshot = stored_session::SessionSnapshot {
            active_board_id: "candidate-board-0".to_string(),
            boards: (0..=max_boards)
                .map(|index| board_snapshot(&format!("candidate-board-{index}"), index as i32))
                .collect(),
            tool_state: None,
        };
        stored_session::save_snapshot(&oversized_snapshot, &candidate_options)
            .expect("save oversized candidate");
        std::fs::remove_file(candidate_options.lock_file_path()).expect("remove save-created lock");

        let mut input = input;
        add_line(&mut input, 121);
        let mut session_state = SessionState::new(Some(current_options.clone()));

        let err = open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_options.session_file_path(),
            Instant::now(),
        )
        .expect_err("oversized candidate should fail before applying");

        assert!(
            format!("{err:#}").contains("current runtime allows"),
            "{err:#}"
        );
        assert_eq!(
            session_state
                .options()
                .map(SessionOptions::session_file_path),
            Some(current_options.session_file_path())
        );
        assert_eq!(input.board_id(), "transparent");
        assert_eq!(input.boards.active_frame().shapes.len(), 1);
        assert_no_candidate_sidecars(&candidate_options);

        let recent = stored_session::catalog::recent_sessions().expect("recent sessions");
        let candidate_entry = recent
            .iter()
            .find(|entry| entry.path == candidate_options.session_file_path().display().to_string())
            .expect("saved candidate catalog entry");
        assert!(
            candidate_entry.last_opened_at_millis.is_none(),
            "failed apply must not be cataloged as opened"
        );
    }

    #[test]
    fn runtime_open_rejects_full_candidate_snapshot_that_omits_overlay_board() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-overlay-preserved");
        let candidate_options = named_options(temp.path(), "candidate-without-overlay");

        let mut input = test_input_state();
        while input.boards.board_count() < input.boards.max_count() {
            assert!(input.boards.create_board());
        }
        assert_eq!(input.boards.board_count(), input.boards.max_count());
        assert!(input.boards.has_board(BOARD_ID_TRANSPARENT));

        let max_boards = input.boards.max_count();
        let full_non_overlay_snapshot = stored_session::SessionSnapshot {
            active_board_id: "candidate-board-0".to_string(),
            boards: (0..max_boards)
                .map(|index| board_snapshot(&format!("candidate-board-{index}"), index as i32))
                .collect(),
            tool_state: None,
        };
        stored_session::save_snapshot(&full_non_overlay_snapshot, &candidate_options)
            .expect("save full candidate");
        std::fs::remove_file(candidate_options.lock_file_path()).expect("remove save-created lock");

        let mut session_state = SessionState::new(Some(current_options.clone()));
        let err = open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_options.session_file_path(),
            Instant::now(),
        )
        .expect_err("candidate should not evict the overlay board");

        assert!(
            format!("{err:#}").contains("preserving the overlay board"),
            "{err:#}"
        );
        assert!(input.boards.has_board(BOARD_ID_TRANSPARENT));
        assert_eq!(input.boards.board_count(), max_boards);
        assert_eq!(
            session_state
                .options()
                .map(SessionOptions::session_file_path),
            Some(current_options.session_file_path())
        );

        let recent = stored_session::catalog::recent_sessions().expect("recent sessions");
        let candidate_entry = recent
            .iter()
            .find(|entry| entry.path == candidate_options.session_file_path().display().to_string())
            .expect("saved candidate catalog entry");
        assert!(
            candidate_entry.last_opened_at_millis.is_none(),
            "failed overlay-preservation open must not be cataloged as opened"
        );
    }

    #[test]
    fn runtime_open_clears_deleted_page_restore_state_from_previous_session() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-with-deleted-page");
        let candidate_options = named_options(temp.path(), "candidate-after-deleted-page");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        input.boards.active_pages_mut().new_page();
        add_line(&mut input, 88);
        assert_eq!(input.page_delete(), PageDeleteOutcome::Pending);
        assert_eq!(input.page_delete(), PageDeleteOutcome::Removed);
        assert_eq!(input.boards.page_count(), 1);
        let mut session_state = SessionState::new(Some(current_options));

        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");
        input.restore_deleted_page();

        assert_eq!(input.boards.page_count(), 1);
        assert_eq!(input.boards.active_frame().shapes.len(), 1);
    }

    #[test]
    fn runtime_open_cancels_active_interaction_from_previous_session() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-active-interaction");
        let candidate_options = named_options(temp.path(), "candidate-after-interaction");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        input.state = DrawingState::Drawing {
            tool: Tool::Pen,
            start_x: 10,
            start_y: 20,
            points: vec![(10, 20), (30, 40)],
            point_thicknesses: vec![2.0, 2.0],
        };
        assert!(input.has_active_pointer_interaction());
        let mut session_state = SessionState::new(Some(current_options));

        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");

        assert!(!input.has_active_pointer_interaction());
        assert!(matches!(input.state, DrawingState::Idle));
        assert_eq!(input.boards.active_frame().shapes.len(), 1);
    }

    #[test]
    fn runtime_open_closes_active_board_picker_drag() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-board-picker-drag");
        let candidate_options = named_options(temp.path(), "candidate-after-board-picker-drag");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        input.open_board_picker();
        assert!(input.board_picker_start_drag(0));
        assert!(input.is_board_picker_open());
        assert!(input.board_picker_is_dragging());
        let mut session_state = SessionState::new(Some(current_options));

        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");

        assert!(!input.is_board_picker_open());
        assert!(!input.board_picker_is_dragging());
        assert!(!input.board_picker_is_page_dragging());
        assert_eq!(input.boards.active_frame().shapes.len(), 1);
    }

    #[test]
    fn runtime_open_closes_active_board_picker_page_drag() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-board-picker-page-drag");
        let candidate_options =
            named_options(temp.path(), "candidate-after-board-picker-page-drag");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        input.open_board_picker();
        assert!(input.board_picker_start_page_drag(0));
        assert!(input.is_board_picker_open());
        assert!(input.board_picker_is_page_dragging());
        let mut session_state = SessionState::new(Some(current_options));

        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");

        assert!(!input.is_board_picker_open());
        assert!(!input.board_picker_is_dragging());
        assert!(!input.board_picker_is_page_dragging());
        assert_eq!(input.boards.active_frame().shapes.len(), 1);
    }

    #[test]
    fn runtime_open_saves_current_after_canceling_active_selection_move() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-active-selection-move");
        let candidate_options = named_options(temp.path(), "candidate-after-selection-move");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        let shape_id = add_line(&mut input, 20);
        input.set_selection(vec![shape_id]);
        let snapshots = input.capture_movable_selection_snapshots();
        assert!(input.apply_translation_to_selection(100, 0));
        input.state = DrawingState::MovingSelection {
            last_x: 100,
            last_y: 0,
            snapshots,
            moved: true,
        };
        input.mark_session_dirty();
        let mut session_state = SessionState::new(Some(current_options.clone()));

        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");

        let outcome =
            stored_session::load_snapshot_with_outcome(&current_options).expect("load current");
        let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
            panic!("expected saved current session, got {outcome:?}");
        };
        let shape = &snapshot.boards[0].pages.pages[0].shapes[0].shape;
        match shape {
            Shape::Line { x1, y1, x2, y2, .. } => {
                assert_eq!((*x1, *y1, *x2, *y2), (0, 0, 20, 10));
            }
            other => panic!("expected saved line, got {other:?}"),
        }
    }

    #[test]
    fn runtime_open_saves_current_after_canceling_active_text_edit() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-active-text-edit");
        let candidate_options = named_options(temp.path(), "candidate-after-text-edit");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        let shape_id = input.boards.active_frame_mut().add_shape(Shape::Text {
            x: 40,
            y: 80,
            text: "Original".to_string(),
            color: input.current_color,
            size: input.current_font_size,
            font_descriptor: input.font_descriptor.clone(),
            background_enabled: input.text_background_enabled,
            wrap_width: Some(180),
        });
        input.set_selection(vec![shape_id]);
        assert!(input.edit_selected_text());
        let Shape::Text { text, .. } = &input
            .boards
            .active_frame()
            .shape(shape_id)
            .expect("text shape")
            .shape
        else {
            panic!("expected text shape");
        };
        assert!(text.is_empty());
        let DrawingState::TextInput { buffer, .. } = &mut input.state else {
            panic!("expected text input");
        };
        buffer.push_str(" unsaved edit");
        input.mark_session_dirty();
        let mut session_state = SessionState::new(Some(current_options.clone()));

        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");

        let outcome =
            stored_session::load_snapshot_with_outcome(&current_options).expect("load current");
        let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
            panic!("expected saved current session, got {outcome:?}");
        };
        let shape = &snapshot.boards[0].pages.pages[0].shapes[0].shape;
        match shape {
            Shape::Text {
                text, wrap_width, ..
            } => {
                assert_eq!(text, "Original");
                assert_eq!(*wrap_width, Some(180));
            }
            other => panic!("expected saved text, got {other:?}"),
        }
    }

    #[test]
    fn runtime_open_saves_current_after_canceling_color_picker_preview() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-color-picker-preview");
        let candidate_options = named_options(temp.path(), "candidate-after-color-picker");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        let original = input.color_for_tool(Tool::Pen);
        input.open_color_picker_popup();
        input.color_picker_popup_set_from_gradient(0.6, 0.1);
        input.color_picker_popup_set_dragging(true);
        assert_ne!(input.color_for_tool(Tool::Pen), original);
        input.mark_session_dirty();
        let mut session_state = SessionState::new(Some(current_options.clone()));

        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");

        assert!(!input.is_color_picker_popup_open());
        let outcome =
            stored_session::load_snapshot_with_outcome(&current_options).expect("load current");
        let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
            panic!("expected saved current session, got {outcome:?}");
        };
        let tool_state = snapshot.tool_state.expect("tool state");
        let tool_settings = tool_state.tool_settings.expect("tool settings");
        assert_eq!(tool_state.current_color, original);
        assert_eq!(tool_settings.pen.color, original);
    }

    #[cfg(unix)]
    #[test]
    fn runtime_open_current_save_failure_preserves_active_selection_move() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let current_options = named_options(temp.path(), "current-active-save-fail");
        let current_target = temp.path().join("current-active-symlink-target");
        std::fs::write(&current_target, b"preserve current target").expect("write target");
        symlink(&current_target, current_options.session_file_path()).expect("current symlink");

        let candidate_options = named_options(temp.path(), "candidate-active-save-fail");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");

        let mut input = test_input_state();
        let shape_id = add_line(&mut input, 9);
        input.set_selection(vec![shape_id]);
        let snapshots = input.capture_movable_selection_snapshots();
        assert!(input.apply_translation_to_selection(100, 0));
        input.state = DrawingState::MovingSelection {
            last_x: 100,
            last_y: 0,
            snapshots,
            moved: true,
        };
        input.mark_session_dirty();
        let mut session_state = SessionState::new(Some(current_options.clone()));

        let err = open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_options.session_file_path(),
            Instant::now(),
        )
        .expect_err("current save failure should abort open");

        assert!(format!("{err:#}").contains("symlink"), "{err:#}");
        assert!(input.has_active_pointer_interaction());
        assert!(matches!(input.state, DrawingState::MovingSelection { .. }));
        let Shape::Line { x1, y1, x2, y2, .. } = &input.boards.active_frame().shapes[0].shape
        else {
            panic!("expected line");
        };
        assert_eq!((*x1, *y1, *x2, *y2), (100, 0, 109, 10));
        assert!(input.is_session_dirty());
        assert_eq!(
            std::fs::read(&current_target).expect("current target bytes"),
            b"preserve current target"
        );
    }

    #[cfg(unix)]
    #[test]
    fn runtime_open_current_save_failure_preserves_spatial_index_for_active_selection_move() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let current_options = named_options(temp.path(), "current-spatial-save-fail");
        let current_target = temp.path().join("current-spatial-symlink-target");
        std::fs::write(&current_target, b"preserve current target").expect("write target");
        symlink(&current_target, current_options.session_file_path()).expect("current symlink");

        let candidate_options = named_options(temp.path(), "candidate-spatial-save-fail");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");

        let mut input = test_input_state();
        input.set_hit_test_threshold(1);
        let shape_id = add_line(&mut input, 9);
        input.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 1000,
            y: 1000,
            w: 10,
            h: 10,
            fill: false,
            color: input.current_color,
            thick: input.current_thickness,
        });
        input.ensure_spatial_index_for_active_frame();
        assert!(input.has_spatial_index());

        input.set_selection(vec![shape_id]);
        let snapshots = input.capture_movable_selection_snapshots();
        assert!(input.apply_translation_to_selection(200, 0));
        assert!(
            input
                .hit_test_all_for_points(&[(205, 5)], input.hit_test_tolerance)
                .contains(&shape_id)
        );
        input.state = DrawingState::MovingSelection {
            last_x: 200,
            last_y: 0,
            snapshots,
            moved: true,
        };
        input.mark_session_dirty();
        let mut session_state = SessionState::new(Some(current_options.clone()));

        let err = open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_options.session_file_path(),
            Instant::now(),
        )
        .expect_err("current save failure should abort open");

        assert!(format!("{err:#}").contains("symlink"), "{err:#}");
        assert!(input.has_spatial_index());
        assert!(
            input
                .hit_test_all_for_points(&[(205, 5)], input.hit_test_tolerance)
                .contains(&shape_id),
            "hit testing should use the restored in-progress selection position"
        );
        assert_eq!(
            std::fs::read(&current_target).expect("current target bytes"),
            b"preserve current target"
        );
    }

    #[test]
    fn runtime_open_clears_stale_selection_and_hit_cache() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-selected-shape");
        let candidate_options = named_options(temp.path(), "candidate-reuses-shape-id");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");
        let candidate_path = candidate_options.session_file_path();

        let mut input = test_input_state();
        let old_shape_id = input.boards.active_frame_mut().add_shape(Shape::Line {
            x1: 500,
            y1: 500,
            x2: 550,
            y2: 550,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            },
            thick: 2.0,
        });
        input.set_selection(vec![old_shape_id]);
        assert!(input.has_selection());
        assert_eq!(input.hit_test_at(510, 510), Some(old_shape_id));
        let mut session_state = SessionState::new(Some(current_options));

        open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_path,
            Instant::now(),
        )
        .expect("runtime open");

        assert!(!input.has_selection());
        assert_eq!(input.hit_test_at(5, 5), Some(1));
    }

    #[test]
    fn runtime_open_candidate_failure_after_current_save_keeps_current_active() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        let current_options = named_options(temp.path(), "current-save-first");
        let candidate_options = named_options(temp.path(), "candidate-corrupt");
        std::fs::write(candidate_options.session_file_path(), b"{not valid json")
            .expect("write corrupt candidate");

        let mut input = test_input_state();
        add_line(&mut input, 7);
        input.mark_session_dirty();
        let mut session_state = SessionState::new(Some(current_options.clone()));

        let err = open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_options.session_file_path(),
            Instant::now(),
        )
        .expect_err("corrupt candidate should abort after current save");

        assert!(
            format!("{err:#}").contains("failed to parse session json"),
            "{err:#}"
        );
        assert_eq!(
            session_state
                .options()
                .map(SessionOptions::session_file_path),
            Some(current_options.session_file_path())
        );
        assert!(
            current_options.session_file_path().exists(),
            "dirty current session should be saved before candidate load"
        );
        assert!(
            !input.is_session_dirty(),
            "dirty flag may clear after the current session is successfully persisted"
        );
        assert!(!session_state.is_dirty());
        assert_eq!(
            std::fs::read(candidate_options.session_file_path()).expect("candidate bytes"),
            b"{not valid json"
        );
        assert_no_candidate_sidecars(&candidate_options);

        let recent = stored_session::catalog::recent_sessions().expect("recent sessions");
        assert!(
            recent
                .iter()
                .all(|entry| entry.path
                    != candidate_options.session_file_path().display().to_string()),
            "failed candidate must not be cataloged as opened"
        );
    }

    #[cfg(unix)]
    #[test]
    fn runtime_open_rejects_readonly_candidate_parent_before_commit() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let current_dir = temp.path().join("current");
        let candidate_dir = temp.path().join("candidate");
        std::fs::create_dir(&current_dir).expect("create current dir");
        std::fs::create_dir(&candidate_dir).expect("create candidate dir");
        let current_options = named_options(&current_dir, "current");
        let candidate_options = named_options(&candidate_dir, "candidate");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");

        let _guard = readonly_dir_guard(&candidate_dir);
        if can_create_probe(&candidate_dir) {
            return;
        }

        let mut input = test_input_state();
        let mut session_state = SessionState::new(Some(current_options.clone()));
        let err = open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_options.session_file_path(),
            Instant::now(),
        )
        .expect_err("read-only candidate parent should abort runtime open");

        assert!(format!("{err:#}").contains("not writable"), "{err:#}");
        assert_eq!(
            session_state
                .options()
                .map(SessionOptions::session_file_path),
            Some(current_options.session_file_path())
        );
        assert!(!session_state.is_loaded());
        assert!(!input.is_session_dirty());
        assert_eq!(input.boards.active_frame().shapes.len(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn runtime_open_current_save_failure_aborts_before_candidate_load() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let current_options = named_options(temp.path(), "current-save-fail");
        let current_target = temp.path().join("current-symlink-target");
        std::fs::write(&current_target, b"preserve current target").expect("write target");
        symlink(&current_target, current_options.session_file_path()).expect("current symlink");

        let candidate_options = named_options(temp.path(), "candidate-valid");
        stored_session::save_snapshot(&sample_snapshot(), &candidate_options)
            .expect("save candidate");

        let mut input = test_input_state();
        add_line(&mut input, 9);
        input.mark_session_dirty();
        let mut session_state = SessionState::new(Some(current_options.clone()));

        let err = open_named_session_runtime(
            &mut input,
            &mut session_state,
            &candidate_options.session_file_path(),
            Instant::now(),
        )
        .expect_err("current save failure should abort open");

        assert!(format!("{err:#}").contains("symlink"), "{err:#}");
        assert_eq!(
            session_state
                .options()
                .map(SessionOptions::session_file_path),
            Some(current_options.session_file_path())
        );
        assert!(input.is_session_dirty());
        assert!(!session_state.is_dirty());
        assert_eq!(
            std::fs::read(&current_target).expect("current target bytes"),
            b"preserve current target"
        );
    }

    #[test]
    fn autosave_failure_backoff_delays_retry() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
        options.autosave_enabled = true;
        options.persist_transparent = true;
        options.autosave_idle = Duration::from_millis(1);
        options.autosave_interval = Duration::from_millis(1);
        options.autosave_failure_backoff = Duration::from_millis(50);

        let mut state = SessionState::new(Some(options.clone()));
        let now = Instant::now();
        state.record_input_dirty(now, true);
        state.mark_autosave_failure(now, options.autosave_failure_backoff);

        assert!(!state.autosave_due(now, &options));
        assert_eq!(
            state.autosave_timeout(now, &options),
            Some(options.autosave_failure_backoff)
        );

        let later = now + options.autosave_failure_backoff;
        assert!(state.autosave_due(later, &options));
        assert_eq!(
            state.autosave_timeout(later, &options),
            Some(Duration::from_millis(0))
        );
    }

    #[test]
    fn autosave_deferral_delays_due_without_clearing_dirty() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
        options.autosave_enabled = true;
        options.persist_transparent = true;
        options.autosave_idle = Duration::from_millis(1);
        options.autosave_interval = Duration::from_millis(1);

        let mut state = SessionState::new(Some(options.clone()));
        let now = Instant::now();
        state.record_input_dirty(now, true);
        let due_at = now + Duration::from_millis(2);
        assert!(state.autosave_due(due_at, &options));

        let defer_for = Duration::from_millis(50);
        state.defer_autosave(due_at, defer_for);
        assert!(!state.autosave_due(due_at, &options));
        assert_eq!(state.autosave_timeout(due_at, &options), Some(defer_for));

        let later = due_at + defer_for;
        assert!(state.autosave_due(later, &options));
        assert_eq!(
            state.autosave_timeout(later, &options),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn mark_saved_clears_autosave_deferral() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
        options.autosave_enabled = true;
        options.persist_transparent = true;

        let mut state = SessionState::new(Some(options));
        let now = Instant::now();
        state.record_input_dirty(now, true);
        state.defer_autosave(now, Duration::from_secs(60));
        state.mark_saved(now, false);

        assert_eq!(state.autosave_deferred_until, None);
    }

    #[test]
    fn protected_session_path_blocks_save_until_session_is_dirty() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
        options.autosave_enabled = true;
        options.persist_transparent = true;
        let path = options.session_file_path();
        let mut state = SessionState::new(Some(options.clone()));

        assert!(!state.should_skip_save_for_protected_path(&path, false));
        state.protect_session_path(path.clone());
        assert!(state.should_skip_save_for_protected_path(&path, false));
        assert!(!state.should_skip_save_for_protected_path(&path, true));

        state.record_input_dirty(Instant::now(), true);
        assert!(!state.should_skip_save_for_protected_path(&path, false));
    }

    #[test]
    fn load_baseline_clears_stale_dirty_before_protected_save_check() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
        options.autosave_enabled = true;
        options.persist_transparent = true;
        let path = options.session_file_path();
        let mut state = SessionState::new(Some(options));

        state.record_input_dirty(Instant::now(), true);
        state.protect_session_path(path.clone());
        assert!(!state.should_skip_save_for_protected_path(&path, false));

        state.mark_clean_after_load();
        assert!(state.should_skip_save_for_protected_path(&path, false));
    }

    #[test]
    fn contentless_save_guard_blocks_clean_unloaded_save_when_primary_exists() {
        assert!(should_skip_unloaded_contentless_save(
            false, false, false, false, true,
        ));
    }

    #[test]
    fn contentless_save_guard_blocks_clean_unloaded_save_when_only_backup_exists() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let options = SessionOptions::new(temp.path().to_path_buf(), "backup-only");
        std::fs::write(options.backup_file_path(), b"backup").expect("backup write");

        assert!(has_session_artifact(&options));
        assert!(should_skip_unloaded_contentless_save(
            false,
            false,
            false,
            false,
            has_session_artifact(&options),
        ));
    }

    #[test]
    fn contentless_save_guard_blocks_clean_unloaded_save_when_only_recovery_exists() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let options = SessionOptions::new(temp.path().to_path_buf(), "recovery-only");
        std::fs::write(options.recovery_file_path(), b"recovery").expect("recovery write");

        assert!(has_session_artifact(&options));
        assert!(should_skip_unloaded_contentless_save(
            false,
            false,
            false,
            false,
            has_session_artifact(&options),
        ));
    }

    #[test]
    fn contentless_save_guard_blocks_clean_loaded_empty_save_when_only_preserved_recovery_exists() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let options = SessionOptions::new(temp.path().to_path_buf(), "preserved-recovery");
        std::fs::write(
            options
                .recovery_file_path()
                .with_extension("recovery.empty"),
            b"recovery",
        )
        .expect("preserved recovery write");

        assert!(has_session_artifact(&options));
        assert!(should_skip_unloaded_contentless_save(
            false,
            false,
            false,
            false,
            has_session_artifact(&options),
        ));
    }

    #[test]
    fn contentless_save_guard_allows_real_board_data_or_dirty_state() {
        assert!(!should_skip_unloaded_contentless_save(
            false, false, false, true, true,
        ));
        assert!(!should_skip_unloaded_contentless_save(
            false, true, false, false, true,
        ));
        assert!(!should_skip_unloaded_contentless_save(
            false, false, true, false, true,
        ));
        assert!(!should_skip_unloaded_contentless_save(
            true, false, false, false, true,
        ));
    }

    #[test]
    fn contentless_save_guard_allows_noop_when_no_session_artifact_exists() {
        assert!(!should_skip_unloaded_contentless_save(
            false, false, false, false, false,
        ));
    }
}
