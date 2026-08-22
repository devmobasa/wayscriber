use crate::backend::wayland::acquisition::ScreenAcquisitionOwner;
use crate::backend::wayland::zoom::ZoomWaiterOwner;
use crate::capture::{
    CaptureDestination, CaptureType, ImageFormatMetadata, ImageOperationKind, RenderImageRequest,
    RenderedImageDeliveryRequest,
    file::{FileSaveConfig, expand_tilde},
};
use crate::config::{Action, RegionPicker};
use crate::input::state::{
    BoardPasteTarget, RegionInputSource, RegionPurposeTag, ScreenCaptureSource, Toast,
    ToastPriority,
};
use crate::screen_pixels::{EmbeddedImageLimits, ImagePixelRect, PackedArgb32};
use crate::ui::RegionAction;

use super::super::capture::should_exit_after_capture;
use super::super::screen_image::{
    CropError, copy_image_rect, displayed_screen_image, screen_source_is,
};
use super::{
    ActiveScreenRegion, FreezeOwnership, RegionCaptureIntent, RegionPickerOptions,
    RegionSelectionFinalize,
};
use crate::backend::wayland::state::WaylandState;

const TOAST_SOURCE: &str = "capture";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionPickerEntry {
    Activate,
    WaitForZoom,
    AutoFreeze,
    Legacy,
    RefuseSolidZoom,
    ZoomImageUnavailable,
}

fn region_picker_entry(
    has_source: bool,
    board_is_transparent: bool,
    zoom_engaged: bool,
    zoom_active: bool,
    frozen_enabled: bool,
) -> RegionPickerEntry {
    if has_source {
        RegionPickerEntry::Activate
    } else if !board_is_transparent && zoom_engaged {
        RegionPickerEntry::RefuseSolidZoom
    } else if zoom_engaged && !zoom_active {
        RegionPickerEntry::WaitForZoom
    } else if !frozen_enabled {
        RegionPickerEntry::Legacy
    } else if zoom_active {
        RegionPickerEntry::ZoomImageUnavailable
    } else {
        // Unlike OCR and the eyedropper, the capture picker deliberately
        // freezes over a solid board so the crop matches the forced backdrop.
        RegionPickerEntry::AutoFreeze
    }
}

fn region_destination(
    action: Action,
    default_destination: CaptureDestination,
) -> Option<CaptureDestination> {
    match action {
        Action::CaptureSelection => Some(default_destination),
        Action::CaptureClipboardSelection | Action::CaptureClipboardRegion => {
            Some(CaptureDestination::ClipboardOnly)
        }
        Action::CaptureFileSelection | Action::CaptureFileRegion => {
            Some(CaptureDestination::FileOnly)
        }
        Action::CaptureRegionInteractive => Some(default_destination),
        _ => None,
    }
}

