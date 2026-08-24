use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use usage_rs as usage;

use crate::tray_action::TrayAction;

mod compat;
mod completion;

use compat::{Scan, scan_tokens};
use completion::{completion_answer, completion_script};

// Command-line surface.
//
// The flag surface (long forms, short forms, value placeholders) is declared
// here and nowhere else: `usage` derives the parser, the spec, and the shell
// completions from these attributes, so adding a flag is a one-line change
// rather than an edit to a parser, a completion script, and a spec by hand.
//
// Semantic rules stay in `Cli::validate`. They are deliberately *not* declared
// as `conflicts`/`requires` attributes: wayscriber's rejections are group-worded
// and pedagogical ("use `--daemon-toggle --session-file` to launch a named
// session"), and a declarative conflict can only say that two named flags
// disagree. Mapping the generic pairwise errors back onto these messages would
// need a `(flag, flag) -> message` table, which is exactly the hand-maintained
// duplication this module is trying to get rid of.
//
// These are ordinary comments, not doc comments: a doc comment here becomes the
// command's `about` text in the spec and in shell completions.
#[derive(Debug, Default, usage::Cli)]
#[usage(
    bin = "wayscriber",
    about = "Screen annotation tool for Wayland compositors",
    // Not bare `version`: that bakes in `CARGO_PKG_VERSION`, which is the wrong
    // number for a packaging-only hotfix build, and would leave `__usage_spec__`
    // disagreeing with `--version`.
    version = crate::build_info::VERSION,
    // Emits the hidden `__complete_word__` command that the generated shell
    // scripts call back into. Declaring it here is what makes `completion_script`
    // available at all, so a script naming a command this binary does not answer
    // is a compile error rather than silence at the prompt.
    completion,
    // wayscriber renders its own help synopsis (`print_help`), so the derive's
    // help output is never shown; it still answers `-h`/`--help` as an event.
    unknown_flags = "error"
)]
pub struct Cli {
    /// Run as background daemon
    #[usage(short = 'd', long)]
    pub daemon: bool,

    /// Toggle the running daemon
    #[usage(long)]
    pub daemon_toggle: bool,

    /// Send an action to the active overlay
    // `choices_strict = false` presents the vocabulary to a shell without making
    // it the accepted set: an unknown word still reaches `TrayAction::parse`, so
    // `--daemon-action not_real` keeps reporting "unknown daemon action
    // 'not_real'" rather than a parse-time list.
    //
    // The spellings have to be literals here, so
    // `the_daemon_action_choices_match_the_dispatch_table` pins them to
    // `TrayAction::ALL`.
    #[usage(
        long,
        value_name = "ACTION",
        choices(
            "toggle_freeze",
            "capture_full",
            "capture_window",
            "capture_region",
            "toggle_help",
            "toggle_board_picker",
            "toggle_light_mode",
            "light_draw_toggle",
            "light_draw_on",
            "light_draw_off"
        ),
        choices_strict = false
    )]
    pub daemon_action: Option<String>,

    /// Toggle light passthrough mode
    #[usage(long)]
    pub light_toggle: bool,

    /// Toggle drawing in light passthrough mode
    #[usage(long)]
    pub light_draw_toggle: bool,

    /// Turn light-mode drawing on
    #[usage(long)]
    pub light_draw_on: bool,

    /// Turn light-mode drawing off
    #[usage(long)]
    pub light_draw_off: bool,

    /// Show overlay immediately
    #[usage(short = 'a', long)]
    pub active: bool,

    /// Initial board id
    // Open-ended: custom board ids come from the user's config, so there is no
    // finite set to offer — but a board id is never a path.
    #[usage(
        short = 'm',
        long,
        value_name = "MODE",
        value_hint = usage::ValueHint::Other
    )]
    pub mode: Option<String>,

    /// Skip system tray
    #[usage(long)]
    pub no_tray: bool,

    /// Start daemon activations frozen
    #[usage(long)]
    pub freeze_on_show: bool,

    /// Start overlay already frozen
    #[usage(long)]
    pub freeze: bool,

    /// Exit after a capture completes
    #[usage(long)]
    pub exit_after_capture: bool,

    /// Keep overlay open after capture
    #[usage(long)]
    pub no_exit_after_capture: bool,

    /// Force session resume on
    #[usage(long)]
    pub resume_session: bool,

    /// Force session resume off
    #[usage(long)]
    pub no_resume_session: bool,

    /// Delete persisted session data and backups
    #[usage(long)]
    pub clear_session: bool,

    /// Remove saved tool defaults but keep boards
    #[usage(long)]
    pub clear_tool_state: bool,

    /// Show session persistence status
    #[usage(long)]
    pub session_info: bool,

    /// Rename a named session catalog label
    // A catalog display name is free text, not a path.
    #[usage(long, value_name = "NAME", value_hint = usage::ValueHint::Other)]
    pub rename_session: Option<String>,

    /// Use a named session file
    #[usage(long, value_name = "PATH", value_hint = usage::ValueHint::FilePath)]
    pub session_file: Option<PathBuf>,

    /// Show the About window
    #[usage(long)]
    pub about: bool,

    /// Check wayscriber.com for a newer release
    #[usage(long)]
    pub check_update: bool,

    /// Print a completion script (bash, zsh, fish, nu, powershell)
    // Finite, so the accepted set is declared once: it validates the value and
    // is what a shell offers on TAB. `nushell` and `pwsh` are accepted because
    // they are what those shells are usually called.
    #[usage(
        long,
        value_name = "SHELL",
        choices("bash", "zsh", "fish", "nu", "nushell", "powershell", "pwsh")
    )]
    pub completions: Option<String>,

    /// Print compiled runtime capabilities for companion tools
    // Hidden: the configurator's IPC probe, not a flag a person types.
    #[usage(long, hide)]
    pub runtime_capabilities: bool,
}

