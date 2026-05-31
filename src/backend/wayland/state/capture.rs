use super::*;
use crate::capture::CaptureRequest;

impl WaylandState {
    fn should_exit_after_capture(&self, destination: CaptureDestination) -> bool {
        let is_clipboard_only = matches!(destination, CaptureDestination::ClipboardOnly);
        match self.exit_after_capture_mode {
            ExitAfterCaptureMode::Always => true,
            ExitAfterCaptureMode::Never => false,
            ExitAfterCaptureMode::Auto => is_clipboard_only,
        }
    }

    pub(in crate::backend::wayland) fn apply_capture_completion(&mut self) {
        if self.frozen.take_capture_done() {
            self.exit_overlay_suppression(OverlaySuppression::Frozen);
        }
        if self.zoom.take_capture_done() {
            self.exit_overlay_suppression(OverlaySuppression::Zoom);
        }
    }

    /// Restore the overlay after screenshot capture completes.
    ///
    /// Re-maps the layer surface to its original size and forces a redraw.
    pub(in crate::backend::wayland) fn show_overlay(&mut self) {
        self.input_state.clear_click_highlights();
        self.exit_overlay_suppression(OverlaySuppression::Capture);
    }

    /// Handles capture actions by delegating to the CaptureManager.
    pub(in crate::backend::wayland) fn handle_capture_action(&mut self, action: Action) {
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

        // Suppress overlay before capture to prevent capturing the overlay itself
        self.enter_overlay_suppression(OverlaySuppression::Capture);
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
        self.capture.queue_preflight(request);
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

        let snapshot = self.canvas_export_snapshot();
        let rendered = match render_canvas_png(&snapshot) {
            Ok(rendered) => rendered,
            Err(err) => {
                let message = ImageOperationKind::CanvasExport.format_error(&err);
                log::error!("Canvas export failed: {}", message);
                self.input_state
                    .set_ui_toast(crate::input::state::UiToastKind::Error, message);
                return;
            }
        };

        let save_config = if matches!(destination, CaptureDestination::ClipboardOnly) {
            None
        } else {
            Some(FileSaveConfig {
                save_directory: expand_tilde(&self.config.capture.save_directory),
                filename_template: self.config.capture.filename_template.clone(),
                format: rendered.format.extension.clone(),
            })
        };

        let exit_on_success = self.should_exit_after_capture(destination);
        self.capture.set_exit_on_success(exit_on_success);
        self.capture.mark_in_progress();

        let request = ImageDeliveryRequest {
            image: rendered,
            destination,
            save_config,
            operation: ImageOperationKind::CanvasExport,
            fallback_format_override: Some(ImageFormatMetadata::png()),
        };

        if let Err(err) = self.capture.manager_mut().request_image_delivery(request) {
            log::error!("Failed to request canvas export delivery: {}", err);
            self.capture.clear_in_progress();
            self.capture.clear_exit_on_success();
            self.input_state.set_ui_toast(
                crate::input::state::UiToastKind::Error,
                format!("Canvas export failed: {err}"),
            );
        }
    }

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

        let destination = CaptureDestination::FileOnly;
        self.capture
            .set_exit_on_success(self.should_exit_after_capture(destination));
        self.capture.mark_in_progress();

        let request = DocumentDeliveryRequest {
            document: RenderedDocument {
                bytes,
                extension: "pdf".to_string(),
                mime_type: "application/pdf".to_string(),
            },
            destination,
            save_config: Some(FileSaveConfig {
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
            }),
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

    pub(in crate::backend::wayland) fn begin_pending_capture(&mut self, request: CaptureRequest) {
        log::info!("Requesting {:?} capture", request.capture_type);
        if let Err(e) = self.capture.manager_mut().request_capture(
            request.capture_type,
            request.destination,
            request.save_config,
        ) {
            log::error!("Failed to request capture: {}", e);
            self.capture.clear_preflight();

            // Restore overlay on error
            self.show_overlay();
            self.capture.clear_in_progress();
            self.capture.clear_exit_on_success();
        }
    }
}
