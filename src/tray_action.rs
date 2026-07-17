use crate::durable_io::{AtomicWriteOptions, OverwriteMode, PermissionPolicy, SymlinkPolicy};
use anyhow::{Context, Result};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static ACTION_QUEUE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TrayAction {
    ToggleFreeze,
    CaptureFull,
    CaptureWindow,
    CaptureRegion,
    ToggleHelp,
    ToggleBoardPicker,
    ToggleLightMode,
    LightDrawToggle,
    LightDrawOn,
    LightDrawOff,
}

impl TrayAction {
    #[cfg_attr(not(feature = "tray"), allow(dead_code))]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            TrayAction::ToggleFreeze => "toggle_freeze",
            TrayAction::CaptureFull => "capture_full",
            TrayAction::CaptureWindow => "capture_window",
            TrayAction::CaptureRegion => "capture_region",
            TrayAction::ToggleHelp => "toggle_help",
            TrayAction::ToggleBoardPicker => "toggle_board_picker",
            TrayAction::ToggleLightMode => "toggle_light_mode",
            TrayAction::LightDrawToggle => "light_draw_toggle",
            TrayAction::LightDrawOn => "light_draw_on",
            TrayAction::LightDrawOff => "light_draw_off",
        }
    }

    pub(crate) fn parse(action: &str) -> Option<Self> {
        match action {
            "toggle_freeze" => Some(TrayAction::ToggleFreeze),
            "capture_full" => Some(TrayAction::CaptureFull),
            "capture_window" => Some(TrayAction::CaptureWindow),
            "capture_region" => Some(TrayAction::CaptureRegion),
            "toggle_help" => Some(TrayAction::ToggleHelp),
            "toggle_board_picker" => Some(TrayAction::ToggleBoardPicker),
            "toggle_light_mode" => Some(TrayAction::ToggleLightMode),
            "light_draw_toggle" => Some(TrayAction::LightDrawToggle),
            "light_draw_on" => Some(TrayAction::LightDrawOn),
            "light_draw_off" => Some(TrayAction::LightDrawOff),
            _ => None,
        }
    }
}

fn action_queue_stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn queued_action_path(dir: &Path) -> PathBuf {
    let sequence = ACTION_QUEUE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    dir.join(format!(
        "{:032x}-{:08x}-{:08x}.action",
        action_queue_stamp(),
        std::process::id(),
        sequence
    ))
}

pub(crate) fn queue_action(action: TrayAction) -> Result<PathBuf> {
    let dir = crate::paths::tray_action_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create runtime directory {}", dir.display()))?;

    let path = queued_action_path(&dir);
    crate::durable_io::write_text_atomic(
        &path,
        action.as_str(),
        AtomicWriteOptions {
            overwrite: OverwriteMode::CreateNew,
            permissions: PermissionPolicy::FixedMode(0o600),
            symlink: SymlinkPolicy::Reject,
            sync_file: false,
            sync_parent: false,
        },
    )
    .with_context(|| format!("failed to queue tray action {}", path.display()))?;
    Ok(path)
}

fn parse_action_file(path: &Path, content: &str) -> Option<TrayAction> {
    let action_str = content.lines().next().unwrap_or("").trim();
    if action_str.is_empty() {
        return None;
    }
    match TrayAction::parse(action_str) {
        Some(action) => Some(action),
        None => {
            warn!("Unknown tray action '{}' in {}", action_str, path.display());
            None
        }
    }
}

pub(crate) fn take_pending_actions() -> Vec<TrayAction> {
    let dir = crate::paths::tray_action_dir();
    let mut paths = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "action")
            })
            .collect::<Vec<_>>(),
        Err(err) if err.kind() == ErrorKind::NotFound => Vec::new(),
        Err(err) => {
            warn!(
                "Failed to read tray action queue {}: {}",
                dir.display(),
                err
            );
            Vec::new()
        }
    };
    paths.sort_by(|left, right| left.file_name().cmp(&right.file_name()));

    let mut actions = Vec::new();
    for path in paths {
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => {
                warn!("Failed to read tray action {}: {}", path.display(), err);
                continue;
            }
        };
        if let Err(err) = fs::remove_file(&path) {
            warn!("Failed to remove tray action {}: {}", path.display(), err);
            continue;
        }
        if let Some(action) = parse_action_file(&path, &content) {
            actions.push(action);
        }
    }

    let legacy_path = crate::paths::tray_action_file();
    match fs::read_to_string(&legacy_path) {
        Ok(content) => {
            if let Err(err) = fs::remove_file(&legacy_path) {
                warn!(
                    "Failed to remove legacy tray action {}: {}",
                    legacy_path.display(),
                    err
                );
            }
            if let Some(action) = parse_action_file(&legacy_path, &content) {
                actions.push(action);
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => warn!(
            "Tray action signal received but failed to read {}: {}",
            legacy_path.display(),
            err
        ),
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::{TrayAction, queue_action, take_pending_actions};
    use crate::env_vars::XDG_RUNTIME_DIR_ENV;
    use std::env;

    #[test]
    fn tray_action_round_trip() {
        let actions = [
            TrayAction::ToggleFreeze,
            TrayAction::CaptureFull,
            TrayAction::CaptureWindow,
            TrayAction::CaptureRegion,
            TrayAction::ToggleHelp,
            TrayAction::ToggleBoardPicker,
            TrayAction::ToggleLightMode,
            TrayAction::LightDrawToggle,
            TrayAction::LightDrawOn,
            TrayAction::LightDrawOff,
        ];

        for action in actions {
            assert_eq!(TrayAction::parse(action.as_str()), Some(action));
        }

        assert_eq!(TrayAction::parse("not-a-tray-action"), None);
    }

    #[test]
    fn queued_tray_actions_round_trip_in_order() {
        let _guard = crate::test_env::lock();
        let tmp = crate::test_temp::tempdir().unwrap();
        let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
        unsafe {
            env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
        }

        queue_action(TrayAction::LightDrawOn).unwrap();
        queue_action(TrayAction::LightDrawOff).unwrap();

        assert_eq!(
            take_pending_actions(),
            vec![TrayAction::LightDrawOn, TrayAction::LightDrawOff]
        );
        assert!(take_pending_actions().is_empty());

        if let Some(prev) = prev {
            unsafe {
                env::set_var(XDG_RUNTIME_DIR_ENV, prev);
            }
        } else {
            unsafe {
                env::remove_var(XDG_RUNTIME_DIR_ENV);
            }
        }
    }
}
