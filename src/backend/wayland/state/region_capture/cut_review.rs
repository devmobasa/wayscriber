use crate::capture::{CutAxis, CutBand, output_size};
use crate::input::InputState;
use crate::input::state::{RegionInputSource, RegionSelection};
use crate::screen_pixels::{ImagePixelRect, ImagePoint, pixel_span};
use crate::util::Rect;

use super::super::screen_image::{ScreenSourceToken, screen_rect_for_native_extent};
use super::*;
use crate::ui::{RegionActionAvailability, RegionCutStatus};

pub(super) const CUT_DRAG_THRESHOLD_PX: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreviewApply {
    Ignored,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum CutMode {
    Idle,
    Armed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::backend::wayland) struct CutDrag {
    pub(super) owner: RegionInputSource,
    pub(super) start: (f64, f64),
    pub(super) current: (f64, f64),
    pub(super) axis: Option<CutAxis>,
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::backend::wayland) struct RegionReviewCorrelation {
    pub(super) generation: u64,
    pub(super) source: ScreenSourceToken,
}

/// Board and overlay facts that distinguish two annotated region renders.
/// Fingerprint and snapshot construction share one of these so halo, Spotlight,
/// and board identity cannot describe different frames.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RegionAnnotatedRenderContext {
    pub(super) board_id: String,
    pub(super) page_index: usize,
    pub(super) page_generation: u64,
    pub(super) canvas_content_generation: u64,
    pub(super) board_view_offset: (f64, f64),
    pub(super) text_halo_enabled: bool,
    pub(super) spotlight: crate::canvas_export::SpotlightPassSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::backend::wayland) enum RegionRenderFingerprint {
    Raw {
        correlation: RegionReviewCorrelation,
        source_rect: ImagePixelRect,
    },
    Annotated {
        correlation: RegionReviewCorrelation,
        source_rect: ImagePixelRect,
        context: RegionAnnotatedRenderContext,
    },
}

impl RegionRenderFingerprint {
    pub(super) fn correlation(&self) -> &RegionReviewCorrelation {
        match self {
            Self::Raw { correlation, .. } | Self::Annotated { correlation, .. } => correlation,
        }
    }

    pub(super) fn source_rect(&self) -> ImagePixelRect {
        match self {
            Self::Raw { source_rect, .. } | Self::Annotated { source_rect, .. } => *source_rect,
        }
    }