/// Which validation groups a flag belongs to.
///
/// One registry, five groupings. These lists used to be written out by hand in
/// `selects_a_launch_command`, `selects_overlay_option`, and three more times
/// inline in `validate`; the copies drifted apart once already. Adding a flag
/// now means adding one row, and a flag cannot be in a grouping in one place
/// and missing from it in another.
mod group {
    /// Launches, controls, or mutates something.
    pub const LAUNCH: u8 = 1 << 0;
    /// Belongs to an overlay launch or a daemon interaction.
    pub const OVERLAY: u8 = 1 << 1;
    /// One of the mutually exclusive daemon overlay actions.
    pub const ACTION: u8 = 1 << 2;
    /// An offline session-catalog command.
    pub const OFFLINE: u8 = 1 << 3;
    /// A command that prints something and exits. These conflict with every
    /// launch flag and with each other, but each is worded its own way, so the
    /// checks stay separate — and stay where they are in `validate`, because
    /// earlier rules deliberately win (see `check_update_rejects_valued_launch_flags`).
    pub const PRINT: u8 = 1 << 4;
}

struct FlagSpec {
    given: fn(&Cli) -> bool,
    groups: u8,
}

const fn flag(given: fn(&Cli) -> bool, groups: u8) -> FlagSpec {
    FlagSpec { given, groups }
}

use group::{ACTION, LAUNCH, OFFLINE, OVERLAY, PRINT};

const FLAGS: &[FlagSpec] = &[
    flag(|c| c.daemon, LAUNCH | OVERLAY),
    flag(|c| c.daemon_toggle, LAUNCH | OVERLAY),
    flag(|c| c.daemon_action.is_some(), LAUNCH | OVERLAY | ACTION),
    flag(|c| c.light_toggle, LAUNCH | OVERLAY | ACTION),
    flag(|c| c.light_draw_toggle, LAUNCH | OVERLAY | ACTION),
    flag(|c| c.light_draw_on, LAUNCH | OVERLAY | ACTION),
    flag(|c| c.light_draw_off, LAUNCH | OVERLAY | ACTION),
    flag(|c| c.active, LAUNCH | OVERLAY),
    flag(|c| c.mode.is_some(), LAUNCH | OVERLAY),
    flag(|c| c.no_tray, LAUNCH | OVERLAY),
    flag(|c| c.freeze_on_show, LAUNCH | OVERLAY),
    flag(|c| c.clear_session, LAUNCH | OFFLINE),
    flag(|c| c.clear_tool_state, LAUNCH | OFFLINE),
    flag(|c| c.session_info, LAUNCH | OFFLINE),
    flag(|c| c.rename_session.is_some(), LAUNCH | OFFLINE),
    flag(|c| c.session_file.is_some(), LAUNCH),
    flag(|c| c.freeze, LAUNCH | OVERLAY),
    flag(|c| c.exit_after_capture, LAUNCH | OVERLAY),
    flag(|c| c.no_exit_after_capture, LAUNCH | OVERLAY),
    flag(|c| c.resume_session, LAUNCH | OVERLAY),
    flag(|c| c.no_resume_session, LAUNCH | OVERLAY),
    // Print-and-exit commands: deliberately not LAUNCH, so they do not make
    // `selects_a_launch_command` true for one another.
    flag(|c| c.about, PRINT),
    flag(|c| c.check_update, PRINT),
    flag(|c| c.completions.is_some(), PRINT),
    flag(|c| c.runtime_capabilities, PRINT),
];

