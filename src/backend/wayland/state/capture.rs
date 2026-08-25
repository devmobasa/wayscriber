use super::*;
use crate::backend::wayland::frozen::FrozenCaptureBackend;
use crate::capture::{CaptureRequest, CaptureRequestId, CaptureSubmitError};
use crate::input::state::{Toast, ToastPriority};

mod backdrop;
mod barrier;
mod pdf;

pub(super) use barrier::OverlayCaptureBarrier;

pub(super) fn should_exit_after_capture(
    mode: ExitAfterCaptureMode,
    destination: CaptureDestination,
) -> bool {
    let is_clipboard_only = matches!(destination, CaptureDestination::ClipboardOnly);
    match mode {
        ExitAfterCaptureMode::Always => true,
        ExitAfterCaptureMode::Never => false,
        ExitAfterCaptureMode::Auto => is_clipboard_only,
    }
}

pub(super) const fn overlay_suppression_for_screenshot(
    capture_type: CaptureType,
    include_drawings: bool,
) -> OverlaySuppression {
    if !include_drawings
        && matches!(
            capture_type,
            CaptureType::FullScreen | CaptureType::Selection { .. }
        )
    {
        OverlaySuppression::DesktopBackdrop
    } else {
        OverlaySuppression::Capture
    }
}

#[cfg(test)]
mod include_drawings_tests {
    use super::*;

    #[test]
    fn drawing_preference_selects_the_capture_frame_for_full_and_region_only() {
        for capture_type in [
            CaptureType::FullScreen,
            CaptureType::Selection {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        ] {
            assert_eq!(
                overlay_suppression_for_screenshot(capture_type, true),
                OverlaySuppression::Capture
            );
            assert_eq!(
                overlay_suppression_for_screenshot(capture_type, false),
                OverlaySuppression::DesktopBackdrop
            );
        }

        assert_eq!(
            overlay_suppression_for_screenshot(CaptureType::ActiveWindow, false),
            OverlaySuppression::Capture
        );
    }
}

impl WaylandState {
    /// Reports an export preflight failure and drops the request.
    ///
    /// Both export paths refuse the same way — log, toast, submit nothing —
    /// because the point of the preflight is that a file is never silently
    /// saved without the magnification it was asked for.
    fn report_export_preflight_failure(
        &mut self,
        error: &crate::capture::CaptureError,
        operation: ImageOperationKind,
        toast_source: &'static str,
    ) {
        let message = operation.format_error(error);
        log::error!("{message}");
        self.input_state
            .push_toast(ToastPriority::Critical, toast_source, Toast::error(message));
    }

    pub(in crate::backend::wayland) fn continue_frozen_capture_after_failure(
        &mut self,
        failed_backend: FrozenCaptureBackend,
        qh: &QueueHandle<Self>,
    ) {
        if let Err(error) =
            self.frozen
                .begin_fallback_capture(failed_backend, &self.shm, qh, &self.tokio_handle)
        {
            log::warn!("No frozen capture fallback succeeded after {failed_backend:?}: {error:#}");
            self.frozen
                .finish_failed_fallback_capture(&mut self.input_state);
        }
    }

    fn should_exit_after_capture(&self, destination: CaptureDestination) -> bool {
        should_exit_after_capture(self.exit_after_capture_mode, destination)
    }

    pub(in crate::backend::wayland) fn apply_capture_completion(&mut self) {
        if self.frozen.take_capture_done() {
            self.exit_overlay_suppression(OverlaySuppression::Frozen);
        }
        if let Some(completion) = self.frozen.take_acquisition_completion() {
            self.route_acquisition_completion(completion);
        }
        if self.zoom.take_capture_done() {
            log::info!(
                "capture.preflight component=zoom phase=complete restoring_overlay=true active={} image_generation={}",
                self.zoom.active,
                self.zoom.image_generation()
            );
            self.exit_overlay_suppression(OverlaySuppression::Zoom);
        }
        self.resolve_pending_zoom_terminal();
        self.cancel_screen_modals_if_source_changed();
    }

    /// Restore the overlay after screenshot capture completes.
    ///
    /// Re-maps the layer surface to its original size and forces a redraw.
    pub(in crate::backend::wayland) fn show_overlay(&mut self) {
        self.input_state.clear_click_highlights();
        self.exit_overlay_suppression(OverlaySuppression::Capture);
        self.exit_overlay_suppression(OverlaySuppression::DesktopBackdrop);
    }

