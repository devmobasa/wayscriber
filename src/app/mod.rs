mod env;
mod session;
mod usage;

use crate::backend::ExitAfterCaptureMode;
use crate::cli::Cli;
use crate::daemon::DaemonToggleRequest;
use crate::env_vars::{DETACHED_ENV, NO_DETACH_ENV, NO_TRAY_ENV, WAYLAND_DISPLAY_ENV};
use crate::paths::overlay_lock_file;
use crate::session::try_lock_exclusive;
use crate::session_override::set_runtime_session_override;
use anyhow::Context;
use env::env_flag_enabled;
use session::run_session_cli_commands;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use usage::{log_overlay_controls, print_usage};

fn acquire_overlay_lock() -> anyhow::Result<Option<File>> {
    let lock_path = overlay_lock_file();
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)?;

    match try_lock_exclusive(&lock_file) {
        Ok(()) => Ok(Some(lock_file)),
        Err(err) if err.kind() == ErrorKind::WouldBlock => {
            log::warn!("Overlay already running; skipping duplicate --active launch");
            Ok(None)
        }
        Err(err) => Err(err.into()),
    }
}

fn maybe_detach_active(cli: &Cli) -> anyhow::Result<bool> {
    if !(cli.active || cli.freeze) {
        return Ok(false);
    }
    if env_flag_enabled(NO_DETACH_ENV) || std::env::var_os(DETACHED_ENV).is_some() {
        return Ok(false);
    }
    let exe = std::env::current_exe()?;
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    crate::process_broker::current()?.spawn(
        crate::process_broker::HelperKind::InitialDetach,
        crate::process_broker::HelperLifetime::DetachedAfterExec,
        exe.as_os_str(),
        args,
        detach_environment(crate::launch_environment::var_os),
    )?;
    Ok(true)
}

/// The detached relaunch's environment: the detached marker and the
/// startup-notification variables this process was launched with. The broker
/// relaunches with the live environment, from which a linked GTK may already
/// have unset the startup token, so the launch values are forwarded.
fn detach_environment(
    launched_with: impl Fn(&str) -> Option<OsString>,
) -> Vec<(OsString, Option<OsString>)> {
    let mut environment = vec![(DETACHED_ENV.into(), Some("1".into()))];

    for name in crate::launch_environment::STARTUP_NOTIFICATION_VARIABLES {
        if let Some(value) = launched_with(name) {
            environment.push((name.into(), Some(value)));
        }
    }

    environment
}

fn normalized_named_session_file(cli: &Cli) -> anyhow::Result<Option<PathBuf>> {
    let Some(raw_path) = cli.session_file.as_ref() else {
        return Ok(None);
    };
    let raw = raw_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("--session-file path must be valid UTF-8"))?;
    Ok(Some(crate::session::normalize_named_session_file_arg(raw)))
}

fn daemon_request_session_file(path: Option<PathBuf>) -> anyhow::Result<Option<PathBuf>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let current_dir = std::env::current_dir()
        .context("failed to resolve current directory for daemon session file")?;
    Ok(Some(anchor_session_file_for_daemon_request(
        path,
        &current_dir,
    )))
}

fn anchor_session_file_for_daemon_request(path: PathBuf, current_dir: &Path) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    }
}

fn preflight_named_overlay_session(cli: &Cli, path: Option<&Path>) -> anyhow::Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    if cli.active || cli.freeze || cli.daemon || cli.daemon_toggle {
        crate::session::validate_named_session_file_for_foreground(path)?;
    }
    if cli.active || cli.freeze {
        crate::backend::preflight_wayland_connection()?;
    }
    Ok(())
}

