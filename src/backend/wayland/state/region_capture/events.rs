use super::*;

/// The grip under `logical`, placed from the rectangle Review is showing. The
/// renderer places its chips from the same geometry, so what is painted and
/// what is grabbable cannot drift.
pub(super) fn review_resize_handle_at(
    region: &ActiveScreenRegion,
    logical: (f64, f64),
) -> Option<SelectionHandle> {
    let selection = region
        .review_geometry()
        .map(RegionSelectionGeometry::display_selection)?;
    crate::ui::RegionResizeHandles::place(selection).hit(logical)
}

fn sync_projection(
    backend: &Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
) {
    input_state.sync_region_projection(
        backend
            .map(ActiveScreenRegion::ui_state)
            .unwrap_or_default(),
    );
}

pub(super) fn begin_region_selection_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    owner: RegionInputSource,
    logical: (f64, f64),
) -> bool {
    let Some(region) = backend.as_mut() else {
        return false;
    };
    if region.selection_owner().is_some() {
        return false;
    }
    if matches!(region.phase(), Some(RegionInteractionPhase::Review { .. })) {
        // A grip is checked before the rectangle it decorates: corner chips sit
        // on the rectangle's own edge, so the interior test would otherwise
        // swallow every resize press.
        if let Some(handle) = review_resize_handle_at(region, logical)
            && region.begin_review_resize(handle, logical)
        {
            region.set_phase(RegionInteractionPhase::Review { owner: Some(owner) });
            sync_projection(backend, input_state);
            return true;
        }
        if region.begin_review_move(logical) {
            region.set_phase(RegionInteractionPhase::Review { owner: Some(owner) });
            sync_projection(backend, input_state);
            return true;
        }
        region.reset_review_for_selection();
        region.set_phase(RegionInteractionPhase::Armed);
    }
    if !region.begin_selection(logical) {
        return false;
    }
    region.set_phase(RegionInteractionPhase::Selecting { owner });
    sync_projection(backend, input_state);
    true
}

pub(super) fn update_region_selection_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    owner: RegionInputSource,
    logical: (f64, f64),
) {
    let state = backend
        .as_ref()
        .copied()
        .map(ActiveScreenRegion::ui_state)
        .unwrap_or_default();
    if backend
        .as_ref()
        .is_some_and(|region| region.purpose().is_capture())
        && state.is_active()
    {
        // Capture chrome follows hover even when no device owns a drag: Armed
        // paints the crosshair/readout, while Review paints bar hover and the
        // optional loupe. Motion adapters call this only after a position
        // event, so schedule the selector's full-surface repaint here.
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
    } else if backend
        .as_ref()
        .is_some_and(|region| region.purpose() == RegionPurposeTag::Measure)
        && state.is_active()
    {
        // Measure has no full-screen scrim. Its old/current chrome strips are
        // added by collect_ui_effect_damage, so motion only schedules a frame.
        input_state.needs_redraw = true;
    }
    let Some(region) = backend.as_mut() else {
        return;
    };
    if region.selection_owner() != Some(owner) {
        return;
    }
    let changed = if matches!(region.phase(), Some(RegionInteractionPhase::Review { .. })) {
        region.update_review_resize(logical) || region.update_review_move(logical)
    } else {
        region.update_endpoint(logical)
    };
    if changed {
        sync_projection(backend, input_state);
    }
}

/// End whichever Review drag a device owned. Exactly one of the two can be in
/// flight, so this reports a single "the drag ended" answer.
fn finish_review_drag(region: &mut ActiveScreenRegion, owner: RegionInputSource) -> bool {
    if region.selection_owner() != Some(owner) {
        return false;
    }
    let finished = region.finish_review_resize() || region.finish_review_move();
    if finished {
        region.set_phase(RegionInteractionPhase::Review { owner: None });
    }
    finished
}

pub(super) fn sync_region_square_modifier_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    shift: bool,
) -> bool {
    let Some(region) = backend.as_mut() else {
        return false;
    };
    if !region.set_square_modifier(shift) {
        return false;
    }
    sync_projection(backend, input_state);
    true
}

pub(super) fn initial_square_modifier(purpose: RegionPurposeTag, shift: bool) -> bool {
    shift && purpose.selection_policy().allow_square()
}

pub(super) fn rearm_region_selection_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
) {
    if let Some(ActiveScreenRegion::Ready {
        anchor,
        raw_edge,
        logical_anchor,
        logical_edge,
        review_resize,
        phase,
        ..
    }) = backend.as_mut()
    {
        *anchor = None;
        *raw_edge = None;
        *logical_anchor = None;
        *logical_edge = None;
        *review_resize = None;
        *phase = RegionInteractionPhase::Armed;
    }
    if let Some(ActiveScreenRegion::Measure {
        anchor,
        edge,
        phase,
        ..
    }) = backend.as_mut()
    {
        *anchor = None;
        *edge = None;
        *phase = RegionInteractionPhase::Armed;
    }
    sync_projection(backend, input_state);
}

