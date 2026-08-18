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
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    crate::process_broker::current()?.spawn(
        crate::process_broker::HelperKind::InitialDetach,
        crate::process_broker::HelperLifetime::DetachedAfterExec,
        exe.as_os_str(),
        args,
        vec![(DETACHED_ENV.into(), Some("1".into()))],
    )?;
    Ok(true)
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
    unsafe {
        let _ = libc::setsid();
    }
    // Best-effort close of stdio if they still point to a TTY.
    for fd in [libc::STDIN_FILENO, libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        let is_tty = unsafe { libc::isatty(fd) } == 1;
        if is_tty {
            let _ = unsafe { libc::close(fd) };
        }
    }
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
}
