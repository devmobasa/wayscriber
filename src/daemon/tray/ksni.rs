#[cfg(feature = "tray")]
use super::super::types::TrayStatus;
#[cfg(feature = "tray")]
use super::WayscriberTray;
#[cfg(feature = "tray")]
use super::runtime::update_session_resume_in_config;
#[cfg(feature = "tray")]
use super::shortcut_hint_io::configured_toggle_shortcut_hint;
#[cfg(feature = "tray")]
use crate::config::{Action, action_label};
#[cfg(feature = "tray")]
use crate::label_format::format_binding_label;
#[cfg(feature = "tray")]
use crate::tray_action::TrayAction;
#[cfg(feature = "tray")]
use log::{info, warn};
#[cfg(feature = "tray")]
use std::env;
#[cfg(feature = "tray")]
use std::path::PathBuf;
#[cfg(feature = "tray")]
use std::sync::atomic::Ordering;
#[cfg(feature = "tray")]
use std::time::{Duration, Instant};

#[cfg(feature = "tray")]
impl ksni::Tray for WayscriberTray {
    fn id(&self) -> String {
        "wayscriber".into()
    }

    fn title(&self) -> String {
        "Wayscriber Screen Annotation".into()
    }

    fn icon_name(&self) -> String {
        if tray_theme_icons_enabled() {
            "wayscriber".into()
        } else {
            String::new()
        }
    }

