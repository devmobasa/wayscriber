use super::super::*;
use super::backdrop::desktop_backdrop_output_geometry_from_info;

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
            let request = self.desktop_backdrop_capture_request(operation);
            if !self.enter_overlay_suppression(OverlaySuppression::DesktopBackdrop) {
                log::warn!(
                    "Board PDF export action {:?} requested while overlay is suppressed; ignoring",
                    action
                );
                self.input_state.set_ui_toast(
                    crate::input::state::UiToastKind::Warning,
                    "Board PDF export is already preparing another overlay operation.",
                );
                return;
            }
            self.capture.set_exit_on_success(exit_on_success);
            self.capture.mark_in_progress();
            self.capture.set_pending_pdf_export(PendingPdfExport {
                action,
                operation,
                save_config,
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
                self.input_state
                    .set_ui_toast(crate::input::state::UiToastKind::Error, message);
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
            self.input_state
                .set_ui_toast(crate::input::state::UiToastKind::Error, message);
            return;
        };

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
                self.input_state
                    .set_ui_toast(crate::input::state::UiToastKind::Error, message);
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
        let bytes = match render_board_pdf(&snapshot) {
            Ok(bytes) => bytes,
            Err(err) => {
                let message = operation.format_error(&err);
                log::error!("Board PDF export failed: {}", message);
                self.input_state
                    .set_ui_toast(crate::input::state::UiToastKind::Error, message);
                return;
            }
        };

        self.capture.set_exit_on_success(exit_on_success);
        self.capture.mark_in_progress();

        let request = DocumentDeliveryRequest {
            document: RenderedDocument {
                bytes,
                extension: "pdf".to_string(),
                mime_type: "application/pdf".to_string(),
            },
            destination: CaptureDestination::FileOnly,
            save_config: Some(save_config),
            operation,
        };

        if let Err(err) = self
            .capture
            .manager_mut()
            .request_document_delivery(request)
        {
            log::error!("Failed to request board PDF export delivery: {}", err);
            self.capture.clear_in_progress();
            self.capture.clear_exit_on_success();
            self.input_state.set_ui_toast(
                crate::input::state::UiToastKind::Error,
                format!("Board PDF export failed: {err}"),
            );
        }
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
    ) -> DesktopBackdropCaptureRequest {
        DesktopBackdropCaptureRequest {
            logical_width: self.surface.width(),
            logical_height: self.surface.height(),
            scale: self.surface.scale(),
            geometry: self.desktop_backdrop_geometry(),
            operation,
        }
    }

    fn desktop_backdrop_geometry(&self) -> Option<DesktopBackdropGeometry> {
        let output = self.surface.current_output()?;
        let active_info = self.output_state.info(&output)?;
        let active = desktop_backdrop_output_geometry_from_info(&active_info)?;
        let mut outputs = Vec::new();
        for output in self.output_state.outputs() {
            let info = self.output_state.info(&output)?;
            outputs.push(desktop_backdrop_output_geometry_from_info(&info)?);
        }

        DesktopBackdropGeometry::from_outputs(active, &outputs, active_info.scale_factor.max(1))
    }
}
