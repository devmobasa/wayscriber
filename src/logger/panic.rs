//! Panic reports through the logger.

use std::backtrace::Backtrace;
use std::fmt::Display;
use std::panic::{self, Location, PanicHookInfo};

use log::{Level, Log, Record};

/// Installs a panic hook that reports each panic through the logger, then
/// runs the hook it replaced.
///
/// The default hook writes only to stderr, and a daemon-launched overlay has
/// its stderr on /dev/null, so a panic there left no message, location or
/// backtrace anywhere. The report goes to every logger sink, the daily file
/// included. The replaced hook still runs afterwards, so an interactive run
/// keeps its usual stderr report and unwinding is unchanged.
pub(super) fn install_hook() {
    chain_hook(|info| report_panic(log::logger(), info));
}

fn chain_hook(report: impl Fn(&PanicHookInfo<'_>) + Send + Sync + 'static) {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        report(info);
        previous(info);
    }));
}

fn report_panic(logger: &dyn Log, info: &PanicHookInfo<'_>) {
    let thread = std::thread::current();
    let report = panic_report(
        thread.name(),
        info.location(),
        info.payload_as_str(),
        &Backtrace::force_capture(),
    );

    logger.log(
        &Record::builder()
            .level(Level::Error)
            .target(module_path!())
            .args(format_args!("{report}"))
            .build(),
    );
}

fn panic_report(
    thread: Option<&str>,
    location: Option<&Location<'_>>,
    payload: Option<&str>,
    backtrace: &dyn Display,
) -> String {
    let thread = thread.unwrap_or("<unnamed>");
    let payload = payload.unwrap_or("Box<dyn Any>");
    match location {
        Some(location) => {
            format!("thread '{thread}' panicked at {location}: {payload}\nbacktrace:\n{backtrace}")
        }
        None => format!("thread '{thread}' panicked: {payload}\nbacktrace:\n{backtrace}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Metadata;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct CapturingLogger {
        records: Arc<Mutex<Vec<(Level, String)>>>,
    }

    impl CapturingLogger {
        fn records(&self) -> Vec<(Level, String)> {
            self.records
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }
    }

    impl Log for CapturingLogger {
        fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
            true
        }

        fn log(&self, record: &Record<'_>) {
            self.records
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push((record.level(), record.args().to_string()));
        }

        fn flush(&self) {}
    }

    #[test]
    fn report_names_the_thread_location_payload_and_backtrace() {
        let location = Location::caller();

        let report = panic_report(Some("worker"), Some(location), Some("boom"), &"frame 0");

        assert_eq!(
            report,
            format!("thread 'worker' panicked at {location}: boom\nbacktrace:\nframe 0")
        );
        assert_eq!(
            panic_report(None, None, None, &"frame 0"),
            "thread '<unnamed>' panicked: Box<dyn Any>\nbacktrace:\nframe 0"
        );
    }

    #[test]
    fn hook_logs_the_panic_and_still_runs_the_hook_it_replaced() {
        let logger = CapturingLogger::default();
        let replaced = Arc::new(Mutex::new(Vec::new()));
        let original = panic::take_hook();
        let replaced_payloads = Arc::clone(&replaced);
        panic::set_hook(Box::new(move |info| {
            replaced_payloads
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(info.payload_as_str().unwrap_or_default().to_owned());
        }));
        let hook_logger = logger.clone();
        chain_hook(move |info| report_panic(&hook_logger, info));

        let result = std::thread::Builder::new()
            .name("panic-hook-probe".into())
            .spawn(|| panic!("panic hook probe payload"))
            .unwrap()
            .join();

        drop(panic::take_hook());
        panic::set_hook(original);
        assert!(result.is_err());
        let records = logger.records();
        let (level, report) = records
            .iter()
            .find(|(_, report)| report.contains("panic hook probe payload"))
            .expect("the panic is logged");
        assert_eq!(*level, Level::Error);
        assert!(report.contains("thread 'panic-hook-probe' panicked at"));
        assert!(report.contains(file!()));
        assert!(report.contains("\nbacktrace:\n"));
        assert!(
            replaced
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .any(|payload| payload == "panic hook probe payload")
        );
    }
}
