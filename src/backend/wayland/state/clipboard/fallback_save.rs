use super::WaylandState;
use crate::backend::wayland::RuntimeOperationPoll;
use crate::backend::wayland::state::clipboard_runtime::ClipboardRuntime;
use crate::input::state::{
    ClipboardFallbackSaveRequest, InputEffect, InputEffectDrain, InputState,
};
use std::sync::Arc;

impl WaylandState {
    pub(in crate::backend::wayland) fn start_clipboard_fallback_save(
        &mut self,
        request: Arc<ClipboardFallbackSaveRequest>,
    ) {
        if let Err(error) = submit_save(&mut self.input_state, &mut self.clipboard, request) {
            log::error!("Failed to submit clipboard fallback save: {error}");
        }
    }

    pub(in crate::backend::wayland) fn poll_clipboard_fallback_save(&mut self) {
        if let Err(error) =
            apply_completion(&mut self.input_state, self.clipboard.poll_fallback_save())
        {
            log::error!("Clipboard fallback save failed: {error}");
        }
    }

    pub(in crate::backend::wayland) fn finish_clipboard_fallback_saves(&mut self) {
        if let Err(error) = finish_fallback_saves(&mut self.input_state, &mut self.clipboard) {
            log::error!("Failed to finish accepted image save before exit: {error}");
            crate::notification::send_notification_async(
                &self.tokio_handle,
                "Failed to Save Image".into(),
                error,
                Some("dialog-error".into()),
            );
        }
    }
}

fn submit_save(
    input: &mut InputState,
    clipboard: &mut ClipboardRuntime,
    request: Arc<ClipboardFallbackSaveRequest>,
) -> Result<(), String> {
    if let Err(failure) = clipboard.submit_fallback_save(request, save_image) {
        let (error, id) = failure.into_parts();
        let message = if matches!(
            error,
            crate::backend::wayland::runtime_operation::RuntimeOperationSubmitError::Busy { .. }
        ) {
            "Another image is being saved. Try again in a moment."
        } else {
            "Could not start the image save. Try again."
        };
        input.complete_clipboard_fallback_save(id, Err(message.into()));

        return Err(error.to_string());
    }

    Ok(())
}

/// Dispatch has stopped: settle the current writer, then drain accepted saves FIFO.
/// Other effects stay queued for their own teardown owners.
fn finish_fallback_saves(
    input: &mut InputState,
    clipboard: &mut ClipboardRuntime,
) -> Result<(), String> {
    let mut result = apply_completion(input, clipboard.wait_fallback_save());
    for effect in input.drain_input_effects(InputEffectDrain::ClipboardFallbackSaves) {
        let InputEffect::ClipboardFallbackSave(request) = effect else {
            unreachable!("fallback save drain returned {effect:?}");
        };
        let outcome = submit_save(input, clipboard, request)
            .and_then(|()| apply_completion(input, clipboard.wait_fallback_save()));
        if result.is_ok() {
            result = outcome;
        }
    }

    result
}

fn save_image(
    request: std::sync::Arc<crate::input::state::ClipboardFallbackSaveRequest>,
) -> Result<std::path::PathBuf, String> {
    crate::capture::file::save_screenshot(&request.image_data, &request.save_config)
        .map_err(|error| request.operation.format_error(&error))
}