    fn icon_theme_path(&self) -> String {
        resolve_icon_theme_path()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let status = self.tray_status.snapshot();
        let overlay_active = self.overlay_pid.load(Ordering::Acquire) > 0;
        let TrayStatus {
            overlay_error,
            watcher_offline,
            watcher_reason,
        } = status;
        let mut description =
            "Toggle overlay, open configurator, or quit from the tray".to_string();
        description.push_str("\nOverlay: ");
        description.push_str(if overlay_active { "active" } else { "hidden" });

        if watcher_offline {
            description.push_str("\nTray watcher offline");
            if let Some(reason) = watcher_reason {
                description.push_str(": ");
                description.push_str(&reason);
            }
        }

        if let Some(error) = overlay_error {
            description.push_str("\nOverlay error: ");
            description.push_str(&error.message);
            if let Some(next_retry_at) = error.next_retry_at {
                let remaining = next_retry_at.saturating_duration_since(Instant::now());
                if remaining > Duration::from_secs(0) {
                    description.push_str(&format!(" (retry in {}s)", remaining.as_secs().max(1)));
                }
            }
        }

        ksni::ToolTip {
            icon_name: if tray_theme_icons_enabled() {
                "wayscriber".into()
            } else {
                String::new()
            },
            icon_pixmap: vec![],
            title: format!("Wayscriber {}", crate::build_info::version()),
            description,
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.tray_icon_pixmap()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let use_theme_icons = tray_theme_icons_enabled();
        let overlay_active = self.overlay_pid.load(Ordering::Acquire) > 0;
        let toggle_label = toggle_overlay_menu_label();

        vec![
            StandardItem {
                label: "About Wayscriber".to_string(),
                icon_name: menu_icon_name("help-about", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.launch_about();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: toggle_label,
                icon_name: menu_icon_name("tool-pointer", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.toggle_flag.store(true, Ordering::Release);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format!(
                    "Overlay: {}",
                    if overlay_active { "active" } else { "hidden" }
                ),
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format_binding_label(action_label(Action::BoardPicker), None),
                icon_name: menu_icon_name("view-grid", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action(TrayAction::ToggleBoardPicker);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format_binding_label(action_label(Action::ToggleFrozenMode), None),
                icon_name: menu_icon_name("media-playback-pause", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action(TrayAction::ToggleFreeze);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format_binding_label(action_label(Action::CaptureFullScreen), None),
                icon_name: menu_icon_name("camera-photo", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action(TrayAction::CaptureFull);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format_binding_label(action_label(Action::CaptureActiveWindow), None),
                icon_name: menu_icon_name("window-duplicate", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action(TrayAction::CaptureWindow);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format_binding_label(action_label(Action::CaptureSelection), None),
                icon_name: menu_icon_name("selection-rectangular", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action(TrayAction::CaptureRegion);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format_binding_label(action_label(Action::ToggleHelp), None),
                icon_name: menu_icon_name("help-browser", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action(TrayAction::ToggleHelp);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format_binding_label(action_label(Action::OpenConfigurator), None),
                icon_name: menu_icon_name("preferences-desktop", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.launch_configurator();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Config File".to_string(),
                icon_name: menu_icon_name("text-x-generic", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.open_config_file();
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: if self.session_resume_enabled {
                    "Session resume: enabled".to_string()
                } else {
                    "Session resume: disabled".to_string()
                },
                checked: self.session_resume_enabled,
                icon_name: menu_icon_name("document-save", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    let target = !this.session_resume_enabled;
                    let persisted =
                        update_session_resume_in_config(target, this.session_resume_enabled);
                    this.session_resume_enabled = persisted;
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Clear saved session data".to_string(),
                icon_name: menu_icon_name("edit-clear", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.clear_session_files();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open log folder".to_string(),
                icon_name: menu_icon_name("folder", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.open_log_folder();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                icon_name: menu_icon_name("window-close", use_theme_icons),
                activate: Box::new(|this: &mut Self| {
                    this.quit_flag.store(true, Ordering::Release);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn watcher_online(&self) {
        if self.tray_status.set_watcher_online() {
            info!("StatusNotifierWatcher is online");
        }
    }

    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        let reason_text = format!("{reason:?}");
        if self.tray_status.set_watcher_offline(reason_text.clone()) {
            warn!("StatusNotifierWatcher is offline: {}", reason_text);
        }
        true
    }
}

#[cfg(feature = "tray")]
fn tray_theme_icons_enabled() -> bool {
    if env::var_os("WAYSCRIBER_TRAY_FORCE_PIXMAP").is_some() {
        return false;
    }
    let desktop_env = env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    let session_env = env::var("XDG_SESSION_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    let desktop_session = env::var("DESKTOP_SESSION")
        .unwrap_or_default()
        .to_lowercase();
    tray_theme_icons_supported(&desktop_env, &session_env, &desktop_session)
}

#[cfg(feature = "tray")]
fn menu_icon_name(name: &str, use_theme_icons: bool) -> String {
    if use_theme_icons {
        name.to_string()
    } else {
        String::new()
    }
}

#[cfg(feature = "tray")]
fn resolve_icon_theme_path() -> String {
    if let Ok(value) = env::var("WAYSCRIBER_ICON_THEME_PATH") {
        return value;
    }
    installed_icon_theme_path()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(feature = "tray")]
fn tray_theme_icons_supported(desktop_env: &str, session_env: &str, desktop_session: &str) -> bool {
    !tray_theme_icons_blocked_by_desktop(desktop_env)
        && !tray_theme_icons_blocked_by_desktop(session_env)
        && !tray_theme_icons_blocked_by_desktop(desktop_session)
}

#[cfg(feature = "tray")]
fn tray_theme_icons_blocked_by_desktop(value: &str) -> bool {
    value.contains("noctalia") || value.contains("quickshell") || value.contains("cosmic")
}

#[cfg(feature = "tray")]
fn installed_icon_theme_path() -> Option<PathBuf> {
    let exe_path = env::current_exe().ok()?;
    let install_root = exe_path.parent()?.parent()?;
    let icon_theme_path = install_root.join("share/icons");
    icon_theme_path.is_dir().then_some(icon_theme_path)
}

#[cfg(feature = "tray")]
fn toggle_overlay_menu_label() -> String {
    let base = "Toggle Overlay";
    match configured_toggle_shortcut_hint() {
        Some(shortcut) => format!("{base} ({shortcut})"),
        None => base.to_string(),
    }
}

#[cfg(all(feature = "tray", test))]
mod tests {
    use super::tray_theme_icons_supported;

    #[test]
    fn tray_theme_icons_supported_allows_common_desktops() {
        assert!(tray_theme_icons_supported("kde", "plasma", "plasmashell"));
    }

    #[test]
    fn tray_theme_icons_supported_blocks_cosmic() {
        assert!(!tray_theme_icons_supported("cosmic", "", ""));
        assert!(!tray_theme_icons_supported("", "cosmic", ""));
        assert!(!tray_theme_icons_supported("", "", "cosmic-session"));
    }

    #[test]
    fn tray_theme_icons_supported_blocks_existing_problem_shells() {
        assert!(!tray_theme_icons_supported("quickshell", "", ""));
        assert!(!tray_theme_icons_supported("", "noctalia", ""));
    }
}
