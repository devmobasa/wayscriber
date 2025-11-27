use clap::{ArgAction, Parser};

mod backend;
mod capture;
mod config;
mod daemon;
mod draw;
mod input;
mod notification;
mod session;
mod ui;
mod util;

#[derive(Parser, Debug)]
#[command(name = "wayscriber")]
#[command(version, about = "Screen annotation tool for Wayland compositors")]
struct Cli {
    /// Run as daemon (background service; bind a toggle like Super+D)
    #[arg(long, short = 'd', action = ArgAction::SetTrue)]
    daemon: bool,

    /// Start active (show overlay immediately, one-shot mode)
    #[arg(long, short = 'a', action = ArgAction::SetTrue)]
    active: bool,

    /// Initial board mode (transparent, whiteboard, or blackboard)
    #[arg(long, short = 'm', value_name = "MODE")]
    mode: Option<String>,

    /// Delete persisted session data and backups
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = [
            "daemon",
            "active",
        ]
    )]
    clear_session: bool,

    /// Show session persistence status and file paths
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = [
            "daemon",
            "active",
            "clear_session"
        ]
    )]
    session_info: bool,

    /// Start with frozen mode active (freeze the screen immediately)
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = ["daemon", "clear_session", "session_info"]
    )]
    freeze: bool,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cli = Cli::parse();

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
        let mut daemon = daemon::Daemon::new(cli.mode);
        daemon.run()?;
    } else if cli.active || cli.freeze {
        // One-shot mode: show overlay immediately and exit when done
        log::info!("Starting Wayland overlay...");
        log::info!("Starting annotation overlay...");
        log::info!("Controls:");
        log::info!("  - Freehand: Just drag");
        log::info!("  - Line: Hold Shift + drag");
        log::info!("  - Rectangle: Hold Ctrl + drag");
        log::info!("  - Ellipse: Hold Tab / Ctrl+Alt + drag");
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
        log::info!("  - Context menu: Right Click or Shift+F10");
        log::info!(
            "  - Help: F1/F10   •   Toolbar: F2/F9   •   Configurator: F11   •   Status bar: F4/F12"
        );
        log::info!("  - Exit: Escape");
        if cli.freeze {
            log::info!("Starting frozen mode (freeze-on-start requested)");
        }
        log::info!("");

        // Run Wayland backend
        backend::run_wayland(cli.mode, cli.freeze)?;

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

fn run_session_cli_commands(cli: &Cli) -> anyhow::Result<()> {
    let loaded = config::Config::load()?;
    let config_dir = config::Config::config_directory_from_source(&loaded.source)?;
    let display_env = std::env::var("WAYLAND_DISPLAY").ok();

    let options =
        session::options_from_config(&loaded.config.session, &config_dir, display_env.as_deref())?;

    if cli.clear_session {
        let outcome = session::clear_session(&options)?;
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
        use chrono::{DateTime, Local};

        let inspection = session::inspect_session(&options)?;
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
            if let Some(modified) = inspection.modified {
                let dt: DateTime<Local> = modified.into();
                println!("    Modified : {}", dt.format("%Y-%m-%d %H:%M:%S"));
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
    use super::Cli;
    use clap::Parser;

    #[test]
    fn active_mode_with_explicit_board_mode() {
        let cli = Cli::try_parse_from(["wayscriber", "--active", "--mode", "whiteboard"]).unwrap();
        assert!(cli.active);
        assert_eq!(cli.mode.as_deref(), Some("whiteboard"));
    }
}
