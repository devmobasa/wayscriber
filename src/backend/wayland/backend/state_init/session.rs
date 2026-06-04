use log::{info, warn};
use std::env;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::{RESUME_SESSION_ENV, paths, session};

use super::super::helpers::resume_override_from_env;

pub(super) fn build_session_options(
    config: &Config,
    config_dir: &Path,
    named_session_file: Option<PathBuf>,
) -> Option<session::SessionOptions> {
    let display_env = env::var("WAYLAND_DISPLAY").ok();
    let resume_override = resume_override_from_env();
    let mut session_options = if let Some(path) = named_session_file {
        let mut options = session::options_from_config_for_named_file(
            &config.session,
            path,
            display_env.as_deref(),
        );
        options.force_resume_persistence();
        info!(
            "Session persistence forced on for named session file {}",
            options.session_file_path().display()
        );
        Some(options)
    } else {
        match session::options_from_config(&config.session, config_dir, display_env.as_deref()) {
            Ok(opts) => Some(opts),
            Err(err) => {
                warn!("Session persistence disabled: {}", err);
                None
            }
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
                options.force_resume_persistence();
                info!(
                    "Session resume forced on via {} (persisting all boards, history, tool state)",
                    RESUME_SESSION_ENV
                );
            }
        }
        Some(false) => {
            if session_options
                .as_ref()
                .is_some_and(session::SessionOptions::is_named_file)
            {
                info!(
                    "Ignoring {}=off because a named session file requires persistence for this run",
                    RESUME_SESSION_ENV
                );
                if let Some(options) = session_options.as_mut() {
                    options.force_resume_persistence();
                }
            } else {
                if session_options.is_some() {
                    info!("Session resume disabled via {}=off", RESUME_SESSION_ENV);
                }
                session_options = None;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_session_file_forces_persistence_even_when_config_disables_it() {
        let mut config = Config::default();
        config.session.persist_transparent = false;
        config.session.persist_whiteboard = false;
        config.session.persist_blackboard = false;
        config.session.persist_history = false;
        config.session.restore_tool_state = false;

        let options = build_session_options(
            &config,
            Path::new("/tmp/config"),
            Some(PathBuf::from("/tmp/lecture-04.wayscriber-session")),
        )
        .expect("named session options should be available");

        assert!(options.is_named_file());
        assert!(options.persist_transparent);
        assert!(options.persist_whiteboard);
        assert!(options.persist_blackboard);
        assert!(options.persist_history);
        assert!(options.restore_tool_state);
    }
}
