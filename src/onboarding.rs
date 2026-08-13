use crate::domain::OnboardingTip;
use crate::durable_io::{AtomicWriteOptions, OverwriteMode, PermissionPolicy, SymlinkPolicy};
use crate::paths::data_dir;
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ONBOARDING_VERSION: u32 = 6;
const STARTUP_NOTICE_ACKNOWLEDGEMENT_MAX: usize = 32;
pub(crate) const DRAWER_HINT_MAX: u32 = 2;
pub(crate) const DEFERRED_HINT_REPEAT_MAX: u32 = 3;
const ONBOARDING_FILE: &str = "onboarding.toml";
const ONBOARDING_DIR: &str = "wayscriber";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FirstRunStep {
    BackgroundModeSetup,
    WaitDraw,
    DrawUndo,
    ColorThickness,
    QuickAccess,
    RadialFlick,
    Reference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingState {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub welcome_shown: bool,
    #[serde(default)]
    pub toolbar_hint_shown: bool,
    /// Whether the guided tour has been shown to the user
    #[serde(default, alias = "tour_completed")]
    pub tour_shown: bool,
    /// Whether the "More" drawer hint has been shown
    #[serde(default)]
    pub drawer_hint_shown: bool,
    /// Number of times the drawer hint has been acknowledged (opened)
    #[serde(default)]
    pub drawer_hint_count: u32,
    /// Number of overlay launches seen by this profile
    #[serde(default)]
    pub sessions_seen: u32,
    /// Whether first-run onboarding has been fully completed
    #[serde(default)]
    pub first_run_completed: bool,
    /// Whether the user explicitly skipped first-run onboarding
    #[serde(default)]
    pub first_run_skipped: bool,
    /// Active first-run onboarding step (if any)
    #[serde(default)]
    pub active_step: Option<FirstRunStep>,
    /// Whether the first-run background mode prompt was answered
    #[serde(default)]
    pub first_run_background_mode_prompted: bool,
    /// Whether background mode setup was completed from first-run prompt
    #[serde(default)]
    pub first_run_background_mode_enabled: bool,
    /// Whether quick-access step requires revealing hidden toolbars
    #[serde(default)]
    pub quick_access_requires_toolbar: bool,
    /// Whether radial menu preview has been shown during quick-access step
    #[serde(default)]
    pub quick_access_radial_preview_shown: bool,
    /// Whether context menu preview has been shown during quick-access step
    #[serde(default)]
    pub quick_access_context_preview_shown: bool,
    /// Whether help overlay preview has been shown during reference step
    #[serde(default)]
    pub reference_help_preview_shown: bool,
    /// Whether command palette preview has been shown during reference step
    #[serde(default)]
    pub reference_palette_preview_shown: bool,
    /// Whether at least one stroke was drawn
    #[serde(default)]
    pub first_stroke_done: bool,
    /// Whether at least one successful undo was performed
    #[serde(default)]
    pub first_undo_done: bool,
    /// Whether a drawing color was changed during first-run (teaching step)
    #[serde(default)]
    pub first_color_done: bool,
    /// Whether stroke thickness was adjusted during first-run (teaching step)
    #[serde(default)]
    pub first_thickness_done: bool,
    /// Legacy progress from the retired radial-flick teaching step.
    #[serde(default)]
    pub radial_flick_done: bool,
    /// Whether toolbar visibility was toggled via an action
    #[serde(default)]
    pub used_toolbar_toggle: bool,
    /// Whether radial menu was opened
    #[serde(default)]
    pub used_radial_menu: bool,
    /// Whether context menu was opened by right click
    #[serde(default)]
    pub used_context_menu_right_click: bool,
    /// Whether context menu was opened via keyboard action
    #[serde(default)]
    pub used_context_menu_keyboard: bool,
    /// Whether help overlay was opened
    #[serde(default)]
    pub used_help_overlay: bool,
    /// Whether command palette was opened
    #[serde(default)]
    pub used_command_palette: bool,
    /// Whether the board picker has been opened by any UI or action path.
    #[serde(default)]
    pub used_board_picker: bool,
    /// Whether any user-facing zoom control has been activated.
    #[serde(default)]
    pub used_zoom_control: bool,
    /// Whether the unified toolbar's Canvas popover has been opened.
    #[serde(default)]
    pub used_canvas_popover: bool,
    /// Whether deferred help hint has already been shown
    #[serde(default)]
    pub hint_help_shown: bool,
    /// Number of deferred help hints shown across sessions
    #[serde(default)]
    pub hint_help_count: u32,
    /// Whether deferred command palette hint has already been shown
    #[serde(default)]
    pub hint_palette_shown: bool,
    /// Number of deferred command palette hints shown across sessions
    #[serde(default)]
    pub hint_palette_count: u32,
    /// Whether deferred quick-access hint has already been shown
    #[serde(default)]
    pub hint_quick_access_shown: bool,
    /// Number of deferred quick-access hints shown across sessions
    #[serde(default)]
    pub hint_quick_access_count: u32,
    /// Whether the shortcut coach has been fully taught (learned): once set,
    /// the coach stays suppressed permanently.
    #[serde(default)]
    pub coach_hint_shown: bool,
    /// Number of shortcut-coach hints shown across sessions (across-session cap)
    #[serde(default)]
    pub coach_hint_count: u32,
    /// Whether the deferred status-bar/board-picker hint has been shown this
    /// session (re-armed at session start until the across-session cap).
    #[serde(default)]
    pub hint_status_bar_shown: bool,
    /// Number of deferred status-bar hints shown across sessions (M9).
    #[serde(default)]
    pub hint_status_bar_count: u32,
    /// Whether the deferred bottom-right zoom-chip hint has been shown this
    /// session (re-armed at session start until the across-session cap).
    #[serde(default)]
    pub hint_zoom_chip_shown: bool,
    /// Number of deferred zoom-chip hints shown across sessions (M9).
    #[serde(default)]
    pub hint_zoom_chip_count: u32,
    /// Whether the deferred "Canvas…" overflow-popover hint has been shown this
    /// session (re-armed at session start until the across-session cap).
    #[serde(default)]
    pub hint_canvas_popover_shown: bool,
    /// Number of deferred Canvas-popover hints shown across sessions (M9).
    #[serde(default)]
    pub hint_canvas_popover_count: u32,
    /// Stable content identifiers for informational startup notices the user
    /// has already seen. Errors and authored conflicts are deliberately not
    /// stored here because they must remain visible until resolved.
    #[serde(default)]
    pub acknowledged_startup_notices: Vec<String>,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            version: ONBOARDING_VERSION,
            welcome_shown: false,
            toolbar_hint_shown: false,
            tour_shown: false,
            drawer_hint_shown: false,
            drawer_hint_count: 0,
            sessions_seen: 0,
            first_run_completed: false,
            first_run_skipped: false,
            active_step: None,
            first_run_background_mode_prompted: false,
            first_run_background_mode_enabled: false,
            quick_access_requires_toolbar: false,
            quick_access_radial_preview_shown: false,
            quick_access_context_preview_shown: false,
            reference_help_preview_shown: false,
            reference_palette_preview_shown: false,
            first_stroke_done: false,
            first_undo_done: false,
            first_color_done: false,
            first_thickness_done: false,
            radial_flick_done: false,
            used_toolbar_toggle: false,
            used_radial_menu: false,
            used_context_menu_right_click: false,
            used_context_menu_keyboard: false,
            used_help_overlay: false,
            used_command_palette: false,
            used_board_picker: false,
            used_zoom_control: false,
            used_canvas_popover: false,
            hint_help_shown: false,
            hint_help_count: 0,
            hint_palette_shown: false,
            hint_palette_count: 0,
            hint_quick_access_shown: false,
            hint_quick_access_count: 0,
            coach_hint_shown: false,
            coach_hint_count: 0,
            hint_status_bar_shown: false,
            hint_status_bar_count: 0,
            hint_zoom_chip_shown: false,
            hint_zoom_chip_count: 0,
            hint_canvas_popover_shown: false,
            hint_canvas_popover_count: 0,
            acknowledged_startup_notices: Vec::new(),
        }
    }
}

