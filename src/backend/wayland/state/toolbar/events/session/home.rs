use super::*;
use crate::session::SessionOptions;

impl WaylandState {
    /// Returns to the home session the way Open switches session: the current
    /// session is saved first, and nothing changes unless home loads.
    pub(super) fn handle_toolbar_open_home_session(&mut self) {
        self.clear_toolbar_save_as_overwrite_prompt();
        let command = SessionCommand::OpenHome(self.home_session_options().map(Box::new));
        if let Err(error) = self.start_session_command(command) {
            self.report_session_command_error("Return to the home session failed", &error);
        }
    }

    /// Home's options for the output the overlay is on now, not those of the
    /// named session it is leaving.
    fn home_session_options(&self) -> Option<SessionOptions> {
        self.session_home
            .options_for_output(self.current_output_identity().as_deref())
    }

    fn current_output_identity(&self) -> Option<String> {
        self.surface
            .current_output()
            .as_ref()
            .and_then(|output| self.output_identity_for(output))
    }

    /// Home is committed. The overlay may have moved to another output while
    /// it loaded, and a per-output home follows it there.
    pub(super) fn finish_open_home_session(&mut self) {
        let output_identity = self.current_output_identity();
        self.begin_session_output_transition(output_identity, "return to the home session");

        self.set_session_toolbar_info(format!("Returned to {}", self.session_home.label()));
    }
}
