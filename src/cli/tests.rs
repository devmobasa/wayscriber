use std::path::PathBuf;

use super::{Cli, CliOutcome};
use crate::tray_action::TrayAction;

fn parse_cli<const N: usize>(args: [&str; N]) -> Cli {
    match Cli::from_argv(args).unwrap() {
        CliOutcome::Run(cli) => cli,
        outcome => panic!("expected CLI run outcome, got {outcome:?}"),
    }
}

/// What a shell is offered at `line`, as that shell's script would read it.
fn completions_for_shell(shell: &str, line: &str) -> String {
    let outcome = Cli::from_argv([
        "wayscriber",
        "__complete_word__",
        "--shell",
        shell,
        "--line",
        line,
    ])
    .unwrap();
    match outcome {
        CliOutcome::PrintAndExit(answer) => answer,
        other => panic!("expected completions, got {other:?}"),
    }
}

fn completions_for(line: &str) -> String {
    completions_for_shell("bash", line)
}

fn completion_script_for(shell: &str) -> String {
    match Cli::from_argv(["wayscriber", "--completions", shell]).unwrap() {
        CliOutcome::PrintAndExit(script) => script,
        other => panic!("expected {shell} completion script, got {other:?}"),
    }
}

#[test]
fn help_and_version_are_explicit_outcomes_including_clusters() {
    for args in [
        ["wayscriber", "--help"],
        ["wayscriber", "-h"],
        ["wayscriber", "-dh"],
    ] {
        assert!(matches!(Cli::from_argv(args).unwrap(), CliOutcome::Help));
    }

    for args in [
        ["wayscriber", "--version"],
        ["wayscriber", "-V"],
        ["wayscriber", "-aV"],
    ] {
        assert!(matches!(Cli::from_argv(args).unwrap(), CliOutcome::Version));
    }
}

#[test]
fn active_mode_with_explicit_board_id() {
    let cli = parse_cli(["wayscriber", "--active", "--mode", "whiteboard"]);
    assert!(cli.active);
    assert_eq!(cli.mode.as_deref(), Some("whiteboard"));
}

#[test]
fn clap_compatible_short_mode_forms_are_supported() {
    let cli = parse_cli(["wayscriber", "-mwhiteboard"]);
    assert_eq!(cli.mode.as_deref(), Some("whiteboard"));

    let cli = parse_cli(["wayscriber", "-m=whiteboard"]);
    assert_eq!(cli.mode.as_deref(), Some("whiteboard"));

    let cli = parse_cli(["wayscriber", "-dm=whiteboard"]);
    assert!(cli.daemon);
    assert_eq!(cli.mode.as_deref(), Some("whiteboard"));

    let cli = parse_cli(["wayscriber", "-am", "whiteboard"]);
    assert!(cli.active);
    assert_eq!(cli.mode.as_deref(), Some("whiteboard"));

    let cli = parse_cli(["wayscriber", "-amwhiteboard"]);
    assert!(cli.active);
    assert_eq!(cli.mode.as_deref(), Some("whiteboard"));
}

#[test]
fn daemon_mode_accepts_freeze_on_show() {
    let cli = parse_cli(["wayscriber", "--daemon", "--freeze-on-show"]);
    assert!(cli.daemon);
    assert!(cli.freeze_on_show);
}

#[test]
fn daemon_mode_accepts_session_file() {
    let cli = parse_cli([
        "wayscriber",
        "--daemon",
        "--session-file",
        "/tmp/wayscriber-daemon.session",
    ]);
    assert!(cli.daemon);
    assert_eq!(
        cli.session_file,
        Some(PathBuf::from("/tmp/wayscriber-daemon.session"))
    );
}

#[test]
fn daemon_toggle_accepts_overlay_launch_args() {
    let cli = parse_cli([
        "wayscriber",
        "--daemon-toggle",
        "--freeze",
        "--mode",
        "whiteboard",
        "--exit-after-capture",
        "--resume-session",
    ]);
    assert!(cli.daemon_toggle);
    assert!(cli.freeze);
    assert_eq!(cli.mode.as_deref(), Some("whiteboard"));
    assert!(cli.exit_after_capture);
    assert!(cli.resume_session);
}

#[test]
fn daemon_toggle_accepts_session_file() {
    let cli = parse_cli([
        "wayscriber",
        "--daemon-toggle",
        "--session-file",
        "/tmp/wayscriber-toggle.session",
    ]);
    assert!(cli.daemon_toggle);
    assert_eq!(
        cli.session_file,
        Some(PathBuf::from("/tmp/wayscriber-toggle.session"))
    );
}

#[test]
fn session_file_accepts_separated_and_attached_values() {
    let cli = parse_cli([
        "wayscriber",
        "--active",
        "--session-file",
        "/tmp/wayscriber-active.session",
    ]);
    assert!(cli.active);
    assert_eq!(
        cli.session_file,
        Some(PathBuf::from("/tmp/wayscriber-active.session"))
    );

    let cli = parse_cli([
        "wayscriber",
        "--session-info",
        "--session-file=/tmp/wayscriber-info.session",
    ]);
    assert!(cli.session_info);
    assert_eq!(
        cli.session_file,
        Some(PathBuf::from("/tmp/wayscriber-info.session"))
    );

    let cli = parse_cli([
        "wayscriber",
        "--clear-tool-state",
        "--session-file",
        "/tmp/wayscriber-tool-state.session",
    ]);
    assert!(cli.clear_tool_state);
    assert_eq!(
        cli.session_file,
        Some(PathBuf::from("/tmp/wayscriber-tool-state.session"))
    );
}

#[test]
fn session_file_requires_supported_command() {
    let result = Cli::from_argv([
        "wayscriber",
        "--session-file",
        "/tmp/wayscriber-missing-command.session",
    ]);
    assert_eq!(
        result.unwrap_err(),
        "--session-file requires --active, --freeze, --daemon, --daemon-toggle, --session-info, --clear-session, --clear-tool-state, or --rename-session"
    );
}