/// `--check-update`: an explicit, user-initiated check. It bypasses the
/// `[updates] check` setting and the opt-out variable — asking for a check is
/// consent to make the request — and never changes anything on disk beyond the
/// cached result.
fn run_update_check() -> anyhow::Result<()> {
    use crate::update_check::{
        COMPILED_OUT_MESSAGE, CheckOutcome, check_now, compiled_out, current_version,
    };

    println!("Installed version: {}", current_version());
    if compiled_out() {
        println!("{COMPILED_OUT_MESSAGE}; ask your package manager for updates.");
        return Ok(());
    }
    match check_now() {
        Ok(CheckOutcome::UpToDate { latest }) => {
            println!("Latest release:    {latest}");
            println!("Wayscriber is up to date.");
            Ok(())
        }
        Ok(CheckOutcome::Update(update)) => {
            println!("Latest release:    {}", update.version);
            if let Some(released) = update.released.as_deref() {
                println!("Released:          {released}");
            }
            println!();
            println!("An update is available. Wayscriber does not install updates itself.");
            println!("Update instructions: {}", update.update_url);
            println!("Release notes:       {}", update.notes_url);
            Ok(())
        }
        Err(err) => Err(anyhow::anyhow!("Update check failed: {err}")),
    }
}

#[cfg(unix)]
fn detach_from_tty() {
    // Start a new session to drop the controlling terminal (prevents stuck shells).
    // SAFETY: setsid takes no arguments; a failure only leaves the session as is.
    unsafe {
        let _ = libc::setsid();
    }
    // Point stdio that still refers to a TTY at /dev/null. Closing it instead
    // frees the descriptor number for the next open, and the logger's stderr
    // writes would then land in whatever file that open returned.
    for fd in [libc::STDIN_FILENO, libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        // SAFETY: isatty only inspects the descriptor number it is given.
        let is_tty = unsafe { libc::isatty(fd) } == 1;
        if is_tty && let Err(error) = redirect_to_dev_null(fd) {
            log::warn!("Failed to detach descriptor {fd} from the terminal: {error}");
        }
    }
}

