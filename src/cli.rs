use std::ffi::OsString;

use crate::tray_action::TrayAction;

#[derive(Debug, Default)]
pub struct Cli {
    /// Run as daemon (background service; bind a toggle like Super+D)
    pub daemon: bool,

    /// Send a toggle request to the running daemon (supports --mode/--freeze/exit/session overrides)
    pub daemon_toggle: bool,

    /// Send an action to the active overlay through the running daemon
    pub daemon_action: Option<String>,

    /// Toggle light passthrough mode through the running daemon
    pub light_toggle: bool,

    /// Toggle drawing while light passthrough mode is active
    pub light_draw_toggle: bool,

    /// Turn light-mode drawing on through the running daemon
    pub light_draw_on: bool,

    /// Turn light-mode drawing off through the running daemon
    pub light_draw_off: bool,

    /// Start active (show overlay immediately, one-shot mode)
    pub active: bool,

    /// Initial board id (transparent, whiteboard, blackboard, or a custom id)
    pub mode: Option<String>,

    /// Disable system tray (run daemon without a tray icon)
    pub no_tray: bool,

    /// Start daemon activations with frozen mode active (same compositor support as --freeze)
    pub freeze_on_show: bool,

    /// Delete persisted session data and backups
    pub clear_session: bool,

    /// Show session persistence status and file paths
    pub session_info: bool,

    /// Start with frozen mode active (freeze the screen immediately)
    pub freeze: bool,

    /// Exit the overlay after a capture completes (overrides auto clipboard exit)
    pub exit_after_capture: bool,

    /// Keep the overlay open after capture (disables auto clipboard exit)
    pub no_exit_after_capture: bool,

    /// Force session resume on (persist/restore all boards + history/tool state)
    pub resume_session: bool,

    /// Force session resume off (ignore persisted session data for this run)
    pub no_resume_session: bool,

    /// Show the About window
    pub about: bool,

    /// Print compiled runtime capabilities for companion tools
    pub runtime_capabilities: bool,
}

impl Cli {
    pub(crate) fn parse() -> Self {
        match Self::try_parse_from(std::env::args_os()) {
            Ok(cli) => cli,
            Err(err) => {
                eprintln!("{err}");
                eprintln!("Try 'wayscriber --help' for usage.");
                std::process::exit(2);
            }
        }
    }