#[test]
fn session_file_rejects_daemon_overlay_action_modes() {
    let action_result = Cli::from_argv([
        "wayscriber",
        "--light-toggle",
        "--session-file",
        "/tmp/wayscriber-light.session",
    ]);
    assert_eq!(
        action_result.unwrap_err(),
        "--session-file cannot be combined with daemon overlay actions; use --daemon-toggle --session-file to launch a named session"
    );
}

#[test]
fn session_file_rejects_no_resume_session() {
    let active_result = Cli::from_argv([
        "wayscriber",
        "--active",
        "--session-file",
        "/tmp/wayscriber-active.session",
        "--no-resume-session",
    ]);
    assert_eq!(
        active_result.unwrap_err(),
        "--session-file conflicts with --no-resume-session because --session-file requires session persistence for this run"
    );

    let freeze_result = Cli::from_argv([
        "wayscriber",
        "--freeze",
        "--session-file",
        "/tmp/wayscriber-freeze.session",
        "--no-resume-session",
    ]);
    assert_eq!(
        freeze_result.unwrap_err(),
        "--session-file conflicts with --no-resume-session because --session-file requires session persistence for this run"
    );

    let daemon_result = Cli::from_argv([
        "wayscriber",
        "--daemon",
        "--session-file",
        "/tmp/wayscriber-daemon.session",
        "--no-resume-session",
    ]);
    assert_eq!(
        daemon_result.unwrap_err(),
        "--session-file conflicts with --no-resume-session because --session-file requires session persistence for this run"
    );

    let toggle_result = Cli::from_argv([
        "wayscriber",
        "--daemon-toggle",
        "--session-file",
        "/tmp/wayscriber-toggle.session",
        "--no-resume-session",
    ]);
    assert_eq!(
        toggle_result.unwrap_err(),
        "--session-file conflicts with --no-resume-session because --session-file requires session persistence for this run"
    );
}

#[test]
fn offline_session_commands_reject_resume_overrides() {
    let info_result = Cli::from_argv(["wayscriber", "--session-info", "--resume-session"]);
    assert_eq!(
        info_result.unwrap_err(),
        "--resume-session conflicts with --clear-session/--session-info/--clear-tool-state/--rename-session"
    );

    let clear_result = Cli::from_argv(["wayscriber", "--clear-session", "--no-resume-session"]);
    assert_eq!(
        clear_result.unwrap_err(),
        "--no-resume-session conflicts with --clear-session/--session-info/--clear-tool-state/--rename-session"
    );

    let tool_state_result =
        Cli::from_argv(["wayscriber", "--clear-tool-state", "--resume-session"]);
    assert_eq!(
        tool_state_result.unwrap_err(),
        "--resume-session conflicts with --clear-session/--session-info/--clear-tool-state/--rename-session"
    );
}

#[test]
fn rename_session_requires_session_file_and_accepts_equals_form() {
    let missing = Cli::from_argv(["wayscriber", "--rename-session", "Lecture"]);
    assert_eq!(
        missing.unwrap_err(),
        "--rename-session requires --session-file"
    );

    let cli = parse_cli([
        "wayscriber",
        "--rename-session=Lecture 04",
        "--session-file",
        "/tmp/lecture.wayscriber-session",
    ]);
    assert_eq!(cli.rename_session.as_deref(), Some("Lecture 04"));
    assert_eq!(
        cli.session_file,
        Some(PathBuf::from("/tmp/lecture.wayscriber-session"))
    );
}

#[test]
fn rename_session_conflicts_with_other_session_commands() {
    let result = Cli::from_argv([
        "wayscriber",
        "--rename-session",
        "Lecture",
        "--session-file",
        "/tmp/lecture.wayscriber-session",
        "--session-info",
    ]);
    assert_eq!(
        result.unwrap_err(),
        "--rename-session conflicts with --session-info"
    );
}

#[test]
fn rename_session_rejects_every_overlay_option() {
    let overlay_options = [
        vec!["--daemon"],
        vec!["--daemon-toggle"],
        vec!["--daemon-action", "toggle_help"],
        vec!["--light-toggle"],
        vec!["--light-draw-toggle"],
        vec!["--light-draw-on"],
        vec!["--light-draw-off"],
        vec!["--active"],
        vec!["--mode", "whiteboard"],
        vec!["--no-tray"],
        vec!["--freeze-on-show"],
        vec!["--freeze"],
        vec!["--exit-after-capture"],
        vec!["--no-exit-after-capture"],
        vec!["--resume-session"],
        vec!["--no-resume-session"],
    ];

    for option in overlay_options {
        let mut args = vec![
            "wayscriber",
            "--rename-session",
            "Lecture",
            "--session-file",
            "/tmp/lecture.wayscriber-session",
        ];
        args.extend(option.iter().copied());

        assert_eq!(
            Cli::from_argv(args).unwrap_err(),
            "--rename-session conflicts with overlay/daemon options",
            "expected rename with {option:?} to be rejected"
        );
    }
}

#[test]
fn offline_session_commands_conflict_with_each_other() {
    let clear_result = Cli::from_argv(["wayscriber", "--clear-tool-state", "--clear-session"]);
    assert_eq!(
        clear_result.unwrap_err(),
        "--clear-tool-state conflicts with --clear-session"
    );

    let info_result = Cli::from_argv(["wayscriber", "--clear-tool-state", "--session-info"]);
    assert_eq!(
        info_result.unwrap_err(),
        "--clear-tool-state conflicts with --session-info"
    );
}

#[test]
fn daemon_action_accepts_light_mode_actions() {
    let cli = parse_cli(["wayscriber", "--daemon-action", "light_draw_toggle"]);
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
        let cli = parse_cli(["wayscriber", flag]);
        assert_eq!(cli.daemon_overlay_action().unwrap(), Some(expected));
    }
}

