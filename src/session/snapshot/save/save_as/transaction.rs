use super::super::sync_session_parent_dir;
use anyhow::{Context, Result};
use log::warn;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
use crate::session::artifacts::SAVE_AS_STAGING_PREFIX;
use crate::session::artifacts::{SaveAsSidecarQuarantine, quarantine_save_as_sidecars};

pub(super) fn commit_save_as_target(
    tmp_path: &Path,
    session_path: &Path,
    sidecars: &[PathBuf],
) -> Result<()> {
    commit_save_as_target_with(
        tmp_path,
        session_path,
        sidecars,
        |source, target| fs::rename(source, target),
        || sync_session_parent_dir(session_path, "Save Session As file"),
        SaveAsSidecarQuarantine::remove_after_commit,
    )
}

fn commit_save_as_target_with<Replace, Sync, Cleanup>(
    tmp_path: &Path,
    session_path: &Path,
    sidecars: &[PathBuf],
    replace_primary: Replace,
    sync_primary: Sync,
    cleanup_sidecars: Cleanup,
) -> Result<()>
where
    Replace: FnOnce(&Path, &Path) -> std::io::Result<()>,
    Sync: FnOnce() -> Result<()>,
    Cleanup: FnOnce(SaveAsSidecarQuarantine) -> Result<()>,
{
    let quarantine = quarantine_save_as_sidecars(sidecars, session_path)?;
    let had_quarantined_sidecars = !quarantine.is_empty();
    if let Err(err) = replace_primary(tmp_path, session_path) {
        return match quarantine.rollback() {
            Ok(()) => Err(err).with_context(|| {
                if had_quarantined_sidecars {
                    format!(
                        "failed to move temporary Save Session As file {} -> {}; restored stale sidecars",
                        tmp_path.display(),
                        session_path.display()
                    )
                } else {
                    format!(
                        "failed to move temporary Save Session As file {} -> {}",
                        tmp_path.display(),
                        session_path.display()
                    )
                }
            }),
            Err(rollback_err) => Err(err).with_context(|| {
                format!(
                    "partial Save Session As failure for {}: primary replacement failed and sidecar rollback also failed: {rollback_err:#}",
                    session_path.display()
                )
            }),
        };
    }

    if let Err(err) = sync_primary() {
        return Err(err).with_context(|| {
            let quarantine = quarantine
                .directory()
                .map(|path| format!("; stale sidecars retained in {}", path.display()))
                .unwrap_or_default();
            format!(
                "Save Session As primary replacement for {} was not durably synced{quarantine}",
                session_path.display()
            )
        });
    }

    let quarantine_path = quarantine.directory().map(Path::to_path_buf);
    if let Err(err) = cleanup_sidecars(quarantine) {
        warn!(
            "Save Session As primary committed to {}, but stale sidecar quarantine cleanup failed{}: {err:#}",
            session_path.display(),
            quarantine_path
                .as_deref()
                .map(|path| format!(" for {}", path.display()))
                .unwrap_or_default()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
