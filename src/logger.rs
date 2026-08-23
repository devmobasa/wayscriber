mod file;
mod filter;

use std::fmt::Write as _;
use std::io::{self, Write as IoWrite};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{Log, Metadata, Record};

use self::file::{DailyFileWriter, resolve_log_target};
use self::filter::LogFilter;

const INITIAL_RECORD_CAPACITY: usize = 256;
const MAX_RETAINED_RECORD_CAPACITY: usize = 64 * 1024;

pub(crate) fn init(log_to_file: bool) {
    let filter = LogFilter::from_env();
    let max_level = filter.max_level();
    let sink = if log_to_file {
        let target = resolve_log_target();
        let file_writer = DailyFileWriter::new(target);
        LogSink::tee(Box::new(io::stderr()), Box::new(file_writer))
    } else {
        LogSink::single(Box::new(io::stderr()))
    };

    let logger = SimpleLogger {
        filter,
        state: Mutex::new(LoggerState {
            sink,
            record_buffer: String::with_capacity(INITIAL_RECORD_CAPACITY),
        }),
        include_timestamp: log_to_file,
    };

    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(max_level);
    }
}

struct SimpleLogger {
    filter: LogFilter,
    state: Mutex<LoggerState>,
    include_timestamp: bool,
}

struct LoggerState {
    sink: LogSink,
    record_buffer: String,
}

impl LoggerState {
    fn write_record(&mut self, record: &Record<'_>, include_timestamp: bool) {
        self.record_buffer.clear();
        if include_timestamp {
            let _ = writeln!(
                self.record_buffer,
                "{} {} {}: {}",
                timestamp_millis(),
                record.level(),
                record.target(),
                record.args()
            );
        } else {
            let _ = writeln!(
                self.record_buffer,
                "{} {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
        }

        self.sink.write_record(self.record_buffer.as_bytes());
        if self.record_buffer.capacity() > MAX_RETAINED_RECORD_CAPACITY {
            self.record_buffer = String::with_capacity(INITIAL_RECORD_CAPACITY);
        }
    }
}

impl Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.filter.enabled(metadata.target(), metadata.level())
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };

        state.write_record(record, self.include_timestamp);
    }

    fn flush(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.sink.flush();
        }
    }
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

enum LogSink {
    Single(Box<dyn IoWrite + Send>),
    Tee {
        primary: Box<dyn IoWrite + Send>,
        secondary: Box<dyn IoWrite + Send>,
    },
}

impl LogSink {
    fn single(writer: Box<dyn IoWrite + Send>) -> Self {
        Self::Single(writer)
    }

    fn tee(primary: Box<dyn IoWrite + Send>, secondary: Box<dyn IoWrite + Send>) -> Self {
        Self::Tee { primary, secondary }
    }

    fn write_record(&mut self, record: &[u8]) {
        match self {
            Self::Single(writer) => {
                let _ = writer.write_all(record);
            }
            Self::Tee { primary, secondary } => {
                let _ = primary.write_all(record);
                let _ = secondary.write_all(record);
            }
        }
    }

    fn flush(&mut self) {
        match self {
            Self::Single(writer) => {
                let _ = writer.flush();
            }
            Self::Tee { primary, secondary } => {
                let _ = primary.flush();
                let _ = secondary.flush();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct SharedWriter {
        state: Arc<Mutex<SharedWriterState>>,
    }

    #[derive(Default)]
    struct SharedWriterState {
        bytes: Vec<u8>,
        flushes: usize,
    }

    impl SharedWriter {
        fn bytes(&self) -> Vec<u8> {
            self.state
                .lock()
                .expect("shared writer state")
                .bytes
                .clone()
        }

        fn flushes(&self) -> usize {
            self.state.lock().expect("shared writer state").flushes
        }
    }

    impl IoWrite for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut state = self.state.lock().expect("shared writer state");
            state.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.state.lock().expect("shared writer state").flushes += 1;
            Ok(())
        }
    }

    struct FailingWriter;

    impl IoWrite for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "sink closed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "sink closed"))
        }
    }

    struct InterruptingWriteAll;

    impl IoWrite for InterruptingWriteAll {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "sink interrupted",
            ))
        }

        fn write_all(&mut self, _buf: &[u8]) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "sink interrupted",
            ))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn warning_record() -> Record<'static> {
        Record::builder()
            .args(format_args!("sink failure record"))
            .level(Level::Warn)
            .target("wayscriber::daemon")
            .build()
    }

    #[test]
    fn failed_primary_sink_does_not_starve_the_secondary_sink() {
        let secondary = SharedWriter::default();
        let mut sink = LogSink::tee(Box::new(FailingWriter), Box::new(secondary.clone()));

        sink.write_record(b"WARN wayscriber::daemon: sink failure record\n");
        assert_eq!(
            secondary.bytes(),
            b"WARN wayscriber::daemon: sink failure record\n"
        );
    }

    #[test]
    fn failed_secondary_sink_does_not_starve_the_primary_sink() {
        let primary = SharedWriter::default();
        let mut sink = LogSink::tee(Box::new(primary.clone()), Box::new(FailingWriter));

        sink.write_record(b"WARN wayscriber::daemon: sink failure record\n");
        assert_eq!(
            primary.bytes(),
            b"WARN wayscriber::daemon: sink failure record\n"
        );
    }

    #[test]
    fn interrupted_sink_does_not_duplicate_the_healthy_sink_record() {
        let secondary = SharedWriter::default();
        let mut sink = LogSink::tee(Box::new(InterruptingWriteAll), Box::new(secondary.clone()));

        sink.write_record(b"WARN wayscriber::daemon: sink failure record\n");

        assert_eq!(
            secondary.bytes(),
            b"WARN wayscriber::daemon: sink failure record\n"
        );
        assert_eq!(
            secondary
                .bytes()
                .windows(b"sink failure record".len())
                .filter(|window| *window == b"sink failure record")
                .count(),
            1,
            "the healthy sink receives the record exactly once"
        );
    }

    #[test]
    fn failed_flush_still_flushes_the_other_sink() {
        let secondary = SharedWriter::default();
        let mut sink = LogSink::tee(Box::new(FailingWriter), Box::new(secondary.clone()));

        sink.flush();
        assert_eq!(secondary.flushes(), 1);
    }

    #[test]
    fn file_record_keeps_timestamp_and_message_in_one_line() {
        let output = SharedWriter::default();
        let mut state = LoggerState {
            sink: LogSink::single(Box::new(output.clone())),
            record_buffer: String::with_capacity(INITIAL_RECORD_CAPACITY),
        };

        state.write_record(&warning_record(), true);

        let line = String::from_utf8(output.bytes()).expect("valid log output");
        let (timestamp, message) = line.split_once(' ').expect("timestamp separator");

        assert!(timestamp.parse::<u128>().is_ok());
        assert_eq!(message, "WARN wayscriber::daemon: sink failure record\n");
    }

    #[test]
    fn oversized_record_buffer_is_not_retained() {
        let output = SharedWriter::default();
        let mut state = LoggerState {
            sink: LogSink::single(Box::new(output)),
            record_buffer: String::with_capacity(MAX_RETAINED_RECORD_CAPACITY + 1),
        };

        state.write_record(&warning_record(), false);

        assert!(state.record_buffer.capacity() <= MAX_RETAINED_RECORD_CAPACITY);
    }
}