    /// Handles capture actions by delegating to the CaptureManager.
    pub(in crate::backend::wayland) fn handle_capture_action(&mut self, action: Action) {
        if action.is_region_capture() {
            if let Some(opening_action) = self.capture.active_region_action() {
                if opening_action == action {
                    self.cancel_region_capture();
                } else {
                    self.input_state.push_toast(
                        ToastPriority::Info,
                        "capture.region.refused",
                        Toast::info("Finish or cancel the current screen selection first."),
                    );
                }
                return;
            }
            if self
                .input_state
                .refuse_region_capture_while_screen_modal_engaged(action)
            {
                return;
            }
            let default_destination = if self.config.capture.copy_to_clipboard {
                CaptureDestination::ClipboardAndFile
            } else {
                CaptureDestination::FileOnly
            };
            self.begin_region_capture_action(action, default_destination);
            return;
        }
        if self
            .input_state
            .refuse_region_capture_while_screen_modal_engaged(action)
        {
            return;
        }
        if !self.config.capture.enabled {
            log::warn!("Capture action triggered but capture is disabled in config");
            return;
        }

        if self.capture.is_in_progress() {
            log::warn!(
                "Capture action {:?} requested while another capture is running; ignoring",
                action
            );
            return;
        }

        let default_destination = if self.config.capture.copy_to_clipboard {
            CaptureDestination::ClipboardAndFile
        } else {
            CaptureDestination::FileOnly
        };

        let (capture_type, destination) = match action {
            Action::CaptureFullScreen => (CaptureType::FullScreen, default_destination),
            Action::CaptureActiveWindow => (CaptureType::ActiveWindow, default_destination),
            Action::CaptureSelection => (
                CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                default_destination,
            ),
            Action::CaptureClipboardFull => {
                (CaptureType::FullScreen, CaptureDestination::ClipboardOnly)
            }
            Action::CaptureFileFull => (CaptureType::FullScreen, CaptureDestination::FileOnly),
            Action::CaptureClipboardSelection => (
                CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                CaptureDestination::ClipboardOnly,
            ),
            Action::CaptureFileSelection => (
                CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                CaptureDestination::FileOnly,
            ),
            Action::CaptureClipboardRegion => {
                log::info!("Region clipboard capture requested");
                (
                    CaptureType::Selection {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    },
                    CaptureDestination::ClipboardOnly,
                )
            }
            Action::CaptureFileRegion => {
                log::info!("Region file capture requested");
                (
                    CaptureType::Selection {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    },
                    CaptureDestination::FileOnly,
                )
            }
            _ => {
                log::error!(
                    "Non-capture action passed to handle_capture_action: {:?}",
                    action
                );
                return;
            }
        };

        // Build file save config from user config when needed
        let save_config = if matches!(destination, CaptureDestination::ClipboardOnly) {
            None
        } else {
            Some(FileSaveConfig {
                save_directory: expand_tilde(&self.config.capture.save_directory),
                filename_template: self.config.capture.filename_template.clone(),
                format: self.config.capture.format.clone(),
            })
        };

        let exit_on_success = self.should_exit_after_capture(destination);
        self.capture.set_exit_on_success(exit_on_success);

        // Full-screen capture deliberately keeps the committed canvas visible
        // unless the user requested raw desktop pixels. Other capture types
        // retain their established suppression behavior.
        let suppression =
            overlay_suppression_for_screenshot(capture_type, self.config.capture.include_drawings);
        if !self.enter_overlay_suppression(suppression) {
            log::warn!(
                "Capture action {:?} requested while overlay is suppressed; ignoring",
                action
            );
            self.capture.finish_capture_lifecycle();
            self.input_state.push_toast(
                ToastPriority::Info,
                "capture",
                Toast::warning("Capture is already preparing another overlay operation."),
            );
            return;
        }
        self.capture.mark_in_progress();

        let request = CaptureRequest {
            capture_type,
            destination,
            save_config,
        };

        log::info!(
            "Queued {:?} capture; waiting for suppression frame",
            request.capture_type
        );
        self.capture
            .queue_preflight(CapturePreflightRequest::Screenshot(request));
    }

    pub(in crate::backend::wayland) fn handle_canvas_export_action(&mut self, action: Action) {
        if self.capture.is_in_progress() {
            log::warn!(
                "Canvas export action {:?} requested while another image operation is running; ignoring",
                action
            );
            return;
        }

        let destination = match action {
            Action::ExportCanvasFile => CaptureDestination::FileOnly,
            Action::ExportCanvasClipboard => CaptureDestination::ClipboardOnly,
            Action::ExportCanvasClipboardAndFile => CaptureDestination::ClipboardAndFile,
            _ => {
                log::error!(
                    "Non-canvas-export action passed to handle_canvas_export_action: {:?}",
                    action
                );
                return;
            }
        };

        // Render on the capture worker, not here: re-rendering the canvas at
        // physical resolution plus PNG encoding froze event dispatch for the
        // whole export on large surfaces.
        let snapshot = self.canvas_export_snapshot();
        let subject = format!("Canvas '{}'", self.input_state.board_name());
        if let Err(error) = snapshot.validate_spotlight_source(&subject) {
            self.report_export_preflight_failure(
                &error,
                ImageOperationKind::CanvasExport,
                "capture.canvas-export",
            );
            return;
        }
        let render: crate::capture::ImageRenderJob = Box::new(move || render_canvas_png(&snapshot));

        let save_config = if matches!(destination, CaptureDestination::ClipboardOnly) {
            None
        } else {
            Some(FileSaveConfig {
                save_directory: expand_tilde(&self.config.capture.save_directory),
                filename_template: self.config.capture.filename_template.clone(),
                // The delivery pipeline replaces this with the rendered
                // image's real extension.
                format: ImageFormatMetadata::png().extension,
            })
        };

        let exit_on_success = self.should_exit_after_capture(destination);
        self.capture.set_exit_on_success(exit_on_success);
        self.capture.mark_in_progress();

        let request = crate::capture::RenderedImageDeliveryRequest {
            render,
            destination,
            save_config,
            operation: ImageOperationKind::CanvasExport,
            fallback_format_override: Some(ImageFormatMetadata::png()),
        };

        let submission = self
            .capture
            .manager_mut()
            .request_rendered_image_delivery(request);
        self.accept_capture_submission(submission, ImageOperationKind::CanvasExport);
    }

