use anyhow::Result;

use super::super::core::Daemon;

impl Daemon {
    pub(in crate::daemon) fn update_overlay_process_state(&mut self) -> Result<()> {
        if self.backend_runner.is_some() {
            return Ok(());
        }

        self.overlay.poll_exit()
    }
}
