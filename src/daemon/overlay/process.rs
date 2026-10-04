use anyhow::Result;

use super::super::core::Daemon;

impl Daemon {
    pub(in crate::daemon) fn update_overlay_process_state(&mut self) -> Result<()> {
        if self.backend_runner.is_some() {
            return Ok(());
        }

        let session = self.overlay.poll_exit()?;
        self.remember_reported_session(session);
        Ok(())
    }
}
