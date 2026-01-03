pub(crate) fn log_overlay_controls(freeze: bool) {
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
        "  - Help: F1/F10   •   Toolbar: F2/F9   •   Presenter: Ctrl+Shift+K   •   Configurator: F11   •   Status bar: F4/F12"
    );
    log::info!("  - Exit: Escape");
    if freeze {
        log::info!("Starting frozen mode (freeze-on-start requested)");
    }
    log::info!("");
}

pub(crate) fn print_usage() {
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
