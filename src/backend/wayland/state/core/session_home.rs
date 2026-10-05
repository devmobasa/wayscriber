use std::path::Path;

use log::{info, warn};

use super::super::*;
use crate::backend::wayland::session::session_target;
use crate::daemon::protocol_v2::publish_session_from_environment;
use crate::input::state::{Toast, ToastPriority};
use crate::session::MissingNamedSessionFile;
use crate::ui::toolbar::session_format::session_display_name;

impl WaylandState {
    pub(in crate::backend::wayland::state) fn notify_remembered_session_abandoned(
        &mut self,
        path: &Path,
        error: &anyhow::Error,
    ) {
        info!(
            "Remembered session {} cannot be continued; starting at home: {error:#}",
            path.display()
        );
        let name = session_display_name(path);
        let message = if error.downcast_ref::<MissingNamedSessionFile>().is_some() {
            format!("Session {name} is no longer available")
        } else {
            format!("Session {name} can no longer be used ({error:#})")
        };
        self.input_state.push_toast(
            ToastPriority::Info,
            "session",
            Toast::warning(format!("{message}; opened {}", self.session_home.label())),
        );
    }

    /// Tells the daemon that launched this overlay the session it is now in,
    /// if the daemon does not know it yet, so the next show starts there. A
    /// failure leaves the daemon with what it knew before, and only that is
    /// reported: the session switch stands, and the next commit tries again.
    pub(in crate::backend::wayland) fn report_session_to_daemon(&mut self) {
        self.session_home
            .enter(session_target(self.session.options()));
        let Some(session) = self.session_home.unreported() else {
            return;
        };

        match publish_session_from_environment(&session) {
            Ok(_) => self.session_home.mark_reported(),
            Err(error) => {
                warn!("Failed to report the session to the daemon: {error:#}");
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "session.report",
                    Toast::warning(format!(
                        "The overlay may not reopen in this session after it hides: {error:#}"
                    )),
                );
            }
        }
    }
}
