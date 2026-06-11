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

/// Run Wayland backend with full event loop
///
/// # Arguments
/// * `initial_mode` - Optional board mode to start in (overrides config default)
/// * `freeze_on_start` - Whether to start with the overlay frozen for immediate capture pause
/// * `exit_after_capture_mode` - Exit behavior after a capture completes
pub fn run_wayland(
    initial_mode: Option<String>,
    freeze_on_start: bool,
    exit_after_capture_mode: ExitAfterCaptureMode,
    named_session_file: Option<std::path::PathBuf>,
) -> Result<()> {
    let mut backend = wayland::WaylandBackend::new(
        initial_mode,
        freeze_on_start,
        exit_after_capture_mode,
        named_session_file,
    )?;
    backend.init()?;
    backend.show()?; // show() calls run() internally
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
        super::run_wayland(None, false, super::ExitAfterCaptureMode::Never, None)
            .expect("Wayland backend should start");
    }
}
