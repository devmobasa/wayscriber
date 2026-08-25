use super::super::*;
use crate::backend::wayland::capture::CaptureLayoutContext;
use crate::input::state::{Toast, ToastPriority};

impl WaylandState {
    pub(in crate::backend::wayland) fn handle_board_pdf_export_action(&mut self, action: Action) {
        if self.capture.is_in_progress() {
            log::warn!(
                "Board PDF export action {:?} requested while another image operation is running; ignoring",
                action
            );
            return;
        }

        if !matches!(
            action,
            Action::ExportBoardPdfFile | Action::ExportAllBoardsPdfFile
        ) {
            log::error!(
                "Non-board-PDF-export action passed to handle_board_pdf_export_action: {:?}",
                action
            );
            return;
        }

        let operation = if matches!(action, Action::ExportAllBoardsPdfFile) {
            ImageOperationKind::AllBoardsPdfExport
        } else {
            ImageOperationKind::BoardPdfExport
        };

        let destination = CaptureDestination::FileOnly;
        let exit_on_success = self.should_exit_after_capture(destination);
        let save_config = self.board_pdf_save_config(action);

        if self.should_capture_desktop_for_pdf_export(action) {
            let Some((request, layout_context)) = self.desktop_backdrop_capture_request(operation)
            else {
                let message =
                    "Board PDF export failed: active output geometry is unavailable".to_string();
                log::error!("{message}");
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    "capture.pdf",
                    Toast::error(message),
                );
                return;
            };
            if !self.enter_overlay_suppression(OverlaySuppression::DesktopBackdrop) {
                log::warn!(
                    "Board PDF export action {:?} requested while overlay is suppressed; ignoring",
                    action
                );
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "capture.pdf",
                    Toast::warning(
                        "Board PDF export is already preparing another overlay operation.",
                    ),
                );
                return;
            }
            self.capture.set_exit_on_success(exit_on_success);
            self.capture.mark_in_progress();
            self.capture.set_pending_pdf_export(PendingPdfExport {
                action,
                operation,
                save_config,
                layout_context,
            });
            log::info!(
                "Queued {:?} desktop backdrop capture for PDF export; waiting for suppression frame",
                operation
            );
            self.capture
                .queue_preflight(CapturePreflightRequest::DesktopBackdrop(request));
            return;
        }

        let snapshot = match self.board_pdf_export_snapshot(action) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                let message = operation.format_error(&err);
                log::error!("Board PDF export failed: {}", message);
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    "capture.pdf",
                    Toast::error(message),
                );
                return;
            }
        };

        self.queue_board_pdf_document_delivery(snapshot, save_config, operation, exit_on_success);
    }

    pub(in crate::backend::wayland) fn finish_pending_board_pdf_export_with_backdrop(
        &mut self,
        backdrop: DesktopBackdropCaptureResult,
        exit_on_success: bool,
    ) {
        let Some(pending) = self.capture.take_pending_pdf_export() else {
            let message =
                "Board PDF export failed: desktop backdrop completed without pending PDF export"
                    .to_string();
            log::error!("{message}");
            self.input_state.push_toast(
                ToastPriority::Critical,
                "capture.pdf",
                Toast::error(message),
            );
            return;
        };

        let active_output_id = self
            .surface
            .current_output()
            .and_then(|output| self.output_state.info(&output).map(|info| info.id));
        if !pending
            .layout_context
            .matches(active_output_id, self.frozen.output_layout_generation())
        {
            let message =
                "Board PDF export failed: output layout changed during desktop capture".to_string();
            log::error!("{message}");
            self.input_state.push_toast(
                ToastPriority::Critical,
                "capture.pdf",
                Toast::error(message),
            );
            return;
        }

        let snapshot = match self.board_pdf_export_snapshot_with_desktop_backdrop(
            pending.action,
            CanvasExportBackdropSnapshot::PersistedImage {
                data: backdrop.data,
                width: backdrop.width,
                height: backdrop.height,
                stride: backdrop.stride,
                logical_to_image_scale_x: backdrop.logical_to_image_scale_x,
                logical_to_image_scale_y: backdrop.logical_to_image_scale_y,
            },
        ) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                let message = pending.operation.format_error(&err);
                log::error!("Board PDF export failed after desktop capture: {}", message);
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    "capture.pdf",
                    Toast::error(message),
                );
                return;
            }
        };

        self.queue_board_pdf_document_delivery(
            snapshot,
            pending.save_config,
            pending.operation,
            exit_on_success,
        );
    }

    fn queue_board_pdf_document_delivery(
        &mut self,
        snapshot: BoardPdfExportSnapshot,
        save_config: FileSaveConfig,
        operation: ImageOperationKind,
        exit_on_success: bool,
    ) {
        if let Err(error) = snapshot.validate_spotlight_sources() {
            self.report_export_preflight_failure(&error, operation, "capture.pdf");
            return;
        }
        // Render on the capture worker, not here: an all-boards export
        // renders every page of every board plus PDF encoding, which stalled
        // event dispatch for seconds on multi-board sessions.
        let render: crate::capture::DocumentRenderJob = Box::new(move || {
            render_board_pdf(&snapshot).map(|bytes| RenderedDocument {
                bytes,
                extension: "pdf".to_string(),
                mime_type: "application/pdf".to_string(),
            })
        });

        self.capture.set_exit_on_success(exit_on_success);
        self.capture.mark_in_progress();

        let request = crate::capture::RenderedDocumentDeliveryRequest {
            render,
            destination: CaptureDestination::FileOnly,
            save_config: Some(save_config),
            operation,
        };

        let submission = self
            .capture
            .manager_mut()
            .request_rendered_document_delivery(request);
        self.accept_capture_submission(submission, operation);
    }

    fn board_pdf_save_config(&self, action: Action) -> FileSaveConfig {
        FileSaveConfig {
            save_directory: expand_tilde(&self.config.capture.save_directory),
            filename_template: if matches!(action, Action::ExportAllBoardsPdfFile) {
                self.config
                    .export
                    .pdf
                    .resolved_all_boards_filename_template(&self.config.capture)
            } else {
                self.config
                    .export
                    .pdf
                    .resolved_filename_template(&self.config.capture)
            },
            format: "pdf".to_string(),
        }
    }

    fn should_capture_desktop_for_pdf_export(&self, action: Action) -> bool {
        self.config.export.pdf.transparent_background
            == crate::config::PdfTransparentBackground::Desktop
            && self.board_pdf_export_scope_has_transparent_pages(action)
    }

    fn desktop_backdrop_capture_request(
        &self,
        operation: ImageOperationKind,
    ) -> Option<(DesktopBackdropCaptureRequest, CaptureLayoutContext)> {
        let output = self.surface.current_output()?;
        let output_id = self.output_state.info(&output)?.id;
        let geometry = self.desktop_backdrop_geometry()?;
        let layout_context =
            CaptureLayoutContext::new(output_id, self.frozen.output_layout_generation());
        let request = DesktopBackdropCaptureRequest {
            logical_width: self.surface.width(),
            logical_height: self.surface.height(),
            scale: self.surface.scale(),
            geometry: Some(geometry),
            operation,
        };
        Some((request, layout_context))
    }
}
