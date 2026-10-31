use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use lexopt::{Arg, Parser};

use crate::domain::Language;
use crate::tray_action::TrayAction;
use crate::{build_info, i18n, runtime_capabilities};

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

    /// Clear saved tool defaults while preserving session drawings
    pub clear_tool_state: bool,

    /// Show session persistence status and file paths
    pub session_info: bool,

    /// Rename a named session's catalog display name (session files are untouched)
    pub rename_session: Option<String>,

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

    /// Check wayscriber.com for a newer release, print the result, and exit
    pub check_update: bool,

    /// Print compiled runtime capabilities for companion tools
    pub runtime_capabilities: bool,

    /// Interface language for this run, beating `[ui] language` and
    /// `WAYSCRIBER_LANGUAGE` (the config file is not changed)
    pub language: Option<Language>,

    /// List the interface languages and the one in effect, then exit
    pub list_languages: bool,
}

#[derive(Debug)]
pub(crate) enum CliOutcome {
    Run(Cli),
    Help,
    Version,
}

impl Cli {
    pub(crate) fn parse() -> Result<CliOutcome, String> {
        Self::try_parse_from(env::args_os())
    }

    pub(crate) fn try_parse_from<I, T>(args: I) -> Result<CliOutcome, String>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let mut parser = Parser::from_iter(args.into_iter().map(Into::into));
        let mut cli = Cli::default();

        while let Some(arg) = parser.next().map_err(|error| parse_error(&error))? {
            match arg {
                Arg::Short('h') => return Ok(CliOutcome::Help),
                Arg::Short('V') => return Ok(CliOutcome::Version),
                Arg::Long("help") => {
                    reject_attached_value(&mut parser, "--help")?;
                    return Ok(CliOutcome::Help);
                }
                Arg::Long("version") => {
                    reject_attached_value(&mut parser, "--version")?;
                    return Ok(CliOutcome::Version);
                }
                Arg::Short('d') | Arg::Long("daemon") => cli.daemon = true,
                Arg::Long("daemon-toggle") => cli.daemon_toggle = true,
                Arg::Long("daemon-action") => {
                    cli.daemon_action = Some(string_value(&mut parser, "--daemon-action")?);
                }
                Arg::Long("light-toggle") => cli.light_toggle = true,
                Arg::Long("light-draw-toggle") => cli.light_draw_toggle = true,
                Arg::Long("light-draw-on") => cli.light_draw_on = true,
                Arg::Long("light-draw-off") => cli.light_draw_off = true,
                Arg::Short('a') | Arg::Long("active") => cli.active = true,
                Arg::Short('m') => cli.mode = Some(string_value(&mut parser, "-m")?),
                Arg::Long("mode") => cli.mode = Some(string_value(&mut parser, "--mode")?),
                Arg::Long("no-tray") => cli.no_tray = true,
                Arg::Long("freeze-on-show") => cli.freeze_on_show = true,
                Arg::Long("clear-session") => cli.clear_session = true,
                Arg::Long("clear-tool-state") => cli.clear_tool_state = true,
                Arg::Long("session-info") => cli.session_info = true,
                Arg::Long("rename-session") => {
                    cli.rename_session = Some(string_value(&mut parser, "--rename-session")?);
                }
                Arg::Long("session-file") => {
                    cli.session_file =
                        Some(PathBuf::from(option_value(&mut parser, "--session-file")?));
                }
                Arg::Long("freeze") => cli.freeze = true,
                Arg::Long("exit-after-capture") => cli.exit_after_capture = true,
                Arg::Long("no-exit-after-capture") => cli.no_exit_after_capture = true,
                Arg::Long("resume-session") => cli.resume_session = true,
                Arg::Long("no-resume-session") => cli.no_resume_session = true,
                Arg::Long("about") => cli.about = true,
                Arg::Long("check-update") => cli.check_update = true,
                Arg::Long("language") => {
                    cli.language = Some(parse_language(&string_value(&mut parser, "--language")?)?);
                }
                Arg::Long("list-languages") => cli.list_languages = true,
                Arg::Long(name)
                    if runtime_capabilities::RUNTIME_CAPABILITIES_FLAG.strip_prefix("--")
                        == Some(name) =>
                {
                    cli.runtime_capabilities = true;
                }
                Arg::Long(name) => {
                    let option = format!("--{name}");
                    reject_attached_value(&mut parser, &option)?;
                    return Err(unknown_argument(&option));
                }
                Arg::Short(flag) => return Err(format!("unknown short option '-{flag}'")),
                Arg::Value(value) => return Err(unknown_argument(&value.display().to_string())),
            }
        }

