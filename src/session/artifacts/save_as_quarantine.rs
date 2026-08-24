use anyhow::{Context, Result, anyhow};
use log::info;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Seek, SeekFrom};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{ArtifactMovePaths, rollback_artifact_moves};

pub(crate) const SAVE_AS_STAGING_PREFIX: &str = ".wayscriber-save-as-sidecars";
static NEXT_SAVE_AS_STAGING_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct QuarantinedSidecar {
    original: PathBuf,
    staged: PathBuf,
}

impl ArtifactMovePaths for QuarantinedSidecar {
    fn source(&self) -> &Path {
        &self.original
    }

    fn target(&self) -> &Path {
        &self.staged
    }
}

#[derive(Debug)]
pub(crate) struct SaveAsSidecarQuarantine {
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

    pub(crate) fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sidecars.is_empty()
    }

    pub(crate) fn rollback(self) -> Result<()> {
        rollback_artifact_moves(&self.sidecars)
            .context("failed to restore quarantined Save Session As sidecars")?;

        if let Some(directory) = &self.directory {
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
        Ok(())
    }

    pub(crate) fn remove_after_commit(self) -> Result<()> {
        self.remove_after_commit_with(
            |path| fs::remove_file(path),
            |path| fs::remove_dir(path),
            |path| crate::durable_io::sync_parent_dir(path).map_err(Into::into),
        )
    }

    fn remove_after_commit_with<RemoveFile, RemoveDir, SyncParent>(
        self,
        mut remove_file: RemoveFile,
        remove_dir: RemoveDir,
        sync_parent: SyncParent,
    ) -> Result<()>
    where
        RemoveFile: FnMut(&Path) -> std::io::Result<()>,
        RemoveDir: FnOnce(&Path) -> std::io::Result<()>,
        SyncParent: FnOnce(&Path) -> Result<()>,
    {
        let Some(directory) = self.directory.as_deref() else {
            return Ok(());
        };
        let mut retained = self
            .sidecars
            .iter()
            .map(RetainedSidecar::open)
            .collect::<Result<Vec<_>>>()?;

        for sidecar in &self.sidecars {
            match remove_file(&sidecar.staged) {
                Ok(()) => {}
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => {
                    let cleanup_err = anyhow!(
                        "failed to remove quarantined Save Session As sidecar {}: {err}",
                        sidecar.staged.display()
                    );
                    return retain_after_cleanup_failure(directory, &mut retained, cleanup_err);
                }
            }
        }

        if let Err(err) = remove_dir(directory) {
            let cleanup_err = anyhow!(
                "failed to remove Save Session As sidecar quarantine {}: {err}",
                directory.display()
            );
            return retain_after_cleanup_failure(directory, &mut retained, cleanup_err);
        }
        if let Err(err) = sync_parent(directory) {
            let cleanup_err = err.context(format!(
                "failed to sync Save Session As sidecar cleanup under {}",
                directory.display()
            ));
            return retain_after_cleanup_failure(directory, &mut retained, cleanup_err);
        }

        for sidecar in &self.sidecars {
            info!(
                "Removed stale Save Session As sidecar after commit: {}",
                sidecar.original.display()
            );
        }
        Ok(())
    }
}

struct RetainedSidecar {
    staged: PathBuf,
    file: File,
}

impl RetainedSidecar {
    fn open(sidecar: &QuarantinedSidecar) -> Result<Self> {
        let file = File::open(&sidecar.staged).with_context(|| {
            format!(
                "failed to retain quarantined Save Session As sidecar {} before cleanup",
                sidecar.staged.display()
            )
        })?;
        Ok(Self {
            staged: sidecar.staged.clone(),
            file,
        })
    }

    fn restore_if_missing(&mut self) -> Result<()> {
        match fs::symlink_metadata(&self.staged) {
            Ok(_) => return Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to inspect retained Save Session As sidecar {}",
                        self.staged.display()
                    )
                });
            }
        }

        self.file.seek(SeekFrom::Start(0)).with_context(|| {
            format!(
                "failed to rewind retained sidecar {}",
                self.staged.display()
            )
        })?;
        let mut restored = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&self.staged)
            .with_context(|| {
                format!(
                    "failed to recreate retained Save Session As sidecar {}",
                    self.staged.display()
                )
            })?;
        std::io::copy(&mut self.file, &mut restored).with_context(|| {
            format!(
                "failed to restore retained Save Session As sidecar {}",
                self.staged.display()
            )
        })?;
        restored.sync_all().with_context(|| {
            format!(
                "failed to sync restored Save Session As sidecar {}",
                self.staged.display()
            )
        })?;
        Ok(())
    }
}

