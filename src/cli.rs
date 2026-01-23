use clap::{ArgAction, Parser};

#[derive(Parser, Debug)]
#[command(name = "wayscriber")]
#[command(version, about = "Screen annotation tool for Wayland compositors")]
pub struct Cli {
    /// Run as daemon (background service; bind a toggle like Super+D)
    #[arg(long, short = 'd', action = ArgAction::SetTrue)]
    pub daemon: bool,

    /// Start active (show overlay immediately, one-shot mode)
    #[arg(long, short = 'a', action = ArgAction::SetTrue)]
    pub active: bool,

    /// Initial board id (transparent, whiteboard, blackboard, or a custom id)
    #[arg(long, short = 'm', value_name = "MODE")]
    pub mode: Option<String>,

    /// Disable system tray (run daemon without a tray icon)
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_tray: bool,

    /// Delete persisted session data and backups
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = [
            "daemon",
            "active",
        ]
    )]
    pub clear_session: bool,

    /// Show session persistence status and file paths
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = [
            "daemon",
            "active",
            "clear_session"
        ]
    )]
    pub session_info: bool,

    /// Start with frozen mode active (freeze the screen immediately)
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = ["daemon", "clear_session", "session_info"]
    )]
    pub freeze: bool,

    /// Exit the overlay after a capture completes (overrides auto clipboard exit)
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "no_exit_after_capture")]
    pub exit_after_capture: bool,

    /// Keep the overlay open after capture (disables auto clipboard exit)
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "exit_after_capture")]
    pub no_exit_after_capture: bool,

    /// Force session resume on (persist/restore all boards + history/tool state)
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "no_resume_session")]
    pub resume_session: bool,

    /// Force session resume off (ignore persisted session data for this run)
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "resume_session")]
    pub no_resume_session: bool,

    /// Show the About window
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = [
            "daemon",
            "active",
            "mode",
            "no_tray",
            "clear_session",
            "session_info",
            "freeze",
            "resume_session",
            "no_resume_session"
        ]
    )]
    pub about: bool,
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn active_mode_with_explicit_board_id() {
        let cli = Cli::try_parse_from(["wayscriber", "--active", "--mode", "whiteboard"]).unwrap();
        assert!(cli.active);
        assert_eq!(cli.mode.as_deref(), Some("whiteboard"));
    }

    #[test]
    fn cli_conflicting_flags_fail() {
        let result = Cli::try_parse_from(["wayscriber", "--active", "--clear-session"]);
        assert!(
            result.is_err(),
            "expected conflicting flags (--active and --clear-session) to error"
        );
    }
}
