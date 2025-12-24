use crate::config::{SessionCompression, SessionConfig, SessionStorageMode};
use crate::paths::{data_dir, expand_tilde};
use anyhow::{Result, anyhow};
use std::{
    env,
    path::{Path, PathBuf},
};

pub const DEFAULT_AUTO_COMPRESS_THRESHOLD_BYTES: u64 = 100 * 1024; // 100 KiB

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

/// Runtime options derived from configuration for session persistence.
#[derive(Debug, Clone)]
pub struct SessionOptions {
    pub base_dir: PathBuf,
    pub persist_transparent: bool,
    pub persist_whiteboard: bool,
    pub persist_blackboard: bool,
    pub persist_history: bool,
    pub restore_tool_state: bool,
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
            base_dir,
            persist_transparent: false,
            persist_whiteboard: false,
            persist_blackboard: false,
            persist_history: true,
            restore_tool_state: true,
            max_shapes_per_frame: 10_000,
            max_persisted_undo_depth: None,
            max_file_size_bytes: 10 * 1024 * 1024,
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

    pub fn session_file_path(&self) -> PathBuf {
        self.base_dir
            .join(format!("{}.json", self.session_file_stem()))
    }

    pub fn backup_file_path(&self) -> PathBuf {
        self.base_dir
            .join(format!("{}.json.bak", self.session_file_stem()))
    }

    pub fn lock_file_path(&self) -> PathBuf {
        self.base_dir
            .join(format!("{}.lock", self.session_file_stem()))
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

/// Build runtime session options from configuration values.
pub fn options_from_config(
    session_cfg: &SessionConfig,
    config_dir: &Path,
    display_id: Option<&str>,
) -> Result<SessionOptions> {
    let base_dir = match session_cfg.storage {
        SessionStorageMode::Auto => {
            let root = data_dir().unwrap_or_else(|| config_dir.to_path_buf());
            root.join("wayscriber")
        }
        SessionStorageMode::Config => config_dir.to_path_buf(),
        SessionStorageMode::Custom => {
            let raw = session_cfg.custom_directory.as_ref().ok_or_else(|| {
                anyhow!("session.custom_directory must be set when storage = \"custom\"")
            })?;
            let expanded = expand_tilde(raw);
            if expanded.as_os_str().is_empty() {
                return Err(anyhow!(
                    "session.custom_directory resolved to an empty path"
                ));
            }
            expanded
        }
    };

    let mut options = SessionOptions::new(base_dir, resolve_display_id(display_id));
    options.persist_transparent = session_cfg.persist_transparent;
    options.persist_whiteboard = session_cfg.persist_whiteboard;
    options.persist_blackboard = session_cfg.persist_blackboard;
    options.persist_history = session_cfg.persist_history;
    options.restore_tool_state = session_cfg.restore_tool_state;
    options.max_shapes_per_frame = session_cfg.max_shapes_per_frame;
    options.max_persisted_undo_depth = session_cfg
        .max_persisted_undo_depth
        .map(|limit| limit.clamp(10, 1000));
    options.max_file_size_bytes = session_cfg
        .max_file_size_mb
        .saturating_mul(1024 * 1024)
        .max(1);
    options.auto_compress_threshold_bytes = session_cfg
        .auto_compress_threshold_kb
        .saturating_mul(1024)
        .max(1);
    options.compression = match session_cfg.compress {
        SessionCompression::Auto => CompressionMode::Auto,
        SessionCompression::On => CompressionMode::On,
        SessionCompression::Off => CompressionMode::Off,
    };
    options.backup_retention = session_cfg.backup_retention;
    options.per_output = session_cfg.per_output;

    Ok(options)
}

pub(crate) fn sanitize_identifier(raw: &str) -> String {
    if raw.is_empty() {
        return "default".to_string();
    }

    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn resolve_display_id(display_id: Option<&str>) -> String {
    if let Some(id) = display_id {
        return sanitize_identifier(id);
    }

    match env::var("WAYLAND_DISPLAY") {
        Ok(value) => {
            log::info!("Session display id from WAYLAND_DISPLAY='{}'", value);
            sanitize_identifier(&value)
        }
        Err(_) => {
            log::info!("Session display id fallback to 'default' (WAYLAND_DISPLAY missing)");
            "default".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SessionConfig, SessionStorageMode};
    use std::path::Path;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn sanitize_identifier_replaces_non_alphanumeric() {
        assert_eq!(sanitize_identifier("DP-1"), "DP_1");
        assert_eq!(sanitize_identifier("output:name"), "output_name");
        assert_eq!(sanitize_identifier("abc/def-01"), "abc_def_01");
    }

    #[test]
    fn sanitize_identifier_empty_defaults_to_default() {
        assert_eq!(sanitize_identifier(""), "default");
    }

    #[test]
    fn resolve_display_id_prefers_argument_and_uses_env_fallback() {
        use std::env;

        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let prev = env::var_os("WAYLAND_DISPLAY");
        // SAFETY: serialized via ENV_MUTEX
        unsafe {
            env::set_var("WAYLAND_DISPLAY", "wayland-0");
        }

        let from_arg = resolve_display_id(Some("custom-display"));
        assert_eq!(from_arg, "custom_display");

        let from_env = resolve_display_id(None);
        assert_eq!(from_env, "wayland_0");

        match prev {
            Some(v) => unsafe { env::set_var("WAYLAND_DISPLAY", v) },
            None => unsafe { env::remove_var("WAYLAND_DISPLAY") },
        }
    }

    #[test]
    fn options_from_config_clamps_max_persisted_undo_depth() {
        let mut cfg = SessionConfig {
            max_persisted_undo_depth: Some(5),
            storage: SessionStorageMode::Config,
            ..SessionConfig::default()
        };

        let opts = options_from_config(&cfg, Path::new("/tmp"), Some("display")).unwrap();
        assert_eq!(opts.max_persisted_undo_depth, Some(10));

        cfg.max_persisted_undo_depth = Some(2_000);
        let opts2 = options_from_config(&cfg, Path::new("/tmp"), Some("display")).unwrap();
        assert_eq!(opts2.max_persisted_undo_depth, Some(1_000));
    }

    #[test]
    fn effective_history_limit_respects_persist_history_flag() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
        options.persist_history = false;
        options.max_persisted_undo_depth = Some(10);

        let limit = options.effective_history_limit(50);
        assert_eq!(limit, 0);
    }

    #[test]
    fn effective_history_limit_clamps_to_runtime_limit() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
        options.persist_history = true;
        options.max_persisted_undo_depth = Some(5);

        let limit = options.effective_history_limit(3);
        assert_eq!(limit, 3);
    }
}
