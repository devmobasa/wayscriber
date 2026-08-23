use crate::canvas_export::{CanvasExportRect, CanvasRegionExportSnapshot, CanvasRegionSource};
use crate::capture::{
    CaptureDestination, ImageFormatMetadata, ImageOperationKind, RenderImageRequest,
    RenderedImageDeliveryRequest,
};
use crate::input::state::{BoardPasteTarget, RegionPurposeTag, Toast, ToastPriority};
use crate::screen_pixels::{EmbeddedImageLimits, ImagePixelRect};
use crate::ui::RegionAction;

use super::super::capture::should_exit_after_capture;
use super::super::screen_image::{
    CropError, copy_image_rect, displayed_screen_image, screen_source_is,
    shared_displayed_screen_image,
};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RegionSubmit {
    Deliver(CaptureDestination),
    Board(BoardPasteTarget),
}

/// What a submission renders from. One or the other, never both and never
/// neither, so the render job cannot be handed an impossible pair.
#[derive(Debug)]
pub(super) enum RegionRenderSource {
    /// Committed board drawings composited over the crop, on the worker.
    Annotated(Box<CanvasRegionExportSnapshot>),
    /// The checked crop taken on the event loop.
    Raw(crate::screen_pixels::PackedArgb32),
}

/// The PNG job for a submission. Shared by every destination, so what Copy
/// writes and what Board pastes can only differ by the Review toggle.
pub(super) fn region_render_job(source: RegionRenderSource) -> crate::capture::ImageRenderJob {
    match source {
        RegionRenderSource::Annotated(snapshot) => {
            Box::new(move || crate::canvas_export::render_canvas_region_png(*snapshot))
        }
        RegionRenderSource::Raw(pixels) => {
            Box::new(move || crate::capture::png::encode_packed_argb32_png(&pixels))
        }
    }
}

pub(super) const fn review_delivery_destination(
    action: RegionAction,
) -> Option<CaptureDestination> {
    match action {
        RegionAction::Copy => Some(CaptureDestination::ClipboardOnly),
        RegionAction::Save => Some(CaptureDestination::FileOnly),
        RegionAction::Both => Some(CaptureDestination::ClipboardAndFile),
        RegionAction::Board | RegionAction::ToggleIncludeDrawings => None,
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
        if action == RegionAction::ToggleIncludeDrawings {
            return self.toggle_region_picker_include_drawings();
        }
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
                // Placement maps the authoritative image rectangle, not the
                // picker's outward-rounded display rectangle: composition uses
                // the same exact world rectangle, so the pasted image lands
                // where the annotations were drawn instead of stretched by a
                // quantization the chrome introduced.
                //
                // A missing source or a rectangle that will not map is a dead
                // end for this capture, not a no-op: falling through silently
                // would leave Review painted over a reservation nothing can
                // ever complete, so it retires through the usual funnel.
                let placement = match self.data.active_screen_region {
                    Some(ActiveScreenRegion::Ready { source, .. }) => {
                        super::world_rect_for_image_rect_exact(
                            rect,
                            self.board_view_offset(),
                            source,
                        )
                        .and_then(super::board_bounds_for_world_rect)
                    }
                    _ => None,
                };
                let Some(world_bounds) = placement else {
                    self.cancel_region_capture();
                    self.input_state.push_toast(
                        ToastPriority::Critical,
                        TOAST_SOURCE,
                        Toast::error("Could not place that region on the board."),
                    );
                    return true;
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
        self.clear_region_window_snap();
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
        self.submit_region_capture_with(rect, RegionSubmit::Deliver(destination));
    }

    fn submit_region_capture_with(&mut self, rect: ImagePixelRect, submit: RegionSubmit) {
        let Some(ActiveScreenRegion::Ready {
            source: token,
            freeze_ownership,
            purpose,
            include_drawings,
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
        // Every destination honours the Review toggle, Board included. Pasting a
        // composited crop onto the board it came from bakes a second, flattened
        // copy of those annotations over the live shapes; the toggle is the
        // control for that, so it is not overridden per destination here.
        //
        // The composed path validates and crops on the capture worker. The raw
        // path retains the existing checked event-loop crop, but never pays
        // that copy when the result would be discarded by composition.
        let raw_pixels = if include_drawings {
            None
        } else {
            match copy_image_rect(source.image, rect) {
                Ok(pixels) => Some(pixels),
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
            }
        };
        let drawing_snapshot = include_drawings
            .then(|| {
                let shared_image =
                    shared_displayed_screen_image(&self.zoom, &self.frozen, source.kind)?;
                let (origin_x, origin_y) = self.board_view_offset();
                super::world_rect_for_image_rect_exact(rect, (origin_x, origin_y), token)?;
                let logical_bounds = CanvasExportRect::new(
                    origin_x,
                    origin_y,
                    f64::from(token.surface.0),
                    f64::from(token.surface.1),
                )?;
                // This explicit option shares the full captured source so blur
                // at a crop edge can sample the same neighboring pixels as the
                // live canvas. The immutable image handle and frame snapshot
                // make the worker job independent of later edits.
                Some(CanvasRegionExportSnapshot {
                    source: CanvasRegionSource {
                        image: shared_image,
                        logical_bounds,
                    },
                    selection: rect,
                    frame: self
                        .input_state
                        .boards
                        .active_frame()
                        .clone_without_history(),
                    spotlight: crate::canvas_export::SpotlightPassSnapshot {
                        dim_opacity: self.input_state.spotlight_dim_opacity,
                        feather: self.input_state.spotlight_feather,
                    },
                })
            })
            .flatten();
        let render_source = match (drawing_snapshot, raw_pixels) {
            (Some(snapshot), _) => RegionRenderSource::Annotated(Box::new(snapshot)),
            (None, Some(pixels)) => RegionRenderSource::Raw(pixels),
            (None, None) => {
                // The composed path skips the event-loop crop, so a snapshot
                // that could not be built leaves nothing at all to render.
                self.cancel_region_capture();
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    TOAST_SOURCE,
                    Toast::error("Could not map the selected drawings into the captured region."),
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

        let render = region_render_job(render_source);
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