#[test]
fn friendly_light_aliases_conflict_with_raw_daemon_action() {
    let result = Cli::from_argv([
        "wayscriber",
        "--daemon-action",
        "toggle_light_mode",
        "--light-toggle",
    ]);
    assert!(result.is_err());
}

#[test]
fn friendly_light_aliases_conflict_with_each_other() {
    let result = Cli::from_argv(["wayscriber", "--light-toggle", "--light-draw-toggle"]);
    assert!(result.is_err());
}

/// Also what keeps `choices_strict = false` honest: the action vocabulary is
/// offered to shells, but it is not the accepted set, so an unknown word reaches
/// `TrayAction::parse` instead of being refused at parse time.
#[test]
fn raw_daemon_action_reports_unknown_action() {
    let cli = parse_cli(["wayscriber", "--daemon-action", "not_real"]);
    assert_eq!(
        cli.daemon_overlay_action().unwrap_err(),
        "unknown daemon action 'not_real'"
    );
}

#[test]
fn cli_conflicting_flags_fail() {
    let result = Cli::from_argv(["wayscriber", "--active", "--clear-session"]);
    assert!(
        result.is_err(),
        "expected conflicting flags (--active and --clear-session) to error"
    );

    let result = Cli::from_argv(["wayscriber", "--active", "--clear-tool-state"]);
    assert_eq!(
        result.unwrap_err(),
        "--clear-tool-state conflicts with --daemon/--active"
    );
}

#[test]
fn freeze_on_show_requires_daemon() {
    let result = Cli::from_argv(["wayscriber", "--freeze-on-show"]);
    assert!(
        result.is_err(),
        "expected --freeze-on-show without --daemon to error"
    );
}

#[test]
fn check_update_parses_on_its_own() {
    let cli = parse_cli(["wayscriber", "--check-update"]);
    assert!(cli.check_update);
    assert!(!cli.about);
}

/// Every print-and-exit command rejects every launch flag. `--about` and
/// `--check-update` used to omit the light aliases and the capture-exit flags,
/// so those combinations ran the print command and silently dropped the rest.
#[test]
fn print_and_exit_commands_reject_every_launch_flag() {
    let launch_flags = [
        "--daemon",
        "--daemon-toggle",
        "--active",
        "--no-tray",
        "--clear-session",
        "--clear-tool-state",
        "--session-info",
        "--freeze",
        "--light-toggle",
        "--light-draw-toggle",
        "--light-draw-on",
        "--light-draw-off",
        "--exit-after-capture",
        "--no-exit-after-capture",
        "--resume-session",
        "--no-resume-session",
    ];

    for command in ["--about", "--check-update", "--runtime-capabilities"] {
        for flag in launch_flags {
            let result = Cli::from_argv(["wayscriber", command, flag]);
            assert!(
                result.is_err(),
                "expected {command} with {flag} to be rejected"
            );
        }
    }
}

#[test]
fn print_and_exit_commands_reject_each_other() {
    assert_eq!(
        Cli::from_argv(["wayscriber", "--about", "--check-update"]).unwrap_err(),
        "--about conflicts with the selected command"
    );
    assert_eq!(
        Cli::from_argv(["wayscriber", "--runtime-capabilities", "--check-update"]).unwrap_err(),
        "--runtime-capabilities conflicts with launch flags"
    );
}

#[test]
fn check_update_rejects_valued_launch_flags() {
    assert_eq!(
        Cli::from_argv(["wayscriber", "--check-update", "--mode", "whiteboard"]).unwrap_err(),
        "--check-update conflicts with the selected command"
    );
    // `--session-file` has its own earlier "requires a session command" check,
    // so the rejection is the same but the wording comes from there.
    assert!(
        Cli::from_argv([
            "wayscriber",
            "--check-update",
            "--session-file",
            "/tmp/x.wayscriber-session",
        ])
        .is_err()
    );
    assert_eq!(
        Cli::from_argv([
            "wayscriber",
            "--check-update",
            "--daemon-action",
            "toggle_help"
        ])
        .unwrap_err(),
        "--check-update conflicts with the selected command"
    );
}

#[test]
fn unknown_flags_suggest_the_closest_real_one() {
    assert_eq!(
        Cli::from_argv(["wayscriber", "--activ"]).unwrap_err(),
        "unknown argument '--activ' (did you mean '--active'?)"
    );
    assert_eq!(
        Cli::from_argv(["wayscriber", "--sesion-info"]).unwrap_err(),
        "unknown argument '--sesion-info' (did you mean '--session-info'?)"
    );
}

/// A typo is worth a suggestion; a word that resembles nothing is not. Offering
/// the nearest flag for every unknown argument is how "did you mean" turns into
/// noise.
#[test]
fn unrecognisable_arguments_are_not_given_a_suggestion() {
    assert_eq!(
        Cli::from_argv(["wayscriber", "--zzzzzzzzzz"]).unwrap_err(),
        "unknown argument '--zzzzzzzzzz'"
    );
}

#[test]
fn flags_that_take_a_value_still_report_a_missing_one() {
    assert_eq!(
        Cli::from_argv(["wayscriber", "--mode"]).unwrap_err(),
        "--mode requires a value"
    );
    assert_eq!(
        Cli::from_argv(["wayscriber", "--session-file"]).unwrap_err(),
        "--session-file requires a value"
    );
}

#[test]
fn completions_print_a_script_for_every_supported_shell() {
    for shell in ["bash", "zsh", "fish", "nu", "powershell"] {
        let script = completion_script_for(shell);
        assert!(
            script.contains("wayscriber"),
            "{shell} script should name the binary"
        );
        assert!(
            script.contains("__complete_word__"),
            "{shell} script should call back into the binary"
        );
    }
}

