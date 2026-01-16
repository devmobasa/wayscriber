mod about_window;
mod app;
mod backend;
mod capture;
mod cli;
mod config;
mod daemon;
mod draw;
mod input;
mod label_format;
mod notification;
mod onboarding;
mod paths;
mod session;
mod session_override;
mod time_utils;
mod toolbar_icons;
mod tray_action;
mod ui;
mod util;

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::Parser;
use log::LevelFilter;

pub use session_override::{
    RESUME_SESSION_ENV, SESSION_OVERRIDE_FOLLOW_CONFIG, SESSION_OVERRIDE_FORCE_OFF,
    SESSION_OVERRIDE_FORCE_ON, SESSION_RESUME_OVERRIDE, decode_session_override,
    encode_session_override, runtime_session_override, set_runtime_session_override,
};

fn main() {
    let cli = cli::Cli::parse();
    init_logging(&cli);

    if let Err(err) = app::run(cli) {
        let already_running = err
            .chain()
            .any(|cause| cause.is::<daemon::AlreadyRunningError>());
        if already_running {
            eprintln!("wayscriber daemon is already running");
            std::process::exit(75);
        }
        eprintln!("{:#}", err);
        std::process::exit(1);
    }
}

fn init_logging(cli: &cli::Cli) {
    let mut builder = env_logger::Builder::from_default_env();
    if env::var_os("RUST_LOG").is_none() {
        builder.filter_level(LevelFilter::Info);
    }

    let log_to_file = cli.daemon || cli.active;
    if log_to_file {
        let target = resolve_log_target();
        let file_writer = DailyFileWriter::new(target);
        let tee = TeeWriter::new(Box::new(io::stderr()), Box::new(file_writer));
        builder.target(env_logger::Target::Pipe(Box::new(tee)));
        builder.format_timestamp_millis();
    }

    builder.init();
}

const BYTES_PER_MB: u64 = 1024 * 1024;
const DEFAULT_LOG_MAX_BYTES: u64 = 10 * BYTES_PER_MB;
const LOG_MAX_SIZE_ENV: &str = "WAYSCRIBER_LOG_MAX_SIZE_MB";

fn resolve_log_max_bytes() -> u64 {
    env::var(LOG_MAX_SIZE_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.saturating_mul(BYTES_PER_MB))
        .unwrap_or(DEFAULT_LOG_MAX_BYTES)
}

fn resolve_log_target() -> LogFileTarget {
    if let Ok(path) = env::var("WAYSCRIBER_LOG_FILE")
        && !path.trim().is_empty()
    {
        let trimmed = path.trim();
        let expanded = paths::expand_tilde(trimmed);
        let mut treat_as_dir = trimmed.ends_with('/') || trimmed.ends_with('\\');
        if !treat_as_dir && let Ok(metadata) = fs::metadata(&expanded) {
            treat_as_dir = metadata.is_dir();
        }
        return LogFileTarget {
            base: expanded,
            treat_as_dir,
        };
    }

    LogFileTarget {
        base: paths::log_dir(),
        treat_as_dir: true,
    }
}

fn current_log_date() -> String {
    time_utils::format_with_template(time_utils::now_local(), "%Y-%m-%d")
}

struct LogFileTarget {
    base: PathBuf,
    treat_as_dir: bool,
}

impl LogFileTarget {
    fn path_for_date(&self, date: &str) -> PathBuf {
        if self.treat_as_dir {
            return self.base.join(format!("wayscriber-{}.log", date));
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

struct DailyFileWriter {
    target: LogFileTarget,
    max_bytes: u64,
    current_date: Option<String>,
    current_size: u64,
    file: Option<fs::File>,
    error_date: Option<String>,
}

impl DailyFileWriter {
    fn new(target: LogFileTarget) -> Self {
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

struct TeeWriter {
    left: Box<dyn Write + Send>,
    right: Box<dyn Write + Send>,
}

impl TeeWriter {
    fn new(left: Box<dyn Write + Send>, right: Box<dyn Write + Send>) -> Self {
        Self { left, right }
    }
}

impl Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.left.write_all(buf)?;
        self.right.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.left.flush()?;
        self.right.flush()
    }
}
