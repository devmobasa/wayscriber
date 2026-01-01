mod env;
mod session;
mod usage;

use clap::Parser;

use crate::backend::ExitAfterCaptureMode;
use crate::cli::Cli;
use crate::session_override::set_runtime_session_override;
use env::env_flag_enabled;
use session::run_session_cli_commands;
use usage::{log_overlay_controls, print_usage};

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
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
        daemon.run()?;
    } else if cli.active || cli.freeze {
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
