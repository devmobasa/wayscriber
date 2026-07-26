use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use wayscriber::session::try_lock_exclusive;

use crate::models::{DaemonRuntimeStatus, SessionCatalogActionResult, SessionCatalogItem};

use super::daemon_setup::load_daemon_runtime_status;
mod duplicate;
mod move_file;

pub(super) use duplicate::duplicate_session_catalog_entry;
pub(super) use move_file::move_session_catalog_entry;

pub(super) fn load_session_catalog(
    paths: &wayscriber::paths::PathResolver,
) -> Result<Vec<SessionCatalogItem>, String> {
    let catalog = wayscriber::session::catalog::SessionCatalog::from_resolver(paths)
        .map_err(|error| error.to_string())?;
    load_session_catalog_sync(&catalog)
}

pub(super) fn forget_session_catalog_entry(
    id: String,
    paths: &wayscriber::paths::PathResolver,
) -> Result<SessionCatalogActionResult, String> {
    let catalog = wayscriber::session::catalog::SessionCatalog::from_resolver(paths)
        .map_err(|error| error.to_string())?;
    let removed = catalog
        .forget_session_by_id(&id)
        .map_err(|err| err.to_string())?;
    let items = load_session_catalog_sync(&catalog)?;
    Ok(SessionCatalogActionResult::success(
        if removed {
            "Forgot session metadata.".to_string()
        } else {
            "Session was already absent from the catalog.".to_string()
        },
        items,
    ))
}

pub(super) fn rename_session_catalog_entry(
    id: String,
    display_name: String,
    paths: &wayscriber::paths::PathResolver,
) -> Result<SessionCatalogActionResult, String> {
    let catalog = wayscriber::session::catalog::SessionCatalog::from_resolver(paths)
        .map_err(|error| error.to_string())?;
    let renamed = catalog
        .rename_session_display_name_by_id(&id, display_name.trim())
        .map_err(|err| err.to_string())?;
    let items = load_session_catalog_sync(&catalog)?;
    Ok(SessionCatalogActionResult::success(
        if let Some(entry) = renamed {
            format!("Renamed session to {}.", entry.display_name)
        } else {
            "Session was already absent from the catalog.".to_string()
        },
        items,
    ))
}

pub(super) fn reveal_session_catalog_entry(
    id: String,
    paths: &wayscriber::paths::PathResolver,
) -> Result<SessionCatalogActionResult, String> {
    let catalog = wayscriber::session::catalog::SessionCatalog::from_resolver(paths)
        .map_err(|error| error.to_string())?;
    let item = find_session_catalog_item(&catalog, &id)?;
    reveal_path_parent(&item.path)?;
    let items = load_session_catalog_sync(&catalog)?;
    Ok(SessionCatalogActionResult::success(
        format!("Opened folder for {}.", item.display_name),
        items,
    ))
}

pub(super) fn clear_session_catalog_entry(
    id: String,
    paths: &wayscriber::paths::PathResolver,
) -> Result<SessionCatalogActionResult, String> {
    let status = load_daemon_runtime_status(paths)?;
    let catalog = wayscriber::session::catalog::SessionCatalog::from_resolver(paths)
        .map_err(|error| error.to_string())?;
    let item = find_session_catalog_item(&catalog, &id)?;
    let runtime_locks = RuntimeLockPaths::prepare(paths)?;
    let _daemon_lock = acquire_runtime_lock_for_inactive_operation(
        &runtime_locks,
        RuntimeLockKind::Daemon,
        CatalogOperation::Clear,
    )?;
    let _overlay_lock = acquire_runtime_lock_for_inactive_operation(
        &runtime_locks,
        RuntimeLockKind::Overlay,
        CatalogOperation::Clear,
    )?;
    if let Some(blocker) = service_status_blocker(Some(&status), CatalogOperation::Clear) {
        return Err(blocker);
    }

    let outcome = wayscriber::session::clear_named_session_non_lock_artifacts(&item.path)
        .map_err(|err| err.to_string())?;
    let items = load_session_catalog_sync(&catalog)?;
    Ok(SessionCatalogActionResult::success(
        if outcome.removed_any() {
            format!("Cleared saved data for {}.", item.display_name)
        } else {
            format!("No saved data found for {}.", item.display_name)
        },
        items,
    ))
}

