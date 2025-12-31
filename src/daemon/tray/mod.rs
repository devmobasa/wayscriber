mod helpers;
mod runtime;

pub(crate) use runtime::start_system_tray;

#[cfg(feature = "tray")]
use runtime::update_session_resume_in_config;

#[cfg(feature = "tray")]
use super::types::{TrayStatus, TrayStatusShared};
#[cfg(all(feature = "tray", test))]
use crate::paths::tray_action_file;
#[cfg(feature = "tray")]
use log::{info, warn};
#[cfg(feature = "tray")]
use std::path::PathBuf;
#[cfg(feature = "tray")]
use std::sync::Arc;
#[cfg(feature = "tray")]
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
#[cfg(feature = "tray")]
use std::time::{Duration, Instant};

#[cfg(feature = "tray")]
pub(crate) struct WayscriberTray {
    toggle_flag: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
    configurator_binary: String,
    session_resume_enabled: bool,
    overlay_pid: Arc<AtomicU32>,
    tray_action_path: PathBuf,
    tray_status: Arc<TrayStatusShared>,
}

#[cfg(feature = "tray")]
impl WayscriberTray {
    fn new(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        configurator_binary: String,
        session_resume_enabled: bool,
        overlay_pid: Arc<AtomicU32>,
        tray_action_path: PathBuf,
        tray_status: Arc<TrayStatusShared>,
    ) -> Self {
        Self {
            toggle_flag,
            quit_flag,
            configurator_binary,
            session_resume_enabled,
            overlay_pid,
            tray_action_path,
            tray_status,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        session_resume_enabled: bool,
    ) -> Self {
        Self::new(
            toggle_flag,
            quit_flag,
            "true".into(),
            session_resume_enabled,
            Arc::new(AtomicU32::new(0)),
            tray_action_file(),
            Arc::new(TrayStatusShared::new()),
        )
    }
}

#[cfg(feature = "tray")]
impl ksni::Tray for WayscriberTray {
    fn id(&self) -> String {
        "wayscriber".into()
    }

    fn title(&self) -> String {
        "Wayscriber Screen Annotation".into()
    }

    fn icon_name(&self) -> String {
        "wayscriber".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let status = self.tray_status.snapshot();
        let TrayStatus {
            overlay_error,
            watcher_offline,
            watcher_reason,
        } = status;
        let mut description =
            "Toggle overlay, open configurator, or quit from the tray".to_string();

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
            icon_name: "wayscriber".into(),
            icon_pixmap: vec![],
            title: format!("Wayscriber {}", env!("CARGO_PKG_VERSION")),
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

        vec![
            StandardItem {
                label: "About Wayscriber".to_string(),
                icon_name: "help-about".into(),
                activate: Box::new(|this: &mut Self| {
                    this.launch_about();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Toggle Overlay".to_string(),
                icon_name: "tool-pointer".into(),
                activate: Box::new(|this: &mut Self| {
                    this.toggle_flag.store(true, Ordering::Release);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Toggle Freeze (overlay)".to_string(),
                icon_name: "media-playback-pause".into(),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action("toggle_freeze");
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Capture: Full Screen".to_string(),
                icon_name: "camera-photo".into(),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action("capture_full");
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Capture: Active Window".to_string(),
                icon_name: "window-duplicate".into(),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action("capture_window");
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Capture: Region".to_string(),
                icon_name: "selection-rectangular".into(),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action("capture_region");
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Toggle Help Overlay".to_string(),
                icon_name: "help-browser".into(),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action("toggle_help");
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Configurator".to_string(),
                icon_name: "preferences-desktop".into(),
                activate: Box::new(|this: &mut Self| {
                    this.launch_configurator();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Config File".to_string(),
                icon_name: "text-x-generic".into(),
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
                icon_name: "document-save".into(),
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
                icon_name: "edit-clear".into(),
                activate: Box::new(|this: &mut Self| {
                    this.clear_session_files();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open log folder".to_string(),
                icon_name: "folder".into(),
                activate: Box::new(|this: &mut Self| {
                    this.open_log_folder();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                icon_name: "window-close".into(),
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
