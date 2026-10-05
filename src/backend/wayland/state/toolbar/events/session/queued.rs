use super::*;

impl WaylandState {
    /// One toast names every queued command that will not run, since each
    /// session toast replaces the last. At exit the toast is never seen, so a
    /// desktop notification names the commands that were requested but lost.
    pub(in crate::backend::wayland) fn fail_queued_session_commands(
        &mut self,
        failures: Vec<(SessionCommand, AnyhowError)>,
        closing: bool,
    ) {
        if closing {
            // Reading info and forgetting a missing entry change nothing worth naming.
            let lost = failures
                .iter()
                .filter(|(command, _)| {
                    !matches!(command, SessionCommand::Inspect | SessionCommand::Forget(_))
                })
                .map(|(command, _)| queued_command_label(command))
                .collect::<Vec<_>>();
            if !lost.is_empty() {
                crate::notification::send_notification_async(
                    &self.tokio_handle,
                    "Session Commands Not Run".to_string(),
                    format!(
                        "Wayscriber closed before it could run: {}. Run them again after restarting.",
                        lost.join("; ")
                    ),
                    Some("dialog-warning".to_string()),
                );
            }
        }

        match failures.as_slice() {
            [] => {}
            [(command, error)] => self.fail_session_command(command, error),
            _ => {
                let summary = failures
                    .iter()
                    .map(|(command, error)| format!("{}: {error:#}", queued_command_label(command)))
                    .collect::<Vec<_>>()
                    .join("; ");
                self.set_session_toolbar_error(format!(
                    "Queued session commands did not run: {summary}"
                ));
            }
        }
    }
}

/// What a queued command would have done, for a report that it did not run.
fn queued_command_label(command: &SessionCommand) -> String {
    match command {
        SessionCommand::Open(path) => format!("open {}", session_display_name(path)),
        SessionCommand::OpenHome(_) => "return to the home session".to_string(),
        SessionCommand::SaveAs(path, _) | SessionCommand::CheckOverwrite(path) => {
            format!("save as {}", session_display_name(path))
        }
        SessionCommand::Clear => "clear the session".to_string(),
        SessionCommand::ClearTools(_) => "reset tool defaults".to_string(),
        SessionCommand::Output { .. } => "switch the output session".to_string(),
        SessionCommand::Inspect => "show session info".to_string(),
        SessionCommand::Forget(path) => {
            format!("forget recent session {}", session_display_name(path))
        }
    }
}