#[test]
fn completions_reject_an_unsupported_shell() {
    assert_eq!(
        Cli::from_argv(["wayscriber", "--completions", "tcsh"]).unwrap_err(),
        "--completions must be one of: bash, zsh, fish, nu, nushell, powershell, pwsh"
    );
}

#[test]
fn completions_conflict_with_launch_commands() {
    for flag in ["--daemon", "--active", "--session-info", "--check-update"] {
        assert_eq!(
            Cli::from_argv(["wayscriber", "--completions", "bash", flag]).unwrap_err(),
            "--completions conflicts with the selected command",
            "expected --completions with {flag} to be rejected"
        );
    }
}

/// The generated shell scripts shell out to `wayscriber __complete_word__`, and
/// the derive puts that interception in its own `parse`, which `from_argv`
/// replaces. If this regresses, every installed completion script goes quiet
/// rather than failing loudly.
#[test]
fn the_completion_callback_the_scripts_invoke_is_answered() {
    let outcome = Cli::from_argv([
        "wayscriber",
        "__complete_word__",
        "--shell",
        "bash",
        "--line",
        "wayscriber --sess",
    ])
    .unwrap();

    let CliOutcome::PrintAndExit(candidates) = outcome else {
        panic!("expected the completion callback to be answered");
    };
    assert!(candidates.contains("--session-file"), "got: {candidates:?}");
    assert!(candidates.contains("--session-info"), "got: {candidates:?}");
}

/// Hidden flags stay out of completions but keep parsing: the configurator
/// shells out to `wayscriber --runtime-capabilities`.
#[test]
fn hidden_flags_are_uncompleted_but_still_parse() {
    let outcome = Cli::from_argv([
        "wayscriber",
        "__complete_word__",
        "--shell",
        "bash",
        "--line",
        "wayscriber --runtime",
    ])
    .unwrap();
    let CliOutcome::PrintAndExit(candidates) = outcome else {
        panic!("expected the completion callback to be answered");
    };
    assert!(
        !candidates.contains("--runtime-capabilities"),
        "hidden flag should not be completed, got: {candidates:?}"
    );

    let cli = parse_cli(["wayscriber", "--runtime-capabilities"]);
    assert!(cli.runtime_capabilities);
}

#[test]
fn the_spec_endpoint_describes_this_cli() {
    let outcome = Cli::from_argv(["wayscriber", "__usage_spec__"]).unwrap();
    let CliOutcome::PrintAndExit(spec) = outcome else {
        panic!("expected the spec endpoint to be answered");
    };
    assert!(spec.contains("name wayscriber"), "got: {spec}");
    assert!(
        spec.contains("Screen annotation tool for Wayland compositors"),
        "the command's `about` should be the user-facing one, got: {spec}"
    );
}

/// Every action `--daemon-action` dispatches round-trips through its spelling.
#[test]
fn every_tray_action_parses_from_its_own_name() {
    for action in TrayAction::ALL {
        assert_eq!(TrayAction::parse(action.as_str()), Some(action));
    }
}

/// The hand-rolled parser rejected an empty *attached* value; `usage` binds an
/// empty string like any other, so the rule is enforced separately now. Only the
/// attached spellings: a separated empty argument was accepted and still is —
/// see `separated_empty_values_reach_their_own_validation`.

#[test]
fn value_flags_reject_an_empty_value() {
    for (args, expected) in [
        (
            vec!["wayscriber", "--active", "--mode="],
            "--mode requires a value",
        ),
        // Named as typed: the short spelling reports the short flag.
        (vec!["wayscriber", "--active", "-m="], "-m requires a value"),
        (vec!["wayscriber", "-am="], "-m requires a value"),
        (
            vec!["wayscriber", "--daemon-action="],
            "--daemon-action requires a value",
        ),
        (
            vec!["wayscriber", "--rename-session="],
            "--rename-session requires a value",
        ),
        (
            vec!["wayscriber", "--active", "--session-file="],
            "--session-file requires a value",
        ),
        // The raw precheck runs before `choices`, so an empty attached value is
        // reported the same way as for every other value flag rather than as a
        // list of shells.
        (
            vec!["wayscriber", "--completions="],
            "--completions requires a value",
        ),
    ] {
        assert_eq!(
            Cli::from_argv(args.clone()).unwrap_err(),
            expected,
            "expected {args:?} to be rejected"
        );
    }
}

/// No flags is a `Run` whose every field is default: `app::run` reads that as
/// "print the usage summary". Previously untested, and easy to break by giving
/// any flag a non-default value.
#[test]
fn no_arguments_is_a_run_that_selects_no_command() {
    let cli = parse_cli(["wayscriber"]);
    assert!(!cli.selects_a_launch_command());
    assert!(!cli.about && !cli.check_update && !cli.runtime_capabilities);
    assert!(cli.completions.is_none());
}

/// Inside a bundle the old parser named the bad letter; a lone short was never
/// a bundle, so it reported the whole token instead. Verified against the
/// `main` binary rather than inferred.
#[test]
fn unknown_short_options_are_named_the_way_main_named_them() {
    for args in [vec!["wayscriber", "-dx"], vec!["wayscriber", "-xd"]] {
        assert_eq!(
            Cli::from_argv(args.clone()).unwrap_err(),
            "unknown short option '-x'",
            "expected {args:?} to name the letter"
        );
    }

    assert_eq!(
        Cli::from_argv(["wayscriber", "-x"]).unwrap_err(),
        "unknown argument '-x'"
    );
}

/// A value that looks like a flag is a missing value, not an unknown argument.
#[test]
fn a_flag_like_value_reports_the_missing_value() {
    assert_eq!(
        Cli::from_argv(["wayscriber", "--active", "--mode", "-whiteboard"]).unwrap_err(),
        "--mode requires a value"
    );
}

