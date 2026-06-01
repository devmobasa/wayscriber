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
        self.exit_overlay_suppression(OverlaySuppression::DesktopBackdrop);
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
        if !self.enter_overlay_suppression(OverlaySuppression::Capture) {
            log::warn!(
                "Capture action {:?} requested while overlay is suppressed; ignoring",
                action
            );
            self.capture.clear_exit_on_success();
            self.input_state.set_ui_toast(
                crate::input::state::UiToastKind::Warning,
                "Capture is already preparing another overlay operation.",
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

    pub(in crate::backend::wayland) fn begin_pending_capture(
        &mut self,
        request: CapturePreflightRequest,
    ) {
        let result = match request {
            CapturePreflightRequest::Screenshot(request) => {
                log::info!("Requesting {:?} capture", request.capture_type);
                self.capture.manager_mut().request_capture(
                    request.capture_type,
                    request.destination,
                    request.save_config,
                )
            }
            CapturePreflightRequest::DesktopBackdrop(request) => {
                log::info!(
                    "Requesting desktop backdrop capture for {:?}",
                    request.operation
                );
                self.capture
                    .manager_mut()
                    .request_desktop_backdrop_capture(request)
            }
        };

        if let Err(e) = result {
            log::error!("Failed to request capture: {}", e);
            self.capture.clear_preflight();
            self.capture.clear_pending_pdf_export();
            self.show_overlay();
            self.capture.clear_in_progress();
            self.capture.clear_exit_on_success();
        }
    }
}

fn desktop_backdrop_output_geometry_from_info(
    info: &smithay_client_toolkit::output::OutputInfo,
) -> Option<DesktopBackdropOutputGeometry> {
    let (logical_x, logical_y) = info.logical_position?;
    let (logical_width, logical_height) = info.logical_size?;
    if logical_width <= 0 || logical_height <= 0 {
        return None;
    }
    let (physical_width, physical_height) = current_or_preferred_mode_size(info)
        .map(|(width, height)| transformed_output_size(width, height, info.transform))
        .or_else(|| {
            let scale = u32::try_from(info.scale_factor.max(1)).ok()?;
            Some((
                u32::try_from(logical_width).ok()?.checked_mul(scale)?,
                u32::try_from(logical_height).ok()?.checked_mul(scale)?,
            ))
        })?;
    if physical_width == 0 || physical_height == 0 {
        return None;
    }

    Some(DesktopBackdropOutputGeometry {
        logical_x,
        logical_y,
        logical_width: logical_width as u32,
        logical_height: logical_height as u32,
        physical_width,
        physical_height,
    })
}

fn current_or_preferred_mode_size(
    info: &smithay_client_toolkit::output::OutputInfo,
) -> Option<(u32, u32)> {
    info.modes
        .iter()
        .find(|mode| mode.current)
        .or_else(|| info.modes.iter().find(|mode| mode.preferred))
        .and_then(|mode| {
            Some((
                u32::try_from(mode.dimensions.0).ok()?,
                u32::try_from(mode.dimensions.1).ok()?,
            ))
        })
        .filter(|(width, height)| *width > 0 && *height > 0)
}

fn transformed_output_size(width: u32, height: u32, transform: wl_output::Transform) -> (u32, u32) {
    if matches!(
        transform,
        wl_output::Transform::_90
            | wl_output::Transform::_270
            | wl_output::Transform::Flipped90
            | wl_output::Transform::Flipped270
    ) {
        (height, width)
    } else {
        (width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transformed_output_size_keeps_unrotated_transforms() {
        assert_eq!(
            transformed_output_size(3840, 2160, wl_output::Transform::Normal),
            (3840, 2160)
        );
        assert_eq!(
            transformed_output_size(3840, 2160, wl_output::Transform::_180),
            (3840, 2160)
        );
        assert_eq!(
            transformed_output_size(3840, 2160, wl_output::Transform::Flipped),
            (3840, 2160)
        );
        assert_eq!(
            transformed_output_size(3840, 2160, wl_output::Transform::Flipped180),
            (3840, 2160)
        );
    }

    #[test]
    fn transformed_output_size_swaps_rotated_transforms() {
        for transform in [
            wl_output::Transform::_90,
            wl_output::Transform::_270,
            wl_output::Transform::Flipped90,
            wl_output::Transform::Flipped270,
        ] {
            assert_eq!(transformed_output_size(3840, 2160, transform), (2160, 3840));
        }
    }
}
