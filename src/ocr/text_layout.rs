//! Where the text is, rather than what it says.
//!
//! The marker's snap mode needs text-row geometry for the whole displayed
//! screen image, once per capture. That is a different job from
//! `Copy text from screen`: it produces boxes instead of a clipboard payload,
//! it runs unprompted while the user is choosing where to draw, and it must
//! never make the copy path wait. So it gets its own capacity-one controller
//! rather than sharing the one in `controller.rs`.
//!
//! The engine's stdout for this job is TSV that still contains the recognized
//! words. It is parsed inside the worker by `text_lines`, which keeps only
//! geometry, and is never logged or returned.

use std::ffi::OsStr;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};
use std::time::Duration;

use crate::backend::wayland::RuntimeWakeHandle;
use crate::capture::png::encode_packed_argb32_png;
use crate::process_broker::HelperKind;
use crate::screen_pixels::PackedArgb32;

use super::tesseract::{
    TESSERACT_PROGRAM, TESSERACT_STDOUT_CAP, classify_broker_error, classify_stderr,
    program_on_path, with_temporary_png,
};
use super::text_lines::{DEFAULT_MIN_WORD_CONFIDENCE, TextLineBox, parse_text_line_boxes};
use super::{OcrFailure, OcrLanguages};

/// Shorter than the copy path's 30s. A layout scan the user is waiting on to
/// start drawing is worthless once it is this late; the marker has already
/// fallen back to freehand and they have moved on.
const LAYOUT_TIMEOUT: Duration = Duration::from_secs(15);

/// The upper bound on rows a single scan may report.
///
/// A pathological capture (a wall of dense terminal text on a 5K display) can
/// produce thousands of rows, and every one of them is walked on every pointer
/// motion. Past this the extra rows buy no accuracy and cost frame time.
const MAX_LINES: usize = 4_000;

pub(crate) struct TextLayoutRequest {
    pub(crate) pixels: PackedArgb32,
    pub(crate) languages: OcrLanguages,
}

impl fmt::Debug for TextLayoutRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextLayoutRequest")
            .field("pixels", &self.pixels)
            .field("languages", &self.languages)
            .finish()
    }
}

pub(crate) type TextLayoutOutcome = Result<Vec<TextLineBox>, OcrFailure>;

/// Turns encoded image bytes into text-row geometry. Tests substitute fakes.
pub(crate) trait TextLayoutDetector {
    fn detect(&self, png: &[u8], languages: &OcrLanguages) -> TextLayoutOutcome;
}

pub(crate) struct TesseractLayoutDetector;

impl TextLayoutDetector for TesseractLayoutDetector {
    fn detect(&self, png: &[u8], languages: &OcrLanguages) -> TextLayoutOutcome {
        if !program_on_path(TESSERACT_PROGRAM) {
            return Err(OcrFailure::EngineMissing);
        }
        with_temporary_png(png, |input| run_layout_scan(input, languages))
    }
}

fn run_layout_scan(input: &Path, languages: &OcrLanguages) -> TextLayoutOutcome {
    let output = crate::process_broker::current()
        .and_then(|broker| {
            broker.run(
                HelperKind::Tesseract,
                OsStr::new(TESSERACT_PROGRAM),
                layout_arguments(input, languages),
                Vec::new(),
                LAYOUT_TIMEOUT,
                TESSERACT_STDOUT_CAP,
            )
        })
        .map_err(|err| {
            log::warn!("Failed to run tesseract for text layout: {err:#}");
            classify_broker_error(&err)
        })?;

    if output.timed_out {
        return Err(OcrFailure::TimedOut);
    }
    if output.status != 0 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!(
            "tesseract layout scan exited with status {}: {}",
            output.status,
            stderr.trim()
        );
        return Err(classify_stderr(&stderr, languages));
    }

    // Never log or return stdout: it is TSV containing the recognized screen
    // text. Only the boxes survive this function.
    let tsv = String::from_utf8_lossy(&output.stdout);
    let mut lines = parse_text_line_boxes(&tsv, DEFAULT_MIN_WORD_CONFIDENCE);
    drop(tsv);
    if lines.len() > MAX_LINES {
        log::debug!(
            "Text layout scan found {} rows; keeping the first {MAX_LINES}",
            lines.len()
        );
        lines.truncate(MAX_LINES);
    }
    Ok(lines)
}