/// Rejected at the front door, as the hand-rolled parser did, rather than
/// several layers later in the app with a different exit code.
#[test]
fn non_utf8_arguments_are_refused_up_front() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let args = vec![
        OsString::from("wayscriber"),
        OsString::from("--mode"),
        OsString::from_vec(b"bad\xff".to_vec()),
    ];
    assert_eq!(
        Cli::from_argv(args).unwrap_err(),
        "arguments must be valid UTF-8"
    );
}

/// Repeating a flag stayed accepted, last value winning, as before the switch.
#[test]
fn repeated_flags_are_accepted_with_the_last_value_winning() {
    let cli = parse_cli(["wayscriber", "--active", "--active"]);
    assert!(cli.active);

    let cli = parse_cli(["wayscriber", "--active", "--mode", "a", "--mode", "b"]);
    assert_eq!(cli.mode.as_deref(), Some("b"));
}

/// The `Options:` table is rendered from the spec, so every advertised flag is
/// present and every hidden one is absent, without a second list to maintain.
#[test]
fn the_options_table_is_generated_from_the_declared_flags() {
    let table = super::options_block();

    for expected in [
        "  -d, --daemon",
        "  -m, --mode MODE",
        "      --session-file PATH",
        "      --completions SHELL",
        "  -h, --help",
        "  -V, --version",
    ] {
        assert!(
            table.contains(expected),
            "missing {expected:?} in:\n{table}"
        );
    }
    assert!(
        !table.contains("--runtime-capabilities"),
        "hidden flags stay out of the table:\n{table}"
    );
}

/// The synopsis is the one hand-written part of `--help`. It may fall behind on
/// which combinations are worth showing, but it must never name a flag that no
/// longer exists.
#[test]
fn the_synopsis_only_names_real_flags() {
    let synopsis = super::synopsis();
    let declared: Vec<String> = super::long_flag_names().collect();

    let named: Vec<String> = synopsis
        .split_whitespace()
        .filter_map(|word| {
            word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-')
                .strip_prefix("--")
                .map(str::to_string)
        })
        .filter(|word| !word.is_empty())
        .collect();

    assert!(!named.is_empty(), "synopsis should name flags");
    for flag in named {
        assert!(
            declared.contains(&flag),
            "synopsis names --{flag}, which is not a declared flag; declared: {declared:?}"
        );
    }
}

/// A packaging-only hotfix builds with `WAYSCRIBER_RELEASE_VERSION=X.Y.Z.N`
/// while Cargo stays on `X.Y.Z`. A bare `#[usage(version)]` would bake in
/// `CARGO_PKG_VERSION` and leave `__usage_spec__` reporting a different version
/// from `--version` for exactly those artifacts.
#[test]
fn the_spec_reports_the_same_version_the_binary_does() {
    assert_eq!(
        super::Cli::spec().version,
        Some(crate::build_info::VERSION),
        "the spec version must track the reported release version"
    );
    assert_eq!(crate::build_info::version(), crate::build_info::VERSION);
}

/// Only `--session-file` takes a path. Without an explicit hint usage-rs falls
/// back to filesystem completion, so a shell would offer local filenames as
/// board ids, actions and catalog labels.
#[test]
fn only_the_path_flag_asks_the_shell_for_files() {
    // `\x01files` is the marker the generated scripts read as "ask the shell".
    assert!(
        completions_for("wayscriber --session-file ").contains("files"),
        "--session-file should still request paths"
    );

    for flag in ["--mode", "--daemon-action", "--rename-session"] {
        let answer = completions_for(&format!("wayscriber {flag} "));
        assert!(
            !answer.contains("files"),
            "{flag} should not ask the shell for filenames, got: {answer:?}"
        );
    }

    let shells = completions_for("wayscriber --completions ");
    for shell in ["bash", "zsh", "fish", "nu", "powershell"] {
        assert!(
            shells.contains(shell),
            "--completions should offer {shell}, got: {shells:?}"
        );
    }
    assert!(!shells.contains("files"), "got: {shells:?}");
}

/// `usage` accepts and ignores text attached to a switch, so without an explicit
/// rejection `--clear-session=false` would *set* `--clear-session` and delete the
/// session the user was plainly trying not to touch. The hand-rolled parser had
/// no such spelling; these tokens stay unknown arguments.
#[test]
fn switches_reject_an_attached_value() {
    for token in [
        "--clear-session=false",
        "--clear-session=garbage",
        "--clear-tool-state=false",
        "--daemon=true",
        "--active=0",
        "--about=0",
        "--freeze=no",
        "--help=x",
        "--version=1",
    ] {
        // The suggestion is appended when a declared flag is close, which for
        // `--clear-session=false` is `--clear-session` itself — the spelling the
        // user should have typed.
        assert!(
            Cli::from_argv(["wayscriber", token])
                .unwrap_err()
                .starts_with(&format!("unknown argument '{token}'")),
            "expected {token} to be rejected outright"
        );
    }
}

/// The destructive flags specifically: none of them may be bound by a token that
/// reads as switching them off.
#[test]
fn destructive_flags_are_never_bound_by_an_attached_value() {
    for token in [
        "--clear-session=false",
        "--clear-session=0",
        "--clear-tool-state=false",
        "--clear-tool-state=0",
    ] {
        let error = Cli::from_argv(["wayscriber", token]).unwrap_err();
        assert!(
            error.starts_with("unknown argument"),
            "{token} must not reach validation, got: {error}"
        );
    }

    // And the flags themselves still work in their real spelling.
    let cli = parse_cli(["wayscriber", "--clear-session"]);
    assert!(cli.clear_session);
}

/// Value-taking flags keep their `--flag=value` spelling.
#[test]
fn value_flags_still_accept_an_attached_value() {
    let cli = parse_cli(["wayscriber", "--active", "--mode=whiteboard"]);
    assert_eq!(cli.mode.as_deref(), Some("whiteboard"));

    let cli = parse_cli([
        "wayscriber",
        "--session-info",
        "--session-file=/tmp/a.session",
    ]);
    assert_eq!(cli.session_file, Some(PathBuf::from("/tmp/a.session")));
}