pub(super) fn clear_session_catalog_tool_state_entry(
    id: String,
    paths: &wayscriber::paths::PathResolver,
) -> Result<SessionCatalogActionResult, String> {
    let status = load_daemon_runtime_status(paths)?;
    let catalog = wayscriber::session::catalog::SessionCatalog::from_resolver(paths)
        .map_err(|error| error.to_string())?;
    let item = find_session_catalog_item(&catalog, &id)?;
    let runtime_locks = RuntimeLockPaths::prepare(paths)?;
    let _daemon_lock = acquire_runtime_lock_for_inactive_operation(
        &runtime_locks,
        RuntimeLockKind::Daemon,
        CatalogOperation::ClearToolState,
    )?;
    let _overlay_lock = acquire_runtime_lock_for_inactive_operation(
        &runtime_locks,
        RuntimeLockKind::Overlay,
        CatalogOperation::ClearToolState,
    )?;
    if let Some(blocker) = service_status_blocker(Some(&status), CatalogOperation::ClearToolState) {
        return Err(blocker);
    }

    let options = named_session_options_for_catalog_item(&item, paths)?;
    let outcome = wayscriber::session::clear_tool_state(&options).map_err(|err| err.to_string())?;
    let items = load_session_catalog_sync(&catalog)?;
    Ok(SessionCatalogActionResult::success(
        clear_tool_state_catalog_message(&item.display_name, outcome),
        items,
    ))
}

pub(super) fn session_clear_cached_status_blocker(
    status: Option<&DaemonRuntimeStatus>,
) -> Option<String> {
    service_status_blocker(status, CatalogOperation::Clear)
}

pub(super) fn session_clear_tool_state_cached_status_blocker(
    status: Option<&DaemonRuntimeStatus>,
) -> Option<String> {
    service_status_blocker(status, CatalogOperation::ClearToolState)
}

pub(super) fn session_duplicate_cached_status_blocker(
    status: Option<&DaemonRuntimeStatus>,
) -> Option<String> {
    service_status_blocker(status, CatalogOperation::Duplicate)
}

pub(super) fn session_move_cached_status_blocker(
    status: Option<&DaemonRuntimeStatus>,
) -> Option<String> {
    service_status_blocker(status, CatalogOperation::Move)
}

fn service_status_blocker(
    status: Option<&DaemonRuntimeStatus>,
    operation: CatalogOperation,
) -> Option<String> {
    match status {
        Some(status) if status.service_active => Some(
            operation
                .running_message(RuntimeLockKind::Daemon)
                .to_string(),
        ),
        Some(_) => None,
        None => Some(operation.waiting_for_status_message().to_string()),
    }
}

fn load_session_catalog_sync(
    catalog: &wayscriber::session::catalog::SessionCatalog,
) -> Result<Vec<SessionCatalogItem>, String> {
    catalog
        .recent_sessions()
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(SessionCatalogItem::from_entry)
        .collect()
}

fn find_session_catalog_item(
    catalog: &wayscriber::session::catalog::SessionCatalog,
    id: &str,
) -> Result<SessionCatalogItem, String> {
    load_session_catalog_sync(catalog)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "Session is no longer in the catalog.".to_string())
}

