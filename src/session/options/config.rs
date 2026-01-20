use std::path::Path;

use anyhow::{Result, anyhow};

use crate::config::{SessionCompression, SessionConfig, SessionStorageMode};
use crate::paths::{data_dir, expand_tilde};

use super::identifiers::resolve_display_id;
use super::types::{CompressionMode, SessionOptions};
use std::time::Duration;

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
    options.autosave_enabled = session_cfg.autosave_enabled;
    options.autosave_idle = Duration::from_millis(session_cfg.autosave_idle_ms.max(1));
    options.autosave_interval = Duration::from_millis(session_cfg.autosave_interval_ms.max(1));
    options.autosave_failure_backoff =
        Duration::from_millis(session_cfg.autosave_failure_backoff_ms.max(1));
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
