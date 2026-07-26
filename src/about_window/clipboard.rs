use anyhow::Result;
use log::warn;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::backend::wayland::RuntimeWakeSender;

const COPY_OPERATION_TIMEOUT: Duration = Duration::from_secs(5);

/// Root-owned sequencing for About clipboard publications.
///
/// One job may be active. While it runs, repeated clicks overwrite the single
/// pending value, so the most recent click is always the final publication and
/// click bursts cannot create an unbounded thread or broker-command queue.
pub(super) struct ClipboardCopyJobs {
    sequence: ClipboardCopySequence<ClipboardCopyJob>,
    completion_wake: RuntimeWakeSender,
}

struct ClipboardCopySequence<J> {
    active: Option<J>,
    pending: Option<String>,
}

pub(super) struct CopyStartRejected<E> {
    text: String,
    source: E,
}

struct ClipboardCopyJob {
    thread: JoinHandle<()>,
    completion: Receiver<()>,
}

struct CompletionSignal {
    completion: Sender<()>,
    wake: RuntimeWakeSender,
}

impl<J> Default for ClipboardCopySequence<J> {
    fn default() -> Self {
        Self {
            active: None,
            pending: None,
        }
    }
}

impl<E> CopyStartRejected<E> {
    fn new(text: String, source: E) -> Self {
        Self { text, source }
    }

    #[cfg(test)]
    fn into_parts(self) -> (String, E) {
        (self.text, self.source)
    }
}

impl<E: fmt::Display> fmt::Display for CopyStartRejected<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "clipboard worker start rejected: {}",
            self.source
        )
    }
}

impl<E: Error + 'static> Error for CopyStartRejected<E> {}

impl<E: fmt::Debug> fmt::Debug for CopyStartRejected<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CopyStartRejected")
            .field("text_bytes", &self.text.len())
            .field("source", &self.source)
            .finish()
    }
}

impl ClipboardCopyJobs {
    pub(super) fn new(completion_wake: RuntimeWakeSender) -> Self {
        Self {
            sequence: ClipboardCopySequence::default(),
            completion_wake,
        }
    }

    pub(super) fn request(
        &mut self,
        process_broker: &crate::process_broker::ProcessBrokerHandle,
        text: &str,
    ) -> std::result::Result<(), CopyStartRejected<io::Error>> {
        if text.is_empty() {
            return Ok(());
        }
        let completion_wake = &self.completion_wake;
        self.sequence.request_with(
            text.to_string(),
            &mut ClipboardCopyJob::completion_ready,
            &mut ClipboardCopyJob::settle,
            &mut |text| start_copy_job(process_broker, completion_wake, text),
        )
    }

    pub(super) fn settle_finished(
        &mut self,
        process_broker: &crate::process_broker::ProcessBrokerHandle,
    ) -> std::result::Result<(), CopyStartRejected<io::Error>> {
        let completion_wake = &self.completion_wake;
        self.sequence.settle_finished_with(
            &mut ClipboardCopyJob::completion_ready,
            &mut ClipboardCopyJob::settle,
            &mut |text| start_copy_job(process_broker, completion_wake, text),
        )
    }

    /// Settle all accepted work before the app-owned broker guard is dropped.
    /// The actor reply has a per-operation bound and this sequence contains at
    /// most one active plus one pending publication, so teardown has one fixed
    /// cumulative bound instead of growing with the number of clicks.
    pub(super) fn settle_all(
        &mut self,
        process_broker: &crate::process_broker::ProcessBrokerHandle,
    ) -> std::result::Result<(), CopyStartRejected<io::Error>> {
        let completion_wake = &self.completion_wake;
        self.sequence
            .settle_all_with(&mut ClipboardCopyJob::settle, &mut |text| {
                start_copy_job(process_broker, completion_wake, text)
            })
    }

    #[cfg(test)]
    fn settlement_wait_bound() -> Duration {
        let one = crate::process_broker::ProcessBrokerHandle::publication_wait_bound(
            COPY_OPERATION_TIMEOUT,
        );
        one.saturating_add(one)
    }
}

