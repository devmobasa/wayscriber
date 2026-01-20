use log::{info, warn};
use std::env;
use std::path::Path;

use crate::config::Config;
use crate::{RESUME_SESSION_ENV, paths, session};

use super::super::helpers::resume_override_from_env;

pub(super) fn build_session_options(
    config: &Config,
    config_dir: &Path,
) -> Option<session::SessionOptions> {
    let display_env = env::var("WAYLAND_DISPLAY").ok();
    let resume_override = resume_override_from_env();
    let mut session_options =
        match session::options_from_config(&config.session, config_dir, display_env.as_deref()) {
            Ok(opts) => Some(opts),
            Err(err) => {
                warn!("Session persistence disabled: {}", err);
                None
            }
        };

    match resume_override {
        Some(true) => {
            if session_options.is_none() {
                let default_base = paths::data_dir()
                    .unwrap_or_else(|| config_dir.to_path_buf())
                    .join("wayscriber");
                let display = display_env.clone().unwrap_or_else(|| "default".to_string());
                session_options = Some(session::SessionOptions::new(default_base, display));
            }
            if let Some(options) = session_options.as_mut() {
                options.persist_transparent = true;
                options.persist_whiteboard = true;
                options.persist_blackboard = true;
                options.persist_history = true;
                options.restore_tool_state = true;
                info!(
                    "Session resume forced on via {} (persisting all boards, history, tool state)",
                    RESUME_SESSION_ENV
                );
            }
        }
        Some(false) => {
            if session_options.is_some() {
                info!("Session resume disabled via {}=off", RESUME_SESSION_ENV);
            }
            session_options = None;
        }
        None => {}
    }

    if let Some(ref opts) = session_options {
        info!(
            "Session persistence: base_dir={}, per_output={}, display_id='{}', output_identity={:?}, boards[T/W/B]={}/{}/{}, history={}, max_persisted_history={:?}, restore_tool_state={}, autosave_enabled={}, autosave_idle_ms={}, autosave_interval_ms={}, autosave_failure_backoff_ms={}, max_file_size={} bytes, compression={:?}",
            opts.base_dir.display(),
            opts.per_output,
            opts.display_id,
            opts.output_identity(),
            opts.persist_transparent,
            opts.persist_whiteboard,
            opts.persist_blackboard,
            opts.persist_history,
            opts.max_persisted_undo_depth,
            opts.restore_tool_state,
            opts.autosave_enabled,
            opts.autosave_idle.as_millis(),
            opts.autosave_interval.as_millis(),
            opts.autosave_failure_backoff.as_millis(),
            opts.max_file_size_bytes,
            opts.compression
        );
    } else {
        info!("Session persistence disabled (no session options available)");
    }

    session_options
}
