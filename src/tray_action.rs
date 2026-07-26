use crate::durable_io::{AtomicWriteOptions, OverwriteMode, PermissionPolicy, SymlinkPolicy};
use anyhow::{Context, Result};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(_) => 0,
    }
}

fn queued_action_path(dir: &Path, stamp: u128, sequence: u64) -> PathBuf {
    dir.join(format!(
        "{stamp:032x}-{:08x}-{sequence:08x}.action",
        std::process::id(),
    ))
}

pub(crate) struct TrayActionQueue {
    dir: PathBuf,
    next_sequence: u64,
}

impl TrayActionQueue {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            next_sequence: 0,
        }
    }

    #[cfg(test)]
    fn for_test(dir: PathBuf) -> Self {
        Self::new(dir)
    }

    fn take_sequence(&mut self, stamp: u128) -> Result<u64> {
        let sequence = self.next_sequence;
        self.next_sequence = sequence.checked_add(1).ok_or_else(|| {
            anyhow::anyhow!(
                "tray action queue identity space exhausted for stamp {}",
                stamp
            )
        })?;
        Ok(sequence)
    }

    pub(crate) fn queue(&mut self, action: TrayAction) -> Result<PathBuf> {
        self.queue_at(action, action_queue_stamp())
    }

    fn queue_at(&mut self, action: TrayAction, stamp: u128) -> Result<PathBuf> {
        fs::create_dir_all(&self.dir).with_context(|| {
            format!("failed to create runtime directory {}", self.dir.display())
        })?;

        loop {
            let sequence = self.take_sequence(stamp)?;
            let path = queued_action_path(&self.dir, stamp, sequence);
            match crate::durable_io::write_text_atomic(
                &path,
                action.as_str(),
                AtomicWriteOptions {
                    overwrite: OverwriteMode::CreateNew,
                    permissions: PermissionPolicy::FixedMode(0o600),
                    symlink: SymlinkPolicy::Reject,
                    sync_file: false,
                    sync_parent: false,
                },
            ) {
                Ok(()) => return Ok(path),
                Err(crate::durable_io::DurableIoError::AlreadyExists { .. }) => continue,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("failed to queue tray action {}", path.display())
                    });
                }
            }
        }
    }
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

pub(crate) fn take_pending_actions(
    runtime_paths: &crate::paths::PreparedRuntimePaths,
) -> Vec<TrayAction> {
    let dir = runtime_paths.tray_action_dir();
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

    let legacy_path = runtime_paths.tray_action_file();
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
    use super::{TrayAction, TrayActionQueue, take_pending_actions};

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
        let tmp = crate::test_temp::tempdir()
            .expect("fixture creates its private tray-action runtime directory");
        let paths =
            crate::paths::PathResolver::from_environment(crate::paths::PathEnvironment::for_test(
                &[(crate::env_vars::XDG_RUNTIME_DIR_ENV, tmp.path().as_os_str())],
            ));
        let runtime_paths = crate::paths::PreparedRuntimePaths::prepare(&paths)
            .expect("fixture prepares a private runtime identity");

        let mut queue = TrayActionQueue::new(runtime_paths.tray_action_dir());
        queue
            .queue(TrayAction::LightDrawOn)
            .expect("fixture queues its first tray action in the private runtime directory");
        queue
            .queue(TrayAction::LightDrawOff)
            .expect("fixture queues its second tray action in the private runtime directory");

        assert_eq!(
            take_pending_actions(&runtime_paths),
            vec![TrayAction::LightDrawOn, TrayAction::LightDrawOff]
        );
        assert!(take_pending_actions(&runtime_paths).is_empty());
    }

    #[test]
    fn independent_queue_owners_resolve_identity_collisions_without_sharing_state() {
        let tmp = crate::test_temp::tempdir()
            .expect("fixture creates its private tray-action queue directory");
        let queue_dir = tmp.path().join("queue");
        let mut first = TrayActionQueue::for_test(queue_dir.clone());
        let mut second = TrayActionQueue::for_test(queue_dir);

        let first_path = first
            .queue_at(TrayAction::LightDrawOn, 42)
            .expect("first fixture owner publishes its action");
        let second_path = second
            .queue_at(TrayAction::LightDrawOff, 42)
            .expect("second fixture owner resolves the colliding identity");
        let third_path = first
            .queue_at(TrayAction::ToggleFreeze, 42)
            .expect("first fixture owner advances past the second owner's identity");

        assert_ne!(first_path, second_path);
        assert_ne!(second_path, third_path);
        assert_ne!(first_path, third_path);
    }
}