fn apply_completion(
    input: &mut InputState,
    completion: RuntimeOperationPoll<u64, Result<std::path::PathBuf, String>>,
) -> Result<(), String> {
    let (id, outcome) = match completion {
        RuntimeOperationPoll::Idle | RuntimeOperationPoll::Pending { .. } => return Ok(()),
        RuntimeOperationPoll::Ready {
            context: id,
            outcome,
            ..
        } => (id, outcome),
        RuntimeOperationPoll::ProducerFailed {
            context: id,
            reason,
            ..
        } => (id, Err(reason)),
        RuntimeOperationPoll::Disconnected { context: id, .. } => (
            id,
            Err("Image save worker exited without a completion".into()),
        ),
    };
    let result = outcome.as_ref().map(|_| ()).map_err(Clone::clone);
    input.complete_clipboard_fallback_save(id, outcome);

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::state::clipboard_runtime::ClipboardRuntime;
    use crate::backend::wayland::{RuntimeOperationIdSource, RuntimeWakeSource};
    use crate::capture::{ImageOperationKind, file::FileSaveConfig};
    use crate::input::state::{InputEffect, InputEffectDrain, PendingBackendAction};
    use std::time::Duration;

    fn queue_image(
        input: &mut InputState,
        path: &std::path::Path,
        bytes: Vec<u8>,
        exit: bool,
    ) -> std::sync::Arc<crate::input::state::ClipboardFallbackSaveRequest> {
        input.set_clipboard_fallback(
            bytes,
            FileSaveConfig {
                save_directory: path.to_path_buf(),
                filename_template: "fallback".into(),
                format: "png".into(),
            },
            ImageOperationKind::CanvasExport,
            exit,
        );
        input.save_pending_clipboard_to_file();
        take_save_request(input)
    }

    fn take_save_request(input: &mut InputState) -> Arc<ClipboardFallbackSaveRequest> {
        let mut requests: Vec<_> = input
            .drain_input_effects(InputEffectDrain::Runtime)
            .into_iter()
            .filter_map(|effect| match effect {
                InputEffect::ClipboardFallbackSave(request) => Some(request),
                _ => None,
            })
            .collect();
        assert_eq!(requests.len(), 1);

        requests.pop().unwrap()
    }

    #[test]
    fn teardown_starts_a_save_accepted_in_the_exit_dispatch_batch() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut input = crate::input::state::test_support::make_test_input_state();
        let wake = RuntimeWakeSource::new().unwrap();
        let mut runtime = ClipboardRuntime::new(RuntimeOperationIdSource::new(), wake.handle());
        input.set_clipboard_fallback(
            vec![4, 5, 6],
            FileSaveConfig {
                save_directory: temp.path().to_path_buf(),
                filename_template: "fallback".into(),
                format: "png".into(),
            },
            ImageOperationKind::CanvasExport,
            false,
        );
        input.save_pending_clipboard_to_file();
        assert!(input.has_pending_backend_actions());
        let unrelated =
            PendingBackendAction::HelperLaunch(crate::input::state::HelperLaunchRequest::About);
        input.set_pending_backend_action(unrelated.clone());
        input.request_explicit_exit();

        finish_fallback_saves(&mut input, &mut runtime).unwrap();

        let paths: Vec<_> = std::fs::read_dir(temp.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(
            paths.len(),
            1,
            "accepted save must run even when Exit skips the runtime drain"
        );
        assert_eq!(std::fs::read(&paths[0]).unwrap(), [4, 5, 6]);
        assert_eq!(input.last_capture_path(), Some(paths[0].as_path()));
        assert!(matches!(
            runtime.poll_fallback_save(),
            RuntimeOperationPoll::Idle
        ));
        assert!(input.should_exit);
        assert_eq!(input.take_pending_backend_action(), Some(unrelated));
        assert!(!input.has_pending_backend_actions());
        finish_fallback_saves(&mut input, &mut runtime).unwrap();
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
    }

    #[test]
    fn teardown_preserves_queued_image_ownership_when_the_fallback_changes() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut input = crate::input::state::test_support::make_test_input_state();
        let wake = RuntimeWakeSource::new().unwrap();
        let mut runtime = ClipboardRuntime::new(RuntimeOperationIdSource::new(), wake.handle());
        for (name, bytes) in [("first", vec![1, 2]), ("second", vec![3, 4])] {
            input.set_clipboard_fallback(
                bytes,
                FileSaveConfig {
                    save_directory: temp.path().join(name),
                    filename_template: "fallback".into(),
                    format: "png".into(),
                },
                ImageOperationKind::CanvasExport,
                name == "first",
            );
            input.save_pending_clipboard_to_file();
        }

        finish_fallback_saves(&mut input, &mut runtime).unwrap();

        for (name, bytes) in [("first", vec![1, 2]), ("second", vec![3, 4])] {
            let paths: Vec<_> = std::fs::read_dir(temp.path().join(name))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();
            assert_eq!(paths.len(), 1);
            assert_eq!(std::fs::read(&paths[0]).unwrap(), bytes);
        }
        assert_eq!(
            input.last_capture_path().unwrap().parent(),
            Some(temp.path().join("second").as_path())
        );
        assert!(
            !input.should_exit,
            "stale save cannot apply its exit-after-save flag"
        );
        assert!(matches!(
            runtime.poll_fallback_save(),
            RuntimeOperationPoll::Idle
        ));
    }

    #[test]
    fn teardown_reports_io_and_producer_failures_and_keeps_retry_bytes() {
        for (panics, expected) in [
            (false, "Failed to save canvas export"),
            (true, "held writer failed"),
        ] {
            let temp = crate::test_temp::tempdir().unwrap();
            let path = if panics {
                temp.path().to_path_buf()
            } else {
                temp.path().join("not-a-directory")
            };
            if !panics {
                std::fs::write(&path, b"obstruction").unwrap();
            }
            let mut input = crate::input::state::test_support::make_test_input_state();
            let wake = RuntimeWakeSource::new().unwrap();
            let mut runtime = ClipboardRuntime::new(RuntimeOperationIdSource::new(), wake.handle());
            let request = queue_image(&mut input, &path, vec![7, 8], true);
            runtime
                .submit_fallback_save(request, move |request| {
                    assert!(!panics, "held writer failed");
                    save_image(request)
                })
                .unwrap();

            let error = finish_fallback_saves(&mut input, &mut runtime).unwrap_err();

            assert!(error.contains(expected));
            if !panics {
                assert_eq!(std::fs::read(&path).unwrap(), b"obstruction");
            }
            assert!(!input.should_exit);
            assert!(input.last_capture_path().is_none());
            assert!(input.active_toast().unwrap().message.contains(expected));
            input.save_pending_clipboard_to_file();
            assert_eq!(&*take_save_request(&mut input).image_data, &[7, 8]);
        }
    }

    #[test]
    fn teardown_failure_does_not_discard_a_newer_accepted_save() {
        let temp = crate::test_temp::tempdir().unwrap();
        let blocked = temp.path().join("not-a-directory");
        std::fs::write(&blocked, b"obstruction").unwrap();
        let mut input = crate::input::state::test_support::make_test_input_state();
        let wake = RuntimeWakeSource::new().unwrap();
        let mut runtime = ClipboardRuntime::new(RuntimeOperationIdSource::new(), wake.handle());
        let first = queue_image(&mut input, &blocked, vec![1], true);
        runtime.submit_fallback_save(first, save_image).unwrap();
        input.set_clipboard_fallback(
            vec![2],
            FileSaveConfig {
                save_directory: temp.path().join("newer"),
                filename_template: "fallback".into(),
                format: "png".into(),
            },
            ImageOperationKind::CanvasExport,
            false,
        );
        input.save_pending_clipboard_to_file();

        let error = finish_fallback_saves(&mut input, &mut runtime).unwrap_err();

        assert!(error.contains("Failed to save canvas export"));
        assert_eq!(std::fs::read(&blocked).unwrap(), b"obstruction");
        let saved = input.last_capture_path().unwrap();
        assert_eq!(saved.parent(), Some(temp.path().join("newer").as_path()));
        assert_eq!(std::fs::read(saved).unwrap(), [2]);
        assert!(!input.should_exit);
        assert!(matches!(
            runtime.poll_fallback_save(),
            RuntimeOperationPoll::Idle
        ));
    }

    #[test]
    fn teardown_waits_for_an_already_started_image_writer() {
        let temp = crate::test_temp::tempdir().unwrap();
        let directory = temp.path().to_path_buf();
        let (release, wait) = std::sync::mpsc::channel();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (finished_tx, finished_rx) = std::sync::mpsc::channel();
        let teardown = std::thread::spawn(move || {
            let mut input = crate::input::state::test_support::make_test_input_state();
            let wake = RuntimeWakeSource::new().unwrap();
            let mut runtime = ClipboardRuntime::new(RuntimeOperationIdSource::new(), wake.handle());
            let request = queue_image(&mut input, &directory, vec![8, 9, 10], true);
            runtime
                .submit_fallback_save(request, move |request| {
                    started_tx.send(()).unwrap();
                    wait.recv().unwrap();
                    save_image(request)
                })
                .unwrap();
            input.request_explicit_exit();

            let result = finish_fallback_saves(&mut input, &mut runtime);
            finished_tx.send(result).unwrap();
        });
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let premature = finished_rx.recv_timeout(Duration::from_millis(100));
        release.send(()).unwrap();
        let result = match premature {
            Ok(ref result) => result.clone(),
            Err(_) => finished_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        };
        teardown.join().unwrap();

        assert!(
            matches!(premature, Err(std::sync::mpsc::RecvTimeoutError::Timeout)),
            "teardown returned while its accepted writer was held"
        );
        result.unwrap();
        let paths: Vec<_> = std::fs::read_dir(temp.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(paths.len(), 1);
        assert_eq!(std::fs::read(&paths[0]).unwrap(), [8, 9, 10]);
    }

    #[test]
    fn fallback_worker_saves_real_bytes_and_only_then_requests_exit() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut input = crate::input::state::test_support::make_test_input_state();
        let wake = RuntimeWakeSource::new().unwrap();
        let mut runtime = ClipboardRuntime::new(RuntimeOperationIdSource::new(), wake.handle());
        let request = queue_image(&mut input, temp.path(), vec![1, 2, 3], true);
        runtime
            .submit_fallback_save(request.clone(), save_image)
            .unwrap();
        assert!(!input.should_exit);

        assert!(wake.wait_readable(Some(Duration::from_secs(2))).unwrap());
        apply_completion(&mut input, runtime.poll_fallback_save()).unwrap();
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
        let path = std::fs::read_dir(temp.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read(path).unwrap(), [1, 2, 3]);
        assert!(input.should_exit);
        assert!(input.take_explicit_exit_requested());
        assert!(!input.take_explicit_exit_requested());
        input.save_pending_clipboard_to_file();
        assert!(
            !input
                .drain_input_effects(InputEffectDrain::Runtime)
                .iter()
                .any(|effect| matches!(effect, InputEffect::ClipboardFallbackSave(_)))
        );
    }

    #[test]
    fn failed_save_retains_bytes_for_retry_and_canvas_error_wording() {
        let temp = crate::test_temp::tempdir().unwrap();
        let blocked = temp.path().join("not-a-directory");
        std::fs::write(&blocked, b"file").unwrap();
        let mut input = crate::input::state::test_support::make_test_input_state();
        let wake = RuntimeWakeSource::new().unwrap();
        let mut runtime = ClipboardRuntime::new(RuntimeOperationIdSource::new(), wake.handle());
        let request = queue_image(&mut input, &blocked, vec![7, 8, 9], false);
        runtime
            .submit_fallback_save(request.clone(), save_image)
            .unwrap();
        assert!(wake.wait_readable(Some(Duration::from_secs(2))).unwrap());
        assert!(apply_completion(&mut input, runtime.poll_fallback_save()).is_err());
        assert!(
            input
                .active_toast()
                .unwrap()
                .message
                .contains("Failed to save canvas export")
        );
        assert!(
            !input
                .active_toast()
                .unwrap()
                .message
                .to_lowercase()
                .contains("screenshot")
        );
        std::fs::remove_file(&blocked).unwrap();
        input.save_pending_clipboard_to_file();
        let retry = take_save_request(&mut input);
        assert_eq!(&*retry.image_data, &[7, 8, 9]);
        runtime.submit_fallback_save(retry, save_image).unwrap();
        assert!(wake.wait_readable(Some(Duration::from_secs(2))).unwrap());
        apply_completion(&mut input, runtime.poll_fallback_save()).unwrap();
        let saved = std::fs::read_dir(&blocked)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read(saved).unwrap(), [7, 8, 9]);
        input.save_pending_clipboard_to_file();
        assert!(
            !input
                .drain_input_effects(InputEffectDrain::Runtime)
                .iter()
                .any(|effect| matches!(effect, InputEffect::ClipboardFallbackSave(_)))
        );
    }

    #[test]
    fn slow_save_is_single_flight_and_stale_completion_preserves_newer_image() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut input = crate::input::state::test_support::make_test_input_state();
        let wake = RuntimeWakeSource::new().unwrap();
        let mut runtime = ClipboardRuntime::new(RuntimeOperationIdSource::new(), wake.handle());
        let first = queue_image(&mut input, temp.path(), vec![1], true);
        let (release, wait) = std::sync::mpsc::channel();
        runtime
            .submit_fallback_save(first, move |request| {
                wait.recv().unwrap();
                save_image(request)
            })
            .unwrap();
        input.save_pending_clipboard_to_file();
        assert!(
            !input
                .drain_input_effects(InputEffectDrain::Runtime)
                .iter()
                .any(|effect| matches!(effect, InputEffect::ClipboardFallbackSave(_)))
        );
        assert!(matches!(
            runtime.poll_fallback_save(),
            RuntimeOperationPoll::Pending { .. }
        ));
        assert!(!input.should_exit);
        let second = queue_image(&mut input, temp.path(), vec![2], false);
        assert!(
            runtime
                .submit_fallback_save(second.clone(), save_image)
                .is_err()
        );
        release.send(()).unwrap();
        assert!(wake.wait_readable(Some(Duration::from_secs(2))).unwrap());
        apply_completion(&mut input, runtime.poll_fallback_save()).unwrap();

        assert!(
            !input.should_exit,
            "stale completion cannot exit for the old image"
        );
        input.complete_clipboard_fallback_save(second.id, Err("busy; retry".into()));
        input.save_pending_clipboard_to_file();
        let retry = take_save_request(&mut input);
        assert_eq!(retry.id, second.id);
        assert_eq!(&*retry.image_data, &[2]);
    }
}