/// Replaces `fd` with a descriptor for /dev/null, keeping the number in use.
#[cfg(unix)]
fn redirect_to_dev_null(fd: std::os::fd::RawFd) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

    let null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")?;
    // SAFETY: both descriptors are open. dup2 atomically replaces `fd` and
    // leaves `null` open for its owner to close.
    if unsafe { libc::dup2(null.as_raw_fd(), fd) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    if cli.runtime_capabilities {
        print!(
            "{}",
            crate::runtime_capabilities::render_runtime_capabilities(
                crate::runtime_capabilities::current_runtime_capabilities()
            )
        );
        return Ok(());
    }

    // Runtime and update-checking modes create their process broker before
    // acquiring locks or starting threads. The guard spans the complete run.
    let _process_broker = needs_process_broker(&cli)
        .then(crate::process_broker::start_for_runtime)
        .transpose()?;
    crate::daemon::protocol_v2::start_daemon_watchdog_from_environment()?;

    #[cfg(unix)]
    if std::env::var_os(DETACHED_ENV).is_some() {
        detach_from_tty();
    }

    let named_session_file = normalized_named_session_file(&cli)?;
    preflight_named_overlay_session(&cli, named_session_file.as_deref())?;

    let named_overlay_session =
        named_session_file.is_some() && (cli.active || cli.freeze || cli.daemon);
    let session_override = if named_overlay_session || cli.resume_session {
        Some(true)
    } else if cli.no_resume_session {
        Some(false)
    } else {
        None
    };

    if cli.about {
        crate::about_window::run_about_window()?;
        return Ok(());
    }

    if cli.check_update {
        return run_update_check();
    }

    if let Some(action) = cli
        .daemon_overlay_action()
        .map_err(|err| anyhow::anyhow!(err))?
    {
        crate::daemon::send_daemon_overlay_action(action)?;
        return Ok(());
    }

    if cli.daemon_toggle {
        let session_file = daemon_request_session_file(named_session_file)?;
        let request = DaemonToggleRequest {
            mode: cli.mode,
            freeze: cli.freeze,
            exit_after_capture: cli.exit_after_capture,
            no_exit_after_capture: cli.no_exit_after_capture,
            resume_session: cli.resume_session,
            no_resume_session: cli.no_resume_session,
            session_file,
            overlay_action: None,
        };
        crate::daemon::send_daemon_toggle_request(&request)?;
        return Ok(());
    }

    if cli.clear_session || cli.clear_tool_state || cli.session_info || cli.rename_session.is_some()
    {
        run_session_cli_commands(&cli)?;
        return Ok(());
    }

    // Check for Wayland environment
    if std::env::var(WAYLAND_DISPLAY_ENV).is_err() && (cli.daemon || cli.active || cli.freeze) {
        return Err(anyhow::anyhow!(
            "{WAYLAND_DISPLAY_ENV} not set - this application requires Wayland."
        ));
    }

    if cli.daemon {
        // Daemon mode: background service with toggle activation
        log::info!("Starting in daemon mode");
        let tray_disabled = cli.no_tray || env_flag_enabled(NO_TRAY_ENV);
        if tray_disabled {
            log::info!("Tray disabled via --no-tray / {NO_TRAY_ENV}");
        }
        let mut daemon = crate::daemon::Daemon::new(
            cli.mode,
            !tray_disabled,
            session_override,
            named_session_file,
        );
        daemon.set_freeze_on_show(cli.freeze_on_show);
        daemon.run()?;
    } else if cli.active || cli.freeze {
        if maybe_detach_active(&cli)? {
            return Ok(());
        }
        let _overlay_lock = match acquire_overlay_lock()? {
            Some(lock) => lock,
            None => return Ok(()),
        };
        crate::daemon::protocol_v2::publish_ready_from_environment()
            .context("failed to publish daemon overlay readiness")?;
        // One-shot mode: show overlay immediately and exit when done
        log_overlay_controls(cli.freeze);

        set_runtime_session_override(session_override);

        let exit_after_capture_mode = if cli.exit_after_capture {
            ExitAfterCaptureMode::Always
        } else if cli.no_exit_after_capture {
            ExitAfterCaptureMode::Never
        } else {
            ExitAfterCaptureMode::Auto
        };

        // Run Wayland backend
        crate::backend::run_wayland(
            cli.mode,
            cli.freeze,
            exit_after_capture_mode,
            named_session_file,
        )?;

        log::info!("Annotation overlay closed.");
    } else {
        // No flags: show usage
        print_usage();
    }

    Ok(())
}

