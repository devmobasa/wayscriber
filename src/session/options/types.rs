use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::identifiers::sanitize_identifier;

pub const DEFAULT_AUTO_COMPRESS_THRESHOLD_BYTES: u64 = 100 * 1024; // 100 KiB
pub const DEFAULT_AUTOSAVE_ENABLED: bool = true;
pub const DEFAULT_AUTOSAVE_IDLE_MS: u64 = 5_000;
pub const DEFAULT_AUTOSAVE_INTERVAL_MS: u64 = 45_000;
pub const DEFAULT_AUTOSAVE_FAILURE_BACKOFF_MS: u64 = 5_000;

/// Compression preference for session files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMode {
    /// Always write plain JSON.
    Off,
    /// Always write gzip-compressed JSON.
    On,
    /// Write gzip when payload exceeds the configured threshold.
    Auto,
}

/// Where session artifacts are stored for this run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTarget {
    /// Existing configured storage under the app-owned session directory.
    Configured,
    /// Direct user-selected session file. Sidecars are derived from this exact path.
    NamedFile(PathBuf),
}

/// Runtime options derived from configuration for session persistence.
#[derive(Debug, Clone)]
pub struct SessionOptions {
    pub target: SessionTarget,
    pub base_dir: PathBuf,
    pub persist_transparent: bool,
    pub persist_whiteboard: bool,
    pub persist_blackboard: bool,
    pub persist_history: bool,
    pub restore_tool_state: bool,
    pub autosave_enabled: bool,
    pub autosave_idle: Duration,
    pub autosave_interval: Duration,
    pub autosave_failure_backoff: Duration,
    pub max_shapes_per_frame: usize,
    pub max_persisted_undo_depth: Option<usize>,
    pub max_file_size_bytes: u64,
    pub compression: CompressionMode,
    pub auto_compress_threshold_bytes: u64,
    pub display_id: String,
    pub backup_retention: usize,
    pub output_identity: Option<String>,
    pub per_output: bool,
}

impl SessionOptions {
    /// Creates a basic options struct with sensible defaults. Intended mainly for tests.
    pub fn new(base_dir: PathBuf, display_id: impl Into<String>) -> Self {
        let raw_display = display_id.into();
        let display_id = sanitize_identifier(&raw_display);
        Self {
            target: SessionTarget::Configured,
            base_dir,
            persist_transparent: false,
            persist_whiteboard: false,
            persist_blackboard: false,
            persist_history: true,
            restore_tool_state: true,
            autosave_enabled: DEFAULT_AUTOSAVE_ENABLED,
            autosave_idle: Duration::from_millis(DEFAULT_AUTOSAVE_IDLE_MS),
            autosave_interval: Duration::from_millis(DEFAULT_AUTOSAVE_INTERVAL_MS),
            autosave_failure_backoff: Duration::from_millis(DEFAULT_AUTOSAVE_FAILURE_BACKOFF_MS),
            max_shapes_per_frame: 10_000,
            max_persisted_undo_depth: None,
            max_file_size_bytes: 50 * 1024 * 1024,
            compression: CompressionMode::Auto,
            auto_compress_threshold_bytes: DEFAULT_AUTO_COMPRESS_THRESHOLD_BYTES,
            display_id,
            backup_retention: 1,
            output_identity: None,
            per_output: true,
        }
    }

    pub fn any_enabled(&self) -> bool {
        self.persist_transparent || self.persist_whiteboard || self.persist_blackboard
    }

    pub fn effective_history_limit(&self, runtime_limit: usize) -> usize {
        if !self.persist_history {
            return 0;
        }
        match self.max_persisted_undo_depth {
            Some(limit) => limit.min(runtime_limit),
            None => runtime_limit,
        }
    }

    pub fn is_named_file(&self) -> bool {
        matches!(self.target, SessionTarget::NamedFile(_))
    }

    pub fn set_named_file_target(&mut self, path: PathBuf) {
        self.base_dir = session_file_parent_dir(&path);
        self.target = SessionTarget::NamedFile(path);
        self.output_identity = None;
        self.per_output = false;
    }

