use anyhow::Result;
use wayland_client::Connection;

use crate::env_vars::WAYLAND_DISPLAY_ENV;

pub mod wayland;

// Removed: Backend trait - no longer needed with single backend
// Removed: BackendChoice enum - Wayland is the only backend

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitAfterCaptureMode {
    Auto,
    Always,
    Never,
}

pub(crate) struct WaylandRunContext {
    pub(crate) initial_mode: Option<String>,
    pub(crate) freeze_on_start: bool,
    pub(crate) exit_after_capture_mode: ExitAfterCaptureMode,
    pub(crate) named_session_file: Option<std::path::PathBuf>,
    pub(crate) session_resume_override: Option<bool>,
    pub(crate) process_broker: crate::process_broker::ProcessBrokerHandle,
    pub(crate) path_resolver: crate::paths::PathResolver,
    pub(crate) runtime_paths: crate::paths::PreparedRuntimePaths,
    pub(crate) config_store: crate::config::ConfigStore,
    pub(crate) logger: crate::logger::LoggerHandle,
}

/// Run Wayland backend with full event loop
///
/// # Arguments
/// * `initial_mode` - Optional board mode to start in (overrides config default)
/// * `freeze_on_start` - Whether to start with the overlay frozen for immediate capture pause
/// * `exit_after_capture_mode` - Exit behavior after a capture completes
pub fn run_wayland(
    context: WaylandRunContext,
    signal_source: &mut dyn crate::unix_signals::SignalEventSource,
) -> Result<()> {
    let mut backend = wayland::WaylandBackend::new(context)?;
    backend.init()?;
    backend.show(signal_source)?; // show() calls run() internally
    backend.hide()?;
    Ok(())
}

pub fn preflight_wayland_connection() -> Result<()> {
    if std::env::var(WAYLAND_DISPLAY_ENV).is_err() {
        return Err(anyhow::anyhow!(
            "{WAYLAND_DISPLAY_ENV} not set - this application requires Wayland."
        ));
    }
    let _conn = Connection::connect_to_env()
        .map_err(|err| anyhow::anyhow!("Failed to connect to Wayland compositor: {err}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn wayland_backend_smoke_test() {
        if std::env::var(super::WAYLAND_DISPLAY_ENV).is_err() {
            eprintln!(
                "{} not set; skipping Wayland smoke test",
                super::WAYLAND_DISPLAY_ENV
            );
            return;
        }
        let process_broker_owner = crate::process_broker::start_for_runtime()
            .expect("Wayland smoke fixture starts its process broker owner");
        let mut signal_source = crate::unix_signals::FakeSignalSource::new()
            .expect("Wayland smoke fixture creates its signal source");
        let resolver = crate::paths::PathResolver::from_process_environment();
        super::run_wayland(
            super::WaylandRunContext {
                initial_mode: None,
                freeze_on_start: false,
                exit_after_capture_mode: super::ExitAfterCaptureMode::Never,
                named_session_file: None,
                session_resume_override: None,
                process_broker: process_broker_owner.handle(),
                runtime_paths: crate::paths::PreparedRuntimePaths::prepare(&resolver)
                    .expect("Wayland smoke fixture provides a private runtime directory"),
                config_store: crate::config::ConfigStore::at_path(
                    "/tmp/wayscriber-smoke-config.toml",
                ),
                logger: crate::logger::LoggerOwner::start(false, &resolver)
                    .expect("Wayland smoke fixture starts its logger owner")
                    .1,
                path_resolver: resolver,
            },
            &mut signal_source,
        )
        .expect("Wayland backend should start");
    }
}