fn named_session_options_for_catalog_item(
    item: &SessionCatalogItem,
    paths: &wayscriber::paths::PathResolver,
) -> Result<wayscriber::session::SessionOptions, String> {
    let config_store =
        wayscriber::config::ConfigStore::from_resolver(paths).map_err(|error| error.to_string())?;
    let loaded = config_store.load().map_err(|err| err.to_string())?;
    let mut options = wayscriber::session::options_from_config_for_named_file(
        &loaded.config.session,
        item.path.clone(),
        None,
    );
    options.force_resume_persistence();
    Ok(options)
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeLockPaths {
    daemon: PathBuf,
    overlay: PathBuf,
}

impl RuntimeLockPaths {
    fn prepare(paths: &wayscriber::paths::PathResolver) -> Result<Self, String> {
        let prepared = wayscriber::paths::PreparedRuntimePaths::prepare(paths)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            daemon: prepared.daemon_lock_file(),
            overlay: prepared.overlay_lock_file(),
        })
    }

    #[cfg(test)]
    fn fixture(root: &Path) -> Self {
        Self {
            daemon: root.join("daemon.lock"),
            overlay: root.join("overlay.lock"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CatalogOperation {
    Clear,
    ClearToolState,
    Duplicate,
    Move,
}

impl RuntimeLockKind {
    fn path(self, paths: &RuntimeLockPaths) -> &Path {
        match self {
            Self::Daemon => &paths.daemon,
            Self::Overlay => &paths.overlay,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Daemon => "daemon",
            Self::Overlay => "overlay",
        }
    }
}

impl CatalogOperation {
    fn running_message(self, kind: RuntimeLockKind) -> &'static str {
        match (self, kind) {
            (Self::Clear, RuntimeLockKind::Daemon) => {
                "Clear saved data is disabled while the background service or a manually started daemon is running. Stop it first or clear from the overlay."
            }
            (Self::Clear, RuntimeLockKind::Overlay) => {
                "Clear saved data is disabled while an overlay is running. Use the overlay Clear action for the active session."
            }
            (Self::ClearToolState, RuntimeLockKind::Daemon) => {
                "Clear saved tool state is disabled while the background service or a manually started daemon is running. Use the overlay command palette for the active session or stop it first."
            }
            (Self::ClearToolState, RuntimeLockKind::Overlay) => {
                "Clear saved tool state is disabled while an overlay is running. Use the command palette for the active session."
            }
            (Self::Duplicate, RuntimeLockKind::Daemon) => {
                "Duplicate Session is disabled while the background service or a manually started daemon is running. Stop it first or duplicate from the overlay after opening the session."
            }
            (Self::Duplicate, RuntimeLockKind::Overlay) => {
                "Duplicate Session is disabled while an overlay is running. Use Save As from the overlay for the active session."
            }
            (Self::Move, RuntimeLockKind::Daemon) => {
                "Move Session is disabled while the background service or a manually started daemon is running. Stop it first or move from the overlay after opening the session."
            }
            (Self::Move, RuntimeLockKind::Overlay) => {
                "Move Session is disabled while an overlay is running. Active session moves must use a runtime transaction."
            }
        }
    }

    fn waiting_for_status_message(self) -> &'static str {
        match self {
            Self::Clear => {
                "Clear saved data is disabled until background service status finishes loading."
            }
            Self::ClearToolState => {
                "Clear saved tool state is disabled until background service status finishes loading."
            }
            Self::Duplicate => {
                "Duplicate Session is disabled until background service status finishes loading."
            }
            Self::Move => {
                "Move Session is disabled until background service status finishes loading."
            }
        }
    }
}

#[cfg(test)]
fn runtime_lock_active(
    paths: &RuntimeLockPaths,
    kind: RuntimeLockKind,
    operation: CatalogOperation,
) -> Result<bool, String> {
    let path = kind.path(paths);
    let file = match OpenOptions::new().read(true).write(true).open(path) {
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
fn acquire_runtime_lock_for_clear(
    paths: &RuntimeLockPaths,
    kind: RuntimeLockKind,
) -> Result<File, String> {
    acquire_runtime_lock_for_inactive_operation(paths, kind, CatalogOperation::Clear)
}

fn acquire_runtime_lock_for_inactive_operation(
    paths: &RuntimeLockPaths,
    kind: RuntimeLockKind,
    operation: CatalogOperation,
) -> Result<File, String> {
    let path = kind.path(paths);
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
        .open(path)
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
            Err(operation.running_message(kind).to_string())
        }
        Err(err) => Err(format!(
            "{} is disabled because the {} lock could not be reserved: {} ({err})",
            operation.label(),
            kind.label(),
            path.display()
        )),
    }
}

impl CatalogOperation {
    fn label(self) -> &'static str {
        match self {
            Self::Clear => "Clear saved data",
            Self::ClearToolState => "Clear saved tool state",
            Self::Duplicate => "Duplicate Session",
            Self::Move => "Move Session",
        }
    }
}

#[cfg(test)]
fn drop_lock(file: File) {
    drop(file);
}

pub(super) fn session_artifact_status_label(item: &SessionCatalogItem) -> String {
    item.artifacts.status_label()
}

#[cfg(test)]
mod tests;
