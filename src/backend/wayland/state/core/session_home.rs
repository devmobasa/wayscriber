use log::{info, warn};

use super::super::*;
use crate::backend::wayland::backend::event_loop::session_save;
use crate::backend::wayland::session::{PersistenceOperation, PersistenceOutcome, session_target};
use crate::daemon::protocol_v2::publish_session_from_environment;
use crate::input::state::{Toast, ToastPriority};
use crate::ui::toolbar::session_format::session_display_name;

impl WaylandState {
    /// Starts at home instead of the remembered session this run was asked to
    /// continue when that session no longer exists: it was moved or deleted
    /// after the daemon chose it. The daemon hears that this overlay is home,
    /// so the next show does not try the missing session again.
    pub(in crate::backend::wayland) fn start_at_home_if_preferred_session_is_gone(&mut self) {
        let Some(preferred) = self.session_home.take_unchecked_preferred() else {
            return;
        };
        let Some(options) = self.session_options().cloned() else {
            return;
        };
        match session_save::run_persistence_operation(
            self,
            PersistenceOperation::HasArtifacts { options },
        ) {
            Ok(PersistenceOutcome::HasArtifacts(true)) => return,
            Ok(PersistenceOutcome::HasArtifacts(false)) => {}
            Ok(other) => {
                warn!("Unexpected outcome checking the remembered session: {other:?}");
                return;
            }
            // The load that follows reports the failure the way it would for
            // a session file given at startup.
            Err(error) => {
                warn!(
                    "Failed to check remembered session {}: {error:#}",
                    preferred.display()
                );
                return;
            }
        }

        info!(
            "Remembered session {} no longer exists; starting at home",
            preferred.display()
        );
        let home = self.session_home.options().cloned();
        self.session.replace_options_before_load(home.clone());
        self.input_state.set_session_preflight_options(home);
        self.input_state.push_toast(
            ToastPriority::Info,
            "session",
            Toast::warning(format!(
                "Session {} is no longer available; opened {}",
                session_display_name(&preferred),
                self.session_home.label()
            )),
        );
        self.session_target_committed();
    }

    /// Tells the daemon that launched this overlay the session it is now in,
    /// if that changed, so the next show starts there. A failure leaves the
    /// daemon with what it knew before, and only that is reported: the session
    /// switch stands.
    pub(in crate::backend::wayland) fn session_target_committed(&mut self) {
        let target = session_target(self.session.options());
        let Some(session) = self.session_home.commit_target(target) else {
            return;
        };
        if let Err(error) = publish_session_from_environment(&session) {
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
