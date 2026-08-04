use relm4::gtk;

use gtk::prelude::*;

use crate::models::{DaemonRuntimeStatus, ShortcutApplyCapability};

use super::super::super::state::ConfiguratorApp;

/// Emphasis for a status line, mapped to the Adwaita state classes that
/// stand in for the Iced view's literal colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    Neutral,
    Positive,
    Caution,
    Negative,
}

impl Tone {
    pub(super) const CLASSES: [&'static str; 4] = ["dim-label", "success", "warning", "error"];

    pub(super) fn css_class(self) -> &'static str {
        match self {
            Self::Neutral => "dim-label",
            Self::Positive => "success",
            Self::Caution => "warning",
            Self::Negative => "error",
        }
    }
}

pub(super) fn overall_status(status: Option<&DaemonRuntimeStatus>) -> (&'static str, Tone) {
    match status {
        None => ("Status: Detecting...", Tone::Neutral),
        Some(status) if status.service_active => ("Status: Running", Tone::Positive),
        Some(status) if status.service_installed => {
            ("Status: Installed, not running", Tone::Caution)
        }
        Some(_) => ("Status: Not installed", Tone::Neutral),
    }
}

pub(super) fn feedback_tone(feedback: &str) -> Tone {
    let feedback = feedback.to_ascii_lowercase();
    if feedback.contains("failed") || feedback.contains("error") {
        Tone::Negative
    } else {
        Tone::Positive
    }
}

pub(super) fn install_status(installed: bool) -> (&'static str, Tone) {
    if installed {
        ("Installed \u{2713}", Tone::Positive)
    } else {
        ("Not installed", Tone::Neutral)
    }
}

pub(super) fn install_button_label(installed: bool) -> &'static str {
    if installed {
        "Update Service"
    } else {
        "Install Service"
    }
}

pub(super) fn light_controls_status(configured: bool) -> (&'static str, Tone) {
    if configured {
        ("Configured \u{2713}", Tone::Positive)
    } else {
        ("Not configured", Tone::Neutral)
    }
}

pub(super) fn service_status(running: bool, enabled: bool) -> (&'static str, Tone) {
    match (running, enabled) {
        (true, true) => ("Running \u{2713}", Tone::Positive),
        (true, false) => ("Running (not enabled)", Tone::Caution),
        (false, true) => ("Enabled, not running", Tone::Caution),
        (false, false) => ("Stopped and disabled", Tone::Neutral),
    }
}

pub(super) fn shortcut_placeholder(capability: Option<ShortcutApplyCapability>) -> &'static str {
    match capability {
        Some(ShortcutApplyCapability::GnomeCustomShortcut) => "e.g. Super+G or <Super>g",
        Some(ShortcutApplyCapability::PortalServiceDropIn) => "e.g. Ctrl+Shift+G or <Ctrl><Shift>g",
        _ => "e.g. Ctrl+Shift+G",
    }
}

pub(super) fn light_controls_details(status: &DaemonRuntimeStatus) -> Option<String> {
    let path = status.light_controls_config_path.as_deref()?;
    Some(if status.light_controls_configured {
        format!("Light controls: configured at {path}")
    } else {
        format!("Light controls include: {path}")
    })
}

/// The tool availability line, shown only when something is missing.
pub(super) fn missing_tools(status: &DaemonRuntimeStatus) -> Option<String> {
    let mut missing = Vec::new();
    if !status.systemctl_available {
        missing.push("systemctl");
    }
    if !status.gsettings_available {
        missing.push("gsettings");
    }
    (!missing.is_empty()).then(|| format!("Missing tools: {}", missing.join(", ")))
}

pub(super) fn service_installed(app: &ConfiguratorApp) -> bool {
    app.daemon_status
        .as_ref()
        .is_some_and(|status| status.service_installed)
}

pub(super) fn apply_tone(widget: &impl IsA<gtk::Widget>, tone: Tone) {
    let wanted = tone.css_class();
    for class in Tone::CLASSES {
        let has_class = widget.has_css_class(class);
        if class == wanted {
            if !has_class {
                widget.add_css_class(class);
            }
        } else if has_class {
            widget.remove_css_class(class);
        }
    }
}
