use clap::{ArgAction, Parser};

use crate::tray_action::TrayAction;

#[derive(Parser, Debug)]
#[command(name = "wayscriber")]
#[command(
    version = crate::build_info::version(),
    about = "Screen annotation tool for Wayland compositors"
)]
pub struct Cli {
    /// Run as daemon (background service; bind a toggle like Super+D)
    #[arg(long, short = 'd', action = ArgAction::SetTrue)]
    pub daemon: bool,

    /// Send a toggle request to the running daemon (supports --mode/--freeze/exit/session overrides)
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = [
            "daemon",
            "active",
            "no_tray",
            "freeze_on_show",
            "clear_session",
            "session_info",
            "about"
        ]
    )]
    pub daemon_toggle: bool,

    /// Send an action to the active overlay through the running daemon
    #[arg(
        long,
        value_name = "ACTION",
        conflicts_with_all = [
            "daemon",
            "daemon_toggle",
            "active",
            "mode",
            "no_tray",
            "freeze_on_show",
            "clear_session",
            "session_info",
            "freeze",
            "exit_after_capture",
            "no_exit_after_capture",
            "resume_session",
            "no_resume_session",
            "about"
        ]
    )]
    pub daemon_action: Option<String>,

    /// Toggle light passthrough mode through the running daemon
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = [
            "daemon",
            "daemon_toggle",
            "daemon_action",
            "light_draw_toggle",
            "light_draw_on",
            "light_draw_off",
            "active",
            "mode",
            "no_tray",
            "freeze_on_show",
            "clear_session",
            "session_info",
            "freeze",
            "exit_after_capture",
            "no_exit_after_capture",
            "resume_session",
            "no_resume_session",
            "about"
        ]
    )]
    pub light_toggle: bool,

    /// Toggle drawing while light passthrough mode is active
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = [
            "daemon",
            "daemon_toggle",
            "daemon_action",
            "light_toggle",
            "light_draw_on",
            "light_draw_off",
            "active",
            "mode",
            "no_tray",
            "freeze_on_show",
            "clear_session",
            "session_info",
            "freeze",
            "exit_after_capture",
            "no_exit_after_capture",
            "resume_session",
            "no_resume_session",
            "about"
        ]
    )]
    pub light_draw_toggle: bool,

    /// Turn light-mode drawing on through the running daemon
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = [
            "daemon",
            "daemon_toggle",
            "daemon_action",
            "light_toggle",
            "light_draw_toggle",
            "light_draw_off",
            "active",
            "mode",
            "no_tray",
            "freeze_on_show",
            "clear_session",
            "session_info",
            "freeze",
            "exit_after_capture",
            "no_exit_after_capture",
            "resume_session",
            "no_resume_session",
            "about"
        ]
    )]
    pub light_draw_on: bool,

    /// Turn light-mode drawing off through the running daemon
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = [
            "daemon",
            "daemon_toggle",
            "daemon_action",
            "light_toggle",
            "light_draw_toggle",
            "light_draw_on",
            "active",
            "mode",
            "no_tray",
            "freeze_on_show",
            "clear_session",
            "session_info",
            "freeze",
            "exit_after_capture",
            "no_exit_after_capture",
            "resume_session",
            "no_resume_session",
            "about"
        ]
    )]
    pub light_draw_off: bool,

    /// Start active (show overlay immediately, one-shot mode)
    #[arg(long, short = 'a', action = ArgAction::SetTrue)]
    pub active: bool,

    /// Initial board id (transparent, whiteboard, blackboard, or a custom id)
    #[arg(long, short = 'm', value_name = "MODE")]
    pub mode: Option<String>,

    /// Disable system tray (run daemon without a tray icon)
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_tray: bool,

    /// Start daemon activations with frozen mode active (same compositor support as --freeze)
    #[arg(
        long,
        action = ArgAction::SetTrue,
        requires = "daemon",
        conflicts_with_all = ["active", "freeze", "clear_session", "session_info"]
    )]
    pub freeze_on_show: bool,

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
            "daemon_toggle",
            "daemon_action",
            "active",
            "mode",
            "no_tray",
            "freeze_on_show",
            "clear_session",
            "session_info",
            "freeze",
            "resume_session",
            "no_resume_session"
        ]
    )]
    pub about: bool,
}

