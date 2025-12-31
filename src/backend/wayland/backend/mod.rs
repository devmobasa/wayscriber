// Coordinates backend startup/shutdown and drives the event loop while delegating
// rendering & protocol state to `WaylandState` and its handler modules.
use anyhow::{Context, Result};

use crate::backend::ExitAfterCaptureMode;

mod event_loop;
mod helpers;
mod run;
mod setup;
mod signals;
mod state_init;
mod surface;
mod tray;

pub struct WaylandBackend {
    pub(super) initial_mode: Option<String>,
    pub(super) freeze_on_start: bool,
    pub(super) exit_after_capture_mode: ExitAfterCaptureMode,
    /// Tokio runtime for async capture operations
    pub(super) tokio_runtime: tokio::runtime::Runtime,
}

impl WaylandBackend {
    pub fn new(
        initial_mode: Option<String>,
        freeze_on_start: bool,
        exit_after_capture_mode: ExitAfterCaptureMode,
    ) -> Result<Self> {
        let tokio_runtime = tokio::runtime::Runtime::new()
            .context("Failed to create Tokio runtime for capture operations")?;
        Ok(Self {
            initial_mode,
            freeze_on_start,
            exit_after_capture_mode,
            tokio_runtime,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        run::run_backend(self)
    }

    pub fn init(&mut self) -> Result<()> {
        log::info!("Initializing Wayland backend");
        Ok(())
    }

    pub fn show(&mut self) -> Result<()> {
        log::info!("Showing Wayland overlay");
        self.run()
    }

    pub fn hide(&mut self) -> Result<()> {
        log::info!("Hiding Wayland overlay");
        Ok(())
    }
}
