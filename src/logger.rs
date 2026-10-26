mod file;
mod filter;
mod panic;

use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::io::{self, Write as IoWrite};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{Log, Metadata, Record};

use self::file::{DailyFileWriter, resolve_log_target};
use self::filter::LogFilter;

const INITIAL_RECORD_CAPACITY: usize = 256;
const MAX_RETAINED_RECORD_CAPACITY: usize = 64 * 1024;

thread_local! {
    /// Whether this thread is inside [`SimpleLogger::log`].
    static WRITING_RECORD: Cell<bool> = const { Cell::new(false) };

    /// This thread's reusable line buffer for [`SimpleLogger::log`].
    static RECORD_BUFFER: RefCell<RecordBuffer> = RefCell::new(RecordBuffer::new());
}

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
        sink: Mutex::new(sink),
        include_timestamp: log_to_file,
    };

    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(max_level);
        panic::install_hook();
    }
}

struct SimpleLogger {
    filter: LogFilter,
    sink: Mutex<LogSink>,
    include_timestamp: bool,
}

/// One formatted log line, reused for the next record on the same thread.
struct RecordBuffer(String);

impl RecordBuffer {
    fn new() -> Self {
        Self(String::with_capacity(INITIAL_RECORD_CAPACITY))
    }

    /// Formats `record` as one physical line, replacing the previous one.
    fn format(&mut self, record: &Record<'_>, include_timestamp: bool) -> &[u8] {
        let line = &mut self.0;
        line.clear();
        if include_timestamp {
            let _ = write!(
                line,
                "{} {} {}: ",
                timestamp_millis(),
                record.level(),
                record.target()
            );
        } else {
            let _ = write!(line, "{} {}: ", record.level(), record.target());
        }
        let _ = write!(line, "{}", record.args());
        escape_record_line_breaks(line);
        line.push('\n');

        line.as_bytes()
    }

    /// Releases an allocation one oversized record grew, instead of keeping it.
    fn shrink_oversized(&mut self) {
        if self.0.capacity() > MAX_RETAINED_RECORD_CAPACITY {
            self.0 = String::with_capacity(INITIAL_RECORD_CAPACITY);
        }
    }
}

fn escape_record_line_breaks(record: &mut String) {
    if !record.bytes().any(|byte| matches!(byte, b'\r' | b'\n')) {
        return;
    }

    let mut escaped = String::with_capacity(record.len());
    for character in record.chars() {
        match character {
            '\r' => escaped.push_str("\\r"),
            '\n' => escaped.push_str("\\n"),
            character => escaped.push(character),
        }
    }
    *record = escaped;
}

impl Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.filter.enabled(metadata.target(), metadata.level())
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        // A record logged while this thread is already writing one would wait
        // forever on the lock this thread holds. The panic hook does exactly
        // that for a panic raised mid-write, from a `Display` impl say, so the
        // nested record is dropped and the panic still unwinds.
        let Some(_writing) = WritingRecord::enter() else {
            return;
        };

        RECORD_BUFFER.with_borrow_mut(|buffer| {
            // Formatting runs `Display` impls, which may panic. Doing it
            // before taking the lock keeps such a panic from poisoning the
            // sink, so only a sink that panicked mid-write is ever poisoned.
            let line = buffer.format(record, self.include_timestamp);

            // A poisoned sink is in an unknown state, so the record is
            // dropped instead of written through it.
            if let Ok(mut sink) = self.sink.lock() {
                sink.write_record(line);
            }

            buffer.shrink_oversized();
        });
    }

    fn flush(&self) {
        if let Ok(mut sink) = self.sink.lock() {
            sink.flush();
        }
    }
}

/// Marks this thread as writing a record until dropped, unwinding included.
struct WritingRecord;

impl WritingRecord {
    fn enter() -> Option<Self> {
        let already_writing = WRITING_RECORD.with(|writing| writing.replace(true));
        (!already_writing).then_some(Self)
    }
}

impl Drop for WritingRecord {
    fn drop(&mut self) {
        WRITING_RECORD.with(|writing| writing.set(false));
    }
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

struct LogSink {
    writers: Vec<Box<dyn IoWrite + Send>>,
}

impl LogSink {
    fn single(writer: Box<dyn IoWrite + Send>) -> Self {
        Self {
            writers: vec![writer],
        }
    }

