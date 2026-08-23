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
fn hidden_pin_host_mode_parses_only_by_itself() {
    let cli = parse_cli(["wayscriber", "--pin-host"]);
    assert!(cli.pin_host);

    for public in ["--active", "--daemon", "--about", "--check-update"] {
        assert!(Cli::try_parse_from(["wayscriber", "--pin-host", public]).is_err());
    }
}