fn retain_after_cleanup_failure(
    directory: &Path,
    retained: &mut [RetainedSidecar],
    cleanup_err: anyhow::Error,
) -> Result<()> {
    let restore_result = (|| {
        match fs::symlink_metadata(directory) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(anyhow!(
                    "Save Session As quarantine path is not a directory: {}",
                    directory.display()
                ));
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let mut builder = fs::DirBuilder::new();
                builder.mode(0o700);
                builder.create(directory).with_context(|| {
                    format!(
                        "failed to recreate Save Session As quarantine {}",
                        directory.display()
                    )
                })?;
            }
            Err(err) => return Err(err).context("failed to inspect Save Session As quarantine"),
        }

        for sidecar in retained.iter_mut() {
            sidecar.restore_if_missing()?;
        }
        if let Some(first) = retained.first() {
            crate::durable_io::sync_parent_dir(&first.staged).with_context(|| {
                format!(
                    "failed to sync retained Save Session As sidecars in {}",
                    directory.display()
                )
            })?;
        }
        crate::durable_io::sync_parent_dir(directory).with_context(|| {
            format!(
                "failed to sync retained Save Session As quarantine {}",
                directory.display()
            )
        })?;
        Ok::<(), anyhow::Error>(())
    })();

    match restore_result {
        Ok(()) => Err(cleanup_err).with_context(|| {
            format!(
                "stale sidecar cleanup failed; recovery bytes retained in private quarantine {}",
                directory.display()
            )
        }),
        Err(restore_err) => Err(cleanup_err).with_context(|| {
            format!(
                "stale sidecar cleanup failed, and recovery-byte retention in {} also failed: {restore_err:#}",
                directory.display()
            )
        }),
    }
}

pub(crate) fn quarantine_save_as_sidecars(
    sidecars: &[PathBuf],
    session_path: &Path,
) -> Result<SaveAsSidecarQuarantine> {
    quarantine_save_as_sidecars_with(
        sidecars,
        session_path,
        |directory| crate::durable_io::sync_directory(directory).map_err(Into::into),
        |directory| crate::durable_io::sync_parent_dir(directory).map_err(Into::into),
    )
}

fn quarantine_save_as_sidecars_with<SyncDirectory, SyncParent>(
    sidecars: &[PathBuf],
    session_path: &Path,
    sync_directory: SyncDirectory,
    sync_parent: SyncParent,
) -> Result<SaveAsSidecarQuarantine>
where
    SyncDirectory: FnOnce(&Path) -> Result<()>,
    SyncParent: FnOnce(&Path) -> Result<()>,
{
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
            Ok(metadata) if metadata.file_type().is_file() => {
                present.push((path.clone(), file_name.to_os_string()));
            }
            Ok(metadata) => {
                let kind = if metadata.file_type().is_symlink() {
                    "symlink"
                } else if metadata.is_dir() {
                    "directory"
                } else {
                    "special file"
                };
                return Err(anyhow!(
                    "refusing to replace Save Session As sidecar {kind} {}",
                    path.display()
                ));
            }
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
        match super::rename_artifact_no_replace(&original, &staged) {
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

    let sync_result = sync_directory(&directory)
        .with_context(|| {
            format!(
                "failed to sync Save Session As sidecar quarantine {}",
                directory.display()
            )
        })
        .and_then(|()| {
            sync_parent(&directory).with_context(|| {
                format!(
                    "failed to sync parent of Save Session As sidecar quarantine {}",
                    directory.display()
                )
            })
        });
    if let Err(err) = sync_result {
        return match quarantine.rollback() {
            Ok(()) => Err(err).context(
                "failed to make quarantined Save Session As sidecars durable; restored sidecars",
            ),
            Err(rollback_err) => Err(err).context(format!(
                "failed to make quarantined Save Session As sidecars durable, and rollback also failed: {rollback_err:#}"
            )),
        };
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