        cli.validate()?;
        Ok(CliOutcome::Run(cli))
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

    /// Whether any flag that launches, controls, or mutates something is set.
    ///
    /// The three print-and-exit commands (`--runtime-capabilities`, `--about`,
    /// `--check-update`) all conflict with every one of these, so the list lives
    /// in one place: three hand-maintained copies drifted apart once already.
    fn selects_a_launch_command(&self) -> bool {
        self.daemon
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
            || self.clear_tool_state
            || self.session_info
            || self.rename_session.is_some()
            || self.session_file.is_some()
            || self.freeze
            || self.exit_after_capture
            || self.no_exit_after_capture
            || self.resume_session
            || self.no_resume_session
    }

    /// Whether an option belongs to an overlay launch or daemon interaction.
    ///
    /// Catalog-only commands must reject these options because they return
    /// before any overlay or daemon behavior can honor them.
    fn selects_overlay_option(&self) -> bool {
        self.daemon
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
            || self.freeze
            || self.exit_after_capture
            || self.no_exit_after_capture
            || self.resume_session
            || self.no_resume_session
    }

    fn validate(&self) -> Result<(), String> {
        self.validate_runtime_capabilities_command()?;
        self.validate_session_flag_pairs()?;
        self.validate_overlay_commands()?;
        self.validate_session_commands()?;
        self.validate_catalog_commands()?;
        self.validate_language_options()
    }

    fn validate_runtime_capabilities_command(&self) -> Result<(), String> {
        if self.runtime_capabilities
            && (self.selects_a_launch_command()
                || self.about
                || self.check_update
                || self.list_languages
                || self.language.is_some())
        {
            return Err("--runtime-capabilities conflicts with launch flags".to_string());
        }
        Ok(())
    }

    fn validate_session_flag_pairs(&self) -> Result<(), String> {
        if self.exit_after_capture && self.no_exit_after_capture {
            return Err(conflict("--exit-after-capture", "--no-exit-after-capture"));
        }
        if self.resume_session && self.no_resume_session {
            return Err(conflict("--resume-session", "--no-resume-session"));
        }
        if self.clear_session && self.session_info {
            return Err(conflict("--clear-session", "--session-info"));
        }
        if self.clear_tool_state && self.clear_session {
            return Err(conflict("--clear-tool-state", "--clear-session"));
        }
        if self.clear_tool_state && self.session_info {
            return Err(conflict("--clear-tool-state", "--session-info"));
        }
        if self.rename_session.is_some() && self.session_info {
            return Err(conflict("--rename-session", "--session-info"));
        }
        if self.rename_session.is_some() && self.clear_session {
            return Err(conflict("--rename-session", "--clear-session"));
        }
        if self.rename_session.is_some() && self.clear_tool_state {
            return Err(conflict("--rename-session", "--clear-tool-state"));
        }

        if self.rename_session.is_some() && self.session_file.is_none() {
            return Err("--rename-session requires --session-file".to_string());
        }
        if self.rename_session.is_some() && self.selects_overlay_option() {
            return Err("--rename-session conflicts with overlay/daemon options".to_string());
        }
        Ok(())
    }

