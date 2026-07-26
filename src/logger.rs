mod file;
mod filter;

use std::fmt;
use std::io::{self, Write};
use std::sync::mpsc::{self, SyncSender};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use log::Level;

use self::file::{DailyFileWriter, resolve_log_target};
use self::filter::LogFilter;

const EVENT_CAPACITY: usize = 1_024;

enum LoggerEvent {
    Record {
        level: Level,
        target: String,
        message: String,
    },
    Shutdown,
}

#[derive(Clone)]
pub(crate) struct LoggerHandle {
    events: Option<SyncSender<LoggerEvent>>,
    filter: LogFilter,
}

impl LoggerHandle {
    #[cfg(test)]
    pub(crate) fn discarding() -> Self {
        Self {
            events: None,
            filter: LogFilter::from_env(),
        }
    }

    pub(crate) fn record(&self, level: Level, target: &str, message: impl Into<String>) {
        if !self.filter.enabled(target, level) {
            return;
        }
        let Some(events) = &self.events else {
            return;
        };
        if events
            .send(LoggerEvent::Record {
                level,
                target: target.to_string(),
                message: message.into(),
            })
            .is_err()
        {
            eprintln!("wayscriber logger is unavailable");
        }
    }

    pub(crate) fn info(&self, target: &str, message: impl Into<String>) {
        self.record(Level::Info, target, message);
    }

    pub(crate) fn debug(&self, target: &str, message: impl Into<String>) {
        self.record(Level::Debug, target, message);
    }

    pub(crate) fn error(&self, target: &str, message: impl Into<String>) {
        self.record(Level::Error, target, message);
    }

    pub(crate) fn warn(&self, target: &str, message: impl Into<String>) {
        self.record(Level::Warn, target, message);
    }
}

pub(crate) struct LoggerOwner {
    events: Option<SyncSender<LoggerEvent>>,
    worker: Option<JoinHandle<io::Result<()>>>,
}

impl LoggerOwner {
    pub(crate) fn start(
        log_to_file: bool,
        paths: &crate::paths::PathResolver,
    ) -> Result<(Self, LoggerHandle), LoggerStartError> {
        let filter = LogFilter::from_env();
        let (events, receiver) = mpsc::sync_channel(EVENT_CAPACITY);
        let worker = std::thread::Builder::new()
            .name("wayscriber-logger".to_string())
            .spawn({
                let target = log_to_file.then(|| resolve_log_target(paths));
                move || {
                    let writer: Box<dyn Write + Send> = match target {
                        Some(resolution) => {
                            if let Some(diagnostic) = resolution.diagnostic {
                                eprintln!("{diagnostic}");
                            }
                            match resolution.target {
                                Some(target) => Box::new(TeeWriter::new(
                                    Box::new(io::stderr()),
                                    Box::new(DailyFileWriter::new(target)),
                                )),
                                None => Box::new(io::stderr()),
                            }
                        }
                        None => Box::new(io::stderr()),
                    };
                    worker_main(receiver, writer, log_to_file)
                }
            })
            .map_err(LoggerStartError::Spawn)?;
        let handle = LoggerHandle {
            events: Some(events.clone()),
            filter,
        };
        Ok((
            Self {
                events: Some(events),
                worker: Some(worker),
            },
            handle,
        ))
    }

    pub(crate) fn finish(&mut self) -> Result<(), LoggerFinishError> {
        if let Some(events) = self.events.take() {
            events
                .send(LoggerEvent::Shutdown)
                .map_err(|_| LoggerFinishError::Disconnected)?;
        }
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        match worker.join() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(LoggerFinishError::Flush(error)),
            Err(_) => Err(LoggerFinishError::WorkerPanicked),
        }
    }
}

impl Drop for LoggerOwner {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            eprintln!("logger shutdown failed: {error}");
        }
    }
}

#[derive(Debug)]
pub(crate) enum LoggerStartError {
    Spawn(io::Error),
}

impl fmt::Display for LoggerStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(error) => write!(formatter, "failed to start logger owner: {error}"),
        }
    }
}

impl std::error::Error for LoggerStartError {}

#[derive(Debug)]
pub(crate) enum LoggerFinishError {
    Disconnected,
    Flush(io::Error),
    WorkerPanicked,
}

impl fmt::Display for LoggerFinishError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => formatter.write_str("logger event receiver disconnected"),
            Self::Flush(error) => write!(formatter, "logger flush failed: {error}"),
            Self::WorkerPanicked => formatter.write_str("logger worker panicked"),
        }
    }
}

impl std::error::Error for LoggerFinishError {}

fn worker_main(
    receiver: mpsc::Receiver<LoggerEvent>,
    mut writer: Box<dyn Write + Send>,
    include_timestamp: bool,
) -> io::Result<()> {
    while let Ok(event) = receiver.recv() {
        match event {
            LoggerEvent::Record {
                level,
                target,
                message,
            } => {
                if include_timestamp {
                    write!(writer, "{} ", timestamp_millis())?;
                }
                writeln!(writer, "{level} {target}: {message}")?;
            }
            LoggerEvent::Shutdown => return writer.flush(),
        }
    }
    writer.flush()
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
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
        if let Err(error) = self.right.write_all(buf) {
            eprintln!("file logging is unavailable: {error}");
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.left.flush()?;
        if let Err(error) = self.right.flush() {
            eprintln!("file logging flush is unavailable: {error}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequential_logger_owners_are_independent() {
        let paths = crate::paths::PathResolver::from_environment(Default::default());
        for message in ["first", "second"] {
            let (mut owner, handle) = LoggerOwner::start(false, &paths)
                .expect("logger fixture starts its root-owned worker");
            handle.info("wayscriber::test", message);
            drop(handle);
            owner
                .finish()
                .expect("logger fixture drains and flushes its accepted record");
        }
    }
}
