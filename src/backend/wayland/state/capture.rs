use super::*;

impl WaylandState {
    fn should_exit_after_capture(&self, destination: CaptureDestination) -> bool {
        let is_clipboard = matches!(
            destination,
            CaptureDestination::ClipboardOnly | CaptureDestination::ClipboardAndFile
        );
        match self.exit_after_capture_mode {
            ExitAfterCaptureMode::Always => true,
            ExitAfterCaptureMode::Never => false,
            ExitAfterCaptureMode::Auto => is_clipboard,
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

        // Request capture
        log::info!("Requesting {:?} capture", capture_type);
        if let Err(e) =
            self.capture
                .manager_mut()
                .request_capture(capture_type, destination, save_config)
        {
            log::error!("Failed to request capture: {}", e);

            // Restore overlay on error
            self.show_overlay();
            self.capture.clear_in_progress();
            self.capture.clear_exit_on_success();
        }
    }
}