    fn tee(primary: Box<dyn IoWrite + Send>, secondary: Box<dyn IoWrite + Send>) -> Self {
        Self {
            writers: vec![primary, secondary],
        }
    }

    fn write_record(&mut self, record: &[u8]) {
        for writer in &mut self.writers {
            let _ = writer.write_all(record);
        }
    }

    fn flush(&mut self) {
        for writer in &mut self.writers {
            let _ = writer.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::fmt::{Display, Formatter};

    use crate::test_env;

    use super::*;
    use log::Level;
    use std::sync::{Arc, Mutex};
    use std::thread;

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
        let mut buffer = RecordBuffer::new();

        let line = buffer.format(&warning_record(), true);

        let line = String::from_utf8(line.to_vec()).expect("valid log output");
        let (timestamp, message) = line.split_once(' ').expect("timestamp separator");

        assert!(timestamp.parse::<u128>().is_ok());
        assert_eq!(message, "WARN wayscriber::daemon: sink failure record\n");
    }

    #[test]
    fn multiline_record_is_escaped_to_one_physical_line() {
        let mut buffer = RecordBuffer::new();
        let record = Record::builder()
            .args(format_args!("first\r\nsecond\nthird\r"))
            .level(Level::Error)
            .target("wayscriber::ocr\nworker")
            .build();

        let line = buffer.format(&record, true);

        let line = String::from_utf8(line.to_vec()).expect("valid log output");
        assert_eq!(line.lines().count(), 1);
        assert!(line.ends_with("ERROR wayscriber::ocr\\nworker: first\\r\\nsecond\\nthird\\r\n"));
    }

    #[test]
    fn a_record_logged_while_the_thread_writes_one_is_dropped_instead_of_deadlocking() {
        struct LogsWhileFormatting<'logger>(&'logger SimpleLogger);

        impl Display for LogsWhileFormatting<'_> {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                self.0.log(&warning_record());
                formatter.write_str("outer record")
            }
        }

        let output = SharedWriter::default();
        let filter = test_env::with_env_var("RUST_LOG", Some("warn".as_ref()), LogFilter::from_env);
        let logger = SimpleLogger {
            filter,
            sink: Mutex::new(LogSink::single(Box::new(output.clone()))),
            include_timestamp: false,
        };

        logger.log(
            &Record::builder()
                .args(format_args!("{}", LogsWhileFormatting(&logger)))
                .level(Level::Warn)
                .target("wayscriber::daemon")
                .build(),
        );
        logger.log(&warning_record());

        let output = String::from_utf8(output.bytes()).expect("valid log output");
        assert_eq!(
            output.lines().collect::<Vec<_>>(),
            [
                "WARN wayscriber::daemon: outer record",
                "WARN wayscriber::daemon: sink failure record"
            ]
        );
    }

    #[test]
    fn oversized_record_buffer_is_not_retained() {
        let mut buffer = RecordBuffer(String::with_capacity(MAX_RETAINED_RECORD_CAPACITY + 1));

        buffer.format(&warning_record(), false);
        buffer.shrink_oversized();

        assert!(buffer.0.capacity() <= MAX_RETAINED_RECORD_CAPACITY);
    }

    #[test]
    fn a_record_whose_formatting_panics_does_not_stop_later_records() {
        struct PanicsWhileFormatting;

        impl Display for PanicsWhileFormatting {
            fn fmt(&self, _formatter: &mut Formatter<'_>) -> fmt::Result {
                panic!("formatting panic");
            }
        }

        let output = SharedWriter::default();
        let filter = test_env::with_env_var("RUST_LOG", Some("warn".as_ref()), LogFilter::from_env);
        let logger = SimpleLogger {
            filter,
            sink: Mutex::new(LogSink::single(Box::new(output.clone()))),
            include_timestamp: false,
        };

        let panicked = thread::scope(|scope| {
            scope
                .spawn(|| {
                    logger.log(
                        &Record::builder()
                            .args(format_args!("{PanicsWhileFormatting}"))
                            .level(Level::Warn)
                            .target("wayscriber::daemon")
                            .build(),
                    );
                })
                .join()
                .is_err()
        });
        logger.log(&warning_record());

        assert!(panicked);
        let output = String::from_utf8(output.bytes()).expect("valid log output");
        assert_eq!(
            output.lines().collect::<Vec<_>>(),
            ["WARN wayscriber::daemon: sink failure record"]
        );
    }
}
