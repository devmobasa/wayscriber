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

pub(super) fn begin_region_selection_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    owner: RegionInputSource,
    logical: (f64, f64),
) -> bool {
    if input_state.region_state().selection_owner().is_some() {
        return false;
    }
    let Some(region) = backend.as_mut() else {
        return false;
    };
    if input_state.region_state().is_review() {
        // A grip is checked before the rectangle it decorates: corner chips sit
        // on the rectangle's own edge, so the interior test would otherwise
        // swallow every resize press.
        if let Some(handle) = review_resize_handle_at(region, logical)
            && region.begin_review_resize(handle, logical)
        {
            return input_state.begin_region_review_move(owner);
        }
        if region.begin_review_move(logical) {
            return input_state.begin_region_review_move(owner);
        }
        region.reset_review_for_selection();
    }
    if !region.begin_selection(logical) {
        return false;
    }
    let Some(preview) = region.display_selection() else {
        return false;
    };
    if !input_state.start_region_selection(owner, preview.start) {
        return false;
    }
    input_state.update_region_selection(owner, preview.end);
    true
}

pub(super) fn update_region_selection_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    owner: RegionInputSource,
    logical: (f64, f64),
) {
    if backend
        .as_ref()
        .is_some_and(|region| region.purpose().is_capture())
        && input_state.region_is_active()
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
        && input_state.region_is_active()
    {
        // Measure has no full-screen scrim. Its old/current chrome strips are
        // added by collect_ui_effect_damage, so motion only schedules a frame.
        input_state.needs_redraw = true;
    }
    if !input_state.region_selection_is_owned_by(owner) {
        return;
    }
    if input_state.region_state().is_review() {
        if let Some(region) = backend.as_mut()
            && (region.update_review_resize(logical) || region.update_review_move(logical))
            && let Some(preview) = region
                .review_geometry()
                .map(RegionSelectionGeometry::display_selection)
        {
            input_state.update_region_review_display(preview);
        }
        return;
    }
    if let Some(region) = backend.as_mut()
        && region.update_endpoint(logical)
        && let Some(preview) = region.display_selection()
    {
        input_state.update_region_selection(owner, preview.end);
    }
}

/// End whichever Review drag a device owned. Exactly one of the two can be in
/// flight, so this reports a single "the drag ended" answer that stays in step
/// with the UI state's own move owner.
fn finish_review_drag(region: &mut ActiveScreenRegion) -> bool {
    region.finish_review_resize() || region.finish_review_move()
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
    if let Some(owner) = input_state.region_state().selection_owner()
        && let Some(preview) = region
            .selection_geometry()
            .map(RegionSelectionGeometry::display_selection)
    {
        input_state.update_region_selection(owner, preview.end);
    }
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
        ..
    }) = backend.as_mut()
    {
        *anchor = None;
        *raw_edge = None;
        *logical_anchor = None;
        *logical_edge = None;
        *review_resize = None;
    }
    if let Some(ActiveScreenRegion::Measure { anchor, edge, .. }) = backend.as_mut() {
        *anchor = None;
        *edge = None;
    }
    input_state.rearm_region_selection();
}

pub(super) fn region_owner_lost_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    source: RegionInputSource,
) -> RegionOwnerLoss {
    if !input_state.region_selection_is_owned_by(source) {
        return RegionOwnerLoss::NotOwned;
    }
    if input_state.region_state().purpose() == Some(RegionPurposeTag::Measure) {
        rearm_region_selection_event(backend, input_state);
        return RegionOwnerLoss::Rearmed;
    }
    let Some(purpose) = backend.as_ref().map(|region| region.purpose()) else {
        return RegionOwnerLoss::NotOwned;
    };
    if !purpose.is_capture() {
        return RegionOwnerLoss::Cancel(purpose);
    }
    if input_state.region_state().is_review() {
        let finished_backend = backend.as_mut().is_some_and(finish_review_drag);
        let finished_ui = input_state.finish_region_review_move(source);
        debug_assert_eq!(finished_backend, finished_ui);
        return RegionOwnerLoss::Rearmed;
    }
    rearm_region_selection_event(backend, input_state);
    RegionOwnerLoss::Rearmed
}

pub(in crate::backend::wayland::state) fn finalize_region_selection_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    owner: RegionInputSource,
    logical: (f64, f64),
) -> RegionSelectionFinalize {
    if !input_state.region_selection_is_owned_by(owner) {
        return RegionSelectionFinalize::NotOwned;
    }
    if input_state.region_state().purpose() == Some(RegionPurposeTag::Measure) {
        update_region_selection_event(backend, input_state, owner, logical);
        return if backend
            .as_ref()
            .and_then(|region| region.measure_selection())
            .is_some()
            && input_state.complete_measurement(owner)
        {
            RegionSelectionFinalize::Measured
        } else {
            RegionSelectionFinalize::NotOwned
        };
    }
    if input_state.region_state().is_review() {
        update_region_selection_event(backend, input_state, owner, logical);
        let finished_backend = backend.as_mut().is_some_and(finish_review_drag);
        let finished_ui = input_state.finish_region_review_move(owner);
        debug_assert_eq!(finished_backend, finished_ui);
        return if finished_backend {
            RegionSelectionFinalize::Reviewed
        } else {
            RegionSelectionFinalize::NotOwned
        };
    }
    update_region_selection_event(backend, input_state, owner, logical);
    let Some(rect) = backend
        .as_ref()
        .copied()
        .and_then(ActiveScreenRegion::selection_rect)
    else {
        rearm_region_selection_event(backend, input_state);
        return RegionSelectionFinalize::Rearmed;
    };
    let purpose = backend
        .as_ref()
        .expect("a selected region still has backend state")
        .purpose();
    if purpose == RegionPurposeTag::CaptureInteractive {
        let generation = backend
            .as_ref()
            .expect("a reviewed region still has backend state")
            .generation();
        let display = backend
            .as_mut()
            .and_then(|region| region.enter_review(rect))
            .expect("an interactive rectangle enters review");
        input_state.activate_region_review(purpose, generation, display);
        return RegionSelectionFinalize::Reviewed;
    }
    RegionSelectionFinalize::Selected { purpose, rect }
}

pub(in crate::backend::wayland::state) fn finalize_region_selection_with_review_edits(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    review_edits: &mut Option<RegionReviewEdits>,
    owner: RegionInputSource,
    logical: (f64, f64),
) -> RegionSelectionFinalize {
    let was_review = input_state.region_state().is_review();
    let result = finalize_region_selection_event(backend, input_state, owner, logical);
    if result == RegionSelectionFinalize::Reviewed
        && (!was_review || review_edits.is_none())
        && let Some(rect) = backend.and_then(ActiveScreenRegion::selection_rect)
    {
        *review_edits = super::cut_review::review_edits_for_active_region(*backend, rect);
    }
    result
}