    fn canvas_export_snapshot(&self) -> CanvasExportSnapshot {
        let (origin_x, origin_y) = self.board_view_offset();
        CanvasExportSnapshot {
            viewport: CanvasExportViewport {
                logical_width: self.surface.width(),
                logical_height: self.surface.height(),
                scale: self.surface.scale(),
                origin_x: origin_x.round() as i32,
                origin_y: origin_y.round() as i32,
            },
            spotlight: crate::canvas_export::SpotlightPassSnapshot {
                dim_opacity: self.input_state.spotlight_dim_opacity,
                feather: self.input_state.spotlight_feather,
            },
            backdrop: match self.input_state.boards.active_background() {
                crate::input::BoardBackground::Transparent => {
                    CanvasExportBackdropSnapshot::Transparent
                }
                crate::input::BoardBackground::Solid(color) => {
                    CanvasExportBackdropSnapshot::Solid(*color)
                }
            },
            board: BoardExportSnapshot {
                frame: self
                    .input_state
                    .boards
                    .active_frame()
                    .clone_without_history(),
            },
            render_profile: self.input_state.export_render_profile(),
        }
    }

    pub(in crate::backend::wayland) fn begin_pending_capture(
        &mut self,
        request: CapturePreflightRequest,
    ) {
        let (operation, result) = match request {
            CapturePreflightRequest::Screenshot(request) => {
                log::info!("Requesting {:?} capture", request.capture_type);
                (
                    ImageOperationKind::Screenshot,
                    self.capture.manager_mut().request_capture(
                        request.capture_type,
                        request.destination,
                        request.save_config,
                    ),
                )
            }
            CapturePreflightRequest::DesktopBackdrop(request) => {
                log::info!(
                    "Requesting desktop backdrop capture for {:?}",
                    request.operation
                );
                (
                    request.operation,
                    self.capture
                        .manager_mut()
                        .request_desktop_backdrop_capture(request),
                )
            }
        };
        self.accept_capture_submission(result, operation);
    }

    pub(super) fn accept_capture_submission(
        &mut self,
        submission: Result<CaptureRequestId, CaptureSubmitError>,
        operation: ImageOperationKind,
    ) -> bool {
        match submission {
            Ok(id) if self.capture.record_accepted(id) => true,
            Ok(id) => {
                log::error!(
                    "Capture manager accepted operation {id} without a matching state lifecycle"
                );
                self.capture.manager_mut().mark_unhealthy();
                self.restore_rejected_capture_submission(
                    operation,
                    "capture state rejected the accepted operation identity",
                );
                false
            }
            Err(CaptureSubmitError::Busy { active_id }) => {
                log::warn!(
                    "{} submission rejected while capture operation {active_id} is active",
                    operation.saved_log_label()
                );
                self.capture.clear_pending_pdf_export();
                self.show_overlay();
                self.capture.finish_capture_lifecycle();
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "capture",
                    Toast::warning("Another capture operation is still in progress."),
                );
                false
            }
            Err(error) => {
                self.restore_rejected_capture_submission(operation, &error.to_string());
                false
            }
        }
    }

    fn restore_rejected_capture_submission(&mut self, operation: ImageOperationKind, error: &str) {
        log::error!("Failed to submit {}: {error}", operation.saved_log_label());
        self.capture.clear_pending_pdf_export();
        self.show_overlay();
        self.capture.finish_capture_lifecycle();
        self.input_state.push_toast(
            ToastPriority::Critical,
            "capture",
            Toast::error(format!("{} failed: {error}", operation.saved_log_label())),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_exit_mode_exits_after_clipboard_only_capture() {
        assert!(should_exit_after_capture(
            ExitAfterCaptureMode::Auto,
            CaptureDestination::ClipboardOnly
        ));
        assert!(!should_exit_after_capture(
            ExitAfterCaptureMode::Auto,
            CaptureDestination::FileOnly
        ));
        assert!(!should_exit_after_capture(
            ExitAfterCaptureMode::Auto,
            CaptureDestination::ClipboardAndFile
        ));
    }
}
