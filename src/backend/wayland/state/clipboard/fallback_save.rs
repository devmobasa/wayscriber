use super::WaylandState;
use crate::backend::wayland::RuntimeOperationPoll;
use crate::input::state::InputState;

impl WaylandState {
    pub(in crate::backend::wayland) fn start_clipboard_fallback_save(&mut self, id: u64) {
        let Some(request) = self.input_state.clipboard_fallback_save_request(id) else {
            return;
        };
        if let Err(failure) = self.clipboard.submit_fallback_save(request, save_image) {
            let (error, id) = failure.into_parts();
            log::error!("Failed to submit clipboard fallback save: {error}");
            let message = if matches!(
                error,
                crate::backend::wayland::runtime_operation::RuntimeOperationSubmitError::Busy { .. }
            ) {
                "Another image is being saved. Try again in a moment."
            } else {
                "Could not start the image save. Try again."
            };
            self.input_state
                .complete_clipboard_fallback_save(id, Err(message.into()));
        }
    }

    pub(in crate::backend::wayland) fn poll_clipboard_fallback_save(&mut self) {
        apply_completion(&mut self.input_state, self.clipboard.poll_fallback_save());
    }
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
) {
    match completion {
        RuntimeOperationPoll::Idle | RuntimeOperationPoll::Pending { .. } => {}
        RuntimeOperationPoll::Ready {
            context: id,
            outcome,
            ..
        } => input.complete_clipboard_fallback_save(id, outcome),
        RuntimeOperationPoll::ProducerFailed {
            context: id,
            reason,
            ..
        } => input.complete_clipboard_fallback_save(id, Err(reason)),
        RuntimeOperationPoll::Disconnected { context: id, .. } => input
            .complete_clipboard_fallback_save(
                id,
                Err("Image save worker exited without a completion".into()),
            ),
    }
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
        let requests: Vec<_> = input
            .drain_input_effects(InputEffectDrain::Runtime)
            .into_iter()
            .filter_map(|effect| match effect {
                InputEffect::Backend(PendingBackendAction::SaveClipboardFallback {
                    request_id,
                }) => Some(request_id),
                _ => None,
            })
            .collect();
        assert_eq!(requests.len(), 1);
        input.clipboard_fallback_save_request(requests[0]).unwrap()
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
        apply_completion(&mut input, runtime.poll_fallback_save());
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
        assert!(input.clipboard_fallback_save_request(request.id).is_none());
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
        apply_completion(&mut input, runtime.poll_fallback_save());

        assert_eq!(
            &*input
                .clipboard_fallback_save_request(request.id)
                .unwrap()
                .image_data,
            &[7, 8, 9]
        );
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
        runtime
            .submit_fallback_save(
                input.clipboard_fallback_save_request(request.id).unwrap(),
                save_image,
            )
            .unwrap();
        assert!(wake.wait_readable(Some(Duration::from_secs(2))).unwrap());
        apply_completion(&mut input, runtime.poll_fallback_save());
        assert!(input.clipboard_fallback_save_request(request.id).is_none());
        assert_eq!(std::fs::read_dir(&blocked).unwrap().count(), 1);
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
                .any(|effect| matches!(
                    effect,
                    InputEffect::Backend(PendingBackendAction::SaveClipboardFallback { .. })
                ))
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
        apply_completion(&mut input, runtime.poll_fallback_save());

        assert!(
            !input.should_exit,
            "stale completion cannot exit for the old image"
        );
        assert_eq!(
            &*input
                .clipboard_fallback_save_request(second.id)
                .unwrap()
                .image_data,
            &[2]
        );
        input.complete_clipboard_fallback_save(second.id, Err("busy; retry".into()));
        input.save_pending_clipboard_to_file();
        assert!(input.drain_input_effects(InputEffectDrain::Runtime).iter().any(|effect| matches!(effect, InputEffect::Backend(PendingBackendAction::SaveClipboardFallback { request_id }) if *request_id == second.id)));
    }
}