    pub(super) fn include_drawings(&self) -> bool {
        matches!(self, Self::Annotated { .. })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::backend::wayland) struct CutPreviewKey {
    pub(super) fingerprint: RegionRenderFingerprint,
    pub(super) revision: u64,
    pub(super) cuts: Vec<CutBand>,
}

#[derive(Debug, Clone)]
pub(in crate::backend::wayland) struct RegionCutPreview {
    pub(super) key: CutPreviewKey,
    pub(super) pixels: std::sync::Arc<crate::screen_pixels::PackedArgb32>,
    pub(super) display: RegionSelection,
}

#[derive(Debug, Clone)]
pub(in crate::backend::wayland) struct RegionCutBase {
    pub(super) fingerprint: RegionRenderFingerprint,
    pub(super) pixels: std::sync::Arc<crate::screen_pixels::PackedArgb32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CutCommit {
    None,
    Applied,
    RejectedFullAxis,
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct RegionReviewEdits {
    pub(super) correlation: RegionReviewCorrelation,
    pub(super) source_rect: ImagePixelRect,
    pub(super) mode: CutMode,
    pub(super) drag: Option<CutDrag>,
    pub(super) cuts: Vec<CutBand>,
    pub(super) redo: Vec<CutBand>,
    pub(super) revision: u64,
    pub(super) desired_preview: Option<CutPreviewKey>,
    pub(super) ready_preview: Option<RegionCutPreview>,
    pub(super) base_cache: Option<RegionCutBase>,
    pub(super) failed_revision: Option<u64>,
}

pub(super) fn review_edits_for_active_region(
    region: Option<ActiveScreenRegion>,
    rect: ImagePixelRect,
) -> Option<RegionReviewEdits> {
    let Some(ActiveScreenRegion::Ready {
        purpose: crate::input::state::RegionPurposeTag::CaptureInteractive,
        generation,
        source,
        ..
    }) = region
    else {
        return None;
    };
    Some(RegionReviewEdits::new(
        RegionReviewCorrelation { generation, source },
        rect,
    ))
}

impl RegionReviewEdits {
    pub(super) fn new(correlation: RegionReviewCorrelation, source_rect: ImagePixelRect) -> Self {
        Self {
            correlation,
            source_rect,
            mode: CutMode::Idle,
            drag: None,
            cuts: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            desired_preview: None,
            ready_preview: None,
            base_cache: None,
            failed_revision: None,
        }
    }

    pub(super) fn crop_locked(&self) -> bool {
        !self.cuts.is_empty()
    }

    pub(super) fn loupe_suppressed(&self) -> bool {
        self.mode == CutMode::Armed || self.crop_locked()
    }

    pub(super) fn preview_is_current(&self) -> bool {
        match &self.desired_preview {
            None => self.cuts.is_empty(),
            Some(desired) => self
                .ready_preview
                .as_ref()
                .is_some_and(|ready| ready.key == *desired),
        }
    }

    pub(super) fn can_start_cut_drag(&self) -> bool {
        self.mode == CutMode::Armed && self.preview_is_current() && self.drag.is_none()
    }

    /// A current failed preview must stay failed until undo, redo, or reset
    /// changes the revision. Resubmitting the same revision churns workers.
    pub(super) fn current_preview_failed(&self) -> bool {
        self.desired_preview.as_ref().is_some_and(|desired| {
            self.failed_revision
                .is_some_and(|revision| revision == desired.revision)
        })
    }

    pub(super) fn status(&self) -> Option<RegionCutStatus> {
        if self.cuts.is_empty() {
            return None;
        }
        if self
            .failed_revision
            .is_some_and(|revision| revision == self.revision)
        {
            return Some(RegionCutStatus::Failed);
        }
        if !self.preview_is_current() {
            return Some(RegionCutStatus::Updating);
        }
        None
    }

    pub(super) fn availability(&self) -> RegionActionAvailability {
        let preview_ok = self.preview_is_current();
        RegionActionAvailability {
            terminal: preview_ok,
            cut: true,
            undo: !self.cuts.is_empty(),
            redo: !self.redo.is_empty(),
            reset: self.crop_locked() || !self.redo.is_empty(),
        }
    }

    /// Toggle Cut arming. Returns the abandoned drag owner, if any, so the
    /// caller can retire the matching `InputState` review-move owner.
    pub(super) fn toggle_mode(&mut self) -> Option<RegionInputSource> {
        let abandoned = if self.mode == CutMode::Armed {
            self.drag.take().map(|drag| drag.owner)
        } else {
            None
        };
        self.mode = match self.mode {
            CutMode::Idle => CutMode::Armed,
            CutMode::Armed => CutMode::Idle,
        };
        abandoned
    }

    pub(super) fn disarm_mode(&mut self) -> bool {
        if self.drag.is_some() {
            self.drag = None;
            return true;
        }
        if self.mode == CutMode::Armed {
            self.mode = CutMode::Idle;
            return true;
        }
        false
    }

    fn bump_revision(&mut self) -> bool {
        match self.revision.checked_add(1) {
            Some(next) => {
                self.revision = next;
                true
            }
            None => false,
        }
    }

    pub(super) fn set_desired_from(&mut self, fingerprint: RegionRenderFingerprint) {
        if self.cuts.is_empty() {
            self.desired_preview = None;
            self.ready_preview = None;
            self.base_cache = None;
            self.failed_revision = None;
            return;
        }
        self.desired_preview = Some(CutPreviewKey {
            fingerprint,
            revision: self.revision,
            cuts: self.cuts.clone(),
        });
    }

    pub(super) fn output_size(&self) -> Option<(u32, u32)> {
        output_size(
            (self.source_rect.width(), self.source_rect.height()),
            &self.cuts,
        )
        .ok()
    }

    pub(super) fn displayed_output_size(&self) -> Option<(u32, u32)> {
        self.ready_preview
            .as_ref()
            .map(|preview| (preview.pixels.width(), preview.pixels.height()))
            .or_else(|| {
                if self.cuts.is_empty() {
                    Some((self.source_rect.width(), self.source_rect.height()))
                } else {
                    None
                }
            })
    }

    pub(super) fn begin_drag(&mut self, owner: RegionInputSource, point: (f64, f64)) -> bool {
        if !self.can_start_cut_drag() {
            return false;
        }
        self.drag = Some(CutDrag {
            owner,
            start: point,
            current: point,
            axis: None,
        });
        true
    }

    pub(super) fn update_drag(&mut self, owner: RegionInputSource, point: (f64, f64)) -> bool {
        let Some(drag) = self.drag.as_mut() else {
            return false;
        };
        if drag.owner != owner {
            return false;
        }
        drag.current = point;
        if drag.axis.is_none() {
            let dx = point.0 - drag.start.0;
            let dy = point.1 - drag.start.1;
            if dx.hypot(dy) >= CUT_DRAG_THRESHOLD_PX {
                drag.axis = Some(dominant_cut_axis(dx, dy));
            }
        }
        true
    }

    pub(super) fn finish_drag(
        &mut self,
        owner: RegionInputSource,
        point: (f64, f64),
        display: RegionSelection,
        fingerprint: RegionRenderFingerprint,
    ) -> CutCommit {
        if !self.drag.as_ref().is_some_and(|drag| drag.owner == owner) {
            return CutCommit::None;
        }
        let _ = self.update_drag(owner, point);
        let Some(drag) = self.drag.take() else {
            return CutCommit::None;
        };
        let Some(axis) = drag.axis else {
            return CutCommit::None;
        };
        let Some(output) = self.output_size() else {
            return CutCommit::None;
        };
        let Some(band) = quantized_cut(axis, display, output, drag.start, drag.current) else {
            return CutCommit::None;
        };
        let mut next_cuts = self.cuts.clone();
        next_cuts.push(band);
        if output_size(
            (self.source_rect.width(), self.source_rect.height()),
            &next_cuts,
        )
        .is_err()
        {
            return CutCommit::RejectedFullAxis;
        }
        if !self.bump_revision() {
            return CutCommit::None;
        }
        self.cuts.push(band);
        self.redo.clear();
        self.failed_revision = None;
        self.set_desired_from(fingerprint);
        CutCommit::Applied
    }

    pub(super) fn undo(&mut self, fingerprint: RegionRenderFingerprint) -> bool {
        let Some(cut) = self.cuts.pop() else {
            return false;
        };
        if !self.bump_revision() {
            self.cuts.push(cut);
            return false;
        }
        self.redo.push(cut);
        self.drag = None;
        self.failed_revision = None;
        self.set_desired_from(fingerprint);
        true
    }

    pub(super) fn redo(&mut self, fingerprint: RegionRenderFingerprint) -> bool {
        let Some(cut) = self.redo.pop() else {
            return false;
        };
        if !self.bump_revision() {
            self.redo.push(cut);
            return false;
        }
        self.cuts.push(cut);
        self.drag = None;
        self.failed_revision = None;
        self.set_desired_from(fingerprint);
        true
    }

    pub(super) fn reset(&mut self) -> bool {
        if self.cuts.is_empty() && self.redo.is_empty() && self.drag.is_none() {
            return false;
        }
        if !self.bump_revision() {
            return false;
        }
        self.drag = None;
        self.cuts.clear();
        self.redo.clear();
        self.failed_revision = None;
        self.set_desired_from(RegionRenderFingerprint::Raw {
            correlation: self.correlation.clone(),
            source_rect: self.source_rect,
        });
        true
    }

    pub(super) fn set_source_rect(&mut self, source_rect: ImagePixelRect) -> bool {
        if self.crop_locked() || self.source_rect == source_rect {
            return false;
        }
        self.source_rect = source_rect;
        self.redo.clear();
        self.ready_preview = None;
        self.base_cache = None;
        self.desired_preview = None;
        self.failed_revision = None;
        true
    }

    pub(super) fn invalidate_base(&mut self, fingerprint: RegionRenderFingerprint) {
        if !self.cuts.is_empty() && !self.bump_revision() {
            return;
        }
        self.base_cache = None;
        self.ready_preview = None;
        self.failed_revision = None;
        self.set_desired_from(fingerprint);
    }

    pub(super) fn mark_preview_failed(&mut self, key: &CutPreviewKey) -> bool {
        if self.desired_preview.as_ref() != Some(key) {
            return false;
        }
        self.failed_revision = Some(key.revision);
        true
    }
}

pub(super) fn dominant_cut_axis(dx: f64, dy: f64) -> CutAxis {
    if dy.abs() > dx.abs() {
        CutAxis::Rows
    } else {
        CutAxis::Columns
    }
}

pub(super) fn output_display_for(
    token: &ScreenSourceToken,
    source_rect: ImagePixelRect,
    cuts: &[CutBand],
) -> Option<RegionSelection> {
    let size = output_size((source_rect.width(), source_rect.height()), cuts).ok()?;
    native_extent_display(token, source_rect, size)
}

pub(super) fn native_extent_display(
    token: &ScreenSourceToken,
    source_rect: ImagePixelRect,
    size: (u32, u32),
) -> Option<RegionSelection> {
    let rect = screen_rect_for_native_extent(token, (source_rect.x(), source_rect.y()), size)?;
    Some(region_selection_from_rect(rect))
}

pub(super) fn region_selection_from_rect(rect: Rect) -> RegionSelection {
    RegionSelection {
        start: (f64::from(rect.x), f64::from(rect.y)),
        end: (
            f64::from(rect.x.saturating_add(rect.width)),
            f64::from(rect.y.saturating_add(rect.height)),
        ),
    }
}

pub(super) fn display_contains(display: RegionSelection, point: (f64, f64)) -> bool {
    let left = display.start.0.min(display.end.0);
    let right = display.start.0.max(display.end.0);
    let top = display.start.1.min(display.end.1);
    let bottom = display.start.1.max(display.end.1);
    point.0 >= left && point.0 < right && point.1 >= top && point.1 < bottom
}

pub(super) fn logical_to_output_point(
    display: RegionSelection,
    output_size: (u32, u32),
    point: (f64, f64),
) -> Option<ImagePoint> {
    if output_size.0 == 0 || output_size.1 == 0 || !point.0.is_finite() || !point.1.is_finite() {
        return None;
    }
    let left = display.start.0.min(display.end.0);
    let right = display.start.0.max(display.end.0);
    let top = display.start.1.min(display.end.1);
    let bottom = display.start.1.max(display.end.1);
    let width = right - left;
    let height = bottom - top;
    if width <= 0.0 || height <= 0.0 || !width.is_finite() || !height.is_finite() {
        return None;
    }
    let x = ((point.0 - left) / width) * f64::from(output_size.0);
    let y = ((point.1 - top) / height) * f64::from(output_size.1);
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some(ImagePoint::new(
        x.clamp(0.0, f64::from(output_size.0)),
        y.clamp(0.0, f64::from(output_size.1)),
    ))
}

fn quantized_cut(
    axis: CutAxis,
    display: RegionSelection,
    output_size: (u32, u32),
    start: (f64, f64),
    current: (f64, f64),
) -> Option<CutBand> {
    let first = logical_to_output_point(display, output_size, start)?;
    let second = logical_to_output_point(display, output_size, current)?;
    let span = pixel_span(first, second, output_size)?;
    match axis {
        CutAxis::Columns => {
            CutBand::from_unordered_edges(axis, span.x(), span.x().checked_add(span.width())?).ok()
        }
        CutAxis::Rows => {
            CutBand::from_unordered_edges(axis, span.y(), span.y().checked_add(span.height())?).ok()
        }
    }
}

pub(super) fn cut_band_display(
    display: RegionSelection,
    output_size: (u32, u32),
    axis: CutAxis,
    start: u32,
    end: u32,
) -> Option<RegionSelection> {
    if end <= start || output_size.0 == 0 || output_size.1 == 0 {
        return None;
    }
    let left = display.start.0.min(display.end.0);
    let right = display.start.0.max(display.end.0);
    let top = display.start.1.min(display.end.1);
    let bottom = display.start.1.max(display.end.1);
    let width = right - left;
    let height = bottom - top;
    match axis {
        CutAxis::Columns => {
            let x0 = left + f64::from(start) * width / f64::from(output_size.0);
            let x1 = left + f64::from(end) * width / f64::from(output_size.0);
            Some(RegionSelection {
                start: (x0, top),
                end: (x1, bottom),
            })
        }
        CutAxis::Rows => {
            let y0 = top + f64::from(start) * height / f64::from(output_size.1);
            let y1 = top + f64::from(end) * height / f64::from(output_size.1);
            Some(RegionSelection {
                start: (left, y0),
                end: (right, y1),
            })
        }
    }
}

fn retire_cut_drag_owner(input: &mut InputState, owner: Option<RegionInputSource>) {
    if let Some(owner) = owner {
        let _ = input.finish_region_review_move(owner);
    }
}

fn apply_cut_history_change(
    edits: &mut Option<RegionReviewEdits>,
    input: &mut InputState,
    change: impl FnOnce(&mut RegionReviewEdits) -> bool,
) -> bool {
    let owner = edits
        .as_ref()
        .and_then(|edits| edits.drag.map(|drag| drag.owner));
    let Some(edits) = edits.as_mut() else {
        return false;
    };
    if !change(edits) {
        return false;
    }
    retire_cut_drag_owner(input, owner);
    true
}

impl WaylandState {
    pub(super) fn region_review_edits(&self) -> Option<&RegionReviewEdits> {
        self.data.region_review_edits.as_ref()
    }

    pub(super) fn region_review_edits_mut(&mut self) -> Option<&mut RegionReviewEdits> {
        self.data.region_review_edits.as_mut()
    }

    pub(in crate::backend::wayland) fn region_review_crop_locked(&self) -> bool {
        self.region_review_edits()
            .is_some_and(RegionReviewEdits::crop_locked)
    }

    pub(in crate::backend::wayland) fn region_review_loupe_suppressed(&self) -> bool {
        self.region_review_edits()
            .is_some_and(RegionReviewEdits::loupe_suppressed)
    }

    pub(in crate::backend::wayland) fn region_cut_displayed_selection(
        &self,
    ) -> Option<RegionSelection> {
        let edits = self.region_review_edits()?;
        if let Some(preview) = &edits.ready_preview {
            return Some(preview.display);
        }
        let token = self.region_picker_source_token()?;
        output_display_for(&token, edits.source_rect, &[])
    }

    pub(in crate::backend::wayland) fn region_cut_availability(&self) -> RegionActionAvailability {
        self.region_review_edits()
            .map(RegionReviewEdits::availability)
            .unwrap_or_default()
    }

    pub(in crate::backend::wayland) fn region_cut_status(&self) -> Option<RegionCutStatus> {
        self.region_review_edits()
            .and_then(RegionReviewEdits::status)
    }

    pub(in crate::backend::wayland) fn region_cut_mode_armed(&self) -> bool {
        self.region_review_edits()
            .is_some_and(|edits| edits.mode == CutMode::Armed)
    }

    pub(super) fn create_region_review_edits(&mut self, rect: ImagePixelRect) {
        self.data.region_review_edits =
            review_edits_for_active_region(self.data.active_screen_region, rect);
    }

    pub(super) fn mark_region_cut_ui_dirty(&mut self) {
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland) fn apply_region_review_edit(
        &mut self,
        action: crate::ui::RegionAction,
    ) -> bool {
        match action {
            crate::ui::RegionAction::CutBand => self.toggle_region_cut_mode(),
            crate::ui::RegionAction::UndoCut => self.undo_region_cut(),
            crate::ui::RegionAction::RedoCut => self.redo_region_cut(),
            crate::ui::RegionAction::ResetCuts => self.reset_region_cuts(),
            crate::ui::RegionAction::ToggleIncludeDrawings => {
                self.toggle_region_picker_include_drawings()
            }
            crate::ui::RegionAction::Copy
            | crate::ui::RegionAction::Save
            | crate::ui::RegionAction::Both
            | crate::ui::RegionAction::Board => false,
        }
    }

    fn retire_region_cut_drag_owner(&mut self, owner: Option<RegionInputSource>) {
        retire_cut_drag_owner(&mut self.input_state, owner);
    }

    fn toggle_region_cut_mode(&mut self) -> bool {
        let Some(edits) = self.region_review_edits_mut() else {
            return false;
        };
        let owner = edits.toggle_mode();
        self.retire_region_cut_drag_owner(owner);
        self.mark_region_cut_ui_dirty();
        true
    }

    fn undo_region_cut(&mut self) -> bool {
        let Some(fingerprint) = self.current_region_fingerprint() else {
            return false;
        };
        if !apply_cut_history_change(
            &mut self.data.region_review_edits,
            &mut self.input_state,
            |edits| edits.undo(fingerprint),
        ) {
            return false;
        }
        self.mark_region_cut_ui_dirty();
        self.schedule_region_cut_preview();
        true
    }

    fn redo_region_cut(&mut self) -> bool {
        let Some(fingerprint) = self.current_region_fingerprint() else {
            return false;
        };
        if !apply_cut_history_change(
            &mut self.data.region_review_edits,
            &mut self.input_state,
            |edits| edits.redo(fingerprint),
        ) {
            return false;
        }
        self.mark_region_cut_ui_dirty();
        self.schedule_region_cut_preview();
        true
    }

    fn reset_region_cuts(&mut self) -> bool {
        if !apply_cut_history_change(
            &mut self.data.region_review_edits,
            &mut self.input_state,
            RegionReviewEdits::reset,
        ) {
            return false;
        }
        self.mark_region_cut_ui_dirty();
        true
    }

    pub(in crate::backend::wayland) fn try_begin_region_cut_drag(
        &mut self,
        owner: RegionInputSource,
        point: (f64, f64),
    ) -> bool {
        let Some(display) = self.region_cut_displayed_selection() else {
            return false;
        };
        if !display_contains(display, point) {
            return false;
        }
        let Some(edits) = self.region_review_edits_mut() else {
            return false;
        };
        if !edits.begin_drag(owner, point) {
            return false;
        }
        if !self.input_state.begin_region_review_move(owner) {
            if let Some(edits) = self.region_review_edits_mut() {
                edits.drag = None;
            }
            return false;
        }
        self.mark_region_cut_ui_dirty();
        true
    }

    pub(in crate::backend::wayland) fn update_region_cut_drag(
        &mut self,
        owner: RegionInputSource,
        point: (f64, f64),
    ) -> bool {
        let Some(edits) = self.region_review_edits_mut() else {
            return false;
        };
        if !edits.update_drag(owner, point) {
            return false;
        }
        self.mark_region_cut_ui_dirty();
        true
    }

    pub(in crate::backend::wayland) fn finish_region_cut_drag(
        &mut self,
        owner: RegionInputSource,
        point: (f64, f64),
    ) -> bool {
        if !self
            .region_review_edits()
            .and_then(|edits| edits.drag)
            .is_some_and(|drag| drag.owner == owner)
        {
            return false;
        }
        let Some(display) = self.region_cut_displayed_selection() else {
            self.abandon_region_cut_drag(owner);
            return true;
        };
        let Some(fingerprint) = self.current_region_fingerprint() else {
            self.abandon_region_cut_drag(owner);
            return true;
        };
        let Some(edits) = self.region_review_edits_mut() else {
            return false;
        };
        let commit = edits.finish_drag(owner, point, display, fingerprint);
        let _ = self.input_state.finish_region_review_move(owner);
        match commit {
            CutCommit::Applied => {
                self.mark_region_cut_ui_dirty();
                self.schedule_region_cut_preview();
            }
            CutCommit::RejectedFullAxis => {
                self.input_state.push_toast(
                    crate::input::state::ToastPriority::Info,
                    "capture",
                    crate::input::state::Toast::warning(
                        "That cut would remove the entire remaining image.",
                    ),
                );
                self.mark_region_cut_ui_dirty();
            }
            CutCommit::None => self.mark_region_cut_ui_dirty(),
        }
        true
    }

    pub(in crate::backend::wayland) fn abandon_region_cut_drag(
        &mut self,
        owner: RegionInputSource,
    ) -> bool {
        let Some(edits) = self.region_review_edits_mut() else {
            return false;
        };
        let Some(drag) = edits.drag else {
            return false;
        };
        if drag.owner != owner {
            return false;
        }
        edits.drag = None;
        let _ = self.input_state.finish_region_review_move(owner);
        self.mark_region_cut_ui_dirty();
        true
    }

    pub(in crate::backend::wayland) fn handle_region_cut_escape(&mut self) -> bool {
        let owner = self
            .region_review_edits()
            .and_then(|edits| edits.drag.map(|drag| drag.owner));
        let Some(edits) = self.region_review_edits_mut() else {
            return false;
        };
        if !edits.disarm_mode() {
            return false;
        }
        if let Some(owner) = owner {
            let _ = self.input_state.finish_region_review_move(owner);
        }
        self.mark_region_cut_ui_dirty();
        true
    }

    pub(super) fn sync_region_review_source_rect(&mut self) {
        let Some(rect) = self.region_review_rect() else {
            return;
        };
        let Some(edits) = self.region_review_edits_mut() else {
            return;
        };
        if edits.set_source_rect(rect) {
            self.mark_region_cut_ui_dirty();
        }
    }

    pub(in crate::backend::wayland) fn region_cut_preview_pixels(
        &self,
    ) -> Option<&crate::screen_pixels::PackedArgb32> {
        self.region_review_edits()
            .and_then(|edits| edits.ready_preview.as_ref())
            .map(|preview| preview.pixels.as_ref())
    }

    pub(in crate::backend::wayland) fn region_cut_drag_overlay(
        &self,
    ) -> Option<(CutAxis, RegionSelection)> {
        let edits = self.region_review_edits()?;
        let drag = edits.drag?;
        let axis = drag.axis?;
        let display = self.region_cut_displayed_selection()?;
        let output = edits.output_size()?;
        let band = quantized_cut(axis, display, output, drag.start, drag.current)?;
        debug_assert_eq!(band.axis(), axis);
        cut_band_display(display, output, axis, band.start(), band.end()).map(|band| (axis, band))
    }

    pub(in crate::backend::wayland) fn consume_region_review_press(
        &mut self,
        owner: RegionInputSource,
        point: (f64, f64),
    ) -> RegionReviewPress {
        if !self.input_state.region_state().is_review() {
            return RegionReviewPress::NotReview;
        }
        if self.region_review_bar_contains(point) {
            let suppress_release = if let Some(action) = self.region_review_action_at(point) {
                let terminal = action.is_terminal();
                self.submit_region_review_action(action);
                terminal
            } else {
                false
            };
            return RegionReviewPress::Consumed { suppress_release };
        }
        if self.try_begin_region_cut_drag(owner, point) {
            return RegionReviewPress::Consumed {
                suppress_release: false,
            };
        }
        if self.region_review_crop_locked() || self.region_cut_mode_armed() {
            return RegionReviewPress::Consumed {
                suppress_release: false,
            };
        }
        RegionReviewPress::Fallthrough
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum RegionReviewPress {
    NotReview,
    Consumed { suppress_release: bool },
    Fallthrough,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::state::screen_image::ScreenImageKind;
    use crate::capture::CutAxis;
    use wayland_client::protocol::wl_output::Transform;

    fn token() -> ScreenSourceToken {
        ScreenSourceToken {
            output_id: 1,
            output_layout_generation: 1,
            kind: ScreenImageKind::Frozen,
            image_generation: 1,
            image_size: (8, 8),
            stride: 32,
            surface: (8, 8),
            output_scale: 1,
            output_transform: Transform::Normal,
            zoom_transformed: false,
            zoom_scale: 1.0,
            zoom_view_offset: (0.0, 0.0),
        }
    }

    fn fingerprint(rect: ImagePixelRect) -> RegionRenderFingerprint {
        RegionRenderFingerprint::Raw {
            correlation: RegionReviewCorrelation {
                generation: 1,
                source: token(),
            },
            source_rect: rect,
        }
    }

    fn edits() -> RegionReviewEdits {
        let rect = ImagePixelRect::new(0, 0, 8, 8, (8, 8)).unwrap();
        RegionReviewEdits::new(
            RegionReviewCorrelation {
                generation: 1,
                source: token(),
            },
            rect,
        )
    }

    fn display() -> RegionSelection {
        RegionSelection {
            start: (0.0, 0.0),
            end: (8.0, 8.0),
        }
    }

    #[test]
    fn arming_does_not_change_history() {
        let mut edits = edits();
        edits.toggle_mode();
        assert_eq!(edits.mode, CutMode::Armed);
        assert!(edits.cuts.is_empty());
        edits.toggle_mode();
        assert_eq!(edits.mode, CutMode::Idle);
        assert!(edits.cuts.is_empty());
    }

    #[test]
    fn sub_threshold_drag_commits_nothing() {
        let mut edits = edits();
        edits.toggle_mode();
        assert!(edits.begin_drag(RegionInputSource::Pointer, (1.0, 1.0)));
        assert!(edits.update_drag(RegionInputSource::Pointer, (3.0, 1.0)));
        assert_eq!(
            edits.finish_drag(
                RegionInputSource::Pointer,
                (3.0, 1.0),
                display(),
                fingerprint(edits.source_rect)
            ),
            CutCommit::None
        );
        assert!(edits.cuts.is_empty());
    }

    #[test]
    fn axis_locks_once_past_the_threshold() {
        let mut edits = edits();
        edits.toggle_mode();
        assert!(edits.begin_drag(RegionInputSource::Pointer, (0.0, 0.0)));
        assert!(edits.update_drag(RegionInputSource::Pointer, (6.0, 1.0)));
        assert_eq!(edits.drag.unwrap().axis, Some(CutAxis::Columns));
        assert!(edits.update_drag(RegionInputSource::Pointer, (6.0, 20.0)));
        assert_eq!(edits.drag.unwrap().axis, Some(CutAxis::Columns));
    }

    #[test]
    fn wrong_owner_cannot_update_or_finish_a_drag() {
        let mut edits = edits();
        edits.toggle_mode();
        assert!(edits.begin_drag(RegionInputSource::Pointer, (0.0, 0.0)));
        assert!(!edits.update_drag(RegionInputSource::Touch, (6.0, 0.0)));
        assert_eq!(
            edits.finish_drag(
                RegionInputSource::Touch,
                (6.0, 0.0),
                display(),
                fingerprint(edits.source_rect)
            ),
            CutCommit::None
        );
        assert!(edits.drag.is_some());
    }

    #[test]
    fn valid_commit_appends_clears_redo_and_increments_revision() {
        let mut edits = edits();
        edits.toggle_mode();
        assert!(edits.begin_drag(RegionInputSource::Pointer, (2.0, 0.0)));
        assert!(edits.update_drag(RegionInputSource::Pointer, (7.0, 0.0)));
        assert_eq!(
            edits.finish_drag(
                RegionInputSource::Pointer,
                (7.0, 0.0),
                display(),
                fingerprint(edits.source_rect)
            ),
            CutCommit::Applied
        );
        assert_eq!(edits.cuts.len(), 1);
        assert!(edits.redo.is_empty());
        assert_eq!(edits.revision, 1);
        assert!(!edits.preview_is_current());
    }

    #[test]
    fn full_axis_commit_is_rejected_without_a_revision_change() {
        let mut edits = edits();
        edits.toggle_mode();
        assert!(edits.begin_drag(RegionInputSource::Pointer, (0.0, 0.0)));
        assert!(edits.update_drag(RegionInputSource::Pointer, (8.0, 0.0)));
        assert_eq!(
            edits.finish_drag(
                RegionInputSource::Pointer,
                (8.0, 0.0),
                display(),
                fingerprint(edits.source_rect)
            ),
            CutCommit::RejectedFullAxis
        );
        assert!(edits.cuts.is_empty());
        assert_eq!(edits.revision, 0);
    }

    #[test]
    fn undo_redo_and_new_commit_clear_redo() {
        let mut edits = edits();
        let fingerprint = fingerprint(edits.source_rect);
        edits
            .cuts
            .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
        edits.revision = 1;
        assert!(edits.undo(fingerprint.clone()));
        assert!(edits.cuts.is_empty());
        assert_eq!(edits.redo.len(), 1);
        assert!(edits.redo(fingerprint.clone()));
        assert_eq!(edits.cuts.len(), 1);
        assert!(edits.undo(fingerprint.clone()));
        edits.toggle_mode();
        assert!(edits.begin_drag(RegionInputSource::Pointer, (2.0, 0.0)));
        assert!(edits.update_drag(RegionInputSource::Pointer, (7.0, 0.0)));
        assert_eq!(
            edits.finish_drag(
                RegionInputSource::Pointer,
                (7.0, 0.0),
                display(),
                fingerprint
            ),
            CutCommit::Applied
        );
        assert!(edits.redo.is_empty());
    }

    #[test]
    fn undo_and_redo_abandon_an_in_flight_drag() {
        let mut edits = edits();
        let fingerprint = fingerprint(edits.source_rect);
        edits
            .cuts
            .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
        edits.revision = 1;
        edits.set_desired_from(fingerprint.clone());
        let desired = edits.desired_preview.clone().unwrap();
        edits.ready_preview = Some(RegionCutPreview {
            key: desired,
            pixels: std::sync::Arc::new(
                crate::screen_pixels::PackedArgb32::new(7, 8, 28, vec![0; 28 * 8]).unwrap(),
            ),
            display: display(),
        });
        edits.toggle_mode();
        assert!(edits.begin_drag(RegionInputSource::Pointer, (1.0, 1.0)));
        assert!(edits.undo(fingerprint.clone()));
        assert!(edits.drag.is_none());
        assert_eq!(
            edits.finish_drag(
                RegionInputSource::Pointer,
                (7.0, 0.0),
                display(),
                fingerprint.clone()
            ),
            CutCommit::None
        );

        assert!(edits.begin_drag(RegionInputSource::Pointer, (1.0, 1.0)));
        assert!(edits.redo(fingerprint.clone()));
        assert!(edits.drag.is_none());
        assert_eq!(
            edits.finish_drag(
                RegionInputSource::Pointer,
                (7.0, 0.0),
                display(),
                fingerprint
            ),
            CutCommit::None
        );
    }

    #[test]
    fn undo_with_nothing_to_undo_leaves_an_in_flight_drag() {
        let mut edits = edits();
        edits.toggle_mode();
        assert!(edits.begin_drag(RegionInputSource::Pointer, (1.0, 1.0)));
        assert!(!edits.undo(fingerprint(edits.source_rect)));
        assert!(edits.drag.is_some());
    }

    fn review_input() -> crate::input::InputState {
        let mut input = crate::input::state::test_support::make_test_input_state();
        input.activate_region_review(
            crate::input::state::RegionPurposeTag::CaptureInteractive,
            1,
            display(),
        );
        input
    }

    fn edits_with_current_preview() -> RegionReviewEdits {
        let mut edits = edits();
        let fingerprint = fingerprint(edits.source_rect);
        edits
            .cuts
            .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
        edits.revision = 1;
        edits.set_desired_from(fingerprint);
        let desired = edits.desired_preview.clone().unwrap();
        edits.ready_preview = Some(RegionCutPreview {
            key: desired,
            pixels: std::sync::Arc::new(
                crate::screen_pixels::PackedArgb32::new(7, 8, 28, vec![0; 28 * 8]).unwrap(),
            ),
            display: display(),
        });
        edits
    }

    #[test]
    fn undo_and_redo_retire_pointer_touch_and_tablet_owners_before_release() {
        for owner in [
            RegionInputSource::Pointer,
            RegionInputSource::Touch,
            RegionInputSource::Stylus,
        ] {
            let mut input = review_input();
            let mut edits = Some(edits_with_current_preview());
            edits.as_mut().unwrap().toggle_mode();
            assert!(edits.as_mut().unwrap().begin_drag(owner, (1.0, 1.0)));
            assert!(input.begin_region_review_move(owner));
            assert!(input.region_selection_is_owned_by(owner));

            let fingerprint = fingerprint(edits.as_ref().unwrap().source_rect);
            assert!(apply_cut_history_change(&mut edits, &mut input, |edits| {
                edits.undo(fingerprint.clone())
            }));
            assert!(edits.as_ref().unwrap().drag.is_none());
            assert!(
                !input.region_selection_is_owned_by(owner),
                "{owner:?} must be retired before release"
            );
            assert_eq!(
                edits.as_mut().unwrap().finish_drag(
                    owner,
                    (7.0, 0.0),
                    display(),
                    fingerprint.clone()
                ),
                CutCommit::None,
                "{owner:?} release must not commit after undo"
            );
            assert!(!input.finish_region_review_move(owner));

            assert!(edits.as_mut().unwrap().begin_drag(owner, (1.0, 1.0)));
            assert!(input.begin_region_review_move(owner));
            assert!(apply_cut_history_change(&mut edits, &mut input, |edits| {
                edits.redo(fingerprint.clone())
            }));
            assert!(edits.as_ref().unwrap().drag.is_none());
            assert!(!input.region_selection_is_owned_by(owner));
            assert_eq!(
                edits
                    .as_mut()
                    .unwrap()
                    .finish_drag(owner, (7.0, 0.0), display(), fingerprint),
                CutCommit::None
            );
        }
    }

    #[test]
    fn toggling_cut_mode_off_during_a_drag_returns_the_owner() {
        let mut edits = edits();
        edits.toggle_mode();
        assert!(edits.begin_drag(RegionInputSource::Pointer, (1.0, 1.0)));
        assert_eq!(edits.toggle_mode(), Some(RegionInputSource::Pointer));
        assert_eq!(edits.mode, CutMode::Idle);
        assert!(edits.drag.is_none());
    }

    #[test]
    fn revision_exhaustion_leaves_reset_and_invalidate_untouched() {
        let mut edits = edits();
        let cut = CutBand::new(CutAxis::Columns, 1, 2).unwrap();
        edits.cuts.push(cut);
        edits.revision = u64::MAX;
        edits.failed_revision = Some(u64::MAX);
        assert!(!edits.reset());
        assert_eq!(edits.cuts, [cut]);
        assert_eq!(edits.failed_revision, Some(u64::MAX));

        let fingerprint = fingerprint(edits.source_rect);
        edits.invalidate_base(fingerprint);
        assert_eq!(edits.cuts, [cut]);
        assert_eq!(edits.failed_revision, Some(u64::MAX));
        assert_eq!(edits.revision, u64::MAX);
    }

    #[test]
    fn undoing_the_last_cut_unlocks_the_crop() {
        let mut edits = edits();
        let fingerprint = fingerprint(edits.source_rect);
        edits
            .cuts
            .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
        edits.revision = 1;
        assert!(edits.crop_locked());
        assert!(edits.undo(fingerprint));
        assert!(!edits.crop_locked());
    }

    #[test]
    fn a_failed_current_preview_stays_failed_until_the_revision_changes() {
        let mut edits = edits();
        let desired = CutPreviewKey {
            fingerprint: fingerprint(edits.source_rect),
            revision: 3,
            cuts: vec![CutBand::new(CutAxis::Columns, 1, 2).unwrap()],
        };
        edits.cuts = desired.cuts.clone();
        edits.revision = 3;
        edits.desired_preview = Some(desired.clone());
        assert!(edits.mark_preview_failed(&desired));
        assert!(edits.current_preview_failed());
        edits.failed_revision = None;
        edits.revision = 4;
        edits.desired_preview = Some(CutPreviewKey {
            revision: 4,
            ..desired
        });
        assert!(!edits.current_preview_failed());
    }

    #[test]
    fn reset_clears_history_and_unlocks_the_crop() {
        let mut edits = edits();
        edits.cuts.push(CutBand::new(CutAxis::Rows, 1, 2).unwrap());
        edits
            .redo
            .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
        edits.mode = CutMode::Armed;
        assert!(edits.reset());
        assert!(edits.cuts.is_empty());
        assert!(edits.redo.is_empty());
        assert!(!edits.crop_locked());
        assert!(edits.preview_is_current());
    }

    #[test]
    fn source_rect_cannot_change_while_cuts_exist() {
        let mut edits = edits();
        let next = ImagePixelRect::new(1, 1, 4, 4, (8, 8)).unwrap();
        assert!(edits.set_source_rect(next));
        edits
            .cuts
            .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
        assert!(!edits.set_source_rect(ImagePixelRect::new(0, 0, 4, 4, (8, 8)).unwrap()));
        assert_eq!(edits.source_rect, next);
    }

    #[test]
    fn cut_start_is_rejected_while_preview_is_pending() {
        let mut edits = edits();
        edits.toggle_mode();
        edits
            .cuts
            .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
        edits.revision = 1;
        edits.set_desired_from(fingerprint(edits.source_rect));
        assert!(!edits.preview_is_current());
        assert!(!edits.can_start_cut_drag());
    }

    #[test]
    fn loupe_is_suppressed_when_armed_or_cuts_exist() {
        let mut edits = edits();
        assert!(!edits.loupe_suppressed());
        edits.toggle_mode();
        assert!(edits.loupe_suppressed());
        edits.toggle_mode();
        edits
            .cuts
            .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
        assert!(edits.loupe_suppressed());
    }

    #[test]
    fn column_cut_preserves_top_left_and_height() {
        let token = token();
        let rect = ImagePixelRect::new(0, 0, 8, 8, (8, 8)).unwrap();
        let full = output_display_for(&token, rect, &[]).unwrap();
        let cut = output_display_for(
            &token,
            rect,
            &[CutBand::new(CutAxis::Columns, 2, 4).unwrap()],
        )
        .unwrap();
        assert_eq!(cut.start, full.start);
        assert_eq!(cut.end.1, full.end.1);
        assert!(cut.end.0 < full.end.0);
    }

    #[test]
    fn dominant_axis_ties_choose_columns() {
        assert_eq!(dominant_cut_axis(4.0, 4.0), CutAxis::Columns);
        assert_eq!(dominant_cut_axis(4.0, 5.0), CutAxis::Rows);
    }

    #[test]
    fn pointer_edges_map_to_the_inclusive_pixel_edge_domain() {
        let display = display();
        assert_eq!(
            logical_to_output_point(display, (8, 8), (0.0, 0.0)).map(|point| (point.x, point.y)),
            Some((0.0, 0.0))
        );
        assert_eq!(
            logical_to_output_point(display, (8, 8), (8.0, 8.0)).map(|point| (point.x, point.y)),
            Some((8.0, 8.0))
        );
        let clamped = logical_to_output_point(display, (8, 8), (-2.0, 20.0)).unwrap();
        assert_eq!((clamped.x, clamped.y), (0.0, 8.0));
    }

    #[test]
    fn composed_board_origin_stays_put_while_size_contracts() {
        let source = crate::canvas_export::CanvasExportRect::new(10.0, 20.0, 80.0, 40.0).unwrap();
        let composed =
            crate::backend::wayland::state::region_capture::world_rect_for_composed_region(
                source,
                (8, 8),
                (6, 4),
            )
            .unwrap();
        assert_eq!(composed.x, 10.0);
        assert_eq!(composed.y, 20.0);
        assert_eq!(composed.width, 60.0);
        assert_eq!(composed.height, 20.0);
    }
}
