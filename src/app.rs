use clap::Parser;

use crate::backend::ExitAfterCaptureMode;
use crate::cli::Cli;
use crate::session_override::set_runtime_session_override;

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
        log::info!("Starting Wayland overlay...");
        log::info!("Starting annotation overlay...");
        log::info!("Controls:");
        log::info!("  - Freehand: Just drag");
        log::info!("  - Line: Hold Shift + drag");
        log::info!("  - Rectangle: Hold Ctrl + drag");
        log::info!("  - Ellipse: Hold Tab + drag");
        log::info!("  - Arrow: Hold Ctrl+Shift + drag");
        log::info!("  - Text: Press T, click to position, type, press Enter");
        log::info!(
            "  - Colors: R (red), G (green), B (blue), Y (yellow), O (orange), P (pink), W (white), K (black)"
        );
        log::info!("  - Undo / Redo: Ctrl+Z / Ctrl+Shift+Z");
        log::info!("  - Clear all: E");
        log::info!("  - Increase thickness: + or = or scroll down");
        log::info!("  - Decrease thickness: - or _ or scroll up");
        log::info!("  - Freeze screen: Ctrl+Shift+F (toggle frozen background)");
        log::info!(
            "  - Zoom: Ctrl+Alt + scroll or +/- (Ctrl+Alt+0 reset, Ctrl+Alt+L lock, middle drag/arrow keys to pan)"
        );
        log::info!("  - Context menu: Right Click or Shift+F10");
        log::info!(
            "  - Help: F1/F10   •   Toolbar: F2/F9   •   Configurator: F11   •   Status bar: F4/F12"
        );
        log::info!("  - Exit: Escape");
        if cli.freeze {
            log::info!("Starting frozen mode (freeze-on-start requested)");
        }
        log::info!("");

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
        println!("wayscriber: Screen annotation tool for Wayland compositors");
        println!();
        println!("Usage:");
        println!(
            "  wayscriber -d, --daemon      Run as background daemon (bind a toggle like Super+D)"
        );
        println!("  wayscriber -a, --active      Show overlay immediately (one-shot mode)");
        println!("  wayscriber --freeze          Start overlay already frozen");
        println!(
            "  wayscriber --exit-after-capture  Exit overlay after a capture completes (override auto clipboard exit)"
        );
        println!(
            "  wayscriber --no-exit-after-capture  Keep overlay open (disable auto clipboard exit)"
        );
        println!("  wayscriber --no-tray         Skip system tray (headless daemon)");
        println!("  wayscriber --about           Show the About window");
        println!(
            "  wayscriber --resume-session  Force session resume on (all boards/history/tool state)"
        );
        println!("  wayscriber --no-resume-session  Disable session resume for this run");
        println!("  wayscriber -h, --help        Show help");
        println!();
        println!("Daemon mode (recommended). Example Hyprland setup:");
        println!("  1. Run: wayscriber --daemon");
        println!("  2. Add to Hyprland config:");
        println!("     exec-once = wayscriber --daemon");
        println!("     bind = SUPER, D, exec, pkill -SIGUSR1 wayscriber");
        println!("  3. Press your bound shortcut (e.g. Super+D) to toggle overlay on/off");
        println!();
        println!("Requirements:");
        println!("  - Wayland compositor (Hyprland, Sway, etc.)");
        println!("  - wlr-layer-shell protocol support");
    }

    Ok(())
}

