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
    Pin,
}

pub(super) const fn include_drawings_for_submit(
    include_drawings: bool,
    submit: &RegionSubmit,
) -> bool {
    include_drawings && matches!(submit, RegionSubmit::Deliver(_) | RegionSubmit::Pin)
}

pub(super) fn exit_on_success_for_submit(
    intent: &RegionCaptureIntent,
    submit: &RegionSubmit,
) -> bool {
    match submit {
        RegionSubmit::Deliver(destination) => {
            should_exit_after_capture(intent.exit_mode(), *destination)
        }
        RegionSubmit::Board(_) | RegionSubmit::Pin => false,
    }
}

pub(super) const fn submit_mutates_board(submit: &RegionSubmit) -> bool {
    matches!(submit, RegionSubmit::Board(_))
}

pub(super) fn region_export_render_job(
    drawing_snapshot: Option<CanvasRegionExportSnapshot>,
    pixels: Option<crate::screen_pixels::PackedArgb32>,
) -> crate::capture::ImageRenderJob {
    match drawing_snapshot {
        Some(snapshot) => {
            Box::new(move || crate::canvas_export::render_canvas_region_png(snapshot))
        }
        None => {
            let pixels = pixels.expect("raw export prepared its checked crop");
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
        RegionAction::Board | RegionAction::Pin | RegionAction::ToggleIncludeDrawings => None,
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
        crate::ui::RegionActionBar::place(
            selection,
            (self.surface.width(), self.surface.height()),
            self.region_pin_eligible(),
        )
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
                self.region_pin_eligible(),
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
            None if action == RegionAction::Pin => {
                if !self.region_pin_eligible() {
                    self.input_state.push_toast(
                        ToastPriority::Critical,
                        "capture.region.pin",
                        Toast::error("Pin is unavailable for the active output."),
                    );
                    return true;
                }
                RegionSubmit::Pin
            }
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
            generation,
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
        let prepared_pin = if matches!(submit, RegionSubmit::Pin) {
            match self.prepare_pin_render(token, rect, generation) {
                Ok(prepared) => Some(prepared),
                Err(message) => {
                    // Pin prevalidation is part of the capture transaction,
                    // not a recoverable Review-bar validation hint. Retire the
                    // picker before reporting the one intended failure so an
                    // auto-freeze and the shared capture reservation cannot be
                    // left ownerless.
                    let cancelled = self.cancel_region_capture();
                    debug_assert!(
                        cancelled,
                        "pin preparation ran only after matching an active capture region"
                    );
                    self.input_state.push_toast(
                        ToastPriority::Critical,
                        "capture.region.pin",
                        Toast::error(message),
                    );
                    return;
                }
            }
        } else {
            None
        };
        // `prepare_pin_render` mutates only overlay correlation state. Reacquire
        // the immutable source handle before preparing raw/composed render data.
        let Some(source) = displayed_screen_image(
            &self.zoom,
            &self.frozen,
            self.input_state.board_is_transparent(),
        ) else {
            self.cancel_region_capture_for_source_change();
            return;
        };
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
        self.capture
            .set_exit_on_success(exit_on_success_for_submit(&intent, &submit));
        let (export_render, board_pixels) = if submit_mutates_board(&submit) {
            (None, pixels)
        } else {
            (
                Some(region_export_render_job(drawing_snapshot, pixels)),
                None,
            )
        };

        match submit {
            RegionSubmit::Deliver(destination) => {
                let render = export_render.expect("delivery prepared one shared export render");
                let request = region_delivery_request(render, &intent, destination);
                let submission = self
                    .capture
                    .manager_mut()
                    .request_rendered_image_delivery(request);
                self.accept_capture_submission(submission, ImageOperationKind::Screenshot);
            }
            RegionSubmit::Board(target) => {
                let pixels = board_pixels.expect("board submission always keeps its raw crop");
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
            RegionSubmit::Pin => {
                let prepared = prepared_pin.expect("pin submission prepared its correlation");
                let render = export_render.expect("pin prepared one shared export render");
                let submission =
                    self.capture
                        .manager_mut()
                        .request_render_image(RenderImageRequest {
                            render,
                            operation: ImageOperationKind::Pin,
                        });
                let accepted_id = submission.as_ref().ok().copied();
                if self.accept_capture_submission(submission, ImageOperationKind::Pin) {
                    let Some(accepted_id) = accepted_id else {
                        unreachable!("an accepted submission has an id")
                    };
                    if !self.capture.set_pending_pin_render(
                        crate::backend::wayland::capture::PendingPinRender {
                            accepted_id,
                            pin_request_id: prepared.pin_request_id,
                            output: prepared.output,
                            placement: prepared.placement,
                            picker_generation: prepared.picker_generation,
                        },
                    ) {
                        self.capture.manager_mut().mark_unhealthy();
                        self.capture.finish_capture_lifecycle();
                        self.input_state.push_toast(
                            ToastPriority::Critical,
                            "capture.region.pin",
                            Toast::error("Region was not pinned."),
                        );
                    }
                }
            }
        }
    }
}
