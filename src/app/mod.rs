mod env;
mod session;
mod usage;

use crate::backend::ExitAfterCaptureMode;
use crate::cli::Cli;
use crate::daemon::DaemonToggleRequest;
use crate::paths::overlay_lock_file;
use crate::session::try_lock_exclusive;
use crate::session_override::set_runtime_session_override;
use env::env_flag_enabled;
use session::run_session_cli_commands;
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::process::{Command, Stdio};
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
    if env_flag_enabled("WAYSCRIBER_NO_DETACH") || std::env::var_os("WAYSCRIBER_DETACHED").is_some()
    {
        return Ok(false);
    }
    let exe = std::env::current_exe()?;
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .env("WAYSCRIBER_DETACHED", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.spawn()?;
    Ok(true)
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
    #[cfg(unix)]
    if std::env::var_os("WAYSCRIBER_DETACHED").is_some() {
        detach_from_tty();
    }

    let session_override = if cli.resume_session {
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

    if cli.daemon_toggle {
        let request = DaemonToggleRequest {
            mode: cli.mode,
            freeze: cli.freeze,
            exit_after_capture: cli.exit_after_capture,
            no_exit_after_capture: cli.no_exit_after_capture,
            resume_session: cli.resume_session,
            no_resume_session: cli.no_resume_session,
        };
        crate::daemon::send_daemon_toggle_request(&request)?;
        return Ok(());
    }

    if cli.clear_session || cli.session_info {
        run_session_cli_commands(&cli)?;
        return Ok(());
    }

    // Check for Wayland environment
    if std::env::var("WAYLAND_DISPLAY").is_err() && (cli.daemon || cli.active) {
        log::error!("WAYLAND_DISPLAY not set - this application requires Wayland.");
        log::error!("Please run on a Wayland compositor (Hyprland, Sway, etc.).");
        return Err(anyhow::anyhow!("Wayland environment required"));
    }

    if cli.daemon {
        // Daemon mode: background service with toggle activation
        log::info!("Starting in daemon mode");
        let tray_disabled = cli.no_tray || env_flag_enabled("WAYSCRIBER_NO_TRAY");
        if tray_disabled {
            log::info!("Tray disabled via --no-tray / WAYSCRIBER_NO_TRAY");
        }
        let mut daemon = crate::daemon::Daemon::new(cli.mode, !tray_disabled, session_override);
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
        crate::backend::run_wayland(cli.mode, cli.freeze, exit_after_capture_mode)?;

        log::info!("Annotation overlay closed.");
    } else {
        // No flags: show usage
        print_usage();
    }

    Ok(())
}