impl OnboardingState {
    pub fn first_run_active(&self) -> bool {
        !self.first_run_completed && !self.first_run_skipped
    }
}

pub struct OnboardingStore {
    state: OnboardingState,
    path: Option<PathBuf>,
    persistence_available: bool,
}

#[derive(Debug)]
pub(crate) enum OnboardingSaveError {
    Unavailable,
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    Serialize(toml::ser::Error),
    Write {
        path: PathBuf,
        source: crate::durable_io::DurableIoError,
    },
}

impl std::fmt::Display for OnboardingSaveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("no user data directory is available"),
            Self::CreateDirectory { path, source } => write!(
                formatter,
                "failed to create onboarding state directory {}: {source}",
                path.display()
            ),
            Self::Serialize(source) => {
                write!(formatter, "failed to serialize onboarding state: {source}")
            }
            Self::Write { path, source } => write!(
                formatter,
                "failed to write onboarding state {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for OnboardingSaveError {}

impl OnboardingStore {
    pub fn load() -> Self {
        let Some(path) = onboarding_path() else {
            return Self {
                state: OnboardingState::default(),
                path: None,
                persistence_available: false,
            };
        };

        Self::load_from_path(path)
    }

    fn load_from_path(path: PathBuf) -> Self {
        match fs::read_to_string(&path) {
            Ok(raw) => match toml::from_str::<OnboardingState>(&raw) {
                Ok(mut state) => {
                    let needs_save = migrate_onboarding_state(&mut state);
                    let mut store = Self {
                        state,
                        path: Some(path),
                        persistence_available: true,
                    };
                    if needs_save {
                        let _ = store.save();
                    }
                    return store;
                }
                Err(err) => {
                    warn!(
                        "Failed to parse onboarding state {}: {}",
                        path.display(),
                        err
                    );
                    return recover_onboarding_file(path, Some(&raw));
                }
            },
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                warn!(
                    "Failed to read onboarding state {}: {}",
                    path.display(),
                    err
                );
                return recover_onboarding_file(path, None);
            }
        }

        Self {
            state: OnboardingState::default(),
            path: Some(path),
            persistence_available: true,
        }
    }

    pub fn state(&self) -> &OnboardingState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut OnboardingState {
        &mut self.state
    }

    pub fn persistence_available(&self) -> bool {
        self.persistence_available
    }

    pub fn startup_notice_acknowledged(&self, notice_id: &str) -> bool {
        self.state
            .acknowledged_startup_notices
            .iter()
            .any(|saved| saved == notice_id)
    }

    pub fn acknowledge_startup_notice(
        &mut self,
        notice_id: &str,
    ) -> Result<(), OnboardingSaveError> {
        if !self.startup_notice_acknowledged(notice_id) {
            self.state
                .acknowledged_startup_notices
                .push(notice_id.to_string());
            let excess = self
                .state
                .acknowledged_startup_notices
                .len()
                .saturating_sub(STARTUP_NOTICE_ACKNOWLEDGEMENT_MAX);
            if excess > 0 {
                self.state.acknowledged_startup_notices.drain(..excess);
            }
        }
        self.save()
    }

    /// Permanently suppress one automatic tip using the same capped-count
    /// encoding as natural expiry. Mutating before `save` intentionally keeps
    /// the tip suppressed for this session when persistence fails; `save`
    /// then disables all remaining automatic guidance for the process.
    pub(crate) fn acknowledge_tip(
        &mut self,
        tip: OnboardingTip,
    ) -> Result<(), OnboardingSaveError> {
        let state = &mut self.state;
        match tip {
            OnboardingTip::Help => {
                state.hint_help_shown = true;
                state.hint_help_count = DEFERRED_HINT_REPEAT_MAX;
            }
            OnboardingTip::CommandPalette => {
                state.hint_palette_shown = true;
                state.hint_palette_count = DEFERRED_HINT_REPEAT_MAX;
            }
            OnboardingTip::QuickAccess => {
                state.hint_quick_access_shown = true;
                state.hint_quick_access_count = DEFERRED_HINT_REPEAT_MAX;
            }
            OnboardingTip::StatusBar => {
                state.hint_status_bar_shown = true;
                state.hint_status_bar_count = DEFERRED_HINT_REPEAT_MAX;
            }
            OnboardingTip::CanvasPopover => {
                state.hint_canvas_popover_shown = true;
                state.hint_canvas_popover_count = DEFERRED_HINT_REPEAT_MAX;
            }
            OnboardingTip::ZoomChip => {
                state.hint_zoom_chip_shown = true;
                state.hint_zoom_chip_count = DEFERRED_HINT_REPEAT_MAX;
            }
            OnboardingTip::ShortcutCoach => {
                state.coach_hint_shown = true;
                state.coach_hint_count = DEFERRED_HINT_REPEAT_MAX;
            }
            OnboardingTip::ToolbarHidden => {
                state.toolbar_hint_shown = true;
            }
        }
        self.save()
    }

    pub fn begin_session(
        &mut self,
        automatic_guidance_enabled: bool,
    ) -> Result<(), OnboardingSaveError> {
        let state = &mut self.state;
        state.sessions_seen = state.sessions_seen.saturating_add(1);

        if automatic_guidance_enabled {
            if !state.used_help_overlay && state.hint_help_count < DEFERRED_HINT_REPEAT_MAX {
                state.hint_help_shown = false;
            }
            if !state.used_command_palette && state.hint_palette_count < DEFERRED_HINT_REPEAT_MAX {
                state.hint_palette_shown = false;
            }
            if !state.used_radial_menu
                && !state.used_context_menu_right_click
                && !state.used_context_menu_keyboard
                && state.hint_quick_access_count < DEFERRED_HINT_REPEAT_MAX
            {
                state.hint_quick_access_shown = false;
            }
            if !state.used_board_picker && state.hint_status_bar_count < DEFERRED_HINT_REPEAT_MAX {
                state.hint_status_bar_shown = false;
            }
            if !state.used_zoom_control && state.hint_zoom_chip_count < DEFERRED_HINT_REPEAT_MAX {
                state.hint_zoom_chip_shown = false;
            }
            if !state.used_canvas_popover
                && state.hint_canvas_popover_count < DEFERRED_HINT_REPEAT_MAX
            {
                state.hint_canvas_popover_shown = false;
            }
        }

        if automatic_guidance_enabled && !state.first_run_completed && !state.first_run_skipped {
            state
                .active_step
                .get_or_insert(FirstRunStep::BackgroundModeSetup);
        } else {
            state.active_step = None;
            state.quick_access_requires_toolbar = false;
        }
        // Keep legacy flags marked so older checks never re-trigger.
        state.welcome_shown = true;
        state.tour_shown = true;
        self.save()
    }

    pub fn save(&mut self) -> Result<(), OnboardingSaveError> {
        let Some(path) = &self.path else {
            self.persistence_available = false;
            return Err(OnboardingSaveError::Unavailable);
        };
        if let Some(parent) = path.parent()
            && let Err(source) = fs::create_dir_all(parent)
        {
            let error = OnboardingSaveError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            };
            warn!("{error}");
            self.persistence_available = false;
            return Err(error);
        }
        let contents = match toml::to_string_pretty(&self.state) {
            Ok(contents) => contents,
            Err(source) => {
                let error = OnboardingSaveError::Serialize(source);
                warn!("{error}");
                self.persistence_available = false;
                return Err(error);
            }
        };
        if let Err(source) = crate::durable_io::write_text_atomic(
            path,
            &contents,
            AtomicWriteOptions {
                overwrite: OverwriteMode::Replace,
                permissions: PermissionPolicy::PreserveExistingOrMode(0o644),
                symlink: SymlinkPolicy::Reject,
                sync_file: true,
                sync_parent: true,
            },
        ) {
            let error = OnboardingSaveError::Write {
                path: path.clone(),
                source,
            };
            warn!("{error}");
            self.persistence_available = false;
            return Err(error);
        }
        self.persistence_available = true;
        Ok(())
    }
}

