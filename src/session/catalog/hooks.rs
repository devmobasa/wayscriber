//! Catalog integration policy for snapshot loading and committed runtime operations.
use anyhow::Result;
use log::warn;

use super::{CatalogEvent, upsert_session_event};
use crate::session::options::SessionOptions;

#[cfg(test)]
use {
    super::identity::normalize_exact_path, crate::env_vars::CATALOG_HOOKS_TEST_ENV, std::path::Path,
};

/// Record a committed named Open, preserving failures for runtime feedback.
pub(crate) fn try_record_named_session_opened(options: &SessionOptions) -> Result<()> {
    if !options.is_named_file() {
        return Ok(());
    }

    let path = options.session_file_path();

    #[cfg(test)]
    if !test_catalog_hooks_enabled_for_path(&path) {
        return Ok(());
    }

    upsert_session_event(&path, CatalogEvent::Opened)?;

    Ok(())
}

/// Snapshot loading remains best effort: a catalog failure must not reject loaded data.
pub(crate) fn record_named_session_opened(options: &SessionOptions) {
    if let Err(err) = try_record_named_session_opened(options) {
        let path = options.session_file_path();
        warn!(
            "Failed to update named session catalog after opening {}: {}",
            path.display(),
            err,
        );
    }
}

/// Saves keep successful disk writes successful even when catalog bookkeeping fails.
pub(crate) fn record_named_session_saved(options: &SessionOptions) {
    if !options.is_named_file() {
        return;
    }

    let path = options.session_file_path();

    #[cfg(test)]
    if !test_catalog_hooks_enabled_for_path(&path) {
        return;
    }

    if path.is_file()
        && let Err(err) = upsert_session_event(&path, CatalogEvent::Saved)
    {
        warn!(
            "Failed to update named session catalog after saving {}: {}",
            path.display(),
            err,
        );
    }
}

#[cfg(test)]
fn test_catalog_hooks_enabled_for_path(path: &Path) -> bool {
    let Some(raw) = std::env::var_os(CATALOG_HOOKS_TEST_ENV) else {
        return false;
    };
    if raw.is_empty() || raw == std::ffi::OsStr::new("1") {
        return true;
    }

    normalize_exact_path(path).starts_with(normalize_exact_path(Path::new(&raw)))
}
