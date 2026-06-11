use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::env_vars::{LOG_FILE_ENV, LOG_MAX_SIZE_ENV};
use crate::{paths, time_utils};

const BYTES_PER_MB: u64 = 1024 * 1024;
const DEFAULT_LOG_MAX_BYTES: u64 = 10 * BYTES_PER_MB;

pub(super) fn resolve_log_target() -> LogFileTarget {
    if let Ok(path) = env::var(LOG_FILE_ENV)
        && !path.trim().is_empty()
    {
        let trimmed = path.trim();
        let expanded = paths::expand_tilde(trimmed);
        let mut treat_as_dir = trimmed.ends_with('/') || trimmed.ends_with('\\');
        if !treat_as_dir && let Ok(metadata) = fs::metadata(&expanded) {
            treat_as_dir = metadata.is_dir();
        }
        let append_date = if treat_as_dir {
            true
        } else {
            expanded.extension().is_none()
        };
        return LogFileTarget {
            base: expanded,
            treat_as_dir,
            append_date,
        };
    }

    LogFileTarget {
        base: paths::log_dir(),
        treat_as_dir: true,
        append_date: true,
    }
}

fn resolve_log_max_bytes() -> u64 {
    env::var(LOG_MAX_SIZE_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.saturating_mul(BYTES_PER_MB))
        .unwrap_or(DEFAULT_LOG_MAX_BYTES)
}

fn current_log_date() -> String {
    time_utils::format_with_template(time_utils::now_local(), "%Y-%m-%d")
}

pub(super) struct LogFileTarget {
    base: PathBuf,
    treat_as_dir: bool,
    append_date: bool,
}

impl LogFileTarget {
    fn path_for_date(&self, date: &str) -> PathBuf {
        if self.treat_as_dir {
            return self.base.join(format!("wayscriber-{}.log", date));
        }
        if !self.append_date {
            return self.base.clone();
        }

        let file_name = dated_log_file_name(&self.base, date);
        self.base.with_file_name(file_name)
    }

    fn path_for_date_and_index(&self, date: &str, index: u32) -> PathBuf {
        let base = self.path_for_date(date);
        if index == 0 {
            base
        } else {
            append_index_to_path(&base, index)
        }
    }
}

fn dated_log_file_name(path: &Path, date: &str) -> OsString {
    let mut name = OsString::new();
    if let Some(stem) = path.file_stem() {
        name.push(stem);
    } else if let Some(file_name) = path.file_name() {
        name.push(file_name);
    } else {
        name.push("wayscriber");
    }
    name.push("-");
    name.push(date);
    if let Some(ext) = path.extension() {
        name.push(".");
        name.push(ext);
    } else {
        name.push(".log");
    }
    name
}

fn append_index_to_path(path: &Path, index: u32) -> PathBuf {
    let mut name = OsString::new();
    if let Some(stem) = path.file_stem() {
        name.push(stem);
    } else if let Some(file_name) = path.file_name() {
        name.push(file_name);
    } else {
        name.push("wayscriber");
    }
    name.push("-");
    name.push(index.to_string());
    if let Some(ext) = path.extension() {
        name.push(".");
        name.push(ext);
    }
    path.with_file_name(name)
}

pub(super) struct DailyFileWriter {
    target: LogFileTarget,
    max_bytes: u64,
    current_date: Option<String>,
    current_size: u64,
    file: Option<fs::File>,
    error_date: Option<String>,
}

impl DailyFileWriter {
    pub(super) fn new(target: LogFileTarget) -> Self {
        Self {
            target,
            max_bytes: resolve_log_max_bytes(),
            current_date: None,
            current_size: 0,
            file: None,
            error_date: None,
        }
    }

    fn ensure_file(&mut self) {
        let date = current_log_date();
        if self.error_date.as_deref() == Some(date.as_str()) {
            return;
        }
        if self.current_date.as_deref() == Some(date.as_str())
            && self.file.is_some()
            && self.current_size < self.max_bytes
        {
            return;
        }

        let (path, size) = match self.select_log_path(&date) {
            Ok(result) => result,
            Err(err) => {
                eprintln!("Failed to resolve log file for {}: {}", date, err);
                self.error_date = Some(date);
                return;
            }
        };
        if let Some(parent) = path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            eprintln!(
                "Failed to create log directory {}: {}",
                parent.display(),
                err
            );
            self.error_date = Some(date);
            return;
        }

        match fs::OpenOptions::new().create(true).append(true).open(&path) {
            Ok(file) => {
                self.file = Some(file);
                self.current_date = Some(date);
                self.current_size = size;
                self.error_date = None;
            }
            Err(err) => {
                eprintln!("Failed to open log file {}: {}", path.display(), err);
                self.error_date = Some(date);
            }
        }
    }

    fn select_log_path(&self, date: &str) -> io::Result<(PathBuf, u64)> {
        let mut index = 0u32;
        loop {
            let path = self.target.path_for_date_and_index(date, index);
            match fs::metadata(&path) {
                Ok(metadata) => {
                    let size = metadata.len();
                    if size < self.max_bytes {
                        return Ok((path, size));
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    return Ok((path, 0));
                }
                Err(err) => return Err(err),
            }

            index = index
                .checked_add(1)
                .ok_or_else(|| io::Error::other("log index overflow"))?;
        }
    }
}

impl Write for DailyFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.ensure_file();
        if let Some(file) = self.file.as_mut() {
            file.write_all(buf)?;
            self.current_size = self.current_size.saturating_add(buf.len() as u64);
            if self.current_size >= self.max_bytes {
                self.file = None;
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.ensure_file();
        if let Some(file) = self.file.as_mut() {
            file.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::LogFileTarget;
    use std::path::PathBuf;

    #[test]
    fn dated_log_file_name_preserves_or_adds_extension() {
        let target = LogFileTarget {
            base: PathBuf::from("/tmp/wayscriber.log"),
            treat_as_dir: false,
            append_date: true,
        };
        assert_eq!(
            target.path_for_date("2026-05-21"),
            PathBuf::from("/tmp/wayscriber-2026-05-21.log")
        );

        let target = LogFileTarget {
            base: PathBuf::from("/tmp/wayscriber"),
            treat_as_dir: false,
            append_date: true,
        };
        assert_eq!(
            target.path_for_date("2026-05-21"),
            PathBuf::from("/tmp/wayscriber-2026-05-21.log")
        );
    }

    #[test]
    fn indexed_log_path_preserves_extension() {
        let target = LogFileTarget {
            base: PathBuf::from("/tmp/wayscriber.log"),
            treat_as_dir: false,
            append_date: true,
        };

        assert_eq!(
            target.path_for_date_and_index("2026-05-21", 2),
            PathBuf::from("/tmp/wayscriber-2026-05-21-2.log")
        );
    }
}