fn onboarding_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(ONBOARDING_DIR).join(ONBOARDING_FILE))
}

fn default_version() -> u32 {
    ONBOARDING_VERSION
}

fn migrate_onboarding_state(state: &mut OnboardingState) -> bool {
    let mut needs_save = false;
    let old_version = state.version;

    if state.version != ONBOARDING_VERSION {
        state.version = ONBOARDING_VERSION;
        needs_save = true;
    }
    if state.drawer_hint_count == 0 && state.drawer_hint_shown {
        state.drawer_hint_count = DRAWER_HINT_MAX;
        needs_save = true;
    }
    if state.drawer_hint_count >= DRAWER_HINT_MAX && !state.drawer_hint_shown {
        state.drawer_hint_shown = true;
        needs_save = true;
    }

    // Existing users already saw onboarding in earlier versions; don't force re-run.
    if old_version < 3 && !state.first_run_completed && (state.welcome_shown || state.tour_shown) {
        state.first_run_completed = true;
        state.first_run_skipped = false;
        state.active_step = None;
        needs_save = true;
    }

    if state.first_run_skipped && !state.first_run_completed {
        state.first_run_completed = true;
        needs_save = true;
    }
    if state.first_run_completed && state.active_step.is_some() {
        state.active_step = None;
        needs_save = true;
    }
    if state.active_step == Some(FirstRunStep::RadialFlick) {
        state.active_step = Some(FirstRunStep::Reference);
        needs_save = true;
    }
    if state.first_run_background_mode_enabled && !state.first_run_background_mode_prompted {
        state.first_run_background_mode_prompted = true;
        needs_save = true;
    }
    if state.first_run_completed && !state.first_run_background_mode_prompted {
        state.first_run_background_mode_prompted = true;
        needs_save = true;
    }
    if old_version < 6 && state.first_run_completed {
        // These surface-discovery hints were added after many profiles had
        // already completed onboarding. A version bump must not silently
        // enroll those users in several new rounds of automatic tips.
        state.hint_status_bar_shown = true;
        state.hint_status_bar_count = state.hint_status_bar_count.max(DEFERRED_HINT_REPEAT_MAX);
        state.hint_zoom_chip_shown = true;
        state.hint_zoom_chip_count = state.hint_zoom_chip_count.max(DEFERRED_HINT_REPEAT_MAX);
        state.hint_canvas_popover_shown = true;
        state.hint_canvas_popover_count = state
            .hint_canvas_popover_count
            .max(DEFERRED_HINT_REPEAT_MAX);
        needs_save = true;
    }
    if state.quick_access_requires_toolbar && state.active_step != Some(FirstRunStep::QuickAccess) {
        state.quick_access_requires_toolbar = false;
        needs_save = true;
    }
    if state.hint_help_shown && state.hint_help_count == 0 {
        state.hint_help_count = 1;
        needs_save = true;
    }
    if state.hint_palette_shown && state.hint_palette_count == 0 {
        state.hint_palette_count = 1;
        needs_save = true;
    }
    if state.hint_quick_access_shown && state.hint_quick_access_count == 0 {
        state.hint_quick_access_count = 1;
        needs_save = true;
    }
    // Shortcut-coach bookkeeping: `coach_hint_shown` is the "fully taught"
    // suppression flag reached once the across-session count caps out. Keep the
    // two in sync for hand-edited or partially-written files.
    if state.coach_hint_shown && state.coach_hint_count == 0 {
        state.coach_hint_count = DEFERRED_HINT_REPEAT_MAX;
        needs_save = true;
    }
    if state.coach_hint_count >= DEFERRED_HINT_REPEAT_MAX && !state.coach_hint_shown {
        state.coach_hint_shown = true;
        needs_save = true;
    }
    // M9 deferred surface hints: keep the per-session `*_shown` flag and the
    // across-session `*_count` consistent for hand-edited or partially-written
    // files, mirroring the help/palette/quick-access bookkeeping above.
    if state.hint_status_bar_shown && state.hint_status_bar_count == 0 {
        state.hint_status_bar_count = 1;
        needs_save = true;
    }
    if state.hint_zoom_chip_shown && state.hint_zoom_chip_count == 0 {
        state.hint_zoom_chip_count = 1;
        needs_save = true;
    }
    if state.hint_canvas_popover_shown && state.hint_canvas_popover_count == 0 {
        state.hint_canvas_popover_count = 1;
        needs_save = true;
    }

    needs_save
}

