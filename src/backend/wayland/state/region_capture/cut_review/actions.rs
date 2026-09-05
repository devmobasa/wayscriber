use super::geometry::{cut_band_display, display_contains, output_display_for, quantized_cut};
use super::model::{CutCommit, CutMode, RegionReviewCorrelation, RegionReviewEdits};
use crate::backend::wayland::state::WaylandState;
use crate::backend::wayland::state::region_capture::ActiveScreenRegion;
use crate::capture::CutAxis;
use crate::input::state::{RegionInputSource, RegionSelection};
use crate::screen_pixels::ImagePixelRect;
use crate::ui::{RegionActionAvailability, RegionCutStatus};

pub(in crate::backend::wayland::state::region_capture) fn review_edits_for_active_region(
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

pub(super) fn apply_cut_history_change(
    edits: &mut Option<RegionReviewEdits>,
    change: impl FnOnce(&mut RegionReviewEdits) -> bool,
) -> bool {
    edits.as_mut().is_some_and(change)
}

impl WaylandState {
    pub(in crate::backend::wayland) fn region_review_crop_locked(&self) -> bool {
        self.region_capture
            .review_edits()
            .is_some_and(RegionReviewEdits::crop_locked)
    }

    pub(in crate::backend::wayland) fn region_review_loupe_suppressed(&self) -> bool {
        self.region_capture
            .review_edits()
            .is_some_and(RegionReviewEdits::loupe_suppressed)
    }

    pub(in crate::backend::wayland) fn region_cut_displayed_selection(
        &self,
    ) -> Option<RegionSelection> {
        let edits = self.region_capture.review_edits()?;
        if let Some(preview) = &edits.ready_preview {
            return Some(preview.display);
        }
        let token = self.region_picker_source_token()?;
        output_display_for(&token, edits.source_rect, &[])
    }

    pub(in crate::backend::wayland) fn region_cut_availability(&self) -> RegionActionAvailability {
        self.region_capture
            .review_edits()
            .map(RegionReviewEdits::availability)
            .unwrap_or_default()
    }

    pub(in crate::backend::wayland) fn region_cut_status(&self) -> Option<RegionCutStatus> {
        self.region_capture
            .review_edits()
            .and_then(RegionReviewEdits::status)
    }

    pub(in crate::backend::wayland) fn region_cut_mode_armed(&self) -> bool {
        self.region_capture
            .review_edits()
            .is_some_and(|edits| edits.mode == CutMode::Armed)
    }

    pub(in crate::backend::wayland::state::region_capture) fn mark_region_cut_ui_dirty(&mut self) {
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
        if let Some(owner) = owner {
            let _ = self
                .region_capture
                .finish_review_aux_drag(&mut self.input_state, owner);
        }
    }

    fn toggle_region_cut_mode(&mut self) -> bool {
        let Some(edits) = self.region_capture.review_edits_mut() else {
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
        let owner = self
            .region_capture
            .review_edits()
            .and_then(|edits| edits.drag.map(|drag| drag.owner));
        if !apply_cut_history_change(self.region_capture.review_edits_slot_mut(), |edits| {
            edits.undo(fingerprint)
        }) {
            return false;
        }
        self.retire_region_cut_drag_owner(owner);
        self.mark_region_cut_ui_dirty();
        self.schedule_region_cut_preview();
        true
    }

    fn redo_region_cut(&mut self) -> bool {
        let Some(fingerprint) = self.current_region_fingerprint() else {
            return false;
        };
        let owner = self
            .region_capture
            .review_edits()
            .and_then(|edits| edits.drag.map(|drag| drag.owner));
        if !apply_cut_history_change(self.region_capture.review_edits_slot_mut(), |edits| {
            edits.redo(fingerprint)
        }) {
            return false;
        }
        self.retire_region_cut_drag_owner(owner);
        self.mark_region_cut_ui_dirty();
        self.schedule_region_cut_preview();
        true
    }

    fn reset_region_cuts(&mut self) -> bool {
        let owner = self
            .region_capture
            .review_edits()
            .and_then(|edits| edits.drag.map(|drag| drag.owner));
        if !apply_cut_history_change(
            self.region_capture.review_edits_slot_mut(),
            RegionReviewEdits::reset,
        ) {
            return false;
        }
        self.retire_region_cut_drag_owner(owner);
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
        let Some(edits) = self.region_capture.review_edits_mut() else {
            return false;
        };
        if !edits.begin_drag(owner, point) {
            return false;
        }
        if !self
            .region_capture
            .begin_review_aux_drag(&mut self.input_state, owner)
        {
            if let Some(edits) = self.region_capture.review_edits_mut() {
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
        let Some(edits) = self.region_capture.review_edits_mut() else {
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
            .region_capture
            .review_edits()
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
        let Some(edits) = self.region_capture.review_edits_mut() else {
            return false;
        };
        let commit = edits.finish_drag(owner, point, display, fingerprint);
        let _ = self
            .region_capture
            .finish_review_aux_drag(&mut self.input_state, owner);
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
        let Some(edits) = self.region_capture.review_edits_mut() else {
            return false;
        };
        let Some(drag) = edits.drag else {
            return false;
        };
        if drag.owner != owner {
            return false;
        }
        edits.drag = None;
        let _ = self
            .region_capture
            .finish_review_aux_drag(&mut self.input_state, owner);
        self.mark_region_cut_ui_dirty();
        true
    }

    pub(in crate::backend::wayland) fn handle_region_cut_escape(&mut self) -> bool {
        let owner = self
            .region_capture
            .review_edits()
            .and_then(|edits| edits.drag.map(|drag| drag.owner));
        let Some(edits) = self.region_capture.review_edits_mut() else {
            return false;
        };
        if !edits.disarm_mode() {
            return false;
        }
        if let Some(owner) = owner {
            let _ = self
                .region_capture
                .finish_review_aux_drag(&mut self.input_state, owner);
        }
        self.mark_region_cut_ui_dirty();
        true
    }

    pub(in crate::backend::wayland::state::region_capture) fn sync_region_review_source_rect(
        &mut self,
    ) {
        let Some(rect) = self.region_review_rect() else {
            return;
        };
        let Some(edits) = self.region_capture.review_edits_mut() else {
            return;
        };
        if edits.set_source_rect(rect) {
            self.mark_region_cut_ui_dirty();
        }
    }

    pub(in crate::backend::wayland) fn region_cut_preview_pixels(
        &self,
    ) -> Option<&crate::screen_pixels::PackedArgb32> {
        self.region_capture
            .review_edits()
            .and_then(|edits| edits.ready_preview.as_ref())
            .map(|preview| preview.pixels.as_ref())
    }

    pub(in crate::backend::wayland) fn region_cut_drag_overlay(
        &self,
    ) -> Option<(CutAxis, RegionSelection)> {
        let edits = self.region_capture.review_edits()?;
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