pub(super) fn region_owner_lost_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    source: RegionInputSource,
) -> RegionOwnerLoss {
    let Some(region) = backend.as_mut() else {
        return RegionOwnerLoss::NotOwned;
    };
    if region.selection_owner() != Some(source) {
        return RegionOwnerLoss::NotOwned;
    }
    let purpose = region.purpose();
    if purpose == RegionPurposeTag::Measure {
        rearm_region_selection_event(backend, input_state);
        return RegionOwnerLoss::Rearmed;
    }
    if !purpose.is_capture() {
        return RegionOwnerLoss::Cancel(purpose);
    }
    if matches!(region.phase(), Some(RegionInteractionPhase::Review { .. })) {
        let finished = finish_review_drag(region, source);
        debug_assert!(finished);
        sync_projection(backend, input_state);
        return RegionOwnerLoss::Rearmed;
    }
    rearm_region_selection_event(backend, input_state);
    RegionOwnerLoss::Rearmed
}

enum SelectionFinalization {
    Complete(RegionSelectionFinalize),
    Review(super::review_state::InteractiveReviewSeed),
}

fn finalize_region_selection(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    owner: RegionInputSource,
    logical: (f64, f64),
) -> SelectionFinalization {
    let Some(region) = backend.as_mut() else {
        return SelectionFinalization::Complete(RegionSelectionFinalize::NotOwned);
    };
    if region.selection_owner() != Some(owner) {
        return SelectionFinalization::Complete(RegionSelectionFinalize::NotOwned);
    }
    if region.purpose() == RegionPurposeTag::Measure {
        region.update_endpoint(logical);
        if region.measure_selection().is_some() {
            region.set_phase(RegionInteractionPhase::Measured);
            sync_projection(backend, input_state);
            return SelectionFinalization::Complete(RegionSelectionFinalize::Measured);
        }
        return SelectionFinalization::Complete(RegionSelectionFinalize::NotOwned);
    }
    if matches!(region.phase(), Some(RegionInteractionPhase::Review { .. })) {
        let changed = region.update_review_resize(logical) || region.update_review_move(logical);
        let finished = finish_review_drag(region, owner);
        if changed || finished {
            sync_projection(backend, input_state);
        }
        return SelectionFinalization::Complete(if finished {
            RegionSelectionFinalize::Reviewed
        } else {
            RegionSelectionFinalize::NotOwned
        });
    }
    let ActiveScreenRegion::Ready { .. } = region else {
        return SelectionFinalization::Complete(RegionSelectionFinalize::NotOwned);
    };
    if region.purpose().is_capture() {
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
    }
    region.update_endpoint(logical);
    sync_projection(backend, input_state);
    let Some(rect) = backend.as_ref().and_then(|region| region.selection_rect()) else {
        rearm_region_selection_event(backend, input_state);
        return SelectionFinalization::Complete(RegionSelectionFinalize::Rearmed);
    };
    let region = backend
        .as_mut()
        .expect("active region retained while finalizing");
    let purpose = region.purpose();
    if purpose == RegionPurposeTag::CaptureInteractive {
        let Some(seed) = region.enter_review_seed(rect) else {
            return SelectionFinalization::Complete(RegionSelectionFinalize::NotOwned);
        };
        region.set_phase(RegionInteractionPhase::Review { owner: None });
        sync_projection(backend, input_state);
        return SelectionFinalization::Review(seed);
    }
    SelectionFinalization::Complete(RegionSelectionFinalize::Selected { purpose, rect })
}

pub(in crate::backend::wayland::state) fn finalize_region_selection_with_review_edits(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    review_edits: &mut Option<RegionReviewEdits>,
    owner: RegionInputSource,
    logical: (f64, f64),
) -> RegionSelectionFinalize {
    match finalize_region_selection(backend, input_state, owner, logical) {
        SelectionFinalization::Review(seed) => {
            *review_edits = Some(seed.into_edits());
            RegionSelectionFinalize::Reviewed
        }
        SelectionFinalization::Complete(result) => {
            if result == RegionSelectionFinalize::Reviewed
                && review_edits.is_none()
                && let Some(rect) = backend.as_ref().and_then(|region| region.selection_rect())
            {
                *review_edits = super::cut_review::review_edits_for_active_region(*backend, rect);
            }
            result
        }
    }
}
