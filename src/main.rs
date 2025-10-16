use clap::{ArgAction, Parser};

mod backend;
mod capture;
mod config;
mod daemon;
mod draw;
mod input;
mod notification;
mod ui;
mod util;

#[derive(Parser, Debug)]
#[command(name = "hyprmarker")]
#[command(version, about = "Screen annotation tool for Wayland compositors")]
struct Cli {
    /// Run as daemon (background, toggle with Super+D)
    #[arg(long, short = 'd', action = ArgAction::SetTrue)]
    daemon: bool,

    /// Start active (show overlay immediately, one-shot mode)
    #[arg(long, short = 'a', action = ArgAction::SetTrue)]
    active: bool,

    /// Initial board mode (transparent, whiteboard, or blackboard)
    #[arg(long, short = 'm', value_name = "MODE")]
    mode: Option<String>,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cli = Cli::parse();

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
        println!("hyprmarker: Screen annotation tool for Wayland compositors");
        println!();
        println!("Usage:");
        println!("  hyprmarker --daemon    Run as background daemon (toggle with Super+D)");
        println!("  hyprmarker --active    Show overlay immediately (one-shot mode)");
        println!("  hyprmarker --help      Show help");
        println!();
        println!("Daemon mode (recommended):");
        println!("  1. Run: hyprmarker --daemon");
        println!("  2. Add to Hyprland config:");
        println!("     exec-once = hyprmarker --daemon");
        println!("     bind = SUPER, D, exec, pkill -SIGUSR1 hyprmarker");
        println!("  3. Press Super+D to toggle overlay on/off");
        println!();
        println!("Requirements:");
        println!("  - Wayland compositor (Hyprland, Sway, etc.)");
        println!("  - wlr-layer-shell protocol support");
    }

    Ok(())
}
