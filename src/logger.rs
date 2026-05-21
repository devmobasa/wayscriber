mod file;
mod filter;

use std::io::{self, Write};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{Log, Metadata, Record};

use self::file::{DailyFileWriter, resolve_log_target};
use self::filter::LogFilter;

pub(crate) fn init(log_to_file: bool) {
    let filter = LogFilter::from_env();
    let max_level = filter.max_level();
    let writer: Box<dyn Write + Send> = if log_to_file {
        let target = resolve_log_target();
        let file_writer = DailyFileWriter::new(target);
        Box::new(TeeWriter::new(
            Box::new(io::stderr()),
            Box::new(file_writer),
        ))
    } else {
        Box::new(io::stderr())
    };

    let logger = SimpleLogger {
        filter,
        writer: Mutex::new(writer),
        include_timestamp: log_to_file,
    };

    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(max_level);
    }
}

struct SimpleLogger {
    filter: LogFilter,
    writer: Mutex<Box<dyn Write + Send>>,
    include_timestamp: bool,
}

impl Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.filter.enabled(metadata.target(), metadata.level())
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let mut writer = match self.writer.lock() {
            Ok(writer) => writer,
            Err(poisoned) => poisoned.into_inner(),
        };

        if self.include_timestamp {
            let _ = write!(writer, "{} ", timestamp_millis());
        }
        let _ = writeln!(
            writer,
            "{} {}: {}",
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.flush();
        }
    }
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
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