#[derive(Debug)]
pub(crate) enum CliOutcome {
    Run(Cli),
    Help,
    Version,
    PrintAndExit(String),
}

impl Cli {
    pub(crate) fn from_env() -> Result<CliOutcome, String> {
        Self::from_argv(std::env::args_os())
    }

    pub(crate) fn from_argv<I, T>(args: I) -> Result<CliOutcome, String>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let owned: Vec<OsString> = args.into_iter().map(Into::into).collect();
        // `usage` is happy to bind non-UTF-8 bytes, and the app rejects them
        // several layers later with a different exit code. The hand-rolled
        // parser refused them at the front door, so that stays the contract.
        if owned.iter().any(|arg| arg.to_str().is_none()) {
            return Err("arguments must be valid UTF-8".to_string());
        }
        // The caller passes argv including the program name; `usage` parses the
        // arguments alone.
        let rest = owned.get(1..).unwrap_or_default();
        let argv: Vec<&OsStr> = rest.iter().map(OsString::as_os_str).collect();

        // Both endpoints are recognised ahead of the parse, because neither is a
        // command wayscriber runs: the generated shell scripts call back into
        // `__complete_word__`, and any usage-aware tool may ask a usage binary for
        // its own spec. The derive puts these in its own `parse`, which this
        // wrapper replaces, so they have to be answered here or they are dead.
        if let Some(answer) = completion_answer(rest) {
            return Ok(CliOutcome::PrintAndExit(answer));
        }
        if let Some(answer) = Self::spec_request(&argv) {
            return Ok(CliOutcome::PrintAndExit(answer));
        }

        // `usage` accepts and silently ignores text attached to a switch, so
        // `--clear-session=false` would *set* `--clear-session` and delete the
        // session. The hand-rolled parser had no such spelling and rejected the
        // whole token; that stays the contract, and it is checked before the
        // parse so no destructive flag is ever bound from one.
        // One left-to-right pass over the tokens before `usage` binds anything.
        // It is where the terminals and the rejections come from, because both
        // are position-sensitive in a way a whole-argv check cannot express: the
        // first `-h` wins over a later bad token, and the first bad token wins
        // over a later one.
        match scan_tokens(&argv)? {
            Scan::Help => return Ok(CliOutcome::Help),
            Scan::Version => return Ok(CliOutcome::Version),
            Scan::Run => {}
        }

        let cli = match Self::parse_from(&argv) {
            Ok(cli) => cli,
            Err(usage::Error::Help { .. })
            | Err(usage::Error::HelpAll { .. })
            | Err(usage::Error::MissingArgsHelp { .. }) => return Ok(CliOutcome::Help),
            Err(usage::Error::Version { .. }) => return Ok(CliOutcome::Version),
            Err(err) => return Err(describe_parse_error(&err)),
        };