    fn validate_overlay_commands(&self) -> Result<(), String> {
        if self.freeze_on_show && !self.daemon {
            return Err("--freeze-on-show requires --daemon".to_string());
        }
        if self.freeze_on_show
            && (self.active
                || self.freeze
                || self.clear_session
                || self.clear_tool_state
                || self.session_info)
        {
            return Err("--freeze-on-show conflicts with overlay/session commands".to_string());
        }

        let overlay_action_count = self.overlay_action_count();

        if self.session_file.is_some() {
            if overlay_action_count > 0 {
                return Err(
                    "--session-file cannot be combined with daemon overlay actions; use --daemon-toggle --session-file to launch a named session"
                        .to_string(),
                );
            }
            if !(self.active
                || self.freeze
                || self.daemon
                || self.daemon_toggle
                || self.clear_session
                || self.clear_tool_state
                || self.session_info
                || self.rename_session.is_some())
            {
                return Err(
                    "--session-file requires --active, --freeze, --daemon, --daemon-toggle, --session-info, --clear-session, --clear-tool-state, or --rename-session"
                        .to_string(),
                );
            }
            if (self.active || self.freeze || self.daemon || self.daemon_toggle)
                && self.no_resume_session
            {
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
                || self.clear_tool_state
                || self.session_info
                || self.rename_session.is_some()
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
                || self.clear_tool_state
                || self.session_info
                || self.rename_session.is_some()
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
        Ok(())
    }

    fn overlay_action_count(&self) -> usize {
        [
            self.daemon_action.is_some(),
            self.light_toggle,
            self.light_draw_toggle,
            self.light_draw_on,
            self.light_draw_off,
        ]
        .into_iter()
        .filter(|selected| *selected)
        .count()
    }

    fn validate_session_commands(&self) -> Result<(), String> {
        if self.clear_session && (self.daemon || self.active) {
            return Err("--clear-session conflicts with --daemon/--active".to_string());
        }
        if self.clear_tool_state && (self.daemon || self.active) {
            return Err("--clear-tool-state conflicts with --daemon/--active".to_string());
        }
        if self.session_info && (self.daemon || self.active) {
            return Err("--session-info conflicts with --daemon/--active".to_string());
        }
        if self.freeze
            && (self.daemon
                || self.clear_session
                || self.clear_tool_state
                || self.session_info
                || self.rename_session.is_some())
        {
            return Err("--freeze conflicts with the selected command".to_string());
        }
        if (self.clear_session
            || self.clear_tool_state
            || self.session_info
            || self.rename_session.is_some())
            && self.resume_session
        {
            return Err(
                "--resume-session conflicts with --clear-session/--session-info/--clear-tool-state/--rename-session"
                    .to_string(),
            );
        }
        if (self.clear_session
            || self.clear_tool_state
            || self.session_info
            || self.rename_session.is_some())
            && self.no_resume_session
        {
            return Err(
                "--no-resume-session conflicts with --clear-session/--session-info/--clear-tool-state/--rename-session"
                    .to_string(),
            );
        }
        Ok(())
    }

    fn validate_catalog_commands(&self) -> Result<(), String> {
        if self.about && (self.selects_a_launch_command() || self.check_update) {
            return Err("--about conflicts with the selected command".to_string());
        }

        if self.check_update && self.selects_a_launch_command() {
            return Err("--check-update conflicts with the selected command".to_string());
        }

        if self.list_languages
            && (self.selects_a_launch_command() || self.about || self.check_update)
        {
            return Err("--list-languages conflicts with the selected command".to_string());
        }

        Ok(())
    }

    /// `--language` sets the language of the process it starts, and of the
    /// overlays and About windows that process starts in turn. A command that
    /// hands its work to a running daemon, or only prints English text, has no
    /// window for it to change.
    fn validate_language_options(&self) -> Result<(), String> {
        if self.language.is_none() {
            return Ok(());
        }
        if self.daemon_toggle || self.overlay_action_count() > 0 {
            return Err(
                "--language cannot change a running daemon's overlays; start the daemon with --language, or set [ui] language"
                    .to_string(),
            );
        }
        if !(self.active || self.freeze || self.daemon || self.about || self.list_languages) {
            return Err(
                "--language applies to --active, --freeze, --daemon, --about and --list-languages"
                    .to_string(),
            );
        }
        Ok(())
    }
}

/// Parses a `--language` value: any spelling `[ui] language` accepts.
fn parse_language(value: &str) -> Result<Language, String> {
    i18n::language_from_code(value).ok_or_else(|| {
        format!("unknown language '{value}'; run 'wayscriber --list-languages' to see the choices")
    })
}

/// Reads an option's value, either attached (`--mode=x`, `-mx`, `-m=x`) or as
/// the next argument. A next argument starting with `-` is the next option, so
/// a value that starts with `-` has to be attached.
fn option_value(parser: &mut Parser, name: &str) -> Result<OsString, String> {
    let requires_value = || format!("{name} requires a value");

    if let Some(value) = parser.optional_value() {
        return if value.is_empty() {
            Err(requires_value())
        } else {
            Ok(value)
        };
    }

    let value = parser.value().map_err(|_| requires_value())?;
    if value.as_encoded_bytes().starts_with(b"-") {
        return Err(requires_value());
    }
    Ok(value)
}

fn string_value(parser: &mut Parser, name: &str) -> Result<String, String> {
    option_value(parser, name)?
        .into_string()
        .map_err(|_| "arguments must be valid UTF-8".to_string())
}

/// Rejects `--flag=value` for an option that is about to stop parsing, which
/// would otherwise skip the check `Parser::next` makes on its following call.
fn reject_attached_value(parser: &mut Parser, name: &str) -> Result<(), String> {
    match parser.optional_value() {
        Some(value) => Err(unknown_argument(&format!("{name}={}", value.display()))),
        None => Ok(()),
    }
}

fn parse_error(error: &lexopt::Error) -> String {
    match error {
        lexopt::Error::UnexpectedValue { option, value } => {
            unknown_argument(&format!("{option}={}", value.display()))
        }
        other => other.to_string(),
    }
}

fn unknown_argument(arg: &str) -> String {
    format!("unknown argument '{arg}'")
}

fn conflict(left: &str, right: &str) -> String {
    format!("{left} conflicts with {right}")
}

pub(crate) fn print_help() {
    println!("wayscriber: Screen annotation tool for Wayland compositors");
    println!();
    println!("Usage:");
    println!("  wayscriber -d, --daemon [--session-file PATH]");
    println!("  wayscriber --daemon --freeze-on-show");
    println!("  wayscriber --daemon-toggle [--freeze] [--mode MODE] [--session-file PATH]");
    println!("  wayscriber --daemon-action ACTION");
    println!(
        "  wayscriber --light-toggle | --light-draw-toggle | --light-draw-on | --light-draw-off"
    );
    println!("  wayscriber -a, --active [--mode MODE]");
    println!("  wayscriber --active --session-file PATH");
    println!("  wayscriber --freeze [--session-file PATH]");
    println!("  wayscriber --session-info [--session-file PATH]");
    println!("  wayscriber --rename-session NAME --session-file PATH");
    println!("  wayscriber --clear-session [--session-file PATH]");
    println!("  wayscriber --clear-tool-state [--session-file PATH]");
    println!("  wayscriber --about");
    println!("  wayscriber --check-update");
    println!("  wayscriber --list-languages");
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
    println!("      --clear-tool-state        Remove saved tool defaults but keep boards");
    println!("      --session-info            Show session persistence status");
    println!("      --rename-session NAME     Rename a named session catalog label");
    println!("      --session-file PATH       Use a named session file");
    println!("      --about                   Show the About window");
    println!("      --check-update            Check wayscriber.com for a newer release");
    println!("      --language CODE           Interface language for this run (auto, en, zh-CN)");
    println!("      --list-languages          List interface languages and the one in effect");
    println!("  -h, --help                    Show help");
    println!("  -V, --version                 Show version");
}

pub(crate) fn print_version() {
    println!("wayscriber {}", build_info::version());
}

#[cfg(test)]
mod tests;
