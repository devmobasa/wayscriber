//! Runtime-owned desktop-open completion.
//!
//! Input handlers record intent only. The bounded broker operation runs on a
//! worker, wakes the Wayland loop, and requests overlay exit only after the
//! opener has completed successfully.

use std::time::Duration;

use super::{RuntimeOperationController, WaylandState};
use crate::backend::wayland::{RuntimeOperationPoll, RuntimeOperationSubmitFailure};
use crate::desktop_open::{DesktopOpenInvocation, DesktopOpenRequest};
use crate::input::state::{Toast, ToastPriority};

enum DesktopOpenCompletion {
    Pending,
    Opened(DesktopOpenRequest),
    Failed {
        request: DesktopOpenRequest,
        reason: String,
    },
}

impl WaylandState {
    pub(in crate::backend::wayland) fn handle_desktop_open(&mut self, request: DesktopOpenRequest) {
        if let Err(failure) =
            queue_desktop_open(&mut self.desktop_open, request, crate::desktop_open::open)
        {
            let (error, request) = failure.into_parts();
            self.report_desktop_open_failure(request, error.to_string());
        }
    }

    pub(in crate::backend::wayland) fn poll_desktop_open_completion(&mut self) {
        let completion = classify_completion(self.desktop_open.poll());
        apply_exit_policy(&completion, &mut self.input_state.should_exit);
        match completion {
            DesktopOpenCompletion::Pending => {}
            DesktopOpenCompletion::Opened(request) => {
                log::info!(
                    "Opened {} at {}",
                    request.target_name(),
                    request.path().display()
                );
            }
            DesktopOpenCompletion::Failed { request, reason } => {
                self.report_desktop_open_failure(request, reason);
            }
        }
    }

    pub(in crate::backend::wayland) fn desktop_open_in_progress(&self) -> bool {
        self.desktop_open.is_active()
    }

    fn report_desktop_open_failure(&mut self, request: DesktopOpenRequest, reason: String) {
        log::warn!(
            "Failed to open {} at {}: {}",
            request.target_name(),
            request.path().display(),
            reason
        );
        // If an opener partially launched an application before failing, keep
        // this failure visible instead of immediately applying focus-loss exit.
        self.suppress_focus_exit_for(Duration::from_millis(1500));
        self.input_state.push_toast(
            ToastPriority::Critical,
            "launcher",
            Toast::error(request.failure_notice()),
        );
    }
}

fn queue_desktop_open(
    controller: &mut RuntimeOperationController<DesktopOpenRequest, Result<(), String>>,
    request: DesktopOpenRequest,
    open: impl FnOnce(&DesktopOpenInvocation) -> anyhow::Result<()> + Send + 'static,
) -> Result<(), RuntimeOperationSubmitFailure<DesktopOpenRequest>> {
    let invocation = request.invocation();
    controller
        .try_submit(request, "wayscriber-desktop-open", move || {
            open(&invocation).map_err(|error| format!("{error:#}"))
        })
        .map(drop)
}

fn classify_completion(
    poll: RuntimeOperationPoll<DesktopOpenRequest, Result<(), String>>,
) -> DesktopOpenCompletion {
    match poll {
        RuntimeOperationPoll::Idle | RuntimeOperationPoll::Pending { .. } => {
            DesktopOpenCompletion::Pending
        }
        RuntimeOperationPoll::Ready {
            context: request,
            outcome,
            ..
        } => classify_result(request, outcome),
        RuntimeOperationPoll::ProducerFailed {
            context: request,
            reason,
            ..
        } => DesktopOpenCompletion::Failed { request, reason },
        RuntimeOperationPoll::Disconnected {
            context: request, ..
        } => DesktopOpenCompletion::Failed {
            request,
            reason: "desktop-open worker disconnected".to_string(),
        },
    }
}

fn classify_result(
    request: DesktopOpenRequest,
    outcome: Result<(), String>,
) -> DesktopOpenCompletion {
    match outcome {
        Ok(()) => DesktopOpenCompletion::Opened(request),
        Err(reason) => DesktopOpenCompletion::Failed { request, reason },
    }
}

fn apply_exit_policy(completion: &DesktopOpenCompletion, should_exit: &mut bool) {
    if matches!(completion, DesktopOpenCompletion::Opened(_)) {
        *should_exit = true;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::backend::wayland::{
        RuntimeOperationIdSource, RuntimeWakeSource,
        handlers::keyboard::{XdgFocusLeaveAction, xdg_focus_leave_action},
    };

    #[test]
    fn dispatch_returns_before_helper_completion_and_exit_waits_for_success() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller =
            RuntimeOperationController::new(RuntimeOperationIdSource::new(), wake.handle());
        let request = DesktopOpenRequest::CaptureFolder("/tmp/capture".into());
        let (release_tx, release_rx) = mpsc::channel();
        let fallback_release = release_tx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            let _ = fallback_release.send(());
        });

        let started = Instant::now();
        queue_desktop_open(&mut controller, request.clone(), move |_| {
            release_rx.recv().unwrap();
            Ok(())
        })
        .unwrap();
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "desktop-open submission blocked event dispatch"
        );
        assert!(controller.is_active());
        assert_eq!(
            xdg_focus_leave_action(true, controller.is_active(), false, true),
            XdgFocusLeaveAction::AwaitDesktopOpen,
        );
        assert!(matches!(
            {
                let completion = classify_completion(controller.poll());
                let mut should_exit = false;
                apply_exit_policy(&completion, &mut should_exit);
                assert!(!should_exit);
                completion
            },
            DesktopOpenCompletion::Pending,
        ));

        release_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match classify_completion(controller.poll()) {
                DesktopOpenCompletion::Pending => {
                    assert!(
                        Instant::now() < deadline,
                        "desktop-open completion was not published"
                    );
                    std::thread::yield_now();
                }
                DesktopOpenCompletion::Opened(completed) => {
                    assert_eq!(completed, request);
                    let mut should_exit = false;
                    apply_exit_policy(&DesktopOpenCompletion::Opened(completed), &mut should_exit);
                    assert!(should_exit);
                    break;
                }
                DesktopOpenCompletion::Failed { reason, .. } => {
                    panic!("desktop-open worker failed: {reason}");
                }
            }
        }
    }

    #[test]
    fn failed_helper_completion_never_requests_exit() {
        let request = DesktopOpenRequest::ConfigFile("/tmp/config.toml".into());
        let completion = classify_result(request.clone(), Err("injected failure".to_string()));
        let mut should_exit = false;
        apply_exit_policy(&completion, &mut should_exit);

        assert!(matches!(
            completion,
            DesktopOpenCompletion::Failed {
                request: failed,
                reason,
            } if failed == request && reason == "injected failure"
        ));
        assert!(!should_exit);
    }
}
