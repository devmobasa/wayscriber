use std::ffi::OsString;
use std::path::PathBuf;

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

    /// Use a named session file for active/freeze/info/clear operations
    pub session_file: Option<PathBuf>,

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
                "--session-file" => {
                    index += 1;
                    cli.session_file =
                        Some(PathBuf::from(value_after(&args, index, "--session-file")?));
                }
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
                _ if arg.starts_with("--session-file=") => {
                    cli.session_file =
                        Some(PathBuf::from(value_from_equals(arg, "--session-file")?));
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
                || self.session_file.is_some()
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

        if self.session_file.is_some() {
            if self.daemon || self.daemon_toggle || overlay_action_count > 0 {
                return Err(
                    "--session-file is not supported with daemon/toggle mode yet; use --active or --freeze for now."
                        .to_string(),
                );
            }
            if !(self.active || self.freeze || self.clear_session || self.session_info) {
                return Err(
                    "--session-file requires --active, --freeze, --session-info, or --clear-session"
                        .to_string(),
                );
            }
            if (self.active || self.freeze) && self.no_resume_session {
                return Err(
                    "--session-file conflicts with --no-resume-session because --session-file requires session persistence for this run"
                        .to_string(),
                );
            }
        }

        if self.daemon_toggle
            && (self.daemon
                || self.active
                || self.no_tray
                || self.freeze_on_show
                || self.clear_session
                || self.session_info
                || self.session_file.is_some()
                || self.about)
        {
            return Err("--daemon-toggle conflicts with the selected command".to_string());
        }

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
                || self.session_file.is_some()
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
        if (self.clear_session || self.session_info) && self.resume_session {
            return Err(
                "--resume-session conflicts with --clear-session/--session-info".to_string(),
            );
        }
        if (self.clear_session || self.session_info) && self.no_resume_session {
            return Err(
                "--no-resume-session conflicts with --clear-session/--session-info".to_string(),
            );
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
                || self.session_file.is_some()
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
    println!("  wayscriber --active --session-file PATH");
    println!("  wayscriber --freeze [--session-file PATH]");
    println!("  wayscriber --session-info [--session-file PATH]");
    println!("  wayscriber --clear-session [--session-file PATH]");
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
    println!("      --session-file PATH       Use a named session file");
    println!("      --about                   Show the About window");
    println!("  -h, --help                    Show help");
    println!("  -V, --version                 Show version");
}

fn print_version() {
    println!("wayscriber {}", crate::build_info::version());
}

#[cfg(test)]
mod tests;