    pub(crate) fn try_parse_from<I, T>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let mut args = args
            .into_iter()
            .map(|arg| {
                arg.into()
                    .into_string()
                    .map_err(|_| "arguments must be valid UTF-8".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;

        if args.is_empty() {
            args.push("wayscriber".to_string());
        }

        let mut cli = Cli::default();
        let mut index = 1;
        while index < args.len() {
            let arg = &args[index];
            match arg.as_str() {
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                "-V" | "--version" => {
                    print_version();
                    std::process::exit(0);
                }
                "-d" | "--daemon" => cli.daemon = true,
                "--daemon-toggle" => cli.daemon_toggle = true,
                "--daemon-action" => {
                    index += 1;
                    cli.daemon_action = Some(value_after(&args, index, "--daemon-action")?);
                }
                "--light-toggle" => cli.light_toggle = true,
                "--light-draw-toggle" => cli.light_draw_toggle = true,
                "--light-draw-on" => cli.light_draw_on = true,
                "--light-draw-off" => cli.light_draw_off = true,
                "-a" | "--active" => cli.active = true,
                "-m" | "--mode" => {
                    index += 1;
                    cli.mode = Some(value_after(&args, index, "--mode")?);
                }
                "--no-tray" => cli.no_tray = true,
                "--freeze-on-show" => cli.freeze_on_show = true,
                "--clear-session" => cli.clear_session = true,
                "--session-info" => cli.session_info = true,
                "--freeze" => cli.freeze = true,
                "--exit-after-capture" => cli.exit_after_capture = true,
                "--no-exit-after-capture" => cli.no_exit_after_capture = true,
                "--resume-session" => cli.resume_session = true,
                "--no-resume-session" => cli.no_resume_session = true,
                "--about" => cli.about = true,
                wayscriber::runtime_capabilities::RUNTIME_CAPABILITIES_FLAG => {
                    cli.runtime_capabilities = true;
                }
                _ if arg.starts_with("--daemon-action=") => {
                    cli.daemon_action = Some(value_from_equals(arg, "--daemon-action")?);
                }
                _ if arg.starts_with("--mode=") => {
                    cli.mode = Some(value_from_equals(arg, "--mode")?);
                }
                _ if is_short_option_cluster(arg) => {
                    index = parse_short_option_cluster(&args, index, &mut cli)?;
                }
                _ => return Err(format!("unknown argument '{arg}'")),
            }
            index += 1;
        }

        cli.validate()?;
        Ok(cli)
    }

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

    fn validate(&self) -> Result<(), String> {
        if self.runtime_capabilities
            && (self.daemon
                || self.daemon_toggle
                || self.daemon_action.is_some()
                || self.light_toggle
                || self.light_draw_toggle
                || self.light_draw_on
                || self.light_draw_off
                || self.active
                || self.mode.is_some()
                || self.no_tray
                || self.freeze_on_show
                || self.clear_session
                || self.session_info
                || self.freeze
                || self.exit_after_capture
                || self.no_exit_after_capture
                || self.resume_session
                || self.no_resume_session
                || self.about)
        {
            return Err("--runtime-capabilities conflicts with launch flags".to_string());
        }

        if self.exit_after_capture && self.no_exit_after_capture {
            return Err(conflict("--exit-after-capture", "--no-exit-after-capture"));
        }
        if self.resume_session && self.no_resume_session {
            return Err(conflict("--resume-session", "--no-resume-session"));
        }
        if self.clear_session && self.session_info {
            return Err(conflict("--clear-session", "--session-info"));
        }

        if self.freeze_on_show && !self.daemon {
            return Err("--freeze-on-show requires --daemon".to_string());
        }
        if self.freeze_on_show
            && (self.active || self.freeze || self.clear_session || self.session_info)
        {
            return Err("--freeze-on-show conflicts with overlay/session commands".to_string());
        }

        if self.daemon_toggle
            && (self.daemon
                || self.active
                || self.no_tray
                || self.freeze_on_show
                || self.clear_session
                || self.session_info
                || self.about)
        {
            return Err("--daemon-toggle conflicts with the selected command".to_string());
        }

        let overlay_action_count = [
            self.daemon_action.is_some(),
            self.light_toggle,
            self.light_draw_toggle,
            self.light_draw_on,
            self.light_draw_off,
        ]
        .into_iter()
        .filter(|selected| *selected)
        .count();
        if overlay_action_count > 1 {
            return Err("daemon overlay actions conflict with each other".to_string());
        }
        if overlay_action_count == 1
            && (self.daemon
                || self.daemon_toggle
                || self.active
                || self.mode.is_some()
                || self.no_tray
                || self.freeze_on_show
                || self.clear_session
                || self.session_info
                || self.freeze
                || self.exit_after_capture
                || self.no_exit_after_capture
                || self.resume_session
                || self.no_resume_session
                || self.about)
        {
            return Err("daemon overlay actions cannot be combined with launch flags".to_string());
        }

        if self.clear_session && (self.daemon || self.active) {
            return Err("--clear-session conflicts with --daemon/--active".to_string());
        }
        if self.session_info && (self.daemon || self.active) {
            return Err("--session-info conflicts with --daemon/--active".to_string());
        }
        if self.freeze && (self.daemon || self.clear_session || self.session_info) {
            return Err("--freeze conflicts with the selected command".to_string());
        }
        if self.about
            && (self.daemon
                || self.daemon_toggle
                || self.daemon_action.is_some()
                || self.active
                || self.mode.is_some()
                || self.no_tray
                || self.freeze_on_show
                || self.clear_session
                || self.session_info
                || self.freeze
                || self.resume_session
                || self.no_resume_session)
        {
            return Err("--about conflicts with the selected command".to_string());
        }

        Ok(())
    }
}

fn is_short_option_cluster(arg: &str) -> bool {
    arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2
}

fn parse_short_option_cluster(
    args: &[String],
    index: usize,
    cli: &mut Cli,
) -> Result<usize, String> {
    let arg = &args[index];
    for (offset, flag) in arg[1..].char_indices() {
        match flag {
            'h' => {
                print_help();
                std::process::exit(0);
            }
            'V' => {
                print_version();
                std::process::exit(0);
            }
            'd' => cli.daemon = true,
            'a' => cli.active = true,
            'm' => {
                let value_start = 1 + offset + flag.len_utf8();
                let value = if value_start < arg.len() {
                    attached_short_value(arg, value_start, "-m")?
                } else {
                    value_after(args, index + 1, "-m")?
                };
                cli.mode = Some(value);
                return Ok(if value_start < arg.len() {
                    index
                } else {
                    index + 1
                });
            }
            _ => return Err(format!("unknown short option '-{flag}'")),
        }
    }
    Ok(index)
}

fn value_after(args: &[String], index: usize, name: &str) -> Result<String, String> {
    let value = args
        .get(index)
        .ok_or_else(|| format!("{name} requires a value"))?;
    if value.starts_with('-') {
        return Err(format!("{name} requires a value"));
    }
    Ok(value.clone())
}

fn attached_short_value(arg: &str, value_start: usize, name: &str) -> Result<String, String> {
    let value = arg[value_start..]
        .strip_prefix('=')
        .unwrap_or(&arg[value_start..]);
    if value.is_empty() {
        return Err(format!("{name} requires a value"));
    }
    Ok(value.to_string())
}

fn value_from_equals(arg: &str, name: &str) -> Result<String, String> {
    let value = arg
        .split_once('=')
        .map(|(_, value)| value)
        .unwrap_or_default();
    if value.is_empty() {
        return Err(format!("{name} requires a value"));
    }
    Ok(value.to_string())
}

fn conflict(left: &str, right: &str) -> String {
    format!("{left} conflicts with {right}")
}

fn print_help() {
    println!("wayscriber: Screen annotation tool for Wayland compositors");
    println!();
    println!("Usage:");
    println!("  wayscriber -d, --daemon");
    println!("  wayscriber --daemon --freeze-on-show");
    println!("  wayscriber --daemon-toggle [--freeze] [--mode MODE]");
    println!("  wayscriber --daemon-action ACTION");
    println!(
        "  wayscriber --light-toggle | --light-draw-toggle | --light-draw-on | --light-draw-off"
    );
    println!("  wayscriber -a, --active [--mode MODE]");
    println!("  wayscriber --freeze");
    println!("  wayscriber --clear-session | --session-info");
    println!("  wayscriber --about");
    println!();
    println!("Options:");
    println!("  -d, --daemon                  Run as background daemon");
    println!("      --daemon-toggle           Toggle the running daemon");
    println!("      --daemon-action ACTION    Send an action to the active overlay");
    println!("      --light-toggle            Toggle light passthrough mode");
    println!("      --light-draw-toggle       Toggle drawing in light passthrough mode");
    println!("      --light-draw-on           Turn light-mode drawing on");
    println!("      --light-draw-off          Turn light-mode drawing off");
    println!("  -a, --active                  Show overlay immediately");
    println!("  -m, --mode MODE               Initial board id");
    println!("      --no-tray                 Skip system tray");
    println!("      --freeze-on-show          Start daemon activations frozen");
    println!("      --freeze                  Start overlay already frozen");
    println!("      --exit-after-capture      Exit after a capture completes");
    println!("      --no-exit-after-capture   Keep overlay open after capture");
    println!("      --resume-session          Force session resume on");
    println!("      --no-resume-session       Force session resume off");
    println!("      --clear-session           Delete persisted session data and backups");
    println!("      --session-info            Show session persistence status");
    println!("      --about                   Show the About window");
    println!("  -h, --help                    Show help");
    println!("  -V, --version                 Show version");
}

fn print_version() {
    println!("wayscriber {}", crate::build_info::version());
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use crate::tray_action::TrayAction;

    #[test]
    fn active_mode_with_explicit_board_id() {
        let cli = Cli::try_parse_from(["wayscriber", "--active", "--mode", "whiteboard"]).unwrap();
        assert!(cli.active);
        assert_eq!(cli.mode.as_deref(), Some("whiteboard"));
    }

    #[test]
    fn clap_compatible_short_mode_forms_are_supported() {
        let cli = Cli::try_parse_from(["wayscriber", "-mwhiteboard"]).unwrap();
        assert_eq!(cli.mode.as_deref(), Some("whiteboard"));

        let cli = Cli::try_parse_from(["wayscriber", "-m=whiteboard"]).unwrap();
        assert_eq!(cli.mode.as_deref(), Some("whiteboard"));

        let cli = Cli::try_parse_from(["wayscriber", "-dm=whiteboard"]).unwrap();
        assert!(cli.daemon);
        assert_eq!(cli.mode.as_deref(), Some("whiteboard"));

        let cli = Cli::try_parse_from(["wayscriber", "-am", "whiteboard"]).unwrap();
        assert!(cli.active);
        assert_eq!(cli.mode.as_deref(), Some("whiteboard"));

        let cli = Cli::try_parse_from(["wayscriber", "-amwhiteboard"]).unwrap();
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
