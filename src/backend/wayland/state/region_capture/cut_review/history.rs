use super::geometry::{dominant_cut_axis, quantized_cut};
use super::model::{
    CutCommit, CutDrag, CutMode, CutPreviewKey, RegionRenderFingerprint, RegionReviewCorrelation,
    RegionReviewEdits,
};
use crate::capture::output_size;
use crate::input::state::{RegionInputSource, RegionSelection};
use crate::screen_pixels::ImagePixelRect;
use crate::ui::{RegionActionAvailability, RegionCutStatus};

const CUT_DRAG_THRESHOLD_PX: f64 = 4.0;

impl RegionReviewEdits {
    pub(in crate::backend::wayland::state::region_capture) fn new(
        correlation: RegionReviewCorrelation,
        source_rect: ImagePixelRect,
    ) -> Self {
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

    pub(in crate::backend::wayland::state::region_capture) fn crop_locked(&self) -> bool {
        !self.cuts.is_empty()
    }

    pub(in crate::backend::wayland::state::region_capture) fn loupe_suppressed(&self) -> bool {
        self.mode == CutMode::Armed || self.crop_locked()
    }

    pub(in crate::backend::wayland::state::region_capture) fn preview_is_current(&self) -> bool {
        match &self.desired_preview {
            None => self.cuts.is_empty(),
            Some(desired) => self
                .ready_preview
                .as_ref()
                .is_some_and(|ready| ready.key == *desired),
        }
    }

    pub(in crate::backend::wayland::state::region_capture) fn can_start_cut_drag(&self) -> bool {
        self.mode == CutMode::Armed && self.preview_is_current() && self.drag.is_none()
    }

    /// A current failed preview must stay failed until undo, redo, or reset
    /// changes the revision. Resubmitting the same revision churns workers.
    pub(in crate::backend::wayland::state::region_capture) fn current_preview_failed(
        &self,
    ) -> bool {
        self.desired_preview.as_ref().is_some_and(|desired| {
            self.failed_revision
                .is_some_and(|revision| revision == desired.revision)
        })
    }

    pub(in crate::backend::wayland::state::region_capture) fn status(
        &self,
    ) -> Option<RegionCutStatus> {
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

    pub(in crate::backend::wayland::state::region_capture) fn availability(
        &self,
    ) -> RegionActionAvailability {
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
    pub(in crate::backend::wayland::state::region_capture) fn toggle_mode(
        &mut self,
    ) -> Option<RegionInputSource> {
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

    pub(in crate::backend::wayland::state::region_capture) fn disarm_mode(&mut self) -> bool {
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

    pub(in crate::backend::wayland::state::region_capture) fn set_desired_from(
        &mut self,
        fingerprint: RegionRenderFingerprint,
    ) {
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

    pub(in crate::backend::wayland::state::region_capture) fn output_size(
        &self,
    ) -> Option<(u32, u32)> {
        output_size(
            (self.source_rect.width(), self.source_rect.height()),
            &self.cuts,
        )
        .ok()
    }

    pub(in crate::backend::wayland::state::region_capture) fn displayed_output_size(
        &self,
    ) -> Option<(u32, u32)> {
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

    pub(in crate::backend::wayland::state::region_capture) fn begin_drag(
        &mut self,
        owner: RegionInputSource,
        point: (f64, f64),
    ) -> bool {
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

    pub(in crate::backend::wayland::state::region_capture) fn update_drag(
        &mut self,
        owner: RegionInputSource,
        point: (f64, f64),
    ) -> bool {
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

    pub(in crate::backend::wayland::state::region_capture) fn finish_drag(
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

    pub(in crate::backend::wayland::state::region_capture) fn undo(
        &mut self,
        fingerprint: RegionRenderFingerprint,
    ) -> bool {
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

    pub(in crate::backend::wayland::state::region_capture) fn redo(
        &mut self,
        fingerprint: RegionRenderFingerprint,
    ) -> bool {
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

    pub(in crate::backend::wayland::state::region_capture) fn reset(&mut self) -> bool {
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

    pub(in crate::backend::wayland::state::region_capture) fn set_source_rect(
        &mut self,
        source_rect: ImagePixelRect,
    ) -> bool {
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

    pub(in crate::backend::wayland::state::region_capture) fn invalidate_base(
        &mut self,
        fingerprint: RegionRenderFingerprint,
    ) {
        if !self.cuts.is_empty() && !self.bump_revision() {
            return;
        }
        self.base_cache = None;
        self.ready_preview = None;
        self.failed_revision = None;
        self.set_desired_from(fingerprint);
    }

    pub(in crate::backend::wayland::state::region_capture) fn mark_preview_failed(
        &mut self,
        key: &CutPreviewKey,
    ) -> bool {
        if self.desired_preview.as_ref() != Some(key) {
            return false;
        }
        self.failed_revision = Some(key.revision);
        true
    }
}
