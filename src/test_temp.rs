#[cfg(unix)]
use std::path::Path;

#[cfg(unix)]
use rustix::fs::{self as rustix_fs, FileType, Mode};
pub(crate) use tempfile::{TempDir, tempdir};

/// Creates a FIFO that only the current user can open.
#[cfg(unix)]
pub(crate) fn make_fifo(path: &Path) {
    rustix_fs::mknodat(
        rustix_fs::CWD,
        path,
        FileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        0,
    )
    .unwrap_or_else(|errno| panic!("mkfifo {} failed: {errno}", path.display()));
}
