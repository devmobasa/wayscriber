use anyhow::Result;
use log::{debug, info};

use crate::{runtime_session_override, set_runtime_session_override};

use super::core::Daemon;
use super::types::OverlayState;

mod process;
mod spawn;

impl Daemon {
    /// Toggle overlay visibility
    pub(super) fn toggle_overlay(&mut self) -> Result<()> {
        match self.overlay_state {
            OverlayState::Hidden => {
                info!("Showing overlay");
                self.show_overlay()?;
            }
            OverlayState::Visible => {
                info!("Hiding overlay");
                self.hide_overlay()?;
            }
        }
        Ok(())
    }

    /// Show overlay (create layer surface and enter drawing mode)
    pub(super) fn show_overlay(&mut self) -> Result<()> {
        if self.overlay_state == OverlayState::Visible {
            debug!("Overlay already visible");
            return Ok(());
        }

        if let Some(runner) = self.backend_runner.clone() {
            self.overlay_state = OverlayState::Visible;
            info!("Overlay state set to Visible");
            self.clear_overlay_spawn_error();
            let previous_override = runtime_session_override();
            set_runtime_session_override(self.session_resume_override());
            let result = runner(self.initial_mode.clone());
            set_runtime_session_override(previous_override);
            self.overlay_state = OverlayState::Hidden;
            info!("Overlay closed, back to daemon mode");
            return result;
        }

        if !self.overlay_spawn_allowed() {
            return Ok(());
        }

        if let Err(err) = self.spawn_overlay_process() {
            self.record_overlay_spawn_failure(err.to_string());
            return Err(err);
        }
        self.clear_overlay_spawn_error();
        Ok(())
    }

    /// Hide overlay (destroy layer surface, return to hidden state)
    pub(super) fn hide_overlay(&mut self) -> Result<()> {
        if self.overlay_state == OverlayState::Hidden {
            debug!("Overlay already hidden");
            return Ok(());
        }

        if self.backend_runner.is_some() {
            // Internal runner does not keep additional state to tear down
            debug!("Internal backend runner hidden");
            self.overlay_state = OverlayState::Hidden;
            return Ok(());
        }

        self.terminate_overlay_process()?;
        self.overlay_state = OverlayState::Hidden;
        Ok(())
    }
}
