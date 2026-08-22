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

pub(super) const fn include_drawings_for_submit(
    include_drawings: bool,
    submit: &RegionSubmit,
) -> bool {
    include_drawings && matches!(submit, RegionSubmit::Deliver(_))
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
        self.clear_region_window_snap();
        self.retire_region_selection_owner(self.input_state.region_state().selection_owner());
        if purpose == RegionPurposeTag::CaptureInteractive {
            self.enter_region_review(rect);
        } else {
            self.submit_region_capture(rect);
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
        let include_drawings = include_drawings_for_submit(include_drawings, &submit);
        // The composed path validates and crops on the capture worker. The raw
        // path retains the existing checked event-loop crop, but never pays
        // that copy when the result would be discarded by composition.
        let pixels = if include_drawings {
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
        if include_drawings && drawing_snapshot.is_none() {
            self.cancel_region_capture();
            self.input_state.push_toast(
                ToastPriority::Critical,
                TOAST_SOURCE,
                Toast::error("Could not map the selected drawings into the captured region."),
            );
            return;
        }

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
                let render: crate::capture::ImageRenderJob = match drawing_snapshot {
                    Some(snapshot) => {
                        Box::new(move || crate::canvas_export::render_canvas_region_png(snapshot))
                    }
                    None => {
                        let pixels = pixels.expect("raw delivery prepared its checked crop");
                        Box::new(move || crate::capture::png::encode_packed_argb32_png(&pixels))
                    }
                };
                let request = region_delivery_request(render, &intent, destination);
                let submission = self
                    .capture
                    .manager_mut()
                    .request_rendered_image_delivery(request);
                self.accept_capture_submission(submission, ImageOperationKind::Screenshot);
            }
            RegionSubmit::Board(target) => {
                self.capture.set_exit_on_success(false);
                let pixels = pixels.expect("board submission always uses the raw checked crop");
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
}
