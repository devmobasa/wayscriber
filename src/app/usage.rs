use std::collections::HashMap;

use crate::config::{Action, KeyBinding, KeybindingsConfig, action_display_label, action_label};

const NOT_BOUND_LABEL: &str = "Not bound";

fn default_action_bindings() -> HashMap<Action, Vec<KeyBinding>> {
    match KeybindingsConfig::default().build_action_bindings() {
        Ok(bindings) => bindings,
        Err(err) => {
            log::warn!("Failed to build default keybindings: {}", err);
            HashMap::new()
        }
    }
}

fn action_binding_labels(
    bindings: &HashMap<Action, Vec<KeyBinding>>,
    action: Action,
) -> Vec<String> {
    bindings
        .get(&action)
        .map(|list| list.iter().map(ToString::to_string).collect())
        .unwrap_or_default()
}

fn action_binding_label(bindings: &HashMap<Action, Vec<KeyBinding>>, action: Action) -> String {
    let labels = action_binding_labels(bindings, action);
    if labels.is_empty() {
        NOT_BOUND_LABEL.to_string()
    } else {
        labels.join(" / ")
    }
}

fn action_primary_binding_label(
    bindings: &HashMap<Action, Vec<KeyBinding>>,
    action: Action,
) -> Option<String> {
    bindings
        .get(&action)
        .and_then(|list| list.first())
        .map(|binding| binding.to_string())
}

fn color_binding_labels(bindings: &HashMap<Action, Vec<KeyBinding>>) -> String {
    let colors = [
        Action::SetColorRed,
        Action::SetColorGreen,
        Action::SetColorBlue,
        Action::SetColorYellow,
        Action::SetColorOrange,
        Action::SetColorPink,
        Action::SetColorWhite,
        Action::SetColorBlack,
    ];
    let mut parts = Vec::new();
    for action in colors {
        let label = action_label(action);
        if let Some(binding) = action_primary_binding_label(bindings, action) {
            parts.push(format!("{binding} ({label})"));
        } else {
            parts.push(label.to_string());
        }
    }
    parts.join(", ")
}

pub(crate) fn log_overlay_controls(freeze: bool) {
    log::info!("Starting Wayland overlay...");
    log::info!("Starting annotation overlay...");
    log::info!("Controls:");
    let bindings = default_action_bindings();
    log::info!(
        "  - {}: Just drag",
        action_display_label(Action::SelectPenTool)
    );
    log::info!(
        "  - {}: Hold Shift + drag",
        action_display_label(Action::SelectLineTool)
    );
    log::info!(
        "  - {}: Hold Ctrl + drag",
        action_display_label(Action::SelectRectTool)
    );
    log::info!(
        "  - {}: Hold Tab + drag",
        action_display_label(Action::SelectEllipseTool)
    );
    log::info!(
        "  - {}: Hold Ctrl+Shift + drag",
        action_display_label(Action::SelectArrowTool)
    );
    log::info!(
        "  - {}: {}, click to position, type, press Enter",
        action_display_label(Action::EnterTextMode),
        action_binding_label(&bindings, Action::EnterTextMode)
    );
    log::info!("  - Colors: {}", color_binding_labels(&bindings));
    log::info!(
        "  - {} / {}: {} / {}",
        action_display_label(Action::Undo),
        action_display_label(Action::Redo),
        action_binding_label(&bindings, Action::Undo),
        action_binding_label(&bindings, Action::Redo)
    );
    log::info!(
        "  - {}: {}",
        action_display_label(Action::ClearCanvas),
        action_binding_label(&bindings, Action::ClearCanvas)
    );
    log::info!(
        "  - {}: {} or scroll down",
        action_display_label(Action::IncreaseThickness),
        action_binding_label(&bindings, Action::IncreaseThickness)
    );
    log::info!(
        "  - {}: {} or scroll up",
        action_display_label(Action::DecreaseThickness),
        action_binding_label(&bindings, Action::DecreaseThickness)
    );
    log::info!(
        "  - {}: {} (toggle frozen background)",
        action_display_label(Action::ToggleFrozenMode),
        action_binding_label(&bindings, Action::ToggleFrozenMode)
    );
    log::info!(
        "  - {} / {}: {} / {} (Ctrl+Alt + scroll)",
        action_display_label(Action::ZoomIn),
        action_display_label(Action::ZoomOut),
        action_binding_label(&bindings, Action::ZoomIn),
        action_binding_label(&bindings, Action::ZoomOut)
    );
    log::info!(
        "  - {}: {}   •   {}: {}   •   Pan view: middle drag/arrow keys",
        action_display_label(Action::ResetZoom),
        action_binding_label(&bindings, Action::ResetZoom),
        action_display_label(Action::ToggleZoomLock),
        action_binding_label(&bindings, Action::ToggleZoomLock)
    );
    log::info!(
        "  - {}: Right Click or {}",
        action_display_label(Action::OpenContextMenu),
        action_binding_label(&bindings, Action::OpenContextMenu)
    );
    log::info!(
        "  - {}: {}   •   {}: {}   •   {}: {}   •   {}: {}   •   {}: {}",
        action_display_label(Action::ToggleHelp),
        action_binding_label(&bindings, Action::ToggleHelp),
        action_display_label(Action::ToggleToolbar),
        action_binding_label(&bindings, Action::ToggleToolbar),
        action_display_label(Action::TogglePresenterMode),
        action_binding_label(&bindings, Action::TogglePresenterMode),
        action_display_label(Action::OpenConfigurator),
        action_binding_label(&bindings, Action::OpenConfigurator),
        action_display_label(Action::ToggleStatusBar),
        action_binding_label(&bindings, Action::ToggleStatusBar)
    );
    log::info!(
        "  - {}: {}",
        action_display_label(Action::Exit),
        action_binding_label(&bindings, Action::Exit)
    );
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