impl Cli {
    pub(crate) fn daemon_overlay_action(&self) -> Result<Option<TrayAction>, String> {
        if let Some(action) = self.daemon_action.as_deref() {
            return TrayAction::parse(action)
                .ok_or_else(|| format!("unknown daemon action '{action}'"))
                .map(Some);
        }

        let action = if self.light_toggle {
            Some(TrayAction::ToggleLightMode)
        } else if self.light_draw_toggle {
            Some(TrayAction::LightDrawToggle)
        } else if self.light_draw_on {
            Some(TrayAction::LightDrawOn)
        } else if self.light_draw_off {
            Some(TrayAction::LightDrawOff)
        } else {
            None
        };
        Ok(action)
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use crate::tray_action::TrayAction;
    use clap::Parser;

    #[test]
    fn active_mode_with_explicit_board_id() {
        let cli = Cli::try_parse_from(["wayscriber", "--active", "--mode", "whiteboard"]).unwrap();
        assert!(cli.active);
        assert_eq!(cli.mode.as_deref(), Some("whiteboard"));
    }

    #[test]
    fn daemon_mode_accepts_freeze_on_show() {
        let cli = Cli::try_parse_from(["wayscriber", "--daemon", "--freeze-on-show"]).unwrap();
        assert!(cli.daemon);
        assert!(cli.freeze_on_show);
    }

    #[test]
    fn daemon_toggle_accepts_overlay_launch_args() {
        let cli = Cli::try_parse_from([
            "wayscriber",
            "--daemon-toggle",
            "--freeze",
            "--mode",
            "whiteboard",
            "--exit-after-capture",
            "--resume-session",
        ])
        .unwrap();
        assert!(cli.daemon_toggle);
        assert!(cli.freeze);
        assert_eq!(cli.mode.as_deref(), Some("whiteboard"));
        assert!(cli.exit_after_capture);
        assert!(cli.resume_session);
    }

    #[test]
    fn daemon_action_accepts_light_mode_actions() {
        let cli =
            Cli::try_parse_from(["wayscriber", "--daemon-action", "light_draw_toggle"]).unwrap();
        assert_eq!(cli.daemon_action.as_deref(), Some("light_draw_toggle"));
        assert_eq!(
            cli.daemon_overlay_action().unwrap(),
            Some(TrayAction::LightDrawToggle)
        );
    }

    #[test]
    fn friendly_light_aliases_resolve_to_tray_actions() {
        let cases = [
            ("--light-toggle", TrayAction::ToggleLightMode),
            ("--light-draw-toggle", TrayAction::LightDrawToggle),
            ("--light-draw-on", TrayAction::LightDrawOn),
            ("--light-draw-off", TrayAction::LightDrawOff),
        ];

        for (flag, expected) in cases {
            let cli = Cli::try_parse_from(["wayscriber", flag]).unwrap();
            assert_eq!(cli.daemon_overlay_action().unwrap(), Some(expected));
        }
    }

    #[test]
    fn friendly_light_aliases_conflict_with_raw_daemon_action() {
        let result = Cli::try_parse_from([
            "wayscriber",
            "--daemon-action",
            "toggle_light_mode",
            "--light-toggle",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn friendly_light_aliases_conflict_with_each_other() {
        let result = Cli::try_parse_from(["wayscriber", "--light-toggle", "--light-draw-toggle"]);
        assert!(result.is_err());
    }

    #[test]
    fn raw_daemon_action_reports_unknown_action() {
        let cli = Cli::try_parse_from(["wayscriber", "--daemon-action", "not_real"]).unwrap();
        assert_eq!(
            cli.daemon_overlay_action().unwrap_err(),
            "unknown daemon action 'not_real'"
        );
    }

    #[test]
    fn cli_conflicting_flags_fail() {
        let result = Cli::try_parse_from(["wayscriber", "--active", "--clear-session"]);
        assert!(
            result.is_err(),
            "expected conflicting flags (--active and --clear-session) to error"
        );
    }

    #[test]
    fn freeze_on_show_requires_daemon() {
        let result = Cli::try_parse_from(["wayscriber", "--freeze-on-show"]);
        assert!(
            result.is_err(),
            "expected --freeze-on-show without --daemon to error"
        );
    }
}