impl ClipboardCopyJob {
    fn spawn(
        task: impl FnOnce() + Send + 'static,
        completion_wake: RuntimeWakeSender,
    ) -> io::Result<Self> {
        let (completion, completion_ready) = channel();
        std::thread::Builder::new()
            .name("wayscriber-about-clipboard".into())
            .spawn(move || {
                let _completion_signal = CompletionSignal {
                    completion,
                    wake: completion_wake,
                };
                task();
            })
            .map(|thread| Self {
                thread,
                completion: completion_ready,
            })
    }

    fn completion_ready(&self) -> bool {
        match self.completion.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => true,
            Err(TryRecvError::Empty) => false,
        }
    }

    fn settle(self) {
        if self.thread.join().is_err() {
            warn!("About clipboard worker stopped unexpectedly");
        }
    }
}

impl Drop for CompletionSignal {
    fn drop(&mut self) {
        if self.completion.send(()).is_ok()
            && let Err(error) = self.wake.wake()
        {
            warn!("Failed to wake About clipboard completion: {error}");
        }
    }
}

impl<J> ClipboardCopySequence<J> {
    fn request_with<E>(
        &mut self,
        text: String,
        completion_ready: &mut impl FnMut(&J) -> bool,
        settle: &mut impl FnMut(J),
        start: &mut impl FnMut(String) -> std::result::Result<J, CopyStartRejected<E>>,
    ) -> std::result::Result<(), CopyStartRejected<E>> {
        if self.active.is_none() {
            self.active = Some(start(text)?);
            return Ok(());
        }

        // The click being handled is newer than any already-pending value. Put
        // it in the one pending slot before observing active completion so a
        // just-finished A promotes C, never the stale B it superseded.
        self.pending = Some(text);
        if self.active.as_ref().is_some_and(completion_ready) {
            self.complete_active_with(settle, start)?;
        }
        Ok(())
    }

    fn settle_finished_with<E>(
        &mut self,
        completion_ready: &mut impl FnMut(&J) -> bool,
        settle: &mut impl FnMut(J),
        start: &mut impl FnMut(String) -> std::result::Result<J, CopyStartRejected<E>>,
    ) -> std::result::Result<(), CopyStartRejected<E>> {
        if self.active.as_ref().is_some_and(completion_ready) {
            self.complete_active_with(settle, start)?;
        }
        Ok(())
    }

    fn complete_active_with<E>(
        &mut self,
        settle: &mut impl FnMut(J),
        start: &mut impl FnMut(String) -> std::result::Result<J, CopyStartRejected<E>>,
    ) -> std::result::Result<(), CopyStartRejected<E>> {
        if let Some(active) = self.active.take() {
            settle(active);
        }
        if let Some(pending) = self.pending.take() {
            self.active = Some(start(pending)?);
        }
        Ok(())
    }

    fn settle_all_with<E>(
        &mut self,
        settle: &mut impl FnMut(J),
        start: &mut impl FnMut(String) -> std::result::Result<J, CopyStartRejected<E>>,
    ) -> std::result::Result<(), CopyStartRejected<E>> {
        if let Some(active) = self.active.take() {
            settle(active);
        }
        if let Some(pending) = self.pending.take() {
            settle(start(pending)?);
        }
        Ok(())
    }

    #[cfg(test)]
    fn work_shape(&self) -> (usize, usize) {
        (
            self.active.is_some() as usize,
            self.pending.is_some() as usize,
        )
    }
}

pub(super) fn open_url(process_broker: &crate::process_broker::ProcessBrokerHandle, url: &str) {
    let (opener, arguments): (&str, Vec<OsString>) = if cfg!(target_os = "macos") {
        ("open", vec![url.into()])
    } else if cfg!(target_os = "windows") {
        (
            "cmd",
            vec!["/C".into(), "start".into(), "".into(), url.into()],
        )
    } else {
        ("xdg-open", vec![url.into()])
    };

    if let Err(err) = process_broker.spawn(
        crate::process_broker::HelperKind::DesktopOpen,
        crate::process_broker::HelperLifetime::DetachedAfterExec,
        OsStr::new(opener),
        arguments,
        Vec::new(),
    ) {
        warn!("Failed to open URL {}: {}", url, err);
    }
}

