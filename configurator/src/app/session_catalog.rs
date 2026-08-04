use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use wayscriber::session::try_lock_exclusive;

use crate::models::{
    DaemonRuntimeStatus, SessionCatalogActionResult, SessionCatalogItem, SessionCatalogOperation,
};

use super::blocking_jobs::{BlockingJobKind, run_blocking};
use super::daemon_setup::load_daemon_runtime_status_sync;
mod duplicate;
mod move_file;

pub(super) use duplicate::duplicate_session_catalog_entry;
pub(super) use move_file::move_session_catalog_entry;

pub(super) async fn load_session_catalog() -> Result<Vec<SessionCatalogItem>, String> {
    run_blocking(
        BlockingJobKind::SessionCatalogLoad,
        load_session_catalog_sync,
    )
    .await
}

pub(super) async fn forget_session_catalog_entry(
    id: String,
) -> Result<SessionCatalogActionResult, String> {
    run_blocking(BlockingJobKind::SessionCatalogMutation, move || {
        let removed = wayscriber::session::catalog::forget_session_by_id(&id)
            .map_err(|err| err.to_string())?;
        let items = load_session_catalog_sync()?;
        Ok(SessionCatalogActionResult::success(
            if removed {
                "Forgot session metadata.".to_string()
            } else {
                "Session was already absent from the catalog.".to_string()
            },
            items,
        ))
    })
    .await
}

pub(super) async fn rename_session_catalog_entry(
    id: String,
    display_name: String,
) -> Result<SessionCatalogActionResult, String> {
    run_blocking(BlockingJobKind::SessionCatalogMutation, move || {
        let renamed = wayscriber::session::catalog::rename_session_display_name_by_id(
            &id,
            display_name.trim(),
        )
        .map_err(|err| err.to_string())?;
        let items = load_session_catalog_sync()?;
        Ok(SessionCatalogActionResult::success(
            if let Some(entry) = renamed {
                format!("Renamed session to {}.", entry.display_name)
            } else {
                "Session was already absent from the catalog.".to_string()
            },
            items,
        ))
    })
    .await
}

pub(super) async fn reveal_session_catalog_entry(
    id: String,
) -> Result<SessionCatalogActionResult, String> {
    run_blocking(BlockingJobKind::SessionCatalogMutation, move || {
        let item = find_session_catalog_item(&id)?;
        reveal_path_parent(&item.path)?;
        let items = load_session_catalog_sync()?;
        Ok(SessionCatalogActionResult::success(
            format!("Opened folder for {}.", item.display_name),
            items,
        ))
    })
    .await
}

pub(super) async fn clear_session_catalog_entry(
    id: String,
) -> Result<SessionCatalogActionResult, String> {
    run_blocking(BlockingJobKind::SessionCatalogMutation, move || {
        let status = load_daemon_runtime_status_sync()?;
        let item = find_session_catalog_item(&id)?;
        let _daemon_lock = acquire_runtime_lock_for_inactive_operation(
            RuntimeLockKind::Daemon,
            SessionCatalogOperation::Clear,
        )?;
        let _overlay_lock = acquire_runtime_lock_for_inactive_operation(
            RuntimeLockKind::Overlay,
            SessionCatalogOperation::Clear,
        )?;
        if let Some(blocker) = service_status_blocker(Some(&status), SessionCatalogOperation::Clear)
        {
            return Err(blocker);
        }

        let outcome = wayscriber::session::clear_named_session_non_lock_artifacts(&item.path)
            .map_err(|err| err.to_string())?;
        let items = load_session_catalog_sync()?;
        Ok(SessionCatalogActionResult::success(
            if outcome.removed_any() {
                format!("Cleared saved data for {}.", item.display_name)
            } else {
                format!("No saved data found for {}.", item.display_name)
            },
            items,
        ))
    })
    .await
}

pub(super) async fn clear_session_catalog_tool_state_entry(
    id: String,
) -> Result<SessionCatalogActionResult, String> {
    run_blocking(BlockingJobKind::SessionCatalogMutation, move || {
        let status = load_daemon_runtime_status_sync()?;
        let item = find_session_catalog_item(&id)?;
        let _daemon_lock = acquire_runtime_lock_for_inactive_operation(
            RuntimeLockKind::Daemon,
            SessionCatalogOperation::ClearToolState,
        )?;
        let _overlay_lock = acquire_runtime_lock_for_inactive_operation(
            RuntimeLockKind::Overlay,
            SessionCatalogOperation::ClearToolState,
        )?;
        if let Some(blocker) =
            service_status_blocker(Some(&status), SessionCatalogOperation::ClearToolState)
        {
            return Err(blocker);
        }

        let options = named_session_options_for_catalog_item(&item)?;
        let outcome =
            wayscriber::session::clear_tool_state(&options).map_err(|err| err.to_string())?;
        let items = load_session_catalog_sync()?;
        Ok(SessionCatalogActionResult::success(
            clear_tool_state_catalog_message(&item.display_name, outcome),
            items,
        ))
    })
    .await
}