fn env_flag_enabled(name: &str) -> bool {
    if let Ok(val) = std::env::var(name) {
        matches!(
            val.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    } else {
        false
    }
}

fn run_session_cli_commands(cli: &Cli) -> anyhow::Result<()> {
    let loaded = crate::config::Config::load()?;
    let config_dir = crate::config::Config::config_directory_from_source(&loaded.source)?;
    let display_env = std::env::var("WAYLAND_DISPLAY").ok();

    let options = crate::session::options_from_config(
        &loaded.config.session,
        &config_dir,
        display_env.as_deref(),
    )?;

    if cli.clear_session {
        let outcome = crate::session::clear_session(&options)?;
        println!("Session file: {}", options.session_file_path().display());
        if outcome.removed_session {
            println!("  Removed session file");
        } else {
            println!("  No session file present");
        }
        if outcome.removed_backup {
            println!("  Removed backup file");
        }
        if outcome.removed_lock {
            println!("  Removed lock file");
        }
        if !outcome.removed_session && !outcome.removed_backup && !outcome.removed_lock {
            println!("  No session artefacts found");
        }
        return Ok(());
    }

    if cli.session_info {
        let inspection = crate::session::inspect_session(&options)?;
        println!("Session persistence status:");
        println!("  Persist transparent: {}", inspection.persist_transparent);
        println!("  Persist whiteboard : {}", inspection.persist_whiteboard);
        println!("  Persist blackboard : {}", inspection.persist_blackboard);
        println!("  Persist history    : {}", inspection.persist_history);
        if inspection.persist_history {
            match inspection.history_limit {
                Some(limit) => println!("    Max persisted undo depth: {}", limit),
                None => println!("    Max persisted undo depth: follows runtime limit"),
            }
        } else {
            println!("    (history disabled; only visible drawings are saved)");
        }
        println!("  Restore tool state : {}", inspection.restore_tool_state);
        println!("  Per-output persistence: {}", inspection.per_output);
        println!(
            "  Session file       : {}",
            inspection.session_path.display()
        );
        if let Some(identity) = &inspection.active_identity {
            println!("    Output identity: {}", identity);
        }
        if inspection.exists {
            if let Some(size) = inspection.size_bytes {
                println!("    Size     : {} bytes", size);
            }
            if let Some(ts) = inspection
                .modified
                .and_then(|m| crate::time_utils::format_system_time(m, "%Y-%m-%d %H:%M:%S"))
            {
                println!("    Modified : {}", ts);
            }
            println!("    Compressed: {}", inspection.compressed);
            if let Some(version) = inspection.file_version {
                println!("    File version: {}", version);
            }
            if let Some(counts) = inspection.frame_counts {
                println!(
                    "    Shapes   : transparent {}, whiteboard {}, blackboard {}",
                    counts.transparent, counts.whiteboard, counts.blackboard
                );
            }
            println!("    History present: {}", inspection.history_present);
            if let Some(hist) = &inspection.history_counts {
                println!(
                    "    History (undo/redo): transparent {} / {}, whiteboard {} / {}, blackboard {} / {}",
                    hist.transparent.undo,
                    hist.transparent.redo,
                    hist.whiteboard.undo,
                    hist.whiteboard.redo,
                    hist.blackboard.undo,
                    hist.blackboard.redo
                );
            }
            println!("    Tool state stored: {}", inspection.tool_state_present);
        } else {
            println!("    (not found)");
        }

        println!("  Backup file       : {}", inspection.backup_path.display());
        if inspection.backup_exists {
            if let Some(size) = inspection.backup_size_bytes {
                println!("    Size     : {} bytes", size);
            }
        } else {
            println!("    (not found)");
        }

        println!("  Storage directory : {}", options.base_dir.display());

        return Ok(());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::env_flag_enabled;
    use std::env;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn env_flag_enabled_accepts_truthy_values() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        for value in ["1", "true", "yes", "on", "TrUe"] {
            // SAFETY: serialized via ENV_MUTEX
            unsafe {
                env::set_var("WAYSCRIBER_TEST_FLAG", value);
            }
            assert!(
                env_flag_enabled("WAYSCRIBER_TEST_FLAG"),
                "expected '{value}' to be treated as truthy"
            );
        }

        unsafe {
            env::remove_var("WAYSCRIBER_TEST_FLAG");
        }
    }

    #[test]
    fn env_flag_enabled_rejects_non_truthy_values() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        for value in ["0", "false", "no", "off", "", "random"] {
            // SAFETY: serialized via ENV_MUTEX
            unsafe {
                env::set_var("WAYSCRIBER_TEST_FLAG", value);
            }
            assert!(
                !env_flag_enabled("WAYSCRIBER_TEST_FLAG"),
                "expected '{value}' to be treated as falsey"
            );
        }

        unsafe {
            env::remove_var("WAYSCRIBER_TEST_FLAG");
        }
    }
}