/// The `choices(...)` spellings have to be literals in the attribute, so this is
/// what keeps them from drifting away from the dispatch table.
#[test]
fn the_daemon_action_choices_match_the_dispatch_table() {
    let meta = super::Cli::spec()
        .root
        .flags
        .iter()
        .find(|meta| meta.flag.longs.contains(&"daemon-action"))
        .expect("--daemon-action should be declared");

    let declared: Vec<&str> = meta.choices.to_vec();
    let dispatched: Vec<&str> = TrayAction::ALL.iter().map(|a| a.as_str()).collect();
    assert_eq!(declared, dispatched);

    assert!(
        meta.allow_unknown_choices,
        "an unknown action must still reach TrayAction::parse"
    );
}

/// A separated empty argument was accepted by the hand-rolled parser and reached
/// the code that owns the value, which has a better error than the CLI can give:
/// `--rename-session ""` reports "session display name cannot be empty" from the
/// catalog. Rejecting it here would replace a domain error with a generic one.
#[test]
fn separated_empty_values_reach_their_own_validation() {
    let cli = parse_cli([
        "wayscriber",
        "--rename-session",
        "",
        "--session-file",
        "/tmp/lecture.wayscriber-session",
    ]);
    assert_eq!(cli.rename_session.as_deref(), Some(""));

    let cli = parse_cli(["wayscriber", "--active", "--mode", ""]);
    assert_eq!(cli.mode.as_deref(), Some(""));
}

/// `usage` answers `-h`/`--help` and `-V`/`--version` itself and keeps them out
/// of the flag tables, so without an overlay the generated scripts offer nothing
/// for `--h` even though the binary accepts it and `--help` advertises it.
#[test]
fn completions_offer_the_flags_usage_answers_itself() {
    assert!(completions_for("wayscriber --h").contains("--help"));
    assert!(completions_for("wayscriber --v").contains("--version"));
    assert!(completions_for("wayscriber -h").contains("-h"));

    let dashes = completions_for("wayscriber --");
    assert!(dashes.contains("--help") && dashes.contains("--version"));

    let single = completions_for("wayscriber -");
    for expected in ["-h", "-V", "--help", "--version"] {
        assert!(
            single.contains(expected),
            "missing {expected} in {single:?}"
        );
    }

    // A value position keeps its own answer: no flag is added to a path request
    // or to a flag's own candidates.
    assert!(!completions_for("wayscriber --session-file ").contains("--help"));
    assert!(!completions_for("wayscriber --daemon-action ").contains("--help"));
}

/// The overlay has to speak each script's protocol. zsh reads
/// `value TAB description TAB insertion` and inserts the third field, so a bare
/// line completes to nothing — the advertised flag would silently not work.
#[test]
fn overlaid_builtins_use_each_shells_protocol() {
    assert_eq!(completions_for_shell("bash", "wayscriber --h"), "--help\n");
    assert_eq!(
        completions_for_shell("zsh", "wayscriber --h"),
        "--help\tShow help\t--help\n"
    );
    for shell in ["fish", "nu", "powershell"] {
        assert_eq!(
            completions_for_shell(shell, "wayscriber --h"),
            "--help\tShow help\n",
            "{shell} takes value and description"
        );
    }
}

/// Every shell's real flag candidates and the overlaid built-ins must be shaped
/// the same way, or one of them completes to nothing.
#[test]
fn overlaid_builtins_match_the_shape_of_real_candidates() {
    for shell in ["bash", "zsh", "fish", "nu", "powershell"] {
        let real = completions_for_shell(shell, "wayscriber --session-inf");
        let overlaid = completions_for_shell(shell, "wayscriber --hel");
        assert_eq!(
            real.trim_end().matches('\t').count(),
            overlaid.trim_end().matches('\t').count(),
            "{shell}: real {real:?} vs overlaid {overlaid:?}"
        );
    }
}

/// wayscriber accepts no positional arguments, so the shell's path fallback is
/// wrong everywhere except the value of `--session-file` — including the root
/// position and the position after a completed switch.
#[test]
fn no_position_but_the_path_flag_asks_for_files() {
    for line in [
        "wayscriber ",
        "wayscriber --session-info ",
        "wayscriber --daemon ",
        "wayscriber --clear-session ",
        "wayscriber --daemon --active ",
    ] {
        for shell in ["bash", "zsh", "fish", "nu", "powershell"] {
            let answer = completions_for_shell(shell, line);
            assert!(
                !answer.contains('\u{1}'),
                "{shell} at [{line}] should not request paths, got {answer:?}"
            );
        }
    }

    for shell in ["bash", "zsh", "fish", "nu", "powershell"] {
        assert!(
            completions_for_shell(shell, "wayscriber --session-file ").contains('\u{1}'),
            "{shell} should still request paths for --session-file"
        );
    }
}

/// The hand-rolled parser named the long flag from its standalone arm and the
/// short one from its bundle arm. Both spellings are preserved.
#[test]
fn a_missing_value_is_named_the_way_it_was_typed() {
    assert_eq!(
        Cli::from_argv(["wayscriber", "-am"]).unwrap_err(),
        "-m requires a value"
    );
    assert_eq!(
        Cli::from_argv(["wayscriber", "-m"]).unwrap_err(),
        "--mode requires a value"
    );
    assert_eq!(
        Cli::from_argv(["wayscriber", "--mode"]).unwrap_err(),
        "--mode requires a value"
    );
}