        if let Some(shell) = cli.completions.as_deref() {
            let script = completion_script(shell)?;
            cli.validate()?;
            return Ok(CliOutcome::PrintAndExit(script));
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

    /// How many flags in `groups` were given.
    fn count_in(&self, groups: u8) -> usize {
        FLAGS
            .iter()
            .filter(|flag| flag.groups & groups != 0 && (flag.given)(self))
            .count()
    }

    /// Whether any flag in `groups` was given.
    fn any_in(&self, groups: u8) -> bool {
        self.count_in(groups) > 0
    }

    /// Whether any flag that launches, controls, or mutates something is set.
    fn selects_a_launch_command(&self) -> bool {
        self.any_in(LAUNCH)
    }

    /// Whether an option belongs to an overlay launch or daemon interaction.
    ///
    /// Catalog-only commands must reject these options because they return
    /// before any overlay or daemon behavior can honor them.
    fn selects_overlay_option(&self) -> bool {
        self.any_in(OVERLAY)
    }

    /// Whether an offline session-catalog command is selected.
    fn selects_offline_session_command(&self) -> bool {
        self.any_in(OFFLINE)
    }

    /// For a print-and-exit command: whether anything else was also selected.
    ///
    /// Each caller has already established that its own flag is set, so more
    /// than one `PRINT` member means a second command was named. The four
    /// callers used to spell out the members that had not been checked yet,
    /// which made the list depend on where the check sat.
    fn conflicts_with_another_command(&self) -> bool {
        self.selects_a_launch_command() || self.count_in(PRINT) > 1
    }

    fn validate(&self) -> Result<(), String> {
        if self.completions.is_some() && self.conflicts_with_another_command() {
            return Err("--completions conflicts with the selected command".to_string());
        }

        if self.runtime_capabilities && self.conflicts_with_another_command() {
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

        if self.freeze_on_show && !self.daemon {
            return Err("--freeze-on-show requires --daemon".to_string());
        }
        // Spelled out rather than driven off `OFFLINE`: this set is the offline
        // commands *except* `--rename-session`, which is not a grouping anything
        // else needs, and expressing it as a subtraction reads worse than the
        // three names.
        if self.freeze_on_show
            && (self.active
                || self.freeze
                || self.clear_session
                || self.clear_tool_state
                || self.session_info)
        {
            return Err("--freeze-on-show conflicts with overlay/session commands".to_string());
        }

        let overlay_action_count = self.count_in(ACTION);

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
        // Every LAUNCH flag that is not itself one of the overlay actions.
        // `ACTION` is a subset of `LAUNCH`, so a larger launch count means
        // something other than the action was given.
        if overlay_action_count == 1 && (self.count_in(LAUNCH) > overlay_action_count || self.about)
        {
            return Err("daemon overlay actions cannot be combined with launch flags".to_string());
        }

        if self.clear_session && (self.daemon || self.active) {
            return Err("--clear-session conflicts with --daemon/--active".to_string());
        }
        if self.clear_tool_state && (self.daemon || self.active) {
            return Err("--clear-tool-state conflicts with --daemon/--active".to_string());
        }
        if self.session_info && (self.daemon || self.active) {
            return Err("--session-info conflicts with --daemon/--active".to_string());
        }
        if self.freeze && (self.daemon || self.selects_offline_session_command()) {
            return Err("--freeze conflicts with the selected command".to_string());
        }
        if self.selects_offline_session_command() && self.resume_session {
            return Err(
                "--resume-session conflicts with --clear-session/--session-info/--clear-tool-state/--rename-session"
                    .to_string(),
            );
        }
        if self.selects_offline_session_command() && self.no_resume_session {
            return Err(
                "--no-resume-session conflicts with --clear-session/--session-info/--clear-tool-state/--rename-session"
                    .to_string(),
            );
        }
        if self.about && self.conflicts_with_another_command() {
            return Err("--about conflicts with the selected command".to_string());
        }

        if self.check_update && self.conflicts_with_another_command() {
            return Err("--check-update conflicts with the selected command".to_string());
        }

        Ok(())
    }
}

/// The parse tables `usage` derived, which is where raw names and shorts live.
///
/// The one walk into the spec's shape for those. `options_block` reads
/// `spec.root.flags` instead, because it needs the per-flag help, value name
/// and `hide` that only the metadata carries.
fn flag_table() -> &'static [&'static usage::Flag<'static>] {
    Cli::spec().root.cmd.flags
}

/// Every long flag this CLI accepts, hidden ones and built-ins included: a typo
/// of a real flag is worth correcting even when the flag is not advertised.
fn long_flag_names() -> impl Iterator<Item = String> {
    flag_table()
        .iter()
        .flat_map(|flag| flag.longs.iter().map(|long| (*long).to_string()))
        .chain(BUILTINS.iter().map(|builtin| builtin.long.to_string()))
}

/// The declared flag with this long name.
fn long_flag(name: &str) -> Option<&'static usage::Flag<'static>> {
    flag_table()
        .iter()
        .copied()
        .find(|flag| flag.longs.contains(&name))
}

/// Where a short bundle hands over to a value.
///
/// A bundle gives its value to the *first* letter that takes one, and every
/// byte after that letter is the value: `-amwhi` is `--active` plus a board id
/// of `whi`, and `-mcustom` is already satisfied. Returns that flag and the
/// offset its value starts at, or `None` when no letter takes a value or an
/// unknown letter comes first.
///
/// The one place that knows this rule. Reading it three different ways is what
/// let `-mcustom` be mistaken for `-m` still waiting.
fn bundle_value_boundary(letters: &str) -> Option<(&'static usage::Flag<'static>, usize)> {
    for (offset, letter) in letters.char_indices() {
        let flag = short_flag(letter)?;
        if flag.takes_value {
            return Some((flag, offset + letter.len_utf8()));
        }
    }
    None
}

/// The declared flag with this short letter.
fn short_flag(letter: char) -> Option<&'static usage::Flag<'static>> {
    let byte = u8::try_from(letter).ok()?;
    flag_table()
        .iter()
        .copied()
        .find(|flag| flag.shorts.contains(&byte))
}

/// A parse failure `scan_tokens` did not already reject.
///
/// The scan walks the same tokens against the same flag table and owns every
/// wording, so reaching here means `usage` refused something the scan accepted
/// — a value's own type or choices, in practice.
fn describe_parse_error(err: &usage::Error<'_, '_>) -> String {
    match err {
        usage::Error::InvalidValue(invalid) => {
            format!("--{}: {}", invalid.name, invalid.reason)
        }
        usage::Error::InvalidChoice { name, choices } => {
            format!("--{name} must be one of: {}", choices.join(", "))
        }
        _ => "invalid arguments".to_string(),
    }
}

/// A flag `usage` answers itself.
///
/// These are deliberately absent from the derived tables, so every place that
/// reasons about a flag — help rendering, switch validation, short-option
/// validation and completions — has to be told about them. One table rather
/// than four copies.
struct Builtin {
    long: &'static str,
    short: char,
    help: &'static str,
}

const BUILTINS: [Builtin; 2] = [
    Builtin {
        long: "help",
        short: 'h',
        help: "Show help",
    },
    Builtin {
        long: "version",
        short: 'V',
        help: "Show version",
    },
];
fn conflict(left: &str, right: &str) -> String {
    format!("{left} conflicts with {right}")
}

pub(crate) fn print_help() {
    println!("wayscriber: Screen annotation tool for Wayland compositors");
    println!();
    println!("Usage:");
    print!("{}", synopsis());
    println!();
    println!("Options:");
    print!("{}", options_block());
}

/// The `Usage:` synopsis.
///
/// Hand-written, unlike the options table: it says which flags go *together*,
/// which is the one thing the flag declarations cannot express. A test checks
/// every flag it names still exists, so it can go stale but not lie.
fn synopsis() -> String {
    [
        "  wayscriber -d, --daemon [--session-file PATH]",
        "  wayscriber --daemon --freeze-on-show",
        "  wayscriber --daemon-toggle [--freeze] [--mode MODE] [--session-file PATH]",
        "  wayscriber --daemon-action ACTION",
        "  wayscriber --light-toggle | --light-draw-toggle | --light-draw-on | --light-draw-off",
        "  wayscriber -a, --active [--mode MODE]",
        "  wayscriber --active --session-file PATH",
        "  wayscriber --freeze [--session-file PATH]",
        "  wayscriber --session-info [--session-file PATH]",
        "  wayscriber --rename-session NAME --session-file PATH",
        "  wayscriber --clear-session [--session-file PATH]",
        "  wayscriber --clear-tool-state [--session-file PATH]",
        "  wayscriber --about",
        "  wayscriber --check-update",
        "  wayscriber --completions SHELL",
    ]
    .map(|line| format!("{line}\n"))
    .concat()
}

/// The `Options:` table, rendered from the derived spec.
///
/// Generated rather than written out, so a new flag needs only its field: the
/// table, the shell completions and the spec all read the same declaration.
/// `-h`/`--help` and `-V`/`--version` are appended because `usage` answers them
/// itself and so keeps them out of the flag tables.
fn options_block() -> String {
    let spec = Cli::spec();
    let mut rows: Vec<(String, &str)> = spec
        .root
        .flags
        .iter()
        .filter(|meta| !meta.hide)
        .map(|meta| {
            let mut left = String::from("  ");
            match meta.flag.shorts.first() {
                Some(&short) => left.push_str(&format!("-{}, ", char::from(short))),
                None => left.push_str("    "),
            }
            left.push_str("--");
            left.push_str(meta.flag.longs.first().copied().unwrap_or(meta.flag.name));
            if let Some(value_name) = meta.value_name {
                left.push(' ');
                left.push_str(value_name);
            }
            (left, meta.help.unwrap_or_default())
        })
        .collect();
    rows.extend(BUILTINS.iter().map(|builtin| {
        (
            format!("  -{}, --{}", builtin.short, builtin.long),
            builtin.help,
        )
    }));

    let width = rows
        .iter()
        .map(|(left, _)| left.len())
        .max()
        .unwrap_or(0)
        .max(30)
        + 2;
    rows.iter()
        .map(|(left, help)| format!("{left:width$}{help}\n"))
        .collect()
}

pub(crate) fn print_version() {
    println!("wayscriber {}", crate::build_info::version());
}

#[cfg(test)]
mod tests;