    pub fn force_resume_persistence(&mut self) {
        self.persist_transparent = true;
        self.persist_whiteboard = true;
        self.persist_blackboard = true;
        self.persist_history = true;
        self.restore_tool_state = true;
    }

    pub fn session_file_path(&self) -> PathBuf {
        match &self.target {
            SessionTarget::Configured => self
                .base_dir
                .join(format!("{}.json", self.session_file_stem())),
            SessionTarget::NamedFile(path) => path.clone(),
        }
    }

    pub fn backup_file_path(&self) -> PathBuf {
        match &self.target {
            SessionTarget::Configured => self
                .base_dir
                .join(format!("{}.json.bak", self.session_file_stem())),
            SessionTarget::NamedFile(path) => append_path_suffix(path, ".bak"),
        }
    }

    pub fn backup_recovery_marker_file_path(&self) -> PathBuf {
        match &self.target {
            SessionTarget::Configured => self
                .base_dir
                .join(format!("{}.json.bak.recoverable", self.session_file_stem())),
            SessionTarget::NamedFile(path) => append_path_suffix(path, ".bak.recoverable"),
        }
    }

    pub fn recovery_file_path(&self) -> PathBuf {
        match &self.target {
            SessionTarget::Configured => self
                .base_dir
                .join(format!("{}.json.recovery", self.session_file_stem())),
            SessionTarget::NamedFile(path) => append_path_suffix(path, ".recovery"),
        }
    }

    pub fn recovery_recoverable_marker_file_path(&self) -> PathBuf {
        match &self.target {
            SessionTarget::Configured => self.base_dir.join(format!(
                "{}.json.recovery.recoverable",
                self.session_file_stem()
            )),
            SessionTarget::NamedFile(path) => append_path_suffix(path, ".recovery.recoverable"),
        }
    }

    pub fn clear_marker_file_path(&self) -> PathBuf {
        match &self.target {
            SessionTarget::Configured => self
                .base_dir
                .join(format!("{}.json.cleared", self.session_file_stem())),
            SessionTarget::NamedFile(path) => append_path_suffix(path, ".cleared"),
        }
    }

    pub fn lock_file_path(&self) -> PathBuf {
        match &self.target {
            SessionTarget::Configured => self
                .base_dir
                .join(format!("{}.lock", self.session_file_stem())),
            SessionTarget::NamedFile(path) => append_path_suffix(path, ".lock"),
        }
    }

    pub(crate) fn recovery_backup_file_path(&self) -> PathBuf {
        append_path_suffix(&self.recovery_file_path(), ".bak")
    }

    pub(crate) fn corrupt_artifact_backup_file_path(&self, artifact_path: &Path) -> PathBuf {
        if artifact_path == self.session_file_path().as_path() {
            return self.backup_file_path();
        }
        if artifact_path == self.recovery_file_path().as_path() {
            return self.recovery_backup_file_path();
        }
        replace_path_extension(artifact_path, "recovery.bak")
    }

    pub fn file_prefix(&self) -> String {
        format!("session-{}", self.display_id)
    }

    fn session_file_stem(&self) -> String {
        if self.per_output {
            match &self.output_identity {
                Some(identity) => format!("{}-{}", self.file_prefix(), identity),
                None => self.file_prefix(),
            }
        } else {
            self.file_prefix()
        }
    }

    pub fn set_output_identity(&mut self, identity: Option<&str>) -> bool {
        if !self.per_output {
            self.output_identity = None;
            return false;
        }
        let sanitized = identity.map(sanitize_identifier);
        if self.output_identity == sanitized {
            false
        } else {
            self.output_identity = sanitized;
            true
        }
    }

    pub fn output_identity(&self) -> Option<&str> {
        self.output_identity.as_deref()
    }
}

pub(crate) fn session_file_parent_dir(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub(crate) fn append_path_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut raw = OsString::from(path.as_os_str());
    raw.push(suffix);
    PathBuf::from(raw)
}

fn replace_path_extension(path: &Path, extension: &str) -> PathBuf {
    let mut path = path.to_path_buf();
    path.set_extension(extension);
    path
}
