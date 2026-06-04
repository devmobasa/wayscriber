use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, anyhow};

use crate::paths::expand_tilde;

use super::super::primary::{
    PrimaryTargetKind, PrimaryTargetState, inspect_named_primary_target,
    open_session_artifact_for_read,
};
use super::types::session_file_parent_dir;

static NEXT_WRITE_PROBE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct MissingNamedSessionFile {
    path: PathBuf,
}

impl MissingNamedSessionFile {
    fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for MissingNamedSessionFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "named session file does not exist: {}",
            self.path.display()
        )
    }
}

impl Error for MissingNamedSessionFile {}

#[derive(Debug)]
pub struct MissingNamedSessionParent {
    path: PathBuf,
    parent: PathBuf,
}

impl MissingNamedSessionParent {
    fn new(path: &Path, parent: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            parent: parent.to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for MissingNamedSessionParent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "named session parent directory does not exist: {}",
            self.parent.display()
        )
    }
}

impl Error for MissingNamedSessionParent {}

pub fn normalize_named_session_file_arg(raw: &str) -> std::path::PathBuf {
    expand_tilde(raw)
}

pub fn validate_named_session_file_for_foreground(path: &Path) -> Result<()> {
    validate_named_session_file_shape(path)?;
    let parent = require_existing_parent_dir(path)?;
    validate_parent_writable_by_probe(&parent, "named session parent directory is not writable")?;
    Ok(())
}

pub fn validate_named_session_file_for_info(path: &Path) -> Result<()> {
    validate_named_session_file_shape(path)?;
    validate_existing_parent_if_present(path, "inspect")?;
    Ok(())
}

pub fn validate_named_session_file_for_clear(path: &Path) -> Result<()> {
    validate_named_session_file_shape(path)?;
    if let Some(parent) = validate_existing_parent_if_present(path, "clear")? {
        validate_parent_writable_by_probe(
            &parent,
            "named session parent directory is not writable for cleanup",
        )?;
    }
    Ok(())
}

pub fn validate_named_session_file_for_open(path: &Path) -> Result<()> {
    validate_named_session_file_shape(path)?;
    let parent = require_existing_parent_dir(path)?;

    match inspect_named_primary_target(path)? {
        PrimaryTargetState::Missing => Err(MissingNamedSessionFile::new(path).into()),
        PrimaryTargetState::Present {
            kind: PrimaryTargetKind::Regular,
            ..
        } => {
            validate_parent_writable_by_probe(
                &parent,
                "named session parent directory is not writable",
            )?;
            open_session_artifact_for_read(path, true).map(drop)?;
            Ok(())
        }
        PrimaryTargetState::Present {
            kind: PrimaryTargetKind::Directory,
            ..
        } => Err(anyhow!(
            "--session-file must name a session file, not a directory: {}",
            path.display()
        )),
        PrimaryTargetState::Present {
            kind: PrimaryTargetKind::Symlink,
            ..
        } => Err(anyhow!(
            "--session-file must name a regular session file, not a symlink: {}",
            path.display()
        )),
        PrimaryTargetState::Present {
            kind: PrimaryTargetKind::Special,
            ..
        } => Err(anyhow!(
            "--session-file must name a regular session file, not a special file: {}",
            path.display()
        )),
    }
}

fn require_existing_parent_dir(path: &Path) -> Result<PathBuf> {
    let parent = session_file_parent_dir(path);
    let parent_metadata = match fs::metadata(&parent) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Err(MissingNamedSessionParent::new(path, &parent).into());
        }
        Err(err) => {
            return Err(err).with_context(|| {
                format!(
                    "failed to inspect named session parent directory {}",
                    parent.display()
                )
            });
        }
    };
    if !parent_metadata.is_dir() {
        return Err(anyhow!(
            "named session parent is not a directory: {}",
            parent.display()
        ));
    }
    Ok(parent)
}

fn validate_parent_writable_by_probe(parent: &Path, message: &str) -> Result<()> {
    for _ in 0..100 {
        let probe_path = write_probe_path(parent);
        let probe_file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe_path)
        {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err).with_context(|| format!("{message}: {}", parent.display()));
            }
        };
        drop(probe_file);
        fs::remove_file(&probe_path).with_context(|| {
            format!(
                "failed to remove named session writability probe {}",
                probe_path.display()
            )
        })?;
        return Ok(());
    }

    Err(anyhow!(
        "failed to create a unique named session writability probe in {}",
        parent.display()
    ))
}

fn write_probe_path(parent: &Path) -> PathBuf {
    let id = NEXT_WRITE_PROBE_ID.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(
        ".wayscriber-session-write-test-{}-{id}",
        std::process::id()
    ))
}

fn validate_named_session_file_shape(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(anyhow!("--session-file requires a non-empty path"));
    }

    if has_trailing_separator(path) {
        return Err(anyhow!(
            "--session-file must name a session file, not a directory: {}",
            path.display()
        ));
    }

    match inspect_named_primary_target(path)? {
        PrimaryTargetState::Present {
            kind: PrimaryTargetKind::Directory,
            ..
        } => Err(anyhow!(
            "--session-file must name a session file, not a directory: {}",
            path.display()
        )),
        PrimaryTargetState::Present {
            kind: PrimaryTargetKind::Regular,
            ..
        }
        | PrimaryTargetState::Missing => Ok(()),
        PrimaryTargetState::Present {
            kind: PrimaryTargetKind::Symlink,
            ..
        } => Err(anyhow!(
            "--session-file must name a regular session file, not a symlink: {}",
            path.display()
        )),
        PrimaryTargetState::Present {
            kind: PrimaryTargetKind::Special,
            ..
        } => Err(anyhow!(
            "--session-file must name a regular session file, not a special file: {}",
            path.display()
        )),
    }
}

fn has_trailing_separator(path: &Path) -> bool {
    path.as_os_str()
        .as_encoded_bytes()
        .last()
        .is_some_and(|last| *last == std::path::MAIN_SEPARATOR as u8)
}

fn validate_existing_parent_if_present(path: &Path, operation: &str) -> Result<Option<PathBuf>> {
    let parent = session_file_parent_dir(path);
    match fs::metadata(&parent) {
        Ok(metadata) if metadata.is_dir() => Ok(Some(parent)),
        Ok(_) => Err(anyhow!(
            "cannot {operation} named session because parent is not a directory: {}",
            parent.display()
        )),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| {
            format!(
                "cannot {operation} named session because parent cannot be inspected: {}",
                parent.display()
            )
        }),
    }
}
