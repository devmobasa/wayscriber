use anyhow::{Context, Result};
use std::error::Error;
use std::fmt;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrimaryTargetKind {
    Regular,
    Directory,
    Symlink,
    Special,
}

#[derive(Debug)]
pub(crate) enum PrimaryTargetState {
    Missing,
    Present {
        metadata: Metadata,
        kind: PrimaryTargetKind,
    },
}

#[derive(Debug)]
pub(crate) struct NonRegularSessionArtifact {
    path: PathBuf,
    kind: PrimaryTargetKind,
}

impl NonRegularSessionArtifact {
    fn new(path: &Path, kind: PrimaryTargetKind) -> Self {
        Self {
            path: path.to_path_buf(),
            kind,
        }
    }
}

impl fmt::Display for NonRegularSessionArtifact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            PrimaryTargetKind::Directory => write!(
                f,
                "session artifact is a directory, not a regular file: {}",
                self.path.display()
            ),
            PrimaryTargetKind::Symlink => write!(
                f,
                "session artifact is a symlink, not a regular file: {}",
                self.path.display()
            ),
            PrimaryTargetKind::Special => write!(
                f,
                "session artifact is not a regular file: {}",
                self.path.display()
            ),
            PrimaryTargetKind::Regular => write!(
                f,
                "session artifact is not a regular file: {}",
                self.path.display()
            ),
        }
    }
}

impl Error for NonRegularSessionArtifact {}

pub(crate) fn is_non_regular_session_artifact(err: &anyhow::Error) -> bool {
    err.downcast_ref::<NonRegularSessionArtifact>().is_some()
}

pub(crate) fn inspect_named_primary_target(path: &Path) -> Result<PrimaryTargetState> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(PrimaryTargetState::Missing),
        Err(err) => {
            return Err(err).with_context(|| {
                format!("failed to inspect named session path {}", path.display())
            });
        }
    };

    Ok(PrimaryTargetState::Present {
        kind: classify_metadata(&metadata),
        metadata,
    })
}

pub(crate) fn session_artifact_metadata(path: &Path, no_follow: bool) -> Result<Metadata> {
    let metadata = raw_metadata(path, no_follow)
        .with_context(|| format!("failed to stat session file {}", path.display()))?;
    ensure_regular_session_artifact(path, metadata)
}

pub(crate) fn session_artifact_metadata_if_exists(
    path: &Path,
    no_follow: bool,
) -> Result<Option<Metadata>> {
    let metadata = match raw_metadata(path, no_follow) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to stat session file {}", path.display()));
        }
    };
    ensure_regular_session_artifact(path, metadata).map(Some)
}

pub(crate) fn open_session_artifact_for_read(path: &Path, no_follow: bool) -> Result<File> {
    session_artifact_metadata(path, no_follow)?;

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    if no_follow {
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }

    let file = match options.open(path) {
        Ok(file) => file,
        Err(err) if no_follow && open_error_is_symlink(&err) => {
            return Err(NonRegularSessionArtifact::new(path, PrimaryTargetKind::Symlink).into());
        }
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to open session file {}", path.display()));
        }
    };
    let metadata = file
        .metadata()
        .with_context(|| format!("failed to inspect opened session file {}", path.display()))?;
    ensure_regular_session_artifact(path, metadata)?;
    Ok(file)
}

fn raw_metadata(path: &Path, no_follow: bool) -> std::io::Result<Metadata> {
    if no_follow {
        fs::symlink_metadata(path)
    } else {
        fs::metadata(path)
    }
}

fn classify_metadata(metadata: &Metadata) -> PrimaryTargetKind {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        PrimaryTargetKind::Symlink
    } else if metadata.is_dir() {
        PrimaryTargetKind::Directory
    } else if metadata.is_file() {
        PrimaryTargetKind::Regular
    } else {
        PrimaryTargetKind::Special
    }
}

fn ensure_regular_session_artifact(path: &Path, metadata: Metadata) -> Result<Metadata> {
    let kind = classify_metadata(&metadata);
    if kind == PrimaryTargetKind::Regular {
        return Ok(metadata);
    }
    Err(NonRegularSessionArtifact::new(path, kind).into())
}

#[cfg(unix)]
fn open_error_is_symlink(err: &std::io::Error) -> bool {
    err.raw_os_error() == Some(libc::ELOOP)
}

#[cfg(not(unix))]
fn open_error_is_symlink(_err: &std::io::Error) -> bool {
    false
}
