use anyhow::Result;

use crate::input::SystemCommand;

pub mod wayland;

// Removed: Backend trait - no longer needed with single backend
// Removed: BackendChoice enum - Wayland is the only backend

/// Run Wayland backend with full event loop
///
/// # Arguments
/// * `initial_mode` - Optional board mode to start in (overrides config default)
pub fn run_wayland(initial_mode: Option<String>) -> Result<Option<SystemCommand>> {
    let mut backend = wayland::WaylandBackend::new(initial_mode)?;
    backend.init()?;
    let exit_command = backend.show()?; // show() calls run() internally
    backend.hide()?;
    Ok(exit_command)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn wayland_backend_smoke_test() {
        if std::env::var("WAYLAND_DISPLAY").is_err() {
            eprintln!("WAYLAND_DISPLAY not set; skipping Wayland smoke test");
            return;
        }
        super::run_wayland(None).expect("Wayland backend should start");
    }
}
