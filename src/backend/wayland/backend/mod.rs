// Coordinates backend startup/shutdown and drives the event loop while delegating
// rendering & protocol state to `WaylandState` and its handler modules.
use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::backend::{ExitAfterCaptureMode, WaylandRunContext};

pub(in crate::backend::wayland) mod event_loop;
mod helpers;
mod run;
pub(crate) mod runtime_wake;
mod setup;
mod signals;
mod state_init;
mod surface;
mod tray;

pub struct WaylandBackend {
    pub(super) initial_mode: Option<String>,
    pub(super) freeze_on_start: bool,
    pub(super) exit_after_capture_mode: ExitAfterCaptureMode,
    pub(super) named_session_file: Option<PathBuf>,
    pub(super) session_resume_override: Option<bool>,
    pub(super) process_broker: crate::process_broker::ProcessBrokerHandle,
    pub(super) path_resolver: crate::paths::PathResolver,
    pub(super) runtime_paths: crate::paths::PreparedRuntimePaths,
    pub(super) config_store: crate::config::ConfigStore,
    pub(super) logger: crate::logger::LoggerHandle,
    /// Tokio runtime for async capture operations
    pub(super) tokio_runtime: tokio::runtime::Runtime,
}

impl WaylandBackend {
    pub fn new(context: WaylandRunContext) -> Result<Self> {
        let tokio_runtime = tokio::runtime::Runtime::new()
            .context("Failed to create Tokio runtime for capture operations")?;
        Ok(Self {
            initial_mode: context.initial_mode,
            freeze_on_start: context.freeze_on_start,
            exit_after_capture_mode: context.exit_after_capture_mode,
            named_session_file: context.named_session_file,
            session_resume_override: context.session_resume_override,
            process_broker: context.process_broker,
            path_resolver: context.path_resolver,
            runtime_paths: context.runtime_paths,
            config_store: context.config_store,
            logger: context.logger,
            tokio_runtime,
        })
    }

    pub fn run(
        &mut self,
        signal_source: &mut dyn crate::unix_signals::SignalEventSource,
    ) -> Result<()> {
        run::run_backend(self, signal_source)
    }

    pub fn init(&mut self) -> Result<()> {
        self.logger.info(
            "wayscriber::backend::wayland",
            "Wayland backend initialized",
        );
        log::info!("Initializing Wayland backend");
        Ok(())
    }

    pub fn show(
        &mut self,
        signal_source: &mut dyn crate::unix_signals::SignalEventSource,
    ) -> Result<()> {
        log::info!("Showing Wayland overlay");
        self.run(signal_source)
    }

    pub fn hide(&mut self) -> Result<()> {
        log::info!("Hiding Wayland overlay");
        Ok(())
    }
}