fn service_status_blocker(
    status: Option<&DaemonRuntimeStatus>,
    operation: SessionCatalogOperation,
) -> Option<String> {
    operation.cached_status_blocker(status).map(str::to_string)
}

fn load_session_catalog_sync() -> Result<Vec<SessionCatalogItem>, String> {
    wayscriber::session::catalog::recent_sessions()
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(SessionCatalogItem::from_entry)
        .collect()
}

fn find_session_catalog_item(id: &str) -> Result<SessionCatalogItem, String> {
    load_session_catalog_sync()?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "Session is no longer in the catalog.".to_string())
}

fn named_session_options_for_catalog_item(
    item: &SessionCatalogItem,
) -> Result<wayscriber::session::SessionOptions, String> {
    let loaded = wayscriber::config::Config::load().map_err(|err| err.to_string())?;
    ensure_session_config_available_for_destructive_action(&loaded)?;
    let mut options = wayscriber::session::options_from_config_for_named_file(
        &loaded.config.session,
        item.path.clone(),
        None,
    );
    options.force_resume_persistence();
    Ok(options)
}

fn ensure_session_config_available_for_destructive_action(
    loaded: &wayscriber::config::LoadedConfig,
) -> Result<(), String> {
    if loaded.section_failed("session") {
        return Err(
            "Clear saved tool state is disabled because config.toml [session] could not be read; fix the section and retry"
                .to_string(),
        );
    }
    Ok(())
}

fn clear_tool_state_catalog_message(
    display_name: &str,
    outcome: wayscriber::session::ClearToolStateOutcome,
) -> String {
    match outcome {
        wayscriber::session::ClearToolStateOutcome::NoSession => {
            format!("No saved session file found for {display_name}.")
        }
        wayscriber::session::ClearToolStateOutcome::NoToolState => {
            format!("No saved tool state found for {display_name}.")
        }
        wayscriber::session::ClearToolStateOutcome::Cleared {
            preserved_board_data: true,
        } => format!(
            "Cleared saved tool state for {display_name}. Boards and history were preserved."
        ),
        wayscriber::session::ClearToolStateOutcome::Cleared {
            preserved_board_data: false,
        } => format!("Cleared saved tool state for {display_name}. No board data was present."),
    }
}

fn reveal_path_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        return Err(format!(
            "Session folder does not exist: {}",
            parent.display()
        ));
    }

    Command::new("xdg-open")
        .arg(parent)
        .spawn()
        .map_err(|err| format!("failed to launch xdg-open: {err}"))?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeLockKind {
    Daemon,
    Overlay,
}

impl RuntimeLockKind {
    fn path(self) -> PathBuf {
        match self {
            Self::Daemon => wayscriber::paths::daemon_lock_file(),
            Self::Overlay => wayscriber::paths::overlay_lock_file(),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Daemon => "daemon",
            Self::Overlay => "overlay",
        }
    }
}

#[cfg(test)]
fn runtime_lock_active(
    kind: RuntimeLockKind,
    operation: SessionCatalogOperation,
) -> Result<bool, String> {
    let path = kind.path();
    let file = match OpenOptions::new().read(true).write(true).open(&path) {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(format!(
                "{} is disabled because the {} lock could not be inspected: {} ({err})",
                operation.label(),
                kind.label(),
                path.display()
            ));
        }
    };

    match try_lock_exclusive(&file) {
        Ok(()) => {
            drop_lock(file);
            Ok(false)
        }
        Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(true),
        Err(err) => Err(format!(
            "{} is disabled because the {} lock could not be checked: {} ({err})",
            operation.label(),
            kind.label(),
            path.display()
        )),
    }
}

#[cfg(test)]
fn acquire_runtime_lock_for_clear(kind: RuntimeLockKind) -> Result<File, String> {
    acquire_runtime_lock_for_inactive_operation(kind, SessionCatalogOperation::Clear)
}

fn acquire_runtime_lock_for_inactive_operation(
    kind: RuntimeLockKind,
    operation: SessionCatalogOperation,
) -> Result<File, String> {
    let path = kind.path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "{} is disabled because the {} lock directory could not be created: {} ({err})",
                operation.label(),
                kind.label(),
                parent.display()
            )
        })?;
    }

    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|err| {
            format!(
                "{} is disabled because the {} lock could not be opened: {} ({err})",
                operation.label(),
                kind.label(),
                path.display()
            )
        })?;

    match try_lock_exclusive(&file) {
        Ok(()) => Ok(file),
        Err(err) if err.kind() == ErrorKind::WouldBlock => {
            let message = match kind {
                RuntimeLockKind::Daemon => operation.running_daemon_message(),
                RuntimeLockKind::Overlay => operation.running_overlay_message(),
            };
            Err(message.to_string())
        }
        Err(err) => Err(format!(
            "{} is disabled because the {} lock could not be reserved: {} ({err})",
            operation.label(),
            kind.label(),
            path.display()
        )),
    }
}

#[cfg(test)]
fn drop_lock(file: File) {
    drop(file);
}

#[cfg(test)]
mod tests;
