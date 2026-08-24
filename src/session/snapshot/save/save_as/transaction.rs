use super::super::sync_session_parent_dir;
use anyhow::{Context, Result, anyhow};
use log::{info, warn};
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const SAVE_AS_STAGING_PREFIX: &str = ".wayscriber-save-as-sidecars";
static NEXT_SAVE_AS_STAGING_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct QuarantinedSidecar {
    original: PathBuf,
    staged: PathBuf,
}

#[derive(Debug)]
struct SaveAsSidecarQuarantine {
    directory: Option<PathBuf>,
    sidecars: Vec<QuarantinedSidecar>,
}

impl SaveAsSidecarQuarantine {
    fn empty() -> Self {
        Self {
            directory: None,
            sidecars: Vec::new(),
        }
    }

    fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }

    fn rollback(self) -> Result<()> {
        let mut failures = Vec::new();
        for sidecar in self.sidecars.iter().rev() {
            if let Err(err) = crate::session::artifacts::rename_artifact_no_replace(
                &sidecar.staged,
                &sidecar.original,
            ) {
                failures.push(format!(
                    "{} -> {}: {err}",
                    sidecar.staged.display(),
                    sidecar.original.display()
                ));
            }
        }

        if failures.is_empty()
            && let Some(directory) = &self.directory
        {
            fs::remove_dir(directory).with_context(|| {
                format!(
                    "failed to remove rolled-back Save Session As quarantine {}",
                    directory.display()
                )
            })?;
            crate::durable_io::sync_parent_dir(directory).with_context(|| {
                format!(
                    "failed to sync restored Save Session As sidecars under {}",
                    directory.display()
                )
            })?;
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(
                "failed to restore quarantined Save Session As sidecars: {}",
                failures.join("; ")
            ))
        }
    }

    fn remove_after_commit(self) -> Result<()> {
        let mut failures = Vec::new();
        for sidecar in &self.sidecars {
            match fs::remove_file(&sidecar.staged) {
                Ok(()) => info!(
                    "Removed stale Save Session As sidecar after commit: {}",
                    sidecar.original.display()
                ),
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => failures.push(format!("{}: {err}", sidecar.staged.display())),
            }
        }

        if failures.is_empty()
            && let Some(directory) = &self.directory
        {
            fs::remove_dir(directory).with_context(|| {
                format!(
                    "failed to remove Save Session As sidecar quarantine {}",
                    directory.display()
                )
            })?;
            crate::durable_io::sync_parent_dir(directory).with_context(|| {
                format!(
                    "failed to sync Save Session As sidecar cleanup under {}",
                    directory.display()
                )
            })?;
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(
                "failed to remove quarantined Save Session As sidecars: {}",
                failures.join("; ")
            ))
        }
    }
}

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
    let had_quarantined_sidecars = !quarantine.sidecars.is_empty();
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

    let sync_result = sync_primary();
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
    sync_result
}

fn quarantine_save_as_sidecars(
    sidecars: &[PathBuf],
    session_path: &Path,
) -> Result<SaveAsSidecarQuarantine> {
    let Some(parent) = session_path.parent() else {
        return Err(anyhow!(
            "Save Session As target has no parent directory: {}",
            session_path.display()
        ));
    };

    let mut present = Vec::new();
    for path in sidecars {
        if path.parent() != Some(parent) {
            return Err(anyhow!(
                "Save Session As sidecar is outside the target directory: {}",
                path.display()
            ));
        }
        let Some(file_name) = path.file_name() else {
            return Err(anyhow!(
                "Save Session As sidecar has no file name: {}",
                path.display()
            ));
        };
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_dir() => {
                return Err(anyhow!(
                    "refusing to replace Save Session As sidecar directory {}",
                    path.display()
                ));
            }
            Ok(_) => present.push((path.clone(), file_name.to_os_string())),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to inspect Save Session As sidecar {}",
                        path.display()
                    )
                });
            }
        }
    }
    if present.is_empty() {
        return Ok(SaveAsSidecarQuarantine::empty());
    }

    let directory = create_save_as_staging_dir(parent)?;
    let mut quarantine = SaveAsSidecarQuarantine {
        directory: Some(directory.clone()),
        sidecars: Vec::with_capacity(present.len()),
    };
    for (index, (original, file_name)) in present.into_iter().enumerate() {
        let mut staged_name = OsString::from(format!("{index}-"));
        staged_name.push(file_name);
        let staged = directory.join(staged_name);
        match crate::session::artifacts::rename_artifact_no_replace(&original, &staged) {
            Ok(()) => quarantine
                .sidecars
                .push(QuarantinedSidecar { original, staged }),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return match quarantine.rollback() {
                    Ok(()) => Err(err).with_context(|| {
                        format!(
                            "failed to quarantine Save Session As sidecar {}; restored earlier sidecars",
                            original.display()
                        )
                    }),
                    Err(rollback_err) => Err(err).with_context(|| {
                        format!(
                            "failed to quarantine Save Session As sidecar {}, and rollback also failed: {rollback_err:#}",
                            original.display()
                        )
                    }),
                };
            }
        }
    }
    Ok(quarantine)
}

fn create_save_as_staging_dir(parent: &Path) -> Result<PathBuf> {
    for _ in 0..1024 {
        let id = NEXT_SAVE_AS_STAGING_ID.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            "{SAVE_AS_STAGING_PREFIX}-{}-{id}",
            std::process::id()
        ));
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        match builder.create(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to create Save Session As sidecar quarantine {}",
                        candidate.display()
                    )
                });
            }
        }
    }
    Err(anyhow!(
        "failed to allocate a unique Save Session As sidecar quarantine under {}",
        parent.display()
    ))
}

#[cfg(test)]
mod tests;