fn region_delivery_request(
    pixels: PackedArgb32,
    intent: &RegionCaptureIntent,
    destination: CaptureDestination,
) -> RenderedImageDeliveryRequest {
    let save_config = intent.save_config().cloned().map(|mut save_config| {
        // The source crop is encoded by the shared Cairo PNG path regardless
        // of the configured screenshot format. Keep both initial delivery and
        // any clipboard fallback named for the bytes they actually contain.
        save_config.format = ImageFormatMetadata::png().extension;
        save_config
    });
    let render: crate::capture::ImageRenderJob =
        Box::new(move || crate::capture::png::encode_packed_argb32_png(&pixels));
    RenderedImageDeliveryRequest {
        render,
        destination,
        save_config,
        operation: ImageOperationKind::Screenshot,
        fallback_format_override: Some(ImageFormatMetadata::png()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RegionSubmit {
    Deliver(CaptureDestination),
    Board(BoardPasteTarget),
}

const fn review_delivery_destination(action: RegionAction) -> Option<CaptureDestination> {
    match action {
        RegionAction::Copy => Some(CaptureDestination::ClipboardOnly),
        RegionAction::Save => Some(CaptureDestination::FileOnly),
        RegionAction::Both => Some(CaptureDestination::ClipboardAndFile),
        RegionAction::Board => None,
    }
}

fn legacy_region_request(intent: &RegionCaptureIntent) -> crate::capture::CaptureRequest {
    crate::capture::CaptureRequest {
        capture_type: CaptureType::Selection {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        },
        destination: intent.destination(),
        save_config: intent.save_config().cloned(),
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn region_review_action_at(
        &self,
        point: (f64, f64),
    ) -> Option<RegionAction> {
        if !self.input_state.region_state().is_review() {
            return None;
        }
        let selection = self.region_selection_geometry()?.display_selection();
        crate::ui::RegionActionBar::place(selection, (self.surface.width(), self.surface.height()))
            .hit(point)
    }

    pub(in crate::backend::wayland) fn region_review_bar_contains(
        &self,
        point: (f64, f64),
    ) -> bool {
        if !self.input_state.region_state().is_review() {
            return false;
        }
        self.region_selection_geometry().is_some_and(|geometry| {
            crate::ui::RegionActionBar::place(
                geometry.display_selection(),
                (self.surface.width(), self.surface.height()),
            )
            .contains(point)
        })
    }

    pub(in crate::backend::wayland) fn submit_region_review_action(
        &mut self,
        action: RegionAction,
    ) -> bool {
        let Some(rect) = self.region_review_rect() else {
            return false;
        };
        let submit = match review_delivery_destination(action) {
            Some(destination) => RegionSubmit::Deliver(destination),
            None => {
                debug_assert_eq!(action, RegionAction::Board);
                let limits = EmbeddedImageLimits::default();
                if !limits.allows_pixels(rect.width(), rect.height()) {
                    self.input_state.push_toast(
                        ToastPriority::Info,
                        TOAST_SOURCE,
                        Toast::warning("Region is too large to add to the board."),
                    );
                    return true;
                }
                let Some(ActiveScreenRegion::Ready { source, .. }) = self.data.active_screen_region
                else {
                    return false;
                };
                let display = super::super::screen_image::screen_rect_for_image_rect(&source, rect);
                let Some(world_bounds) =
                    super::world_rect_for_screen_rect(display, self.board_view_offset(), source)
                else {
                    return false;
                };
                RegionSubmit::Board(BoardPasteTarget {
                    board_id: self.input_state.boards.active_board_id().to_string(),
                    page_index: self.input_state.boards.active_page_index(),
                    page_generation: self.input_state.boards.active_page_generation(),
                    world_bounds,
                })
            }
        };
        self.submit_region_capture_with(rect, submit);
        true
    }

    /// Submit the whole displayed image and retire any in-flight drag owner.
    ///
    /// Keeping this transaction on WaylandState leaves the keyboard protocol
    /// callback responsible only for translating Ctrl+A into one state action.
    pub(in crate::backend::wayland) fn submit_whole_region_capture(&mut self) {
        let Some(RegionSelectionFinalize::Selected { purpose, rect }) =
            self.whole_image_region_selection()
        else {
            return;
        };
        match self.input_state.region_state().selection_owner() {
            Some(RegionInputSource::Pointer | RegionInputSource::Touch) => {
                self.set_suppress_next_release(true);
            }
            Some(RegionInputSource::Stylus) => self.retire_stylus_contact(),
            None => {}
        }
        if purpose == RegionPurposeTag::CaptureInteractive {
            self.enter_region_review(rect);
        } else {
            self.submit_region_capture(rect);
        }
    }

    pub(in crate::backend::wayland::state) fn begin_region_capture_action(
        &mut self,
        action: Action,
        default_destination: CaptureDestination,
    ) {
        let Some(destination) = region_destination(action, default_destination) else {
            return;
        };
        if !self.config.capture.enabled {
            self.input_state.push_toast(
                ToastPriority::Info,
                TOAST_SOURCE,
                Toast::warning("Screen capture is disabled."),
            );
            return;
        }
        if self.capture.is_in_progress() {
            self.input_state.push_toast(
                ToastPriority::Info,
                TOAST_SOURCE,
                Toast::warning("Another capture operation is still in progress."),
            );
            return;
        }

        let interactive = action == Action::CaptureRegionInteractive;
        let purpose = if interactive {
            RegionPurposeTag::CaptureInteractive
        } else {
            RegionPurposeTag::CaptureDeliver
        };
        let save_config =
            (interactive || !matches!(destination, CaptureDestination::ClipboardOnly)).then(|| {
                FileSaveConfig {
                    save_directory: expand_tilde(&self.config.capture.save_directory),
                    filename_template: self.config.capture.filename_template.clone(),
                    // Keep the configured format in the immutable intent so an
                    // explicit slurp handoff preserves legacy behavior. Native
                    // delivery converts its own save metadata to PNG below.
                    format: self.config.capture.format.clone(),
                }
            });
        let region = &self.config.capture.region;
        let options = RegionPickerOptions::new(
            region.show_size_readout,
            region.show_loupe,
            region.show_legend,
        );
        let intent = RegionCaptureIntent::new(
            action,
            purpose,
            destination,
            save_config,
            self.exit_after_capture_mode,
            options,
        );
        if !self.capture.reserve_region(intent) {
            self.input_state.push_toast(
                ToastPriority::Info,
                TOAST_SOURCE,
                Toast::warning("Another capture operation is still in progress."),
            );
            return;
        }

        if interactive && !self.frozen_enabled() {
            self.cancel_region_capture_ui_and_lifecycle();
            self.input_state.push_toast(
                ToastPriority::Info,
                TOAST_SOURCE,
                Toast::warning("Interactive region capture requires the native screen picker."),
            );
            return;
        }
        if !interactive && (region.picker == RegionPicker::Slurp || !self.frozen_enabled()) {
            self.handoff_region_capture_to_legacy();
            return;
        }

        self.input_state.prepare_for_screen_modal();
        self.zoom.stop_pan();
        self.stop_board_pan();
        self.set_board_pan_key_held(false);
        self.cancel_toolbar_move_drag();
        self.unlock_pointer();
        self.retire_stylus_contact();

        let generation = self.next_screen_region_generation();
        let has_source = displayed_screen_image(
            &self.zoom,
            &self.frozen,
            self.input_state.board_is_transparent(),
        )
        .is_some();
        match region_picker_entry(
            has_source,
            self.input_state.board_is_transparent(),
            self.zoom.is_engaged(),
            self.zoom.active,
            self.frozen_enabled(),
        ) {
            RegionPickerEntry::Activate => {
                if !self.activate_screen_region(purpose, generation, FreezeOwnership::PreExisting) {
                    self.finish_region_activation_rejection();
                }
            }
            RegionPickerEntry::WaitForZoom => {
                if self.wait_for_current_zoom_capture(ZoomWaiterOwner::RegionCapture) {
                    self.set_pending_screen_region(
                        purpose,
                        generation,
                        ScreenCaptureSource::Zoom,
                        None,
                    );
                } else {
                    self.cancel_region_capture_ui_and_lifecycle();
                    self.report_region_zoom_unavailable();
                }
            }
            RegionPickerEntry::AutoFreeze => {
                match self.request_screen_acquisition(ScreenAcquisitionOwner::RegionCapture) {
                    Ok(acquisition) => self.set_pending_screen_region(
                        purpose,
                        generation,
                        ScreenCaptureSource::Frozen,
                        Some(acquisition),
                    ),
                    Err(_) => {
                        self.cancel_region_capture_ui_and_lifecycle();
                        self.input_state.push_toast(
                            ToastPriority::Info,
                            TOAST_SOURCE,
                            Toast::warning(
                                "Capture is already preparing another overlay operation.",
                            ),
                        );
                    }
                }
            }
            RegionPickerEntry::Legacy => self.handoff_region_capture_to_legacy(),
            RegionPickerEntry::RefuseSolidZoom => {
                self.cancel_region_capture_ui_and_lifecycle();
                self.input_state.push_toast(
                    ToastPriority::Info,
                    TOAST_SOURCE,
                    Toast::warning("Region capture is unavailable while zoomed on a solid board."),
                );
            }
            RegionPickerEntry::ZoomImageUnavailable => {
                self.cancel_region_capture_ui_and_lifecycle();
                self.report_region_zoom_unavailable();
            }
        }
    }

    fn report_region_zoom_unavailable(&mut self) {
        self.input_state.push_toast(
            ToastPriority::Info,
            TOAST_SOURCE,
            Toast::warning("Region capture is unavailable because zoom has no screen image."),
        );
    }

    fn finish_region_activation_rejection(&mut self) {
        self.cancel_region_capture_ui_and_lifecycle();
        super::super::acquisition::report_screen_source_activation_rejected_to(
            &mut self.input_state,
            ScreenAcquisitionOwner::RegionCapture,
        );
    }

    pub(in crate::backend::wayland::state) fn finish_pending_region_capture(
        &mut self,
        source: ScreenCaptureSource,
        installed_generation: u64,
    ) -> bool {
        let Some(region) = self.data.active_screen_region else {
            return false;
        };
        if !region.purpose().is_capture() {
            return false;
        }
        let waiting = matches!(
            (region, source),
            (
                ActiveScreenRegion::PendingFrozen { .. },
                ScreenCaptureSource::Frozen
            ) | (
                ActiveScreenRegion::PendingZoom { .. },
                ScreenCaptureSource::Zoom
            )
        );
        if !waiting {
            return false;
        }
        let ownership = if source == ScreenCaptureSource::Frozen {
            FreezeOwnership::PickerOwned {
                image_generation: installed_generation,
            }
        } else {
            FreezeOwnership::PreExisting
        };
        self.activate_screen_region(region.purpose(), region.generation(), ownership)
    }

    pub(in crate::backend::wayland) fn cancel_region_capture(&mut self) -> bool {
        let Some(region) = self.data.active_screen_region else {
            let reserved = self.capture.active_region_action().is_some();
            if reserved {
                self.clear_zoom_waiter_for(ZoomWaiterOwner::RegionCapture);
                self.clear_screen_region_ui_only();
                self.capture.finish_capture_lifecycle();
            }
            return reserved;
        };
        if !region.purpose().is_capture() {
            return false;
        }
        self.cancel_modal_owner_resources(
            ScreenAcquisitionOwner::RegionCapture,
            region.pending_acquisition(),
            region.owned_frozen_generation(),
        );
        true
    }

    /// Settle native picker ownership before the Wayland state is dropped.
    ///
    /// Accepted manager work and legacy preflight already own their terminal
    /// paths. Reserved native UI, acquisition waiters, and picker-owned frozen
    /// generations must instead leave through the normal correlated cancel so
    /// XDG restoration and unfreeze happen exactly once.
    pub(in crate::backend::wayland) fn cancel_region_capture_for_teardown(&mut self) {
        let ui_owns_region = self
            .data
            .active_screen_region
            .is_some_and(|region| region.purpose().is_capture());
        if ui_owns_region || self.capture.active_region_action().is_some() {
            self.cancel_region_capture();
        }
    }

    pub(in crate::backend::wayland::state) fn cancel_region_capture_ui_and_lifecycle(&mut self) {
        self.clear_zoom_waiter_for(ZoomWaiterOwner::RegionCapture);
        self.clear_screen_region_ui_only();
        self.capture.finish_capture_lifecycle();
    }

    pub(super) fn clear_region_capture_ui_for_handoff(&mut self) {
        self.clear_zoom_waiter_for(ZoomWaiterOwner::RegionCapture);
        self.clear_screen_region_ui_only();
    }

    pub(in crate::backend::wayland::state) fn handoff_region_capture_to_legacy(&mut self) {
        self.clear_region_capture_ui_for_handoff();
        let Some(intent) = self.capture.handoff_region_to_legacy() else {
            self.capture.finish_capture_lifecycle();
            return;
        };
        debug_assert!(intent.purpose().is_capture());
        self.capture.set_exit_on_success(should_exit_after_capture(
            intent.exit_mode(),
            intent.destination(),
        ));
        if !self.enter_overlay_suppression(super::super::OverlaySuppression::Capture) {
            self.capture.finish_capture_lifecycle();
            self.input_state.push_toast(
                ToastPriority::Info,
                TOAST_SOURCE,
                Toast::warning("Capture is already preparing another overlay operation."),
            );
            return;
        }
        let request = legacy_region_request(&intent);
        self.capture.queue_preflight(
            crate::backend::wayland::capture::CapturePreflightRequest::Screenshot(request),
        );
    }

    pub(in crate::backend::wayland) fn submit_region_capture(&mut self, rect: ImagePixelRect) {
        let destination = match self.capture.region_phase() {
            crate::backend::wayland::capture::RegionCapturePhase::Reserved(intent) => {
                intent.destination()
            }
            _ => return,
        };
        self.submit_region_capture_with(rect, RegionSubmit::Deliver(destination));
    }

    fn submit_region_capture_with(&mut self, rect: ImagePixelRect, submit: RegionSubmit) {
        let Some(ActiveScreenRegion::Ready {
            source: token,
            freeze_ownership,
            purpose,
            ..
        }) = self.data.active_screen_region
        else {
            self.cancel_region_capture_ui_and_lifecycle();
            return;
        };
        if !purpose.is_capture() {
            return;
        }
        let Some(source) = displayed_screen_image(
            &self.zoom,
            &self.frozen,
            self.input_state.board_is_transparent(),
        ) else {
            self.cancel_region_capture_for_source_change();
            return;
        };
        if !screen_source_is(
            &token,
            &source,
            &self.zoom,
            &self.frozen,
            (self.surface.width(), self.surface.height()),
        ) {
            self.cancel_region_capture_for_source_change();
            return;
        }
        let pixels = match copy_image_rect(source.image, rect) {
            Ok(pixels) => pixels,
            Err(error) => {
                let message = match error {
                    CropError::Empty => "That selection has no screen pixels.",
                    CropError::OutOfBounds => "Could not read that region of the screen image.",
                };
                self.cancel_region_capture();
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    TOAST_SOURCE,
                    Toast::error(message),
                );
                return;
            }
        };

        let Some(intent) = self.capture.begin_region_submission() else {
            self.cancel_region_capture();
            self.input_state.push_toast(
                ToastPriority::Critical,
                TOAST_SOURCE,
                Toast::error("Region capture state was inconsistent; try again."),
            );
            return;
        };
        self.clear_screen_region_ui_only();
        if let FreezeOwnership::PickerOwned { image_generation } = freeze_ownership {
            self.release_owned_frozen_generation(image_generation);
        }

        match submit {
            RegionSubmit::Deliver(destination) => {
                self.capture.set_exit_on_success(should_exit_after_capture(
                    intent.exit_mode(),
                    destination,
                ));
                let request = region_delivery_request(pixels, &intent, destination);
                let submission = self
                    .capture
                    .manager_mut()
                    .request_rendered_image_delivery(request);
                self.accept_capture_submission(submission, ImageOperationKind::Screenshot);
            }
            RegionSubmit::Board(target) => {
                self.capture.set_exit_on_success(false);
                let render =
                    Box::new(move || crate::capture::png::encode_packed_argb32_png(&pixels));
                let submission =
                    self.capture
                        .manager_mut()
                        .request_render_image(RenderImageRequest {
                            render,
                            operation: ImageOperationKind::Screenshot,
                        });
                let accepted_id = submission.as_ref().ok().copied();
                if self.accept_capture_submission(submission, ImageOperationKind::Screenshot) {
                    let Some(id) = accepted_id else {
                        unreachable!("an accepted submission has an id")
                    };
                    if !self.capture.set_pending_board_paste(id, target) {
                        self.capture.manager_mut().mark_unhealthy();
                        self.capture.finish_capture_lifecycle();
                        self.input_state.push_toast(
                            ToastPriority::Critical,
                            TOAST_SOURCE,
                            Toast::error("Region was not added to the board."),
                        );
                    }
                }
            }
        }
    }

    pub(super) fn cancel_region_capture_for_source_change(&mut self) {
        if self.cancel_region_capture() {
            self.input_state.push_toast(
                ToastPriority::Info,
                TOAST_SOURCE,
                Toast::warning("The screen changed, so region capture was cancelled."),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::ExitAfterCaptureMode;

    #[test]
    fn every_bound_region_action_has_the_declared_destination() {
        let default = CaptureDestination::ClipboardAndFile;
        assert_eq!(
            region_destination(Action::CaptureSelection, default),
            Some(default)
        );
        for action in [
            Action::CaptureClipboardSelection,
            Action::CaptureClipboardRegion,
        ] {
            assert_eq!(
                region_destination(action, default),
                Some(CaptureDestination::ClipboardOnly)
            );
        }
        for action in [Action::CaptureFileSelection, Action::CaptureFileRegion] {
            assert_eq!(
                region_destination(action, default),
                Some(CaptureDestination::FileOnly)
            );
        }
        assert_eq!(
            region_destination(Action::CaptureRegionInteractive, default),
            Some(default)
        );
        assert_eq!(region_destination(Action::CaptureFullScreen, default), None);
    }

    #[test]
    fn review_destination_labels_match_the_delivery_they_request() {
        assert_eq!(
            review_delivery_destination(RegionAction::Copy),
            Some(CaptureDestination::ClipboardOnly)
        );
        assert_eq!(
            review_delivery_destination(RegionAction::Save),
            Some(CaptureDestination::FileOnly)
        );
        assert_eq!(
            review_delivery_destination(RegionAction::Both),
            Some(CaptureDestination::ClipboardAndFile),
            "Both must always match its label"
        );
        assert_eq!(review_delivery_destination(RegionAction::Board), None);
    }

    #[test]
    fn native_entry_auto_freezes_solid_boards_but_refuses_solid_zoom() {
        assert_eq!(
            region_picker_entry(false, false, false, false, true),
            RegionPickerEntry::AutoFreeze
        );
        assert_eq!(
            region_picker_entry(false, false, true, true, true),
            RegionPickerEntry::RefuseSolidZoom
        );
    }

    #[test]
    fn native_entry_uses_existing_waiting_legacy_and_missing_zoom_paths() {
        assert_eq!(
            region_picker_entry(true, false, true, true, true),
            RegionPickerEntry::Activate
        );
        assert_eq!(
            region_picker_entry(false, true, true, false, true),
            RegionPickerEntry::WaitForZoom
        );
        assert_eq!(
            region_picker_entry(false, true, false, false, false),
            RegionPickerEntry::Legacy
        );
        assert_eq!(
            region_picker_entry(false, true, true, true, true),
            RegionPickerEntry::ZoomImageUnavailable
        );
    }

    #[test]
    fn picker_options_snapshot_is_independent_of_later_config_changes() {
        let mut live = crate::config::RegionCaptureConfig::default();
        let options =
            RegionPickerOptions::new(live.show_size_readout, live.show_loupe, live.show_legend);
        let intent = RegionCaptureIntent::new(
            Action::CaptureSelection,
            RegionPurposeTag::CaptureDeliver,
            CaptureDestination::ClipboardOnly,
            None,
            ExitAfterCaptureMode::Auto,
            options,
        );
        live.show_size_readout = false;
        live.show_loupe = true;
        live.show_legend = false;
        live.picker = RegionPicker::Slurp;

        assert_eq!(live.picker, RegionPicker::Slurp);
        assert!(!live.show_size_readout);
        assert!(live.show_loupe);
        assert!(!live.show_legend);
        assert!(intent.options().show_size_readout());
        assert!(!intent.options().show_loupe());
        assert!(intent.options().show_legend());
    }

    #[test]
    fn native_delivery_forces_png_naming_metadata_and_bytes() {
        let intent = RegionCaptureIntent::new(
            Action::CaptureFileRegion,
            RegionPurposeTag::CaptureDeliver,
            CaptureDestination::FileOnly,
            Some(FileSaveConfig {
                save_directory: std::path::PathBuf::from("/tmp/captures"),
                filename_template: "region-{timestamp}".to_string(),
                format: "jpg".to_string(),
            }),
            ExitAfterCaptureMode::Never,
            RegionPickerOptions::new(true, false, true),
        );
        let pixels = PackedArgb32::new(1, 1, 4, 0xff33_2211_u32.to_ne_bytes().to_vec())
            .expect("one native ARGB32 pixel");

        let request = region_delivery_request(pixels, &intent, intent.destination());

        assert_eq!(request.destination, CaptureDestination::FileOnly);
        assert_eq!(request.operation, ImageOperationKind::Screenshot);
        assert_eq!(
            request
                .save_config
                .as_ref()
                .map(|save| save.format.as_str()),
            Some("png")
        );
        assert_eq!(
            request.fallback_format_override,
            Some(ImageFormatMetadata::png())
        );
        let rendered = (request.render)().expect("native region crop encodes");
        assert_eq!(rendered.format, ImageFormatMetadata::png());
        assert_eq!((rendered.width, rendered.height), (1, 1));
        assert!(rendered.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn legacy_handoff_preserves_the_reserved_selection_request_snapshot() {
        let intent = RegionCaptureIntent::new(
            Action::CaptureFileSelection,
            RegionPurposeTag::CaptureDeliver,
            CaptureDestination::FileOnly,
            Some(FileSaveConfig {
                save_directory: std::path::PathBuf::from("/tmp/original"),
                filename_template: "shot-{timestamp}".to_string(),
                format: "jpg".to_string(),
            }),
            ExitAfterCaptureMode::Always,
            RegionPickerOptions::new(false, true, false),
        );

        let request = legacy_region_request(&intent);

        assert!(matches!(
            request.capture_type,
            CaptureType::Selection {
                x: 0,
                y: 0,
                width: 0,
                height: 0
            }
        ));
        assert_eq!(request.destination, CaptureDestination::FileOnly);
        let save = request
            .save_config
            .expect("file selection keeps save config");
        assert_eq!(
            save.save_directory,
            std::path::PathBuf::from("/tmp/original")
        );
        assert_eq!(save.filename_template, "shot-{timestamp}");
        assert_eq!(
            save.format, "jpg",
            "explicit slurp keeps the legacy configured format"
        );
    }
}