/// The invocation: an explicit argument vector, never a shell line.
///
/// `--psm 3` rather than the copy path's `6`. Automatic page segmentation is
/// what numbers blocks, paragraphs, and lines across a whole desktop; `6`
/// assumes one uniform block and would merge separate columns into one row.
fn layout_arguments<'a>(input: &'a Path, languages: &'a OcrLanguages) -> Vec<&'a OsStr> {
    vec![
        input.as_os_str(),
        OsStr::new("stdout"),
        OsStr::new("--oem"),
        OsStr::new("1"),
        OsStr::new("--psm"),
        OsStr::new("3"),
        OsStr::new("-l"),
        OsStr::new(languages.as_str()),
        OsStr::new("--dpi"),
        OsStr::new("96"),
        OsStr::new("tsv"),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TextLayoutRequestId(u64);

impl fmt::Display for TextLayoutRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TextLayoutSubmitError {
    Busy,
    IdentityExhausted,
    Unhealthy,
    SpawnFailed { reason: String },
}

impl fmt::Display for TextLayoutSubmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => formatter.write_str("a text layout scan is still active"),
            Self::IdentityExhausted => formatter.write_str("text layout request IDs exhausted"),
            Self::Unhealthy => formatter.write_str("text layout controller is unhealthy"),
            Self::SpawnFailed { reason } => {
                write!(formatter, "failed to spawn text layout worker: {reason}")
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum TextLayoutPoll {
    Idle,
    Pending,
    Ready {
        id: TextLayoutRequestId,
        outcome: TextLayoutOutcome,
    },
    WorkerLost {
        id: TextLayoutRequestId,
        reason: String,
    },
}

enum WorkerMessage {
    Ready {
        id: TextLayoutRequestId,
        outcome: TextLayoutOutcome,
    },
    Panicked {
        id: TextLayoutRequestId,
        reason: String,
    },
}

struct ActiveScan {
    id: TextLayoutRequestId,
    receiver: Receiver<WorkerMessage>,
}

/// Capacity-one transport for layout scans.
///
/// Capacity one for the same reason the copy controller has it: a queued scan
/// describes a screen image the user has already replaced. A newer request is
/// refused, not queued — the caller retries once the active one lands, keyed by
/// the screen source it belongs to.
pub(crate) struct TextLayoutController {
    next_id: Option<u64>,
    runtime_wake: RuntimeWakeHandle,
    active: Option<ActiveScan>,
    healthy: bool,
}

impl TextLayoutController {
    pub(crate) fn new(runtime_wake: RuntimeWakeHandle) -> Self {
        Self {
            next_id: Some(1),
            runtime_wake,
            active: None,
            healthy: true,
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) fn try_submit(
        &mut self,
        request: TextLayoutRequest,
    ) -> Result<TextLayoutRequestId, TextLayoutSubmitError> {
        self.try_submit_with(request, TesseractLayoutDetector)
    }

    pub(crate) fn try_submit_with(
        &mut self,
        request: TextLayoutRequest,
        detector: impl TextLayoutDetector + Send + 'static,
    ) -> Result<TextLayoutRequestId, TextLayoutSubmitError> {
        self.try_submit_with_spawner(request, detector, |job| {
            std::thread::Builder::new()
                .name("wayscriber-text-layout".to_string())
                .spawn(job)
                .map(drop)
        })
    }

    fn try_submit_with_spawner(
        &mut self,
        request: TextLayoutRequest,
        detector: impl TextLayoutDetector + Send + 'static,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<()>,
    ) -> Result<TextLayoutRequestId, TextLayoutSubmitError> {
        if !self.healthy {
            return Err(TextLayoutSubmitError::Unhealthy);
        }
        if self.active.is_some() {
            return Err(TextLayoutSubmitError::Busy);
        }
        let value = self
            .next_id
            .ok_or(TextLayoutSubmitError::IdentityExhausted)?;
        let id = TextLayoutRequestId(value);

        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let runtime_wake = self.runtime_wake.clone();
        let job = Box::new(move || {
            let guard = WorkerExitGuard::new(id, sender, runtime_wake);
            let message = match catch_unwind(AssertUnwindSafe(|| run_scan(request, &detector))) {
                Ok(outcome) => WorkerMessage::Ready { id, outcome },
                Err(payload) => WorkerMessage::Panicked {
                    id,
                    reason: panic_reason(&payload),
                },
            };
            guard.publish(message);
        });
        if let Err(err) = spawn(job) {
            return Err(TextLayoutSubmitError::SpawnFailed {
                reason: err.to_string(),
            });
        }

        self.next_id = value.checked_add(1);
        self.active = Some(ActiveScan { id, receiver });
        Ok(id)
    }

    pub(crate) fn poll(&mut self) -> TextLayoutPoll {
        let Some(active) = self.active.as_ref() else {
            return TextLayoutPoll::Idle;
        };
        let active_id = active.id;
        match active.receiver.try_recv() {
            Err(TryRecvError::Empty) => TextLayoutPoll::Pending,
            Err(TryRecvError::Disconnected) => {
                self.active = None;
                TextLayoutPoll::WorkerLost {
                    id: active_id,
                    reason: "text layout worker exited without an outcome".to_string(),
                }
            }
            Ok(WorkerMessage::Ready { id, outcome }) if id == active_id => {
                self.active = None;
                TextLayoutPoll::Ready { id, outcome }
            }
            Ok(WorkerMessage::Panicked { id, reason }) if id == active_id => {
                self.active = None;
                TextLayoutPoll::WorkerLost { id, reason }
            }
            Ok(WorkerMessage::Ready { id, .. } | WorkerMessage::Panicked { id, .. }) => {
                self.healthy = false;
                self.active = None;
                TextLayoutPoll::WorkerLost {
                    id: active_id,
                    reason: format!(
                        "text layout worker reported request identity {id}, expected {active_id}"
                    ),
                }
            }
        }
    }
}

/// Run one scan on the worker thread: encode, detect, done.
fn run_scan(request: TextLayoutRequest, detector: &dyn TextLayoutDetector) -> TextLayoutOutcome {
    let png = encode_packed_argb32_png(&request.pixels).map_err(|err| {
        log::warn!("Text layout PNG encoding failed: {err}");
        OcrFailure::EncodeFailed
    })?;
    detector.detect(&png.bytes, &request.languages)
}

fn panic_reason(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "text layout worker panicked with a non-string payload".to_string()
    }
}

/// Guarantees a terminal publication and an event-loop wake on every exit path,
/// including a panic that never reaches `publish`.
struct WorkerExitGuard {
    id: TextLayoutRequestId,
    sender: Option<SyncSender<WorkerMessage>>,
    runtime_wake: RuntimeWakeHandle,
    terminal_published: bool,
}

impl WorkerExitGuard {
    fn new(
        id: TextLayoutRequestId,
        sender: SyncSender<WorkerMessage>,
        runtime_wake: RuntimeWakeHandle,
    ) -> Self {
        Self {
            id,
            sender: Some(sender),
            runtime_wake,
            terminal_published: false,
        }
    }

    fn publish(mut self, message: WorkerMessage) {
        let sender = self
            .sender
            .take()
            .expect("text layout worker still holds its sender until publish");
        let result = sender.try_send(message);
        self.terminal_published = true;
        match result {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(_)) => {
                log::error!(
                    "text layout worker {} found an impossible full channel",
                    self.id
                );
            }
        }
        if let Err(err) = self.runtime_wake.wake() {
            log::error!(
                "Failed to wake runtime for text layout request {}: {err}",
                self.id
            );
        }
    }
}

impl Drop for WorkerExitGuard {
    fn drop(&mut self) {
        if self.terminal_published {
            return;
        }
        self.sender.take();
        if let Err(err) = self.runtime_wake.wake() {
            log::error!(
                "Failed to wake runtime for disconnected text layout request {}: {err}",
                self.id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;

    fn box_at(top: i32) -> TextLineBox {
        TextLineBox {
            left: 10,
            top,
            width: 100,
            height: 18,
        }
    }

    struct FakeDetector {
        outcome: fn() -> TextLayoutOutcome,
    }

    impl TextLayoutDetector for FakeDetector {
        fn detect(&self, _png: &[u8], _languages: &OcrLanguages) -> TextLayoutOutcome {
            (self.outcome)()
        }
    }

    struct BlockingDetector {
        started: mpsc::Sender<()>,
        release: std::sync::Mutex<mpsc::Receiver<()>>,
    }

    impl TextLayoutDetector for BlockingDetector {
        fn detect(&self, _png: &[u8], _languages: &OcrLanguages) -> TextLayoutOutcome {
            self.started.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            Ok(vec![box_at(0)])
        }
    }

    struct PanickingDetector;

    impl TextLayoutDetector for PanickingDetector {
        fn detect(&self, _png: &[u8], _languages: &OcrLanguages) -> TextLayoutOutcome {
            panic!("expected text layout worker panic");
        }
    }

    fn request() -> TextLayoutRequest {
        TextLayoutRequest {
            pixels: PackedArgb32::new(2, 2, 8, vec![0xFF; 16]).unwrap(),
            languages: OcrLanguages::from_validated("eng".to_string()),
        }
    }

    fn controller() -> (RuntimeWakeSource, TextLayoutController) {
        let wake = RuntimeWakeSource::new().unwrap();
        let controller = TextLayoutController::new(wake.handle());
        (wake, controller)
    }

    fn wait_for_wake(wake: &RuntimeWakeSource) {
        assert!(
            wake.wait_readable(Some(Duration::from_secs(5))).unwrap(),
            "text layout completion did not wake the event loop"
        );
    }

    #[test]
    fn a_scan_runs_off_thread_and_wakes_the_event_loop_with_its_rows() {
        let (wake, mut controller) = controller();
        let id = controller
            .try_submit_with(
                request(),
                FakeDetector {
                    outcome: || Ok(vec![box_at(10), box_at(40)]),
                },
            )
            .unwrap();

        wait_for_wake(&wake);
        match controller.poll() {
            TextLayoutPoll::Ready {
                id: ready,
                outcome: Ok(lines),
            } => {
                assert_eq!(ready, id);
                assert_eq!(lines.len(), 2);
            }
            other => panic!("expected rows, got {other:?}"),
        }
        assert!(!controller.is_active());
    }

    #[test]
    fn a_second_scan_while_one_is_active_is_refused_rather_than_queued() {
        let (_wake, mut controller) = controller();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        controller
            .try_submit_with(
                request(),
                BlockingDetector {
                    started: started_tx,
                    release: std::sync::Mutex::new(release_rx),
                },
            )
            .unwrap();
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        assert!(matches!(controller.poll(), TextLayoutPoll::Pending));
        assert_eq!(
            controller
                .try_submit_with(
                    request(),
                    FakeDetector {
                        outcome: || panic!("a refused scan must not run"),
                    },
                )
                .unwrap_err(),
            TextLayoutSubmitError::Busy
        );

        release_tx.send(()).unwrap();
    }

    #[test]
    fn an_engine_failure_arrives_as_a_typed_outcome_rather_than_a_lost_worker() {
        let (wake, mut controller) = controller();
        controller
            .try_submit_with(
                request(),
                FakeDetector {
                    outcome: || Err(OcrFailure::EngineMissing),
                },
            )
            .unwrap();

        wait_for_wake(&wake);
        assert!(matches!(
            controller.poll(),
            TextLayoutPoll::Ready {
                outcome: Err(OcrFailure::EngineMissing),
                ..
            }
        ));
    }

    #[test]
    fn a_worker_panic_is_reported_without_poisoning_the_controller() {
        let (wake, mut controller) = controller();
        let id = controller
            .try_submit_with(request(), PanickingDetector)
            .unwrap();

        wait_for_wake(&wake);
        assert!(matches!(
            controller.poll(),
            TextLayoutPoll::WorkerLost { id: lost, reason }
                if lost == id && reason.contains("expected text layout worker panic")
        ));
        assert!(
            controller
                .try_submit_with(
                    request(),
                    FakeDetector {
                        outcome: || Ok(Vec::new()),
                    },
                )
                .is_ok(),
            "one panicked scan must not disable snapping for the session"
        );
    }

    #[test]
    fn spawn_failure_leaves_no_active_scan() {
        let (_wake, mut controller) = controller();
        let error = controller
            .try_submit_with_spawner(
                request(),
                FakeDetector {
                    outcome: || Ok(Vec::new()),
                },
                |_job| Err(std::io::Error::other("injected spawn failure")),
            )
            .unwrap_err();

        assert_eq!(
            error,
            TextLayoutSubmitError::SpawnFailed {
                reason: "injected spawn failure".to_string(),
            }
        );
        assert!(!controller.is_active());
    }

    #[test]
    fn the_layout_invocation_asks_for_tsv_under_automatic_page_segmentation() {
        let path = Path::new("/tmp/wayscriber-layout-test.png");
        let languages = OcrLanguages::from_validated("eng".to_string());
        let arguments: Vec<_> = layout_arguments(path, &languages)
            .into_iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            arguments,
            [
                "/tmp/wayscriber-layout-test.png",
                "stdout",
                "--oem",
                "1",
                "--psm",
                "3",
                "-l",
                "eng",
                "--dpi",
                "96",
                "tsv",
            ]
        );
    }

    #[test]
    fn a_request_debug_rendering_carries_geometry_but_no_pixels() {
        let rendered = format!("{:?}", request());

        assert!(rendered.contains("width: 2"));
        assert!(!rendered.contains("255"));
    }
}