fn recover_onboarding_file(path: PathBuf, _raw: Option<&str>) -> OnboardingStore {
    if path.exists() {
        let backup = backup_path(&path);
        if let Err(err) = fs::rename(&path, &backup) {
            warn!(
                "Failed to back up onboarding state {}: {}",
                path.display(),
                err
            );
        }
    }

    let welcome_shown = true;
    let toolbar_hint_shown = true;

    let state = OnboardingState {
        version: ONBOARDING_VERSION,
        welcome_shown,
        toolbar_hint_shown,
        tour_shown: true,        // Don't show legacy tour for recovered state
        drawer_hint_shown: true, // Don't show drawer hint for recovered state
        drawer_hint_count: DRAWER_HINT_MAX,
        sessions_seen: 0,
        first_run_completed: true,
        first_run_skipped: false,
        active_step: None,
        first_run_background_mode_prompted: true,
        first_run_background_mode_enabled: false,
        quick_access_requires_toolbar: false,
        quick_access_radial_preview_shown: false,
        quick_access_context_preview_shown: false,
        reference_help_preview_shown: false,
        reference_palette_preview_shown: false,
        first_stroke_done: false,
        first_undo_done: false,
        first_color_done: false,
        first_thickness_done: false,
        radial_flick_done: false,
        used_toolbar_toggle: false,
        used_radial_menu: false,
        used_context_menu_right_click: false,
        used_context_menu_keyboard: false,
        used_help_overlay: false,
        used_command_palette: false,
        used_board_picker: false,
        used_zoom_control: false,
        used_canvas_popover: false,
        hint_help_shown: true,
        hint_help_count: DEFERRED_HINT_REPEAT_MAX,
        hint_palette_shown: true,
        hint_palette_count: DEFERRED_HINT_REPEAT_MAX,
        hint_quick_access_shown: true,
        hint_quick_access_count: DEFERRED_HINT_REPEAT_MAX,
        coach_hint_shown: true,
        coach_hint_count: DEFERRED_HINT_REPEAT_MAX,
        hint_status_bar_shown: true,
        hint_status_bar_count: DEFERRED_HINT_REPEAT_MAX,
        hint_zoom_chip_shown: true,
        hint_zoom_chip_count: DEFERRED_HINT_REPEAT_MAX,
        hint_canvas_popover_shown: true,
        hint_canvas_popover_count: DEFERRED_HINT_REPEAT_MAX,
        acknowledged_startup_notices: Vec::new(),
    };
    let mut store = OnboardingStore {
        state,
        path: Some(path),
        persistence_available: true,
    };
    let _ = store.save();
    store
}

fn backup_path(path: &Path) -> PathBuf {
    let base = path.with_extension("bak");
    if !base.exists() {
        return base;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let mut candidate = path.with_extension(format!("bak.{nanos}.{pid}"));
    if !candidate.exists() {
        return candidate;
    }
    for index in 1..=1000 {
        candidate = path.with_extension(format!("bak.{nanos}.{pid}.{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    candidate
}

#[cfg(test)]
mod tests;