fn needs_process_broker(cli: &Cli) -> bool {
    cli.daemon
        || cli.active
        || cli.freeze
        // About's URL and clipboard helpers need the broker even when network
        // update checks were compiled out of this build.
        || cli.about
        || (cli.check_update && !crate::update_check::compiled_out())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_always_starts_the_process_broker() {
        assert!(needs_process_broker(&Cli {
            about: true,
            ..Cli::default()
        }));
    }

    #[test]
    fn print_only_mode_does_not_start_the_process_broker() {
        assert!(!needs_process_broker(&Cli {
            runtime_capabilities: true,
            ..Cli::default()
        }));
    }

    #[test]
    fn daemon_request_session_file_anchors_relative_paths_to_caller_directory() {
        let anchored = anchor_session_file_for_daemon_request(
            PathBuf::from("meeting.wayscriber-session"),
            Path::new("/tmp/wayscriber-caller"),
        );

        assert_eq!(
            anchored,
            PathBuf::from("/tmp/wayscriber-caller/meeting.wayscriber-session")
        );
    }

    #[test]
    fn daemon_request_session_file_preserves_absolute_paths() {
        let path = PathBuf::from("/tmp/meeting.wayscriber-session");
        let anchored =
            anchor_session_file_for_daemon_request(path.clone(), Path::new("/tmp/other-cwd"));

        assert_eq!(anchored, path);
    }

    #[cfg(unix)]
    #[test]
    fn detached_descriptor_keeps_its_number_and_writes_go_nowhere() {
        use std::io::Write;
        use std::os::fd::AsRawFd;

        let temp = crate::test_temp::tempdir().unwrap();
        let path = temp.path().join("was-a-terminal");
        let mut file = File::create(&path).unwrap();
        file.write_all(b"before ").unwrap();

        redirect_to_dev_null(file.as_raw_fd()).unwrap();
        file.write_all(b"after").unwrap();

        // SAFETY: F_GETFD only inspects the descriptor the file still owns.
        assert!(unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) } >= 0);
        assert_eq!(std::fs::read(&path).unwrap(), b"before ");
    }

    /// Names the report file, and so marks the detached child run, of the
    /// relaunch test below.
    const DETACH_CHILD_REPORT_ENV: &str = "WAYSCRIBER_TEST_DETACH_CHILD_REPORT";
    const DETACH_LAUNCH_TOKEN: &str = "detach-launch-token";

    fn launched_with_token(name: &str) -> Option<OsString> {
        (name == crate::env_vars::XDG_ACTIVATION_TOKEN_ENV).then(|| DETACH_LAUNCH_TOKEN.into())
    }

    #[test]
    fn the_detached_relaunch_forwards_the_launch_startup_notification() {
        let detached = (OsString::from(DETACHED_ENV), Some(OsString::from("1")));
        let token = (
            OsString::from(crate::env_vars::XDG_ACTIVATION_TOKEN_ENV),
            Some(OsString::from(DETACH_LAUNCH_TOKEN)),
        );

        assert_eq!(
            detach_environment(launched_with_token),
            vec![detached.clone(), token]
        );
        assert_eq!(detach_environment(|_| None), vec![detached]);
    }

    /// The broker relaunches with the live environment, which here lacks the
    /// startup variables, as it does once GTK has unset them. Only a forwarded
    /// token reaches the detached process.
    #[test]
    fn a_detached_relaunch_keeps_the_launch_token_only_when_forwarded() {
        if let Some(report) = std::env::var_os(DETACH_CHILD_REPORT_ENV) {
            let token = crate::launch_environment::startup_activation_token().unwrap_or_default();
            crate::durable_io::write_atomic(
                Path::new(&report),
                token.as_bytes(),
                crate::durable_io::AtomicWriteOptions::private_runtime_file(),
            )
            .unwrap();
            return;
        }

        let temp = crate::test_temp::tempdir().unwrap();
        let report = temp.path().join("detached-token");
        let test_name = concat!(
            module_path!(),
            "::a_detached_relaunch_keeps_the_launch_token_only_when_forwarded"
        )
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .expect("test path contains the crate prefix");
        let removed = crate::launch_environment::STARTUP_NOTIFICATION_VARIABLES
            .map(|name| (name, None::<&std::ffi::OsStr>));
        let mut variables = vec![(DETACH_CHILD_REPORT_ENV, Some(report.as_os_str()))];
        variables.extend(removed);

        crate::test_env::with_env_vars(&variables, || {
            let guard = crate::process_broker::start_for_runtime().unwrap();
            let relaunch = |environment| {
                let _ = std::fs::remove_file(&report);
                guard
                    .broker()
                    .spawn(
                        crate::process_broker::HelperKind::InitialDetach,
                        crate::process_broker::HelperLifetime::DetachedAfterExec,
                        std::env::current_exe().unwrap().as_os_str(),
                        [test_name, "--exact", "--test-threads=1"],
                        environment,
                    )
                    .unwrap();

                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                loop {
                    if let Ok(token) = std::fs::read_to_string(&report) {
                        break token;
                    }
                    assert!(
                        std::time::Instant::now() < deadline,
                        "the detached child did not report"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            };

            let unforwarded = relaunch(vec![(DETACHED_ENV.into(), Some("1".into()))]);
            let forwarded = relaunch(detach_environment(launched_with_token));

            assert_eq!(unforwarded, "");
            assert_eq!(forwarded, DETACH_LAUNCH_TOKEN);
        });
    }
}