/// The parser accepts a value attached to its flag, so completion has to answer
/// the same spelling. `usage`'s `complete` does not look inside one, and for a
/// short flag it offers the flag itself — which would replace what was typed.
///
/// Whether a candidate carries the attached prefix depends on what the shell
/// replaces: bash breaks words at `=`, so its current word is the value alone.
#[test]
fn attached_values_complete_as_their_separated_spelling() {
    // bash: the value alone, because `=` is one of its word breaks. A prefixed
    // candidate would land over the value as `--completions=--completions=bash`.
    assert_eq!(completions_for("wayscriber --completions=ba"), "bash\n");
    assert_eq!(
        completions_for("wayscriber --daemon-action=cap"),
        "capture_full\ncapture_region\ncapture_window\n"
    );

    // Every other shell hands over the whole token, so the prefix comes back.
    assert_eq!(
        completions_for_shell("fish", "wayscriber --completions=ba"),
        "--completions=bash\n"
    );
    assert_eq!(
        completions_for_shell("zsh", "wayscriber --completions=ba"),
        "--completions=bash\t\t--completions=bash\n"
    );

    // bash broke the word at `=`, so its own path completion is already looking
    // at the value: hand the job back, and it keeps quoting and expansion.
    assert!(
        completions_for_shell("bash", "wayscriber --session-file=/tm").contains('\u{1}'),
        "bash should complete an attached path itself"
    );

    // zsh, fish and PowerShell isolate the attached value in their generated
    // scripts, so their callbacks keep the native file marker. Nu's external
    // completer cannot delegate a substituted span and gets a direct fallback.
    let directory = tempfile::tempdir().expect("temp dir");
    std::fs::write(directory.path().join("alpha.session"), b"").expect("temp file");
    let typed = format!(
        "wayscriber --session-file={}/al",
        directory.path().display()
    );
    for shell in ["zsh", "fish", "powershell"] {
        let answer = completions_for_shell(shell, &typed);
        assert!(
            answer.contains('\u{1}'),
            "{shell} should hand the isolated value to native paths: {answer:?}"
        );
    }
    let nu = completions_for_shell("nu", &typed);
    assert!(
        nu.contains(&format!(
            "--session-file={}/alpha.session",
            directory.path().display()
        )),
        "nu got: {nu:?}"
    );

    // Nothing to offer for an open-ended value — but crucially not `-m`, which
    // would replace `-mwhi` with `-m`.
    assert_eq!(completions_for("wayscriber -mwhi"), "");
    assert_eq!(completions_for("wayscriber --mode=wh"), "");

    // The separated spellings are unchanged.
    assert_eq!(completions_for("wayscriber --completions ba"), "bash\n");
}

/// The generated scripts, not the Rust callback, own native attached-path
/// expansion. Checked anchors make an upstream template change fail during
/// generation instead of silently restoring the whole-token bug.
#[test]
fn generated_scripts_isolate_attached_paths_before_native_completion() {
    let zsh = completion_script_for("zsh");
    assert!(zsh.contains("compset -P \"${(b)__usage_attached}\""));

    let fish = completion_script_for("fish");
    assert!(fish.contains("__fish_complete_path \"$__usage_cur\""));
    assert!(fish.contains("printf '%s%s\\n' \"$__usage_attached\" \"$candidate\""));

    let powershell = completion_script_for("powershell");
    assert!(powershell.contains("CompleteFilename($pathWord)"));
    assert!(powershell.contains("$attachedPrefix + $path.CompletionText"));
}

/// Nu's fallback follows directory symlinks but refuses names that would split
/// its line/tab completion records.
#[test]
fn nu_attached_path_fallback_is_directory_aware_and_protocol_safe() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("temp dir");
    let target = directory.path().join("target");
    std::fs::create_dir(&target).expect("target dir");
    symlink(&target, directory.path().join("linkdir")).expect("directory symlink");
    std::fs::write(directory.path().join("bad\tname.session"), b"").expect("tab file");
    std::fs::write(directory.path().join("bad\nname.session"), b"").expect("newline file");

    let link = completions_for_shell(
        "nu",
        &format!(
            "wayscriber --session-file={}/link",
            directory.path().display()
        ),
    );
    assert!(
        link.contains(&format!(
            "--session-file={}/linkdir/",
            directory.path().display()
        )),
        "nu got: {link:?}"
    );

    let unsafe_names = completions_for_shell(
        "nu",
        &format!(
            "wayscriber --session-file={}/bad",
            directory.path().display()
        ),
    );
    assert!(!unsafe_names.contains("bad\tname"), "got: {unsafe_names:?}");
    assert!(!unsafe_names.contains("bad\nname"), "got: {unsafe_names:?}");
}

/// A value already attached to a bundle is not a position waiting for one, so
/// nothing there may ask the shell for paths. `custom` ends in `m`, which is
/// `--mode`'s letter, and a naive last-character test read that as `-m` waiting.
#[test]
fn a_satisfied_bundle_does_not_ask_for_files() {
    for line in [
        "wayscriber -mcustom ",
        "wayscriber -amcustom ",
        "wayscriber -am ",
        "wayscriber -m ",
    ] {
        for shell in ["bash", "zsh", "fish"] {
            let answer = completions_for_shell(shell, line);
            assert!(
                !answer.contains('\u{1}'),
                "{shell} at [{line}] should not request paths, got {answer:?}"
            );
        }
    }
}

/// `split_whitespace` loses the shell's token boundary, which made a quoted or
/// escaped path look like a fresh word and stripped the path marker from it.
#[test]
fn quoted_and_escaped_paths_still_ask_for_files() {
    for line in [
        "wayscriber --session-file /tmp/a",
        r"wayscriber --session-file /tmp/a\ b",
        "wayscriber --session-file \"/tmp/a b",
        "wayscriber --session-file '/tmp/a b",
    ] {
        for shell in ["bash", "zsh", "fish"] {
            assert!(
                completions_for_shell(shell, line).contains('\u{1}'),
                "{shell} at [{line}] should request paths"
            );
        }
    }
}

