use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use super::{Cli, CliOutcome};
use crate::tray_action::TrayAction;

fn parse_cli<const N: usize>(args: [&str; N]) -> Cli {
    match Cli::try_parse_from(args).unwrap() {
        CliOutcome::Run(cli) => cli,
        outcome => panic!("expected CLI run outcome, got {outcome:?}"),
    }
}

#[test]
fn help_and_version_are_explicit_outcomes_including_clusters() {
    for args in [
        ["wayscriber", "--help"],
        ["wayscriber", "-h"],
        ["wayscriber", "-dh"],
    ] {
        assert!(matches!(
            Cli::try_parse_from(args).unwrap(),
            CliOutcome::Help
        ));
    }

    for args in [
        ["wayscriber", "--version"],
        ["wayscriber", "-V"],
        ["wayscriber", "-aV"],
    ] {
        assert!(matches!(
            Cli::try_parse_from(args).unwrap(),
            CliOutcome::Version
        ));
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
    let result = Cli::try_parse_from([
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
    let action_result = Cli::try_parse_from([
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
    let active_result = Cli::try_parse_from([
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

    let freeze_result = Cli::try_parse_from([
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

    let daemon_result = Cli::try_parse_from([
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

    let toggle_result = Cli::try_parse_from([
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
    let info_result = Cli::try_parse_from(["wayscriber", "--session-info", "--resume-session"]);
    assert_eq!(
        info_result.unwrap_err(),
        "--resume-session conflicts with --clear-session/--session-info/--clear-tool-state/--rename-session"
    );

    let clear_result =
        Cli::try_parse_from(["wayscriber", "--clear-session", "--no-resume-session"]);
    assert_eq!(
        clear_result.unwrap_err(),
        "--no-resume-session conflicts with --clear-session/--session-info/--clear-tool-state/--rename-session"
    );

    let tool_state_result =
        Cli::try_parse_from(["wayscriber", "--clear-tool-state", "--resume-session"]);
    assert_eq!(
        tool_state_result.unwrap_err(),
        "--resume-session conflicts with --clear-session/--session-info/--clear-tool-state/--rename-session"
    );
}

#[test]
fn rename_session_requires_session_file_and_accepts_equals_form() {
    let missing = Cli::try_parse_from(["wayscriber", "--rename-session", "Lecture"]);
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
    let result = Cli::try_parse_from([
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
            Cli::try_parse_from(args).unwrap_err(),
            "--rename-session conflicts with overlay/daemon options",
            "expected rename with {option:?} to be rejected"
        );
    }
}

#[test]
fn offline_session_commands_conflict_with_each_other() {
    let clear_result = Cli::try_parse_from(["wayscriber", "--clear-tool-state", "--clear-session"]);
    assert_eq!(
        clear_result.unwrap_err(),
        "--clear-tool-state conflicts with --clear-session"
    );

    let info_result = Cli::try_parse_from(["wayscriber", "--clear-tool-state", "--session-info"]);
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
    let cli = parse_cli(["wayscriber", "--daemon-action", "not_real"]);
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

    let result = Cli::try_parse_from(["wayscriber", "--active", "--clear-tool-state"]);
    assert_eq!(
        result.unwrap_err(),
        "--clear-tool-state conflicts with --daemon/--active"
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
            let result = Cli::try_parse_from(["wayscriber", command, flag]);
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
        Cli::try_parse_from(["wayscriber", "--about", "--check-update"]).unwrap_err(),
        "--about conflicts with the selected command"
    );
    assert_eq!(
        Cli::try_parse_from(["wayscriber", "--runtime-capabilities", "--check-update"])
            .unwrap_err(),
        "--runtime-capabilities conflicts with launch flags"
    );
}

#[test]
fn check_update_rejects_valued_launch_flags() {
    assert_eq!(
        Cli::try_parse_from(["wayscriber", "--check-update", "--mode", "whiteboard"]).unwrap_err(),
        "--check-update conflicts with the selected command"
    );
    // `--session-file` has its own earlier "requires a session command" check,
    // so the rejection is the same but the wording comes from there.
    assert!(
        Cli::try_parse_from([
            "wayscriber",
            "--check-update",
            "--session-file",
            "/tmp/x.wayscriber-session",
        ])
        .is_err()
    );
    assert_eq!(
        Cli::try_parse_from([
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
fn language_selects_this_runs_interface_language() {
    use crate::domain::Language;

    for (args, expected) in [
        (
            ["wayscriber", "--active", "--language", "zh-CN"],
            Language::SimplifiedChinese,
        ),
        (
            ["wayscriber", "--daemon", "--language", "en"],
            Language::English,
        ),
        (
            ["wayscriber", "--about", "--language", "auto"],
            Language::Auto,
        ),
        // Every spelling [ui] language accepts.
        (
            ["wayscriber", "--freeze", "--language", "zh"],
            Language::SimplifiedChinese,
        ),
    ] {
        assert_eq!(parse_cli(args).language, Some(expected), "{args:?}");
    }

    let cli = parse_cli(["wayscriber", "--active", "--language=zh-CN"]);
    assert_eq!(cli.language, Some(Language::SimplifiedChinese));
}

#[test]
fn language_rejects_unknown_codes_and_commands_it_cannot_change() {
    let error = |args: &[&str]| match Cli::try_parse_from(args.iter().copied()) {
        Err(error) => error,
        Ok(outcome) => panic!("{args:?} parsed as {outcome:?}"),
    };

    assert_eq!(
        error(&["wayscriber", "--active", "--language", "fr"]),
        "unknown language 'fr'; run 'wayscriber --list-languages' to see the choices"
    );
    assert_eq!(
        error(&["wayscriber", "--active", "--language"]),
        "--language requires a value"
    );
    for args in [
        &["wayscriber", "--daemon-toggle", "--language", "zh-CN"][..],
        &["wayscriber", "--light-toggle", "--language", "zh-CN"],
    ] {
        assert!(
            error(args).starts_with("--language cannot change a running daemon's overlays"),
            "{args:?}"
        );
    }
    for args in [
        &["wayscriber", "--language", "zh-CN"][..],
        &["wayscriber", "--session-info", "--language", "zh-CN"],
        &["wayscriber", "--check-update", "--language", "zh-CN"],
    ] {
        assert_eq!(
            error(args),
            "--language applies to --active, --freeze, --daemon, --about and --list-languages",
            "{args:?}"
        );
    }
    assert_eq!(
        error(&[
            "wayscriber",
            "--runtime-capabilities",
            "--language",
            "zh-CN"
        ]),
        "--runtime-capabilities conflicts with launch flags"
    );
}

#[test]
fn list_languages_is_a_print_and_exit_command() {
    assert!(parse_cli(["wayscriber", "--list-languages"]).list_languages);
    // It shows what --language would select.
    let cli = parse_cli(["wayscriber", "--list-languages", "--language", "zh-CN"]);
    assert!(cli.list_languages && cli.language.is_some());

    for args in [
        ["wayscriber", "--list-languages", "--active"],
        ["wayscriber", "--list-languages", "--about"],
    ] {
        assert_eq!(
            Cli::try_parse_from(args).unwrap_err(),
            "--list-languages conflicts with the selected command",
            "{args:?}"
        );
    }
}

#[test]
fn a_value_starting_with_a_dash_must_be_attached() {
    for args in [
        &["wayscriber", "--mode", "-x"][..],
        &["wayscriber", "-m", "-x"],
        &["wayscriber", "--mode", "--active"],
    ] {
        assert!(
            Cli::try_parse_from(args.iter().copied())
                .unwrap_err()
                .ends_with("requires a value"),
            "{args:?}"
        );
    }

    assert_eq!(
        parse_cli(["wayscriber", "--mode=-x"]).mode.as_deref(),
        Some("-x")
    );
    assert_eq!(
        parse_cli(["wayscriber", "-m=-x"]).mode.as_deref(),
        Some("-x")
    );
    assert_eq!(
        parse_cli(["wayscriber", "-m-x"]).mode.as_deref(),
        Some("-x")
    );
}

#[test]
fn an_empty_attached_value_is_missing_but_a_separate_one_is_not() {
    for (args, name) in [
        (&["wayscriber", "--mode="][..], "--mode"),
        (&["wayscriber", "-m="], "-m"),
        (&["wayscriber", "--language="], "--language"),
    ] {
        assert_eq!(
            Cli::try_parse_from(args.iter().copied()).unwrap_err(),
            format!("{name} requires a value")
        );
    }

    assert_eq!(
        parse_cli(["wayscriber", "--mode", ""]).mode.as_deref(),
        Some("")
    );
}

#[test]
fn unknown_and_misused_options_are_rejected() {
    for (args, expected) in [
        (&["wayscriber", "--nope"][..], "unknown argument '--nope'"),
        (&["wayscriber", "--nope=1"], "unknown argument '--nope=1'"),
        (
            &["wayscriber", "--active=1"],
            "unknown argument '--active=1'",
        ),
        (&["wayscriber", "--help=1"], "unknown argument '--help=1'"),
        (
            &["wayscriber", "--version=1"],
            "unknown argument '--version=1'",
        ),
        (&["wayscriber", "stray"], "unknown argument 'stray'"),
        (&["wayscriber", "-"], "unknown argument '-'"),
        (&["wayscriber", "--", "stray"], "unknown argument 'stray'"),
        (&["wayscriber", "-x"], "unknown short option '-x'"),
        (&["wayscriber", "-dx"], "unknown short option '-x'"),
    ] {
        assert_eq!(
            Cli::try_parse_from(args.iter().copied()).unwrap_err(),
            expected,
            "{args:?}"
        );
    }
}

#[test]
fn a_session_path_keeps_its_bytes_but_other_values_must_be_utf8() {
    let raw_path = OsString::from_vec(b"/tmp/s\xff.wayscriber-session".to_vec());

    let outcome = Cli::try_parse_from([
        OsString::from("wayscriber"),
        OsString::from("--active"),
        OsString::from("--session-file"),
        raw_path.clone(),
    ])
    .unwrap();
    let CliOutcome::Run(cli) = outcome else {
        panic!("expected CLI run outcome, got {outcome:?}");
    };
    assert_eq!(cli.session_file, Some(PathBuf::from(raw_path)));

    let error = Cli::try_parse_from([
        OsString::from("wayscriber"),
        OsString::from("--mode"),
        OsString::from_vec(b"\xff".to_vec()),
    ])
    .unwrap_err();
    assert_eq!(error, "arguments must be valid UTF-8");
}
