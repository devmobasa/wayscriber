use crate::backend::wayland::acquisition::ScreenAcquisitionOwner;
use crate::backend::wayland::zoom::ZoomWaiterOwner;
use crate::capture::{
    CaptureDestination, CaptureType,
    file::{FileSaveConfig, expand_tilde},
};
use crate::config::{Action, RegionPicker};
use crate::input::state::{RegionPurposeTag, ScreenCaptureSource, Toast, ToastPriority};

use super::super::capture::{overlay_suppression_for_screenshot, should_exit_after_capture};
use super::super::screen_image::displayed_screen_image;
use super::{ActiveScreenRegion, FreezeOwnership, RegionCaptureIntent, RegionPickerOptions};
use crate::backend::wayland::state::WaylandState;

const TOAST_SOURCE: &str = "capture";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RegionPickerEntry {
    Activate,
    WaitForZoom,
    AutoFreeze,
    Legacy,
    RefuseSolidZoom,
    ZoomImageUnavailable,
}

pub(super) fn region_picker_entry(
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

pub(super) fn region_destination(
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

pub(super) fn legacy_region_request(
    intent: &RegionCaptureIntent,
) -> crate::capture::CaptureRequest {
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
            self.config.capture.include_drawings,
        );
        let include_drawings = intent.include_drawings();
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
                if !self.activate_screen_region(
                    purpose,
                    generation,
                    FreezeOwnership::PreExisting,
                    include_drawings,
                ) {
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
        let include_drawings = match self.capture.region_phase() {
            crate::backend::wayland::capture::RegionCapturePhase::Reserved(intent) => {
                intent.include_drawings()
            }
            crate::backend::wayland::capture::RegionCapturePhase::Idle
            | crate::backend::wayland::capture::RegionCapturePhase::Submitting(_)
            | crate::backend::wayland::capture::RegionCapturePhase::Accepted => return false,
        };
        self.activate_screen_region(
            region.purpose(),
            region.generation(),
            ownership,
            include_drawings,
        )
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
        let suppression = overlay_suppression_for_screenshot(
            CaptureType::Selection {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            intent.include_drawings(),
        );
        if !self.enter_overlay_suppression(suppression) {
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