fn start_copy_job(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    completion_wake: &RuntimeWakeSender,
    text: String,
) -> std::result::Result<ClipboardCopyJob, CopyStartRejected<io::Error>> {
    let wake = completion_wake
        .try_duplicate()
        .map_err(|error| CopyStartRejected::new(text.clone(), error))?;
    let process_broker = process_broker.clone();
    let worker_text = text.clone();
    ClipboardCopyJob::spawn(
        move || {
            if let Err(err) = copy_text_with_command(&worker_text, |text| {
                copy_text_via_command(&process_broker, text)
            }) {
                warn!("Failed to copy About text to clipboard: {err}");
            }
        },
        wake,
    )
    .map_err(|error| CopyStartRejected::new(text, error))
}

fn copy_text_with_command<C>(text: &str, mut command_copy: C) -> Result<()>
where
    C: FnMut(&str) -> Result<()>,
{
    if text.is_empty() {
        return Ok(());
    }
    command_copy(text)
}

fn copy_text_via_command(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    text: &str,
) -> Result<()> {
    let output = process_broker.publish(
        crate::process_broker::HelperKind::WlCopy,
        OsStr::new("wl-copy"),
        [OsStr::new("--type"), OsStr::new("text/plain")],
        text.as_bytes().to_vec(),
        COPY_OPERATION_TIMEOUT,
    )?;
    if output.timed_out {
        return Err(anyhow::anyhow!("wl-copy timed out"));
    }
    if output.status != 0 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("wl-copy failed: {}", stderr.trim()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::fd::{AsRawFd, RawFd};
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;

    use wayland_client::backend::WaylandError;

    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;

    struct TestJob {
        text: String,
        finished: bool,
    }

    struct IdlePreparedRead {
        connection: UnixStream,
    }

    impl super::super::PreparedAboutRead for IdlePreparedRead {
        fn connection_raw_fd(&self) -> RawFd {
            self.connection.as_raw_fd()
        }

        fn read(self) -> std::result::Result<usize, WaylandError> {
            Ok(0)
        }
    }

    #[test]
    fn copy_text_with_command_short_circuits_for_empty_text() {
        let mut command_calls = 0;

        copy_text_with_command("", |_| {
            command_calls += 1;
            Ok(())
        })
        .expect("empty-text fixture short-circuits without a command failure");

        assert_eq!(command_calls, 0);
    }

    #[test]
    fn copy_text_with_command_uses_command_when_available() {
        let mut command_calls = 0;

        copy_text_with_command("abc123", |_| {
            command_calls += 1;
            Ok(())
        })
        .expect("available-command fixture returns success");

        assert_eq!(command_calls, 1);
    }

    #[test]
    fn copy_text_with_command_returns_command_error() {
        let err = copy_text_with_command("abc123", |_| Err(anyhow::anyhow!("command failed")))
            .expect_err("failing-command fixture returns its typed clipboard error");

        assert!(err.to_string().contains("command failed"));
    }

    #[test]
    fn a_finished_with_b_pending_and_current_c_promotes_c() {
        let mut jobs = ClipboardCopySequence {
            active: Some(TestJob {
                text: "A".into(),
                finished: true,
            }),
            pending: Some("B".into()),
        };
        let mut started = Vec::new();
        let mut settled = Vec::new();

        jobs.request_with(
            "C".into(),
            &mut |job| job.finished,
            &mut |job| settled.push(job.text),
            &mut |text| {
                started.push(text.clone());
                Ok::<_, CopyStartRejected<&'static str>>(TestJob {
                    text,
                    finished: false,
                })
            },
        )
        .expect("current-click fixture starts its latest pending job");

        assert_eq!(settled, ["A"]);
        assert_eq!(started, ["C"]);
        assert_eq!(jobs.active.as_ref().map(|job| job.text.as_str()), Some("C"));
        assert_eq!(jobs.work_shape(), (1, 0));
    }

    #[test]
    fn injected_start_failure_returns_latest_text_without_silent_loss() {
        let mut jobs = ClipboardCopySequence {
            active: Some(TestJob {
                text: "A".into(),
                finished: true,
            }),
            pending: Some("B".into()),
        };
        let mut settled = Vec::new();

        let rejection = jobs
            .request_with(
                "C".into(),
                &mut |job| job.finished,
                &mut |job| settled.push(job.text),
                &mut |text| Err::<TestJob, _>(CopyStartRejected::new(text, "injected failure")),
            )
            .expect_err("injected start failure rejects the superseding click explicitly");
        let (text, reason) = rejection.into_parts();

        assert_eq!(settled, ["A"]);
        assert_eq!(text, "C");
        assert_eq!(reason, "injected failure");
        assert_eq!(jobs.work_shape(), (0, 0));
    }

    #[test]
    fn repeated_clicks_coalesce_to_the_latest_pending_value() {
        let mut jobs = ClipboardCopySequence::<TestJob>::default();
        let mut started = Vec::new();
        let mut settled = Vec::new();

        for text in ["commit", "old diagnostics", "latest diagnostics"] {
            jobs.request_with(
                text.into(),
                &mut |job| job.finished,
                &mut |job| settled.push(job.text),
                &mut |text| {
                    started.push(text.clone());
                    Ok::<_, CopyStartRejected<&'static str>>(TestJob {
                        text,
                        finished: false,
                    })
                },
            )
            .expect("coalescing fixture accepts a bounded copy request");
        }

        assert_eq!(started, ["commit"]);
        assert_eq!(jobs.work_shape(), (1, 1));

        jobs.settle_all_with(&mut |job| settled.push(job.text), &mut |text| {
            started.push(text.clone());
            Ok::<_, CopyStartRejected<&'static str>>(TestJob {
                text,
                finished: false,
            })
        })
        .expect("coalescing fixture settles its active and latest pending jobs");

        assert_eq!(started, ["commit", "latest diagnostics"]);
        assert_eq!(settled, ["commit", "latest diagnostics"]);
        assert_eq!(jobs.work_shape(), (0, 0));
    }

    #[test]
    fn worker_completion_wakes_prepared_read_and_promotes_pending_job() {
        let wake = RuntimeWakeSource::new().expect("wake fixture creates its root-owned eventfd");
        let sender = wake
            .try_sender()
            .expect("wake fixture duplicates its root-owned eventfd");
        let (release_a, wait_for_a) = mpsc::channel();
        let a_sender = sender
            .try_duplicate()
            .expect("wake fixture duplicates the sender for job A");
        let active = ClipboardCopyJob::spawn(
            move || {
                let _ = wait_for_a.recv();
            },
            a_sender,
        )
        .expect("wake fixture starts job A");
        let mut jobs = ClipboardCopySequence {
            active: Some(active),
            pending: Some("B".into()),
        };
        let (idle_connection, _idle_peer) =
            UnixStream::pair().expect("wake fixture creates an idle Wayland-like socket");

        release_a
            .send(())
            .expect("wake fixture releases job A after polling can begin");
        let outcome = super::super::read_about_events_with_wake(
            IdlePreparedRead {
                connection: idle_connection,
            },
            &wake,
            Some(Duration::from_secs(1)),
        )
        .expect("job A completion wakes the prepared-read poll");
        assert!(outcome.completion_wake);
        assert!(!outcome.wayland_read);

        let mut started = Vec::new();
        jobs.settle_finished_with(
            &mut ClipboardCopyJob::completion_ready,
            &mut ClipboardCopyJob::settle,
            &mut |text| {
                started.push(text);
                ClipboardCopyJob::spawn(
                    || {},
                    sender
                        .try_duplicate()
                        .map_err(|error| CopyStartRejected::new("B".into(), error))?,
                )
                .map_err(|error| CopyStartRejected::new("B".into(), error))
            },
        )
        .expect("completion fixture promotes pending job B");

        assert_eq!(started, ["B"]);
        assert_eq!(jobs.work_shape(), (1, 0));
        jobs.settle_all_with(&mut ClipboardCopyJob::settle, &mut |_text| {
            Err::<ClipboardCopyJob, _>(CopyStartRejected::new(
                "unexpected".into(),
                io::Error::other("fixture has no second pending job"),
            ))
        })
        .expect("completion fixture joins promoted job B");
    }

    #[test]
    fn shutdown_settlement_has_two_job_cumulative_bound() {
        assert_eq!(
            ClipboardCopyJobs::settlement_wait_bound(),
            Duration::from_secs(32)
        );
    }
}
