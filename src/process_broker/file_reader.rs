//! Killable internal worker for bounded clipboard file reads.

use std::ffi::{OsStr, OsString};
use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result, bail};

use super::wire::MAX_OUTPUT_BYTES;

const MODE_ENV: &str = "WAYSCRIBER_INTERNAL_FILE_READER";
const PATH_ENV: &str = "WAYSCRIBER_INTERNAL_FILE_READER_PATH";
const LIMIT_ENV: &str = "WAYSCRIBER_INTERNAL_FILE_READER_LIMIT";
const MODE_V1: &str = "v1";

pub(super) const EXIT_READY: u8 = 0;
pub(super) const EXIT_EMPTY: u8 = 10;
pub(super) const EXIT_TOO_LARGE: u8 = 11;
pub(super) const EXIT_NOT_REGULAR: u8 = 12;
pub(super) const EXIT_READ_FAILED: u8 = 13;
const EXIT_INVALID_BOOTSTRAP: u8 = 126;

enum Classification {
    Ordinary,
    Reader { path: PathBuf, limit: usize },
    Invalid,
}

pub(super) fn run_if_requested() -> Option<ExitCode> {
    let classification = classify(
        std::env::var_os(MODE_ENV),
        std::env::var_os(PATH_ENV),
        std::env::var_os(LIMIT_ENV),
    );
    match classification {
        Classification::Ordinary => None,
        Classification::Invalid => Some(ExitCode::from(EXIT_INVALID_BOOTSTRAP)),
        Classification::Reader { path, limit } => Some(run(path, limit)),
    }
}

pub(super) fn command(path: &Path, limit: usize) -> Result<Command> {
    validate_request(path, limit)?;
    let executable = std::env::current_exe()
        .context("failed to resolve executable for internal clipboard file reader")?;
    let mut command = Command::new(executable);
    command
        .env(MODE_ENV, MODE_V1)
        .env(PATH_ENV, path)
        .env(LIMIT_ENV, limit.to_string())
        .env_remove(super::wire::BROKER_FD_ENV)
        .env_remove(super::wire::BROKER_SHUTDOWN_FD_ENV)
        .env_remove(super::wire::BROKER_TOKEN_ENV)
        .env_remove(crate::env_vars::DAEMON_WATCHDOG_FD_ENV);
    Ok(command)
}

fn classify(
    mode: Option<OsString>,
    path: Option<OsString>,
    limit: Option<OsString>,
) -> Classification {
    match (mode, path, limit) {
        (None, None, None) => Classification::Ordinary,
        (Some(mode), Some(path), Some(limit)) if mode == OsStr::new(MODE_V1) => {
            let Some(limit) = limit.to_str().and_then(|value| value.parse::<usize>().ok()) else {
                return Classification::Invalid;
            };
            let path = PathBuf::from(path);
            if validate_request(&path, limit).is_err() {
                Classification::Invalid
            } else {
                Classification::Reader { path, limit }
            }
        }
        _ => Classification::Invalid,
    }
}

fn validate_request(path: &Path, limit: usize) -> Result<()> {
    if !path.is_absolute() {
        bail!("clipboard file reader requires an absolute path");
    }
    if limit == 0 || limit > MAX_OUTPUT_BYTES {
        bail!("clipboard file reader byte limit is outside broker bounds");
    }
    Ok(())
}

fn run(path: PathBuf, limit: usize) -> ExitCode {
    match read_regular_file(&path, limit) {
        Ok(ReadOutcome::Ready(bytes)) => match io::stdout().lock().write_all(&bytes) {
            Ok(()) => ExitCode::from(EXIT_READY),
            Err(error) => {
                write_failure(&format!("failed to publish clipboard file bytes: {error}"))
            }
        },
        Ok(ReadOutcome::Empty) => ExitCode::from(EXIT_EMPTY),
        Ok(ReadOutcome::TooLarge) => ExitCode::from(EXIT_TOO_LARGE),
        Ok(ReadOutcome::NotRegular) => ExitCode::from(EXIT_NOT_REGULAR),
        Err(error) => write_failure(&error.to_string()),
    }
}

enum ReadOutcome {
    Ready(Vec<u8>),
    Empty,
    TooLarge,
    NotRegular,
}

fn read_regular_file(path: &Path, limit: usize) -> io::Result<ReadOutcome> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
            return Ok(ReadOutcome::NotRegular);
        }
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Ok(ReadOutcome::NotRegular);
    }
    if metadata.len() > limit as u64 {
        return Ok(ReadOutcome::TooLarge);
    }

    let initial_capacity =
        usize::try_from(metadata.len()).map_or(limit, |length| length.min(limit));
    let mut bytes = Vec::with_capacity(initial_capacity);
    let mut buffer = [0_u8; 8192];
    loop {
        let read = match file.read(&mut buffer) {
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if read == 0 {
            break;
        }
        if bytes.len().saturating_add(read) > limit {
            return Ok(ReadOutcome::TooLarge);
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    if bytes.is_empty() {
        Ok(ReadOutcome::Empty)
    } else {
        Ok(ReadOutcome::Ready(bytes))
    }
}

fn write_failure(reason: &str) -> ExitCode {
    let _ = io::stderr().lock().write_all(reason.as_bytes());
    ExitCode::from(EXIT_READ_FAILED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_temp::TempDir;
    use std::fs;
    use std::os::unix::fs::symlink;

    #[test]
    fn ordinary_entry_has_no_internal_reader_markers() {
        assert!(matches!(
            classify(None, None, None),
            Classification::Ordinary
        ));
    }

    #[test]
    fn partial_or_relative_bootstrap_is_invalid() {
        assert!(matches!(
            classify(Some(MODE_V1.into()), None, None),
            Classification::Invalid
        ));
        assert!(matches!(
            classify(
                Some(MODE_V1.into()),
                Some("relative.png".into()),
                Some("10".into())
            ),
            Classification::Invalid
        ));
    }

    #[test]
    fn regular_file_read_reports_ready_empty_and_too_large() {
        let temp = TempDir::new().expect("file-reader fixture creates an isolated directory");
        let path = temp.path().join("image.bin");
        fs::write(&path, b"abc").expect("file-reader fixture writes its regular file");
        assert!(matches!(
            read_regular_file(&path, 3),
            Ok(ReadOutcome::Ready(bytes)) if bytes == b"abc"
        ));
        assert!(matches!(
            read_regular_file(&path, 2),
            Ok(ReadOutcome::TooLarge)
        ));
        fs::write(&path, []).expect("file-reader fixture replaces its file with empty content");
        assert!(matches!(
            read_regular_file(&path, 3),
            Ok(ReadOutcome::Empty)
        ));
    }

    #[test]
    fn symlink_and_directory_are_not_regular_file_capabilities() {
        let temp = TempDir::new().expect("file-reader fixture creates an isolated directory");
        let target = temp.path().join("target.bin");
        let link = temp.path().join("link.bin");
        fs::write(&target, b"abc").expect("file-reader fixture writes its symlink target");
        symlink(&target, &link).expect("file-reader fixture creates its symlink");
        assert!(matches!(
            read_regular_file(&link, 3),
            Ok(ReadOutcome::NotRegular)
        ));
        assert!(matches!(
            read_regular_file(temp.path(), 3),
            Ok(ReadOutcome::NotRegular)
        ));
    }
}
