use clap::{ArgAction, Parser};

use crate::config::{MigrationActions, MigrationReport};

mod backend;
mod capture;
mod config;
mod daemon;
mod draw;
mod input;
mod legacy;
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

    /// Copy configuration files from ~/.config/hyprmarker to ~/.config/wayscriber
    #[arg(long, action = ArgAction::SetTrue)]
    migrate_config: bool,

    /// Preview the migration without copying files (requires --migrate-config)
    #[arg(long = "dry-run", action = ArgAction::SetTrue, requires = "migrate_config")]
    migrate_config_dry_run: bool,

    /// Delete persisted session data and backups
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = [
            "daemon",
            "active",
            "migrate_config",
            "migrate_config_dry_run"
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
            "migrate_config",
            "migrate_config_dry_run",
            "clear_session"
        ]
    )]
    session_info: bool,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    maybe_print_alias_notice();

    if cli.clear_session || cli.session_info {
        run_session_cli_commands(&cli)?;
        return Ok(());
    }

    if cli.migrate_config {
        run_config_migration(cli.migrate_config_dry_run)?;
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
    } else if cli.active {
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
        log::info!("  - Undo: Ctrl+Z");
        log::info!("  - Clear all: E");
        log::info!("  - Increase thickness: + or = or scroll down");
        log::info!("  - Decrease thickness: - or _ or scroll up");
        log::info!("  - Help: F10");
        log::info!("  - Exit: Escape");
        log::info!("");

        // Run Wayland backend
        backend::run_wayland(cli.mode)?;

        log::info!("Annotation overlay closed.");
    } else {
        // No flags: show usage
        println!("wayscriber: Screen annotation tool for Wayland compositors");
        println!();
        println!("Usage:");
        println!(
            "  wayscriber -d, --daemon    Run as background daemon (bind a toggle like Super+D)"
        );
        println!("  wayscriber -a, --active    Show overlay immediately (one-shot mode)");
        println!("  wayscriber -h, --help      Show help");
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

fn run_config_migration(dry_run: bool) -> anyhow::Result<()> {
    let report = config::migrate_config(dry_run)?;
    print_migration_report(&report);
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
        println!("  Restore tool state : {}", inspection.restore_tool_state);
        println!(
            "  Session file       : {}",
            inspection.session_path.display()
        );
        if inspection.exists {
            if let Some(size) = inspection.size_bytes {
                println!("    Size     : {} bytes", size);
            }
            if let Some(modified) = inspection.modified {
                let dt: DateTime<Local> = modified.into();
                println!("    Modified : {}", dt.format("%Y-%m-%d %H:%M:%S"));
            }
            println!("    Compressed: {}", inspection.compressed);
            if let Some(counts) = inspection.frame_counts {
                println!(
                    "    Shapes   : transparent {}, whiteboard {}, blackboard {}",
                    counts.transparent, counts.whiteboard, counts.blackboard
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

fn print_migration_report(report: &MigrationReport) {
    match &report.actions {
        MigrationActions::NoLegacyConfig => {
            println!(
                "No legacy hyprmarker configuration found at {}. Nothing to migrate.",
                report.legacy_dir.display()
            );
        }
        MigrationActions::DryRun {
            target_exists,
            files_to_copy,
        } => {
            println!(
                "Dry-run: would copy {} {} from {} to {}.",
                files_to_copy,
                pluralize(*files_to_copy, "file", "files"),
                report.legacy_dir.display(),
                report.target_dir.display()
            );

            if *target_exists {
                println!("An existing Wayscriber config would be backed up before copying.");
            }

            println!(
                "Run without --dry-run to perform the migration. See docs/MIGRATION.md for the full checklist."
            );
        }
        MigrationActions::Migrated {
            target_existed,
            files_copied,
            backup_path,
        } => {
            println!(
                "Copied {} {} from {} to {}.",
                files_copied,
                pluralize(*files_copied, "file", "files"),
                report.legacy_dir.display(),
                report.target_dir.display()
            );

            if let Some(path) = backup_path {
                println!(
                    "Existing Wayscriber config was moved to {} before copying.",
                    path.display()
                );
            } else if *target_existed {
                println!("Existing Wayscriber config was overwritten after creating a backup.");
            }

            println!(
                "Legacy files remain at {}. Remove them when you are comfortable.",
                report.legacy_dir.display()
            );
            println!("See docs/MIGRATION.md for next steps.");
        }
    }
}

fn maybe_print_alias_notice() {
    if let Some(alias) = legacy::alias_invocation() {
        if legacy::warnings_suppressed() {
            return;
        }

        eprintln!("{alias} has been renamed to wayscriber.");
        eprintln!("Update your shortcuts and scripts to invoke `wayscriber` directly.");
        eprintln!("Run `wayscriber --migrate-config` to copy existing settings.");
        eprintln!("Set HYPRMARKER_SILENCE_RENAME=1 to silence this warning during scripted runs.");
    }
}

fn pluralize<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn dry_run_requires_migrate_flag() {
        let result = Cli::try_parse_from(["wayscriber", "--dry-run"]);
        assert!(result.is_err(), "dry-run without migrate should fail");
    }

    #[test]
    fn migrate_with_dry_run_parses_successfully() {
        let cli = Cli::try_parse_from(["wayscriber", "--migrate-config", "--dry-run"]).unwrap();
        assert!(cli.migrate_config);
        assert!(cli.migrate_config_dry_run);
    }

    #[test]
    fn active_mode_with_explicit_board_mode() {
        let cli = Cli::try_parse_from(["wayscriber", "--active", "--mode", "whiteboard"]).unwrap();
        assert!(cli.active);
        assert_eq!(cli.mode.as_deref(), Some("whiteboard"));
    }
}
