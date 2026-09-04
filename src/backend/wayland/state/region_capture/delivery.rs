use crate::capture::{
    CaptureDestination, ImageFormatMetadata, ImageOperationKind, RenderImageRequest,
    RenderedImageDeliveryRequest, output_size as band_cut_output_size,
};
use crate::input::state::{BoardPasteTarget, RegionPurposeTag, Toast, ToastPriority};
use crate::screen_pixels::{EmbeddedImageLimits, ImagePixelRect};
use crate::ui::RegionAction;

use super::super::capture::should_exit_after_capture;
use super::render::{RegionRenderRequest, region_render_job};
use super::{ActiveScreenRegion, FreezeOwnership, RegionCaptureIntent, RegionSelectionFinalize};
use crate::backend::wayland::state::WaylandState;

const TOAST_SOURCE: &str = "capture";

pub(super) fn region_delivery_request(
    render: crate::capture::ImageRenderJob,
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
    RenderedImageDeliveryRequest {
        render,
        destination,
        save_config,
        operation: ImageOperationKind::Screenshot,
        fallback_format_override: Some(ImageFormatMetadata::png()),
    }
}

pub(super) const fn review_delivery_destination(
    action: RegionAction,
) -> Option<CaptureDestination> {
    match action {
        RegionAction::Copy => Some(CaptureDestination::ClipboardOnly),
        RegionAction::Save => Some(CaptureDestination::FileOnly),
        RegionAction::Both => Some(CaptureDestination::ClipboardAndFile),
        RegionAction::Board
        | RegionAction::CutBand
        | RegionAction::UndoCut
        | RegionAction::RedoCut
        | RegionAction::ResetCuts
        | RegionAction::ToggleIncludeDrawings => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RegionSubmit {
    Deliver(CaptureDestination),
    Board(BoardPasteTarget),
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
            .enabled_hit(point, self.region_cut_availability())
    }

    /// Whether the pointer sits over the reviewed rectangle — the area a press
    /// would start moving. Cursor feedback only; `begin_review_move` remains
    /// the authoritative test in image space.
    pub(in crate::backend::wayland) fn region_review_selection_contains(
        &self,
        point: (f64, f64),
    ) -> bool {
        if !self.input_state.region_state().is_review() {
            return false;
        }
        self.region_selection_geometry().is_some_and(|geometry| {
            let selection = geometry.display_selection();
            let left = selection.start.0.min(selection.end.0);
            let right = selection.start.0.max(selection.end.0);
            let top = selection.start.1.min(selection.end.1);
            let bottom = selection.start.1.max(selection.end.1);
            point.0 >= left && point.0 < right && point.1 >= top && point.1 < bottom
        })
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
        if !action.is_terminal() {
            return self.apply_region_review_edit(action);
        }
        if !self.region_cut_availability().allows(action) {
            return true;
        }
        let Some(rect) = self.region_review_rect() else {
            return false;
        };
        let cuts = self
            .region_capture
            .review_edits()
            .map(|edits| edits.cuts.clone())
            .unwrap_or_default();
        if !self.preview_permits_submit(&cuts) {
            return true;
        }
        let Ok(output) = band_cut_output_size((rect.width(), rect.height()), &cuts) else {
            self.input_state.push_toast(
                ToastPriority::Critical,
                TOAST_SOURCE,
                Toast::error("Could not apply the requested cut."),
            );
            return true;
        };
        let submit = match review_delivery_destination(action) {
            Some(destination) => RegionSubmit::Deliver(destination),
            None => match self.board_submit_for_composed(rect, output) {
                Ok(target) => RegionSubmit::Board(target),
                Err(BoardSubmitError::TooLarge) => {
                    self.input_state.push_toast(
                        ToastPriority::Info,
                        TOAST_SOURCE,
                        Toast::warning("Region is too large to add to the board."),
                    );
                    return true;
                }
                Err(BoardSubmitError::Unplaceable) => {
                    self.cancel_region_capture();
                    self.input_state.push_toast(
                        ToastPriority::Critical,
                        TOAST_SOURCE,
                        Toast::error("Could not place that region on the board."),
                    );
                    return true;
                }
            },
        };
        self.submit_region_capture_with(rect, cuts, submit);
        true
    }

    fn preview_permits_submit(&self, cuts: &[crate::capture::CutBand]) -> bool {
        cuts.is_empty()
            || self
                .region_capture
                .review_edits()
                .is_some_and(super::cut_review::RegionReviewEdits::preview_is_current)
    }

    fn board_submit_for_composed(
        &self,
        rect: ImagePixelRect,
        output: (u32, u32),
    ) -> Result<BoardPasteTarget, BoardSubmitError> {
        let limits = EmbeddedImageLimits::default();
        if !limits.allows_pixels(output.0, output.1) {
            return Err(BoardSubmitError::TooLarge);
        }
        let Some(ActiveScreenRegion::Ready { source, .. }) = self.region_capture.active() else {
            return Err(BoardSubmitError::Unplaceable);
        };
        let Some(source_world) =
            super::world_rect_for_image_rect_exact(rect, self.board_view_offset(), source)
        else {
            return Err(BoardSubmitError::Unplaceable);
        };
        let composed = if output == (rect.width(), rect.height()) {
            source_world
        } else {
            super::world_rect_for_composed_region(
                source_world,
                (rect.width(), rect.height()),
                output,
            )
            .ok_or(BoardSubmitError::Unplaceable)?
        };
        let world_bounds =
            super::board_bounds_for_world_rect(composed).ok_or(BoardSubmitError::Unplaceable)?;
        Ok(BoardPasteTarget {
            board_id: self.input_state.boards.active_board_id().to_string(),
            page_index: self.input_state.boards.active_page_index(),
            page_generation: self.input_state.boards.active_page_generation(),
            world_bounds,
        })
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
        if purpose == RegionPurposeTag::CaptureInteractive
            && self.input_state.region_state().is_review()
            && self.region_review_crop_locked()
        {
            return;
        }
        self.region_capture.clear_window_snap();
        self.retire_region_selection_owner(self.input_state.region_state().selection_owner());
        match purpose {
            RegionPurposeTag::CaptureInteractive => {
                self.enter_region_review(rect);
            }
            RegionPurposeTag::Ocr => self.submit_whole_image_ocr(rect),
            RegionPurposeTag::CaptureDeliver | RegionPurposeTag::Measure => {
                self.submit_region_capture(rect);
            }
        }
    }

    pub(in crate::backend::wayland) fn submit_region_capture(&mut self, rect: ImagePixelRect) {
        let destination = match self.capture.region_phase() {
            crate::backend::wayland::capture::RegionCapturePhase::Reserved(intent) => {
                intent.destination()
            }
            _ => return,
        };
        self.submit_region_capture_with(rect, Vec::new(), RegionSubmit::Deliver(destination));
    }

    fn submit_region_capture_with(
        &mut self,
        rect: ImagePixelRect,
        cuts: Vec<crate::capture::CutBand>,
        submit: RegionSubmit,
    ) {
        let Some(ActiveScreenRegion::Ready {
            freeze_ownership,
            purpose,
            include_drawings,
            ..
        }) = self.region_capture.active()
        else {
            self.cancel_region_capture_ui_and_lifecycle();
            return;
        };
        if !purpose.is_capture() {
            return;
        }
        let snapshot = match self.snapshot_region_render(rect, include_drawings) {
            Ok(snapshot) => snapshot,
            Err(crate::capture::CaptureError::Cancelled(_)) => {
                self.cancel_region_capture_for_source_change();
                return;
            }
            Err(error) => {
                self.cancel_region_capture();
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    TOAST_SOURCE,
                    Toast::error(error.to_string()),
                );
                return;
            }
        };
        if !cuts.is_empty() {
            let fingerprint_ok = self.region_capture.review_edits().is_some_and(|edits| {
                edits.ready_preview.as_ref().is_some_and(|preview| {
                    preview.key.fingerprint == snapshot.fingerprint && preview.key.cuts == cuts
                })
            });
            if !fingerprint_ok {
                if let Some(edits) = self.region_capture.review_edits_mut() {
                    edits.invalidate_base(snapshot.fingerprint);
                }
                self.schedule_region_cut_preview();
                self.mark_region_cut_ui_dirty();
                return;
            }
        }
        let request = RegionRenderRequest {
            source: snapshot.source,
            cuts,
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

        let render = region_render_job(request);
        match submit {
            RegionSubmit::Deliver(destination) => {
                self.capture.set_exit_on_success(should_exit_after_capture(
                    intent.exit_mode(),
                    destination,
                ));
                let request = region_delivery_request(render, &intent, destination);
                let submission = self
                    .capture
                    .manager_mut()
                    .request_rendered_image_delivery(request);
                self.accept_capture_submission(submission, ImageOperationKind::Screenshot);
            }
            RegionSubmit::Board(target) => {
                self.capture.set_exit_on_success(false);
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
}

enum BoardSubmitError {
    TooLarge,
    Unplaceable,
}