/// The occurrence that ran out of a value is the last one: an earlier bundle was
/// satisfied by the argument after it.
#[test]
fn a_missing_value_is_attributed_to_the_failing_occurrence() {
    for (args, expected) in [
        (vec!["wayscriber", "-am"], "-m requires a value"),
        (
            vec!["wayscriber", "--mode", "a", "-am"],
            "-m requires a value",
        ),
        (
            vec!["wayscriber", "-am", "whiteboard", "--mode"],
            "--mode requires a value",
        ),
        (
            vec!["wayscriber", "-am", "whiteboard", "-m"],
            "--mode requires a value",
        ),
    ] {
        assert_eq!(
            Cli::from_argv(args.clone()).unwrap_err(),
            expected,
            "for {args:?}"
        );
    }
}

/// Dash tokens the old parser refused. `-` reaching a value position mattered
/// most: `-am -` used to bind `-` as a board id and reach overlay dispatch.
#[test]
fn bare_dash_tokens_are_refused_the_way_main_refused_them() {
    assert_eq!(
        Cli::from_argv(["wayscriber", "--"]).unwrap_err(),
        "unknown argument '--'"
    );
    assert_eq!(
        Cli::from_argv(["wayscriber", "-"]).unwrap_err(),
        "unknown argument '-'"
    );
    for (args, expected) in [
        (vec!["wayscriber", "-am", "-"], "-m requires a value"),
        (vec!["wayscriber", "--mode", "-"], "--mode requires a value"),
        (
            vec!["wayscriber", "--session-file", "-"],
            "--session-file requires a value",
        ),
        (
            vec!["wayscriber", "--rename-session", "-"],
            "--rename-session requires a value",
        ),
    ] {
        assert_eq!(
            Cli::from_argv(args.clone()).unwrap_err(),
            expected,
            "{args:?}"
        );
    }
}

/// The scan runs left to right, so the first terminal wins over a later bad
/// token and the first bad token wins over a later one.
#[test]
fn the_first_terminal_or_error_in_the_line_wins() {
    assert!(matches!(
        Cli::from_argv(["wayscriber", "--help", "--clear-session=false"]).unwrap(),
        CliOutcome::Help
    ));
    assert!(matches!(
        Cli::from_argv(["wayscriber", "-hx"]).unwrap(),
        CliOutcome::Help
    ));
    assert!(matches!(
        Cli::from_argv(["wayscriber", "-Vx"]).unwrap(),
        CliOutcome::Version
    ));
    assert!(
        Cli::from_argv(["wayscriber", "--unknown", "--mode="])
            .unwrap_err()
            .starts_with("unknown argument '--unknown'")
    );
}

/// A bundle hands its value to the first letter that takes one, and the rest of
/// the token is that value — `-mam=` is a board id of `am=`.
#[test]
fn a_bundle_stops_at_its_first_value_taking_letter() {
    let cli = parse_cli(["wayscriber", "-mam="]);
    assert_eq!(cli.mode.as_deref(), Some("am="));

    let cli = parse_cli(["wayscriber", "-amwhiteboard"]);
    assert!(cli.active);
    assert_eq!(cli.mode.as_deref(), Some("whiteboard"));
}

/// The first occurrence that runs out of a value is the one blamed, and it is
/// named the way it was typed.
#[test]
fn the_first_failing_occurrence_is_the_one_blamed() {
    assert_eq!(
        Cli::from_argv(["wayscriber", "-am", "--mode", "whiteboard"]).unwrap_err(),
        "-m requires a value"
    );
    assert_eq!(
        Cli::from_argv(["wayscriber", "--mode", "-am", "whiteboard"]).unwrap_err(),
        "--mode requires a value"
    );
}

/// A bundle is already several flags; offering the first letter alone would
/// delete the rest of what was typed.
#[test]
fn completing_a_bundle_never_offers_something_shorter() {
    for line in [
        "wayscriber -am",
        "wayscriber -ad",
        "wayscriber -da",
        "wayscriber -amwhi",
        "wayscriber -dmwhi",
        "wayscriber -am=whi",
    ] {
        let answer = completions_for(line);
        assert!(
            !answer.lines().any(|candidate| candidate.len()
                < line.split_whitespace().next_back().unwrap_or("").len()),
            "[{line}] offered something shorter: {answer:?}"
        );
    }

    // A single short is still its own completion.
    assert_eq!(completions_for("wayscriber -a"), "-a\n");
}

/// The old parser decided "is this a bundle?" with `arg.len() > 2`, which is a
/// *byte* count: `-é` is two bytes behind a dash, so it counted as one and named
/// the letter. A scalar count would call it a lone short and report the token.
#[test]
fn a_non_ascii_short_is_bundled_the_way_byte_length_said() {
    assert_eq!(
        Cli::from_argv(["wayscriber", "-é"]).unwrap_err(),
        "unknown short option '-é'"
    );
    assert_eq!(
        Cli::from_argv(["wayscriber", "-x"]).unwrap_err(),
        "unknown argument '-x'"
    );
}

/// The script patches match one exact anchor in each generated template. If a
/// `usage-rs` bump moves it, `--completions` has to fail loudly: an unpatched
/// script is one whose attached-path completion silently returns nothing, which
/// is precisely the bug the patch exists to fix.
#[test]
fn a_changed_upstream_template_fails_instead_of_emitting_an_unpatched_script() {
    use usage_rs::complete::Shell;

    for shell in [Shell::Zsh, Shell::Fish, Shell::PowerShell] {
        let error = super::completion::adapt_completion_script(
            "# a template that no longer contains the anchor\n".to_string(),
            shell,
        )
        .expect_err("a missing anchor must be an error");
        assert!(
            error.contains("completion template changed"),
            "got: {error}"
        );
    }

    // bash and Nu are not patched, so any script passes through unchanged.
    for shell in [Shell::Bash, Shell::Nu] {
        let script = "# untouched\n".to_string();
        assert_eq!(
            super::completion::adapt_completion_script(script.clone(), shell).unwrap(),
            script
        );
    }
}
