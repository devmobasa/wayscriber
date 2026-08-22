use crate::backend::wayland::acquisition::ScreenAcquisitionId;
use crate::input::state::{
    RegionInputSource, RegionPurposeTag, RegionSelectUiState, RegionSelection, ScreenCaptureSource,
};

mod board;
mod geometry;
mod intent;
mod picker;

use crate::screen_pixels::{ImagePixelRect, ImagePoint, clamp_edge};
pub(in crate::backend::wayland) use board::world_rect_for_screen_rect;
pub(in crate::backend::wayland) use geometry::{RegionPickerMeasurement, RegionSelectionGeometry};
pub(in crate::backend::wayland) use intent::{RegionCaptureIntent, RegionPickerOptions};

use super::WaylandState;
use super::screen_image::{
    ScreenSourceToken, current_screen_source_token, image_point_for_screen_point, screen_source_is,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FreezeOwnership {
    PreExisting,
    PickerOwned { image_generation: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ActiveScreenRegion {
    PendingFrozen {
        purpose: RegionPurposeTag,
        generation: u64,
        acquisition: ScreenAcquisitionId,
    },
    PendingZoom {
        purpose: RegionPurposeTag,
        generation: u64,
    },
    Ready {
        purpose: RegionPurposeTag,
        generation: u64,
        source: ScreenSourceToken,
        freeze_ownership: FreezeOwnership,
        anchor: Option<ImagePoint>,
        raw_edge: Option<ImagePoint>,
        logical_anchor: Option<(f64, f64)>,
        logical_edge: Option<(f64, f64)>,
        square_modifier: bool,
        legend_dismissed: bool,
    },
}

impl ActiveScreenRegion {
    pub const fn purpose(self) -> RegionPurposeTag {
        match self {
            Self::PendingFrozen { purpose, .. }
            | Self::PendingZoom { purpose, .. }
            | Self::Ready { purpose, .. } => purpose,
        }
    }

    pub const fn generation(self) -> u64 {
        match self {
            Self::PendingFrozen { generation, .. }
            | Self::PendingZoom { generation, .. }
            | Self::Ready { generation, .. } => generation,
        }
    }

    pub const fn pending_acquisition(self) -> Option<ScreenAcquisitionId> {
        match self {
            Self::PendingFrozen { acquisition, .. } => Some(acquisition),
            Self::PendingZoom { .. } | Self::Ready { .. } => None,
        }
    }

    pub fn waits_for_acquisition(self, id: ScreenAcquisitionId) -> bool {
        matches!(self, Self::PendingFrozen { acquisition, .. } if acquisition == id)
    }

    pub const fn owned_frozen_generation(self) -> Option<u64> {
        match self {
            Self::Ready {
                freeze_ownership: FreezeOwnership::PickerOwned { image_generation },
                ..
            } => Some(image_generation),
            Self::PendingFrozen { .. }
            | Self::PendingZoom { .. }
            | Self::Ready {
                freeze_ownership: FreezeOwnership::PreExisting,
                ..
            } => None,
        }
    }

    pub const fn legend_dismissed(self) -> bool {
        matches!(
            self,
            Self::Ready {
                legend_dismissed: true,
                ..
            }
        )
    }

    fn selection_rect(self) -> Option<ImagePixelRect> {
        if let Some(rect) = self.stored_review_rect() {
            return Some(rect);
        }
        let Self::Ready {
            purpose,
            logical_anchor,
            logical_edge,
            ..
        } = self
        else {
            return None;
        };
        if let Some(minimum) = purpose.selection_policy().min_submit_logical_px() {
            let (Some(logical_anchor), Some(logical_edge)) = (logical_anchor, logical_edge) else {
                return None;
            };
            if (logical_edge.0 - logical_anchor.0).abs() < minimum
                || (logical_edge.1 - logical_anchor.1).abs() < minimum
            {
                return None;
            }
        }
        self.selection_geometry()?.image_rect()
    }

    fn selection_geometry(self) -> Option<RegionSelectionGeometry> {
        if self.stored_review_rect().is_some() {
            return self.review_geometry();
        }
        let Self::Ready {
            purpose,
            source,
            anchor: Some(anchor),
            raw_edge: Some(raw_edge),
            logical_anchor,
            logical_edge,
            square_modifier,
            ..
        } = self
        else {
            return None;
        };
        geometry::selection_geometry(
            purpose,
            source,
            anchor,
            raw_edge,
            logical_anchor
                .zip(logical_edge)
                .map(|(start, end)| RegionSelection { start, end }),
            square_modifier,
        )
    }

    fn review_geometry(self) -> Option<RegionSelectionGeometry> {
        let Self::Ready {
            purpose, source, ..
        } = self
        else {
            return None;
        };
        let image_rect = self.stored_review_rect()?;
        let display = super::screen_image::screen_rect_for_image_rect(&source, image_rect);
        Some(RegionSelectionGeometry::review(
            purpose,
            image_rect,
            RegionSelection {
                start: (f64::from(display.x), f64::from(display.y)),
                end: (
                    f64::from(display.x.saturating_add(display.width)),
                    f64::from(display.y.saturating_add(display.height)),
                ),
            },
        ))
    }

    fn stored_review_rect(self) -> Option<ImagePixelRect> {
        let Self::Ready {
            purpose: RegionPurposeTag::CaptureInteractive,
            source,
            anchor: Some(anchor),
            raw_edge: Some(raw_edge),
            logical_edge: None,
            ..
        } = self
        else {
            return None;
        };
        ImagePixelRect::from_points(anchor, raw_edge, source.image_size)
    }

    fn store_review_rect(&mut self, rect: ImagePixelRect) -> bool {
        let Self::Ready {
            anchor,
            raw_edge,
            logical_edge,
            ..
        } = self
        else {
            return false;
        };
        *anchor = Some(ImagePoint::new(f64::from(rect.x()), f64::from(rect.y())));
        *raw_edge = Some(ImagePoint::new(
            f64::from(rect.x() + rect.width()),
            f64::from(rect.y() + rect.height()),
        ));
        *logical_edge = None;
        true
    }

    fn set_square_modifier(&mut self, active: bool) -> bool {
        let Self::Ready {
            purpose,
            square_modifier,
            ..
        } = self
        else {
            return false;
        };
        let next = active && purpose.selection_policy().allow_square();
        if *square_modifier == next {
            return false;
        }
        *square_modifier = next;
        true
    }

    fn whole_image_selection(self) -> Option<RegionSelectionFinalize> {
        let Self::Ready {
            purpose, source, ..
        } = self
        else {
            return None;
        };
        Some(RegionSelectionFinalize::Selected {
            purpose,
            rect: geometry::whole_image_rect(purpose, source.image_size)?,
        })
    }

    fn picker_measurement(self, pointer: (f64, f64)) -> Option<RegionPickerMeasurement> {
        let Self::Ready {
            purpose, source, ..
        } = self
        else {
            return None;
        };
        if !purpose.is_capture() {
            return None;
        }
        if let Some(geometry) = self.selection_geometry() {
            let span = geometry.image_span();
            return Some(RegionPickerMeasurement::Size {
                width: span.width(),
                height: span.height(),
            });
        }
        geometry::point_measurement(
            purpose,
            image_point_for_screen_point(&source, pointer),
            source.image_size,
        )
    }

    fn enter_review(&mut self, rect: ImagePixelRect) -> Option<RegionSelection> {
        let Self::Ready {
            purpose,
            anchor,
            raw_edge,
            logical_anchor,
            logical_edge,
            ..
        } = self
        else {
            return None;
        };
        if *purpose != RegionPurposeTag::CaptureInteractive {
            return None;
        }
        *anchor = None;
        *raw_edge = None;
        *logical_anchor = None;
        *logical_edge = None;
        *anchor = Some(ImagePoint::new(f64::from(rect.x()), f64::from(rect.y())));
        *raw_edge = Some(ImagePoint::new(
            f64::from(rect.x() + rect.width()),
            f64::from(rect.y() + rect.height()),
        ));
        self.review_geometry()
            .map(|geometry| geometry.display_selection())
    }

    fn begin_review_move(&mut self, logical: (f64, f64)) -> bool {
        let Some(rect) = (*self).stored_review_rect() else {
            return false;
        };
        let Self::Ready {
            source,
            logical_anchor,
            ..
        } = self
        else {
            return false;
        };
        if logical_anchor.is_some() {
            return false;
        }
        let display = super::screen_image::screen_rect_for_image_rect(source, rect);
        let x = logical.0.floor() as i32;
        let y = logical.1.floor() as i32;
        if x < display.x
            || y < display.y
            || x >= display.x.saturating_add(display.width)
            || y >= display.y.saturating_add(display.height)
        {
            return false;
        }
        let origin = image_point_for_screen_point(source, logical);
        *logical_anchor = Some((origin.x, origin.y));
        true
    }

    fn update_review_move(&mut self, logical: (f64, f64)) -> bool {
        let Some(image_rect) = (*self).stored_review_rect() else {
            return false;
        };
        let Self::Ready {
            source,
            logical_anchor: Some(origin),
            ..
        } = self
        else {
            return false;
        };
        let current = image_point_for_screen_point(source, logical);
        let delta_x = (current.x - origin.0).round() as i64;
        let delta_y = (current.y - origin.1).round() as i64;
        if delta_x == 0 && delta_y == 0 {
            return false;
        }
        let Some(next) = image_rect.translated_clamped(delta_x, delta_y, source.image_size) else {
            return false;
        };
        if image_rect == next {
            return false;
        }
        let applied_x = i64::from(next.x()) - i64::from(image_rect.x());
        let applied_y = i64::from(next.y()) - i64::from(image_rect.y());
        let next_origin = (origin.0 + applied_x as f64, origin.1 + applied_y as f64);
        if !self.store_review_rect(next) {
            return false;
        }
        if let Self::Ready { logical_anchor, .. } = self {
            *logical_anchor = Some(next_origin);
        }
        true
    }

    fn finish_review_move(&mut self) -> bool {
        let Self::Ready { logical_anchor, .. } = self else {
            return false;
        };
        if logical_anchor.is_none() {
            return false;
        }
        *logical_anchor = None;
        true
    }

    fn reset_review_for_selection(&mut self) {
        if let Self::Ready {
            anchor,
            raw_edge,
            logical_anchor,
            ..
        } = self
        {
            *anchor = None;
            *raw_edge = None;
            *logical_anchor = None;
        }
    }

    fn nudge_review(&mut self, delta_x: i64, delta_y: i64) -> Option<RegionSelection> {
        let review_rect = (*self).stored_review_rect()?;
        let Self::Ready {
            source,
            logical_anchor,
            ..
        } = self
        else {
            return None;
        };
        if logical_anchor.is_some() {
            return None;
        }
        let rect = review_rect.translated_clamped(delta_x, delta_y, source.image_size)?;
        self.store_review_rect(rect);
        self.review_geometry()
            .map(|geometry| geometry.display_selection())
    }

    fn begin_selection(&mut self, logical: (f64, f64)) -> bool {
        let Self::Ready {
            purpose,
            source,
            anchor,
            raw_edge,
            logical_anchor,
            logical_edge,
            legend_dismissed,
            ..
        } = self
        else {
            return false;
        };
        if anchor.is_some()
            || raw_edge.is_some()
            || logical_anchor.is_some()
            || logical_edge.is_some()
        {
            return false;
        }
        let Some(mapped) = clamp_edge(
            image_point_for_screen_point(source, logical),
            source.image_size,
        ) else {
            return false;
        };
        let Some(anchor_point) = geometry::selection_anchor(*purpose, mapped, source.image_size)
        else {
            return false;
        };
        *anchor = Some(anchor_point);
        *raw_edge = Some(if purpose.is_capture() {
            anchor_point
        } else {
            mapped
        });
        *logical_anchor = Some(logical);
        *logical_edge = Some(logical);
        *legend_dismissed = true;
        true
    }

    fn update_endpoint(&mut self, logical: (f64, f64)) -> bool {
        let Self::Ready {
            source,
            raw_edge,
            logical_edge,
            ..
        } = self
        else {
            return false;
        };
        let Some(mapped) = clamp_edge(
            image_point_for_screen_point(source, logical),
            source.image_size,
        ) else {
            return false;
        };
        *raw_edge = Some(mapped);
        *logical_edge = Some(logical);
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum RegionSelectionFinalize {
    NotOwned,
    Rearmed,
    Reviewed,
    Selected {
        purpose: RegionPurposeTag,
        rect: ImagePixelRect,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum RegionOwnerLoss {
    NotOwned,
    RearmedCapture,
    Cancel(RegionPurposeTag),
}

fn begin_region_selection_event(
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
        if region.begin_review_move(logical) {
            return input_state.begin_region_review_move(owner);
        }
        region.reset_review_for_selection();
    }
    if !region.begin_selection(logical) {
        return false;
    }
    let Some(preview) = region
        .selection_geometry()
        .map(RegionSelectionGeometry::display_selection)
    else {
        return false;
    };
    if !input_state.start_region_selection(owner, preview.start) {
        return false;
    }
    input_state.update_region_selection(owner, preview.end);
    true
}

fn update_region_selection_event(
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
    }
    if !input_state.region_selection_is_owned_by(owner) {
        return;
    }
    if input_state.region_state().is_review() {
        if let Some(region) = backend.as_mut()
            && region.update_review_move(logical)
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
        && let Some(preview) = region
            .selection_geometry()
            .map(RegionSelectionGeometry::display_selection)
    {
        input_state.update_region_selection(owner, preview.end);
    }
}

fn sync_region_square_modifier_event(
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

fn initial_square_modifier(purpose: RegionPurposeTag, shift: bool) -> bool {
    shift && purpose.selection_policy().allow_square()
}

fn rearm_region_selection_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
) {
    if let Some(ActiveScreenRegion::Ready {
        anchor,
        raw_edge,
        logical_anchor,
        logical_edge,
        ..
    }) = backend.as_mut()
    {
        *anchor = None;
        *raw_edge = None;
        *logical_anchor = None;
        *logical_edge = None;
    }
    input_state.rearm_region_selection();
}

fn region_owner_lost_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    source: RegionInputSource,
) -> RegionOwnerLoss {
    if !input_state.region_selection_is_owned_by(source) {
        return RegionOwnerLoss::NotOwned;
    }
    let Some(purpose) = backend.as_ref().map(|region| region.purpose()) else {
        return RegionOwnerLoss::NotOwned;
    };
    if !purpose.is_capture() {
        return RegionOwnerLoss::Cancel(purpose);
    }
    if input_state.region_state().is_review() {
        let finished_backend = backend
            .as_mut()
            .is_some_and(ActiveScreenRegion::finish_review_move);
        let finished_ui = input_state.finish_region_review_move(source);
        debug_assert_eq!(finished_backend, finished_ui);
        return RegionOwnerLoss::RearmedCapture;
    }
    rearm_region_selection_event(backend, input_state);
    RegionOwnerLoss::RearmedCapture
}

pub(super) fn finalize_region_selection_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    owner: RegionInputSource,
    logical: (f64, f64),
) -> RegionSelectionFinalize {
    if !input_state.region_selection_is_owned_by(owner) {
        return RegionSelectionFinalize::NotOwned;
    }
    if input_state.region_state().is_review() {
        update_region_selection_event(backend, input_state, owner, logical);
        let finished_backend = backend
            .as_mut()
            .is_some_and(ActiveScreenRegion::finish_review_move);
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

impl WaylandState {
    pub(super) fn next_screen_region_generation(&mut self) -> u64 {
        let generation = self.data.next_screen_region_generation;
        self.data.next_screen_region_generation = generation
            .checked_add(1)
            .expect("screen region generation space exhausted");
        generation
    }

    pub(super) fn set_pending_screen_region(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        source: ScreenCaptureSource,
        acquisition: Option<ScreenAcquisitionId>,
    ) {
        self.data.active_screen_region = Some(match source {
            ScreenCaptureSource::Frozen => ActiveScreenRegion::PendingFrozen {
                purpose,
                generation,
                acquisition: acquisition.expect("frozen region wait has an acquisition id"),
            },
            ScreenCaptureSource::Zoom => ActiveScreenRegion::PendingZoom {
                purpose,
                generation,
            },
        });
        self.input_state
            .set_region_pending_capture(purpose, generation, source);
        self.debug_assert_screen_region_invariant();
    }

    pub(super) fn activate_screen_region(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        freeze_ownership: FreezeOwnership,
    ) -> bool {
        let Some(source) = super::screen_image::displayed_screen_image(
            &self.zoom,
            &self.frozen,
            self.input_state.board_is_transparent(),
        ) else {
            return false;
        };
        let Some(token) = current_screen_source_token(
            &source,
            &self.zoom,
            &self.frozen,
            (self.surface.width(), self.surface.height()),
        ) else {
            return false;
        };
        self.data.active_screen_region = Some(ActiveScreenRegion::Ready {
            purpose,
            generation,
            source: token,
            freeze_ownership,
            anchor: None,
            raw_edge: None,
            logical_anchor: None,
            logical_edge: None,
            square_modifier: initial_square_modifier(purpose, self.input_state.modifiers.shift),
            legend_dismissed: false,
        });
        self.input_state.activate_region(purpose, generation);
        self.debug_assert_screen_region_invariant();
        true
    }

    pub(super) fn clear_screen_region_ui_only(&mut self) {
        self.data.active_screen_region = None;
        self.input_state.cancel_region_ui_only();
        self.debug_assert_screen_region_invariant();
    }

    pub(in crate::backend::wayland) fn begin_region_selection(
        &mut self,
        owner: RegionInputSource,
        x: f64,
        y: f64,
    ) -> bool {
        self.cancel_screen_modals_if_source_changed();
        begin_region_selection_event(
            &mut self.data.active_screen_region,
            &mut self.input_state,
            owner,
            (x, y),
        )
    }

    pub(in crate::backend::wayland) fn update_region_selection(
        &mut self,
        owner: RegionInputSource,
        x: f64,
        y: f64,
    ) {
        self.cancel_screen_modals_if_source_changed();
        update_region_selection_event(
            &mut self.data.active_screen_region,
            &mut self.input_state,
            owner,
            (x, y),
        );
    }

    pub(in crate::backend::wayland) fn region_selection_geometry(
        &self,
    ) -> Option<RegionSelectionGeometry> {
        self.data
            .active_screen_region
            .and_then(ActiveScreenRegion::selection_geometry)
    }

    pub(super) fn region_picker_source_token(&self) -> Option<ScreenSourceToken> {
        match self.data.active_screen_region {
            Some(ActiveScreenRegion::Ready { source, .. }) => Some(source),
            Some(ActiveScreenRegion::PendingFrozen { .. })
            | Some(ActiveScreenRegion::PendingZoom { .. })
            | None => None,
        }
    }

    pub(in crate::backend::wayland) fn region_picker_legend_dismissed(&self) -> bool {
        self.data
            .active_screen_region
            .is_some_and(ActiveScreenRegion::legend_dismissed)
    }

    pub(in crate::backend::wayland) fn whole_image_region_selection(
        &self,
    ) -> Option<RegionSelectionFinalize> {
        self.data
            .active_screen_region
            .and_then(ActiveScreenRegion::whole_image_selection)
    }

    pub(in crate::backend::wayland) fn enter_region_review(
        &mut self,
        rect: ImagePixelRect,
    ) -> bool {
        let Some(region) = self.data.active_screen_region.as_mut() else {
            return false;
        };
        let purpose = region.purpose();
        let generation = region.generation();
        let Some(display) = region.enter_review(rect) else {
            return false;
        };
        self.input_state
            .activate_region_review(purpose, generation, display);
        self.debug_assert_screen_region_invariant();
        true
    }

    pub(in crate::backend::wayland) fn nudge_region_review(
        &mut self,
        delta_x: i64,
        delta_y: i64,
    ) -> bool {
        self.cancel_screen_modals_if_source_changed();
        if !self.input_state.region_state().is_review() {
            return false;
        }
        let Some(display) = self
            .data
            .active_screen_region
            .as_mut()
            .and_then(|region| region.nudge_review(delta_x, delta_y))
        else {
            return false;
        };
        self.input_state.update_region_review_display(display);
        true
    }

    pub(in crate::backend::wayland) fn region_review_rect(&self) -> Option<ImagePixelRect> {
        match self.data.active_screen_region {
            Some(region) if self.input_state.region_state().is_review() => {
                region.stored_review_rect()
            }
            _ => None,
        }
    }

    pub(in crate::backend::wayland) fn region_picker_measurement(
        &self,
        pointer: (f64, f64),
    ) -> Option<RegionPickerMeasurement> {
        self.data
            .active_screen_region
            .and_then(|region| region.picker_measurement(pointer))
    }

    /// Recompute capture geometry from compositor-authoritative Shift state.
    /// Individual key events remain swallowed by the modal selector.
    pub(in crate::backend::wayland) fn sync_region_square_modifier(&mut self, shift: bool) -> bool {
        sync_region_square_modifier_event(
            &mut self.data.active_screen_region,
            &mut self.input_state,
            shift,
        )
    }

    /// Resolve loss of the device that owns a drag without crossing lifecycle
    /// ownership. Capture keeps its reservation and image; OCR asks its owner
    /// to run the existing terminal cancellation path.
    pub(in crate::backend::wayland) fn region_owner_lost(
        &mut self,
        source: RegionInputSource,
    ) -> RegionOwnerLoss {
        region_owner_lost_event(
            &mut self.data.active_screen_region,
            &mut self.input_state,
            source,
        )
    }

    pub(super) fn debug_assert_screen_region_invariant(&self) {
        debug_assert!(screen_region_invariant(
            self.data.active_screen_region,
            self.input_state.region_state(),
        ));
    }

    pub(in crate::backend::wayland) fn cancel_screen_modals_if_source_changed(&mut self) {
        let (cancel_eyedropper, cancel_region) = {
            let current_source = super::screen_image::displayed_screen_image(
                &self.zoom,
                &self.frozen,
                self.input_state.board_is_transparent(),
            );
            let source_matches = |expected| {
                current_source.as_ref().is_some_and(|source| {
                    screen_source_is(
                        &expected,
                        source,
                        &self.zoom,
                        &self.frozen,
                        (self.surface.width(), self.surface.height()),
                    )
                })
            };
            (
                active_eyedropper_source_changed(
                    self.input_state.eyedropper_is_active(),
                    self.data.active_eyedropper_source,
                    &source_matches,
                ),
                active_region_source_changed(self.data.active_screen_region, &source_matches),
            )
        };
        if cancel_eyedropper {
            self.cancel_eyedropper();
        }
        if cancel_region {
            match self
                .data
                .active_screen_region
                .map(ActiveScreenRegion::purpose)
            {
                Some(RegionPurposeTag::Ocr) => {
                    self.cancel_ocr();
                }
                Some(purpose) if purpose.is_capture() => {
                    self.cancel_region_capture_for_source_change();
                }
                Some(_) | None => {}
            }
        }
    }
}

fn screen_region_invariant(backend: Option<ActiveScreenRegion>, ui: RegionSelectUiState) -> bool {
    match (backend, ui) {
        (None, RegionSelectUiState::Inactive) => true,
        (None, _) | (Some(_), RegionSelectUiState::Inactive) => false,
        (Some(region), ui) => {
            ui.generation() == Some(region.generation()) && ui.purpose() == Some(region.purpose())
        }
    }
}

fn active_region_source_changed(
    region: Option<ActiveScreenRegion>,
    source_matches: &impl Fn(ScreenSourceToken) -> bool,
) -> bool {
    matches!(
        region,
        Some(ActiveScreenRegion::Ready { source, .. }) if !source_matches(source)
    )
}

fn active_eyedropper_source_changed(
    active: bool,
    expected_source: Option<ScreenSourceToken>,
    source_matches: &impl Fn(ScreenSourceToken) -> bool,
) -> bool {
    active && !expected_source.is_some_and(source_matches)
}

pub(super) fn owned_generation_is_current(
    expected: u64,
    current: u64,
    frozen_active: bool,
) -> bool {
    frozen_active && expected == current
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn pending_and_ready_region_state_preserve_generation_and_freeze_ownership() {
        let pending = ActiveScreenRegion::PendingZoom {
            purpose: RegionPurposeTag::Ocr,
            generation: 9,
        };
        assert_eq!(pending.generation(), 9);
        assert_eq!(pending.purpose(), RegionPurposeTag::Ocr);
        assert_eq!(pending.owned_frozen_generation(), None);

        let ready = ActiveScreenRegion::Ready {
            purpose: RegionPurposeTag::Ocr,
            generation: 9,
            source: ScreenSourceToken {
                output_id: 1,
                output_layout_generation: 0,
                kind: super::super::screen_image::ScreenImageKind::Frozen,
                image_generation: 44,
                image_size: (100, 80),
                stride: 400,
                surface: (100, 80),
                output_scale: 1,
                output_transform: wayland_client::protocol::wl_output::Transform::Normal,
                zoom_transformed: false,
                zoom_scale: 1.0,
                zoom_view_offset: (0.0, 0.0),
            },
            freeze_ownership: FreezeOwnership::PickerOwned {
                image_generation: 44,
            },
            anchor: None,
            raw_edge: None,
            logical_anchor: None,
            logical_edge: None,
            square_modifier: false,
            legend_dismissed: false,
        };
        assert_eq!(ready.generation(), pending.generation());
        assert_eq!(ready.owned_frozen_generation(), Some(44));
    }

    #[test]
    fn backend_and_ui_generation_must_match() {
        let backend = Some(ActiveScreenRegion::PendingZoom {
            purpose: RegionPurposeTag::Ocr,
            generation: 7,
        });
        assert!(screen_region_invariant(
            backend,
            RegionSelectUiState::PendingCapture {
                purpose: RegionPurposeTag::Ocr,
                generation: 7,
                source: ScreenCaptureSource::Zoom,
            }
        ));
        assert!(!screen_region_invariant(
            backend,
            RegionSelectUiState::PendingCapture {
                purpose: RegionPurposeTag::Ocr,
                generation: 8,
                source: ScreenCaptureSource::Zoom,
            }
        ));
        assert!(!screen_region_invariant(
            backend,
            RegionSelectUiState::Inactive
        ));
    }

    #[test]
    fn armed_freeze_release_is_generation_checked_and_consumed_once() {
        assert!(owned_generation_is_current(44, 44, true));
        assert!(!owned_generation_is_current(44, 45, true));
        assert!(!owned_generation_is_current(44, 44, false));

        let mut owned = ocr_region(1.0);
        if let ActiveScreenRegion::Ready {
            freeze_ownership, ..
        } = &mut owned
        {
            *freeze_ownership = FreezeOwnership::PickerOwned {
                image_generation: 44,
            };
        }
        let mut armed = Some(owned);
        let first = armed
            .take()
            .and_then(ActiveScreenRegion::owned_frozen_generation);
        let second = armed
            .take()
            .and_then(ActiveScreenRegion::owned_frozen_generation);
        assert_eq!(first, Some(44));
        assert_eq!(
            second, None,
            "a second cancellation has no region to release"
        );
    }

    fn ocr_region(scale: f64) -> ActiveScreenRegion {
        ActiveScreenRegion::Ready {
            purpose: RegionPurposeTag::Ocr,
            generation: 1,
            source: ScreenSourceToken {
                output_id: 1,
                output_layout_generation: 0,
                kind: super::super::screen_image::ScreenImageKind::Frozen,
                image_generation: 1,
                image_size: ((100.0 * scale) as u32, (80.0 * scale) as u32),
                stride: (400.0 * scale) as i32,
                surface: (100, 80),
                output_scale: 1,
                output_transform: wayland_client::protocol::wl_output::Transform::Normal,
                zoom_transformed: false,
                zoom_scale: 1.0,
                zoom_view_offset: (0.0, 0.0),
            },
            freeze_ownership: FreezeOwnership::PreExisting,
            anchor: None,
            raw_edge: None,
            logical_anchor: None,
            logical_edge: None,
            square_modifier: false,
            legend_dismissed: false,
        }
    }

    #[test]
    fn ready_regions_detect_replaced_or_missing_source_but_pending_regions_keep_waiting() {
        let ready = ocr_region(1.0);
        let ActiveScreenRegion::Ready { source, .. } = ready else {
            unreachable!("OCR fixture is ready")
        };
        let mut replacement = source;
        replacement.image_generation += 1;

        assert!(!active_region_source_changed(Some(ready), &|expected| {
            expected == source
        }));
        assert!(active_region_source_changed(Some(ready), &|expected| {
            expected == replacement
        }));
        assert!(active_region_source_changed(Some(ready), &|_| false));
        assert!(!active_region_source_changed(
            Some(ActiveScreenRegion::PendingZoom {
                purpose: RegionPurposeTag::Ocr,
                generation: 1,
            }),
            &|_| false,
        ));

        let capture = capture_region();
        assert!(active_region_source_changed(Some(capture), &|_| false));
    }

    #[test]
    fn active_eyedropper_detects_replaced_missing_or_untracked_source() {
        let ActiveScreenRegion::Ready { source, .. } = ocr_region(1.0) else {
            unreachable!("OCR fixture is ready")
        };
        let mut replacement = source;
        replacement.image_generation += 1;

        assert!(!active_eyedropper_source_changed(
            true,
            Some(source),
            &|expected| expected == source,
        ));
        assert!(active_eyedropper_source_changed(
            true,
            Some(source),
            &|expected| expected == replacement,
        ));
        assert!(active_eyedropper_source_changed(
            true,
            Some(source),
            &|_| false,
        ));
        assert!(active_eyedropper_source_changed(true, None, &|_| true));
        assert!(!active_eyedropper_source_changed(
            false,
            Some(source),
            &|_| false,
        ));
    }

    #[test]
    fn ocr_three_logical_pixels_rearms_but_exactly_four_submits() {
        let mut region = ocr_region(1.0);
        assert!(region.begin_selection((10.0, 20.0)));
        assert!(region.update_endpoint((13.0, 23.0)));
        assert_eq!(region.selection_rect(), None);

        assert!(region.update_endpoint((14.0, 24.0)));
        assert_eq!(
            region
                .selection_rect()
                .map(|rect| (rect.x(), rect.y(), rect.size())),
            Some((10, 20, (4, 4)))
        );
    }

    #[test]
    fn ocr_release_endpoint_without_motion_matches_forward_and_reversed_scale_oracle() {
        for (scale, expected_forward, expected_reversed) in [
            (1.0, (10, 20, (5, 5)), (10, 20, (5, 5))),
            (2.0, (20, 41, (9, 8)), (20, 41, (9, 8))),
            (1.5, (15, 30, (7, 7)), (15, 30, (7, 7))),
        ] {
            let mut forward = ocr_region(scale);
            assert!(forward.begin_selection((10.25, 20.5)));
            assert!(forward.update_endpoint((14.25, 24.5)));
            let forward_rect = forward.selection_rect().unwrap();
            assert_eq!(
                (forward_rect.x(), forward_rect.y(), forward_rect.size()),
                expected_forward,
                "forward at scale {scale}"
            );

            let mut reversed = ocr_region(scale);
            assert!(reversed.begin_selection((14.25, 24.5)));
            assert!(reversed.update_endpoint((10.25, 20.5)));
            let reversed_rect = reversed.selection_rect().unwrap();
            assert_eq!(
                (reversed_rect.x(), reversed_rect.y(), reversed_rect.size()),
                expected_reversed,
                "reversed at scale {scale}"
            );
        }
    }

    #[test]
    fn ocr_release_replaces_the_last_motion_endpoint() {
        let mut region = ocr_region(1.0);
        assert!(region.begin_selection((10.0, 20.0)));
        assert!(region.update_endpoint((18.0, 32.0)));
        assert_eq!(region.selection_rect().unwrap().size(), (8, 12));

        assert!(region.update_endpoint((14.0, 24.0)));
        assert_eq!(region.selection_rect().unwrap().size(), (4, 4));
    }

    #[test]
    fn second_device_press_cannot_replace_an_in_progress_backend_region() {
        let mut region = ocr_region(1.0);
        assert!(region.begin_selection((10.0, 20.0)));
        assert!(region.update_endpoint((18.0, 28.0)));

        assert!(!region.begin_selection((50.0, 60.0)));
        assert_eq!(
            region
                .selection_rect()
                .map(|rect| (rect.x(), rect.y(), rect.size())),
            Some((10, 20, (8, 8)))
        );
    }

    #[test]
    fn ocr_policy_keeps_shift_held_drag_rectangular() {
        let mut region = ocr_region(1.0);
        assert!(region.begin_selection((10.0, 20.0)));
        assert!(region.update_endpoint((14.0, 28.0)));
        let rect = region.selection_rect().unwrap();
        assert_eq!(rect.size(), (4, 8));
        assert!(!RegionPurposeTag::Ocr.selection_policy().allow_square());
    }

    #[test]
    fn held_shift_seeds_capture_but_not_ocr_before_the_first_press() {
        assert!(initial_square_modifier(
            RegionPurposeTag::CaptureDeliver,
            true
        ));
        assert!(initial_square_modifier(
            RegionPurposeTag::CaptureInteractive,
            true
        ));
        assert!(!initial_square_modifier(RegionPurposeTag::Ocr, true));
        assert!(!initial_square_modifier(
            RegionPurposeTag::CaptureDeliver,
            false
        ));
    }

    #[test]
    fn production_ocr_event_adapter_uses_release_endpoint_at_every_scale() {
        for (scale, expected) in [
            (1.0, (10, 20, (5, 5))),
            (2.0, (20, 41, (9, 8))),
            (1.5, (15, 30, (7, 7))),
        ] {
            for reversed in [false, true] {
                for has_motion in [false, true] {
                    let (press, release) = if reversed {
                        ((14.25, 24.5), (10.25, 20.5))
                    } else {
                        ((10.25, 20.5), (14.25, 24.5))
                    };
                    let mut backend = Some(ocr_region(scale));
                    let mut input = make_test_input_state();
                    input.activate_region(RegionPurposeTag::Ocr, 1);

                    assert!(begin_region_selection_event(
                        &mut backend,
                        &mut input,
                        RegionInputSource::Pointer,
                        press,
                    ));
                    if has_motion {
                        update_region_selection_event(
                            &mut backend,
                            &mut input,
                            RegionInputSource::Pointer,
                            (30.0, 35.0),
                        );
                    }
                    let RegionSelectionFinalize::Selected {
                        purpose: RegionPurposeTag::Ocr,
                        rect,
                    } = finalize_region_selection_event(
                        &mut backend,
                        &mut input,
                        RegionInputSource::Pointer,
                        release,
                    )
                    else {
                        panic!(
                            "release must submit at scale={scale} reversed={reversed} motion={has_motion}"
                        );
                    };

                    assert_eq!(
                        (rect.x(), rect.y(), rect.size()),
                        expected,
                        "scale={scale} reversed={reversed} motion={has_motion}"
                    );
                    assert_eq!(
                        input
                            .region_state()
                            .selection()
                            .map(|selection| selection.end),
                        Some(release),
                        "the UI preview and crop adapter diverged"
                    );
                }
            }
        }
    }

    #[test]
    fn production_ocr_event_adapter_rearms_small_drag_and_ignores_shift_square_policy() {
        let mut backend = Some(ocr_region(1.0));
        let mut input = make_test_input_state();
        input.activate_region(RegionPurposeTag::Ocr, 1);
        input.sync_modifiers(true, false, false, false);

        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (10.0, 20.0),
        ));
        let RegionSelectionFinalize::Selected {
            purpose: RegionPurposeTag::Ocr,
            rect,
        } = finalize_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (14.0, 28.0),
        )
        else {
            panic!("Shift-held OCR drag must submit");
        };
        assert_eq!(rect.size(), (4, 8));

        let mut backend = Some(ocr_region(1.0));
        let mut input = make_test_input_state();
        input.activate_region(RegionPurposeTag::Ocr, 1);
        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (10.0, 20.0),
        ));
        assert_eq!(
            finalize_region_selection_event(
                &mut backend,
                &mut input,
                RegionInputSource::Pointer,
                (13.0, 23.0),
            ),
            RegionSelectionFinalize::Rearmed
        );
        assert!(matches!(
            input.region_state(),
            RegionSelectUiState::Armed { .. }
        ));
    }

    fn capture_region() -> ActiveScreenRegion {
        capture_region_at_scale(1.0)
    }

    fn interactive_region() -> ActiveScreenRegion {
        let mut region = capture_region();
        if let ActiveScreenRegion::Ready { purpose, .. } = &mut region {
            *purpose = RegionPurposeTag::CaptureInteractive;
        }
        region
    }

    fn capture_region_at_scale(scale: f64) -> ActiveScreenRegion {
        let ActiveScreenRegion::Ready {
            generation,
            source,
            freeze_ownership,
            ..
        } = ocr_region(scale)
        else {
            unreachable!("OCR fixture is ready")
        };
        ActiveScreenRegion::Ready {
            purpose: RegionPurposeTag::CaptureDeliver,
            generation,
            source,
            freeze_ownership,
            anchor: None,
            raw_edge: None,
            logical_anchor: None,
            logical_edge: None,
            square_modifier: false,
            legend_dismissed: false,
        }
    }

    #[test]
    fn capture_press_snaps_its_anchor_and_starts_with_a_zero_pixel_span() {
        let mut region = capture_region();

        assert!(region.begin_selection((10.8, 20.6)));

        let geometry = region.selection_geometry().expect("fresh drag geometry");
        assert_eq!(geometry.image_span().size(), (0, 0));
        assert_eq!(geometry.display_selection().start, (10.0, 20.0));
        assert_eq!(geometry.display_selection().end, (10.0, 20.0));
        assert_eq!(geometry.image_rect(), None);
    }

    #[test]
    fn interactive_release_enters_integer_authoritative_review() {
        let mut backend = Some(interactive_region());
        let mut input = make_test_input_state();
        input.activate_region(RegionPurposeTag::CaptureInteractive, 1);

        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (10.2, 20.7),
        ));
        assert_eq!(
            finalize_region_selection_event(
                &mut backend,
                &mut input,
                RegionInputSource::Pointer,
                (30.1, 42.2),
            ),
            RegionSelectionFinalize::Reviewed
        );
        assert!(input.region_state().is_review());
        let rect = backend
            .and_then(ActiveScreenRegion::selection_rect)
            .expect("review owns a non-empty pixel rectangle");
        assert_eq!(
            (rect.x(), rect.y(), rect.width(), rect.height()),
            (10, 20, 21, 23)
        );
        assert_eq!(
            input.region_state().selection(),
            backend
                .and_then(ActiveScreenRegion::selection_geometry)
                .map(|geometry| geometry.display_selection())
        );
    }

    #[test]
    fn capture_hover_motion_requests_a_repaint_while_armed_and_in_review() {
        let mut armed_backend = Some(capture_region());
        let mut armed_input = make_test_input_state();
        armed_input.activate_region(RegionPurposeTag::CaptureDeliver, 1);
        let _ = armed_input.dirty_tracker.take_region_report(100, 80);
        armed_input.needs_redraw = false;

        update_region_selection_event(
            &mut armed_backend,
            &mut armed_input,
            RegionInputSource::Pointer,
            (35.0, 45.0),
        );

        assert!(armed_input.needs_redraw, "armed crosshair follows hover");
        assert_eq!(
            armed_input
                .dirty_tracker
                .take_region_report(100, 80)
                .regions,
            vec![crate::util::Rect::new(0, 0, 100, 80).unwrap()]
        );

        let mut review_region = interactive_region();
        let rect = ImagePixelRect::new(20, 20, 30, 25, (100, 80)).unwrap();
        let display = review_region.enter_review(rect).unwrap();
        let mut review_backend = Some(review_region);
        let mut review_input = make_test_input_state();
        review_input.activate_region_review(RegionPurposeTag::CaptureInteractive, 1, display);
        let _ = review_input.dirty_tracker.take_region_report(100, 80);
        review_input.needs_redraw = false;

        update_region_selection_event(
            &mut review_backend,
            &mut review_input,
            RegionInputSource::Pointer,
            (70.0, 70.0),
        );

        assert!(
            review_input.needs_redraw,
            "review hover, action bar, and loupe follow the pointer"
        );
        assert_eq!(
            review_input
                .dirty_tracker
                .take_region_report(100, 80)
                .regions,
            vec![crate::util::Rect::new(0, 0, 100, 80).unwrap()]
        );
    }

    #[test]
    fn review_nudge_and_move_clamp_without_resizing_and_owner_loss_keeps_review() {
        let mut region = interactive_region();
        let rect = ImagePixelRect::new(70, 60, 20, 15, (100, 80)).unwrap();
        let display = region.enter_review(rect).unwrap();
        let mut backend = Some(region);
        let mut input = make_test_input_state();
        input.activate_region_review(RegionPurposeTag::CaptureInteractive, 1, display);

        let nudged = backend
            .as_mut()
            .and_then(|region| region.nudge_review(50, 50))
            .unwrap();
        input.update_region_review_display(nudged);
        let clamped = backend
            .and_then(ActiveScreenRegion::selection_rect)
            .unwrap();
        assert_eq!(
            (clamped.x(), clamped.y(), clamped.size()),
            (80, 65, (20, 15))
        );

        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Stylus,
            (85.0, 70.0),
        ));
        update_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Stylus,
            (20.0, 20.0),
        );
        assert_eq!(
            region_owner_lost_event(&mut backend, &mut input, RegionInputSource::Stylus),
            RegionOwnerLoss::RearmedCapture
        );
        assert!(input.region_state().is_review());
        assert!(input.region_state().selection_owner().is_none());
        assert!(
            backend
                .and_then(ActiveScreenRegion::selection_rect)
                .is_some()
        );
    }

    #[test]
    fn review_move_preserves_subpixel_motion_until_it_reaches_a_pixel() {
        let mut region = interactive_region();
        let rect = ImagePixelRect::new(20, 20, 30, 25, (100, 80)).unwrap();
        region.enter_review(rect).unwrap();
        assert!(region.begin_review_move((25.0, 25.0)));

        for x in [25.6, 26.2, 26.8, 27.4, 28.0] {
            region.update_review_move((x, 25.0));
        }

        let moved = region.stored_review_rect().unwrap();
        assert_eq!(
            (moved.x(), moved.y(), moved.size()),
            (23, 20, (30, 25)),
            "three logical pixels of motion must move exactly three image pixels"
        );
    }

    #[test]
    fn second_device_press_cannot_replace_an_in_progress_review_move() {
        let mut region = interactive_region();
        let rect = ImagePixelRect::new(20, 20, 30, 25, (100, 80)).unwrap();
        let display = region.enter_review(rect).unwrap();
        let mut backend = Some(region);
        let mut input = make_test_input_state();
        input.activate_region_review(RegionPurposeTag::CaptureInteractive, 1, display);

        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (25.0, 25.0),
        ));
        assert!(!begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Touch,
            (70.0, 70.0),
        ));

        assert!(input.region_state().is_review());
        assert!(input.region_selection_is_owned_by(RegionInputSource::Pointer));
        assert_eq!(
            backend.and_then(ActiveScreenRegion::selection_rect),
            Some(rect)
        );
    }

    #[test]
    fn capture_pixel_span_reports_one_axis_empty_without_submitting_it() {
        let mut region = capture_region();
        assert!(region.begin_selection((10.8, 20.6)));
        assert!(region.update_endpoint((10.0, 40.0)));

        let geometry = region.selection_geometry().expect("drag geometry");
        assert_eq!(geometry.image_span().size(), (0, 20));
        assert_eq!(geometry.image_rect(), None);
        assert_eq!(region.selection_rect(), None);
    }

    #[test]
    fn capture_finalize_is_purpose_aware_and_one_axis_empty_rearms() {
        let mut backend = Some(capture_region());
        let mut input = make_test_input_state();
        input.activate_region(RegionPurposeTag::CaptureDeliver, 1);
        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (10.8, 20.6),
        ));
        assert_eq!(
            finalize_region_selection_event(
                &mut backend,
                &mut input,
                RegionInputSource::Pointer,
                (18.2, 35.8),
            ),
            RegionSelectionFinalize::Selected {
                purpose: RegionPurposeTag::CaptureDeliver,
                rect: ImagePixelRect::new(10, 20, 9, 16, (100, 80)).unwrap(),
            }
        );

        let mut backend = Some(capture_region());
        let mut input = make_test_input_state();
        input.activate_region(RegionPurposeTag::CaptureDeliver, 1);
        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Touch,
            (10.8, 20.6),
        ));
        assert_eq!(
            finalize_region_selection_event(
                &mut backend,
                &mut input,
                RegionInputSource::Touch,
                (10.0, 40.0),
            ),
            RegionSelectionFinalize::Rearmed
        );
        assert!(matches!(
            input.region_state(),
            RegionSelectUiState::Armed {
                purpose: RegionPurposeTag::CaptureDeliver,
                generation: 1,
            }
        ));
        assert_eq!(backend.unwrap().selection_geometry(), None);
    }

    #[test]
    fn capture_shift_square_uses_the_dominant_image_axis_and_restores_the_raw_edge() {
        let mut region = capture_region();
        assert!(region.begin_selection((10.8, 20.6)));
        assert!(region.update_endpoint((18.2, 35.8)));
        assert_eq!(
            region.selection_geometry().unwrap().image_span().size(),
            (9, 16)
        );

        assert!(region.set_square_modifier(true));
        assert_eq!(
            region.selection_geometry().unwrap().image_span().size(),
            (16, 16)
        );

        assert!(region.set_square_modifier(false));
        assert_eq!(
            region.selection_geometry().unwrap().image_span().size(),
            (9, 16),
            "releasing Shift must recompute from the canonical raw edge"
        );
    }

    #[test]
    fn capture_shift_square_caps_the_side_to_the_first_image_edge() {
        let mut region = capture_region();
        assert!(region.begin_selection((90.8, 70.4)));
        assert!(region.update_endpoint((10.0, 40.0)));
        assert!(region.set_square_modifier(true));

        let geometry = region.selection_geometry().unwrap();
        assert_eq!(geometry.image_span().size(), (70, 70));
        assert_eq!(geometry.image_span().x(), 20);
        assert_eq!(geometry.image_span().y(), 0);
    }

    #[test]
    fn capture_square_and_readout_share_image_pixels_at_integer_fractional_and_zoom_views() {
        let mut cases = [
            ("scale-1", capture_region_at_scale(1.0)),
            ("scale-2", capture_region_at_scale(2.0)),
            ("fractional-1.5", capture_region_at_scale(1.5)),
            ("zoom-2", capture_region_at_scale(1.0)),
        ];
        if let ActiveScreenRegion::Ready { source, .. } = &mut cases[3].1 {
            source.zoom_transformed = true;
            source.zoom_scale = 2.0;
            source.zoom_view_offset = (10.0, 5.0);
        }

        for (name, mut region) in cases {
            assert!(region.begin_selection((20.25, 20.5)), "{name}");
            assert!(region.update_endpoint((40.25, 50.5)), "{name}");
            assert!(region.set_square_modifier(true), "{name}");

            let geometry = region.selection_geometry().expect("square geometry");
            let (width, height) = geometry.image_span().size();
            assert_eq!(
                width, height,
                "square must be square in image pixels: {name}"
            );
            assert!(width > 0, "test drag must cover pixels: {name}");
            assert_eq!(
                region.picker_measurement((0.0, 0.0)),
                Some(RegionPickerMeasurement::Size { width, height }),
                "readout must describe the exact square crop: {name}"
            );
            assert_eq!(
                geometry.image_rect().map(ImagePixelRect::size),
                Some((width, height)),
                "submitted crop must share the readout's pixel span: {name}"
            );
        }
    }

    #[test]
    fn capture_measurement_maps_armed_pointer_and_reports_exact_selecting_span() {
        let mut region = capture_region_at_scale(2.0);
        assert_eq!(
            region.picker_measurement((10.25, 20.5)),
            Some(RegionPickerMeasurement::Point { x: 20, y: 41 })
        );

        assert!(region.begin_selection((10.25, 20.5)));
        assert_eq!(
            region.picker_measurement((999.0, 999.0)),
            Some(RegionPickerMeasurement::Size {
                width: 0,
                height: 0,
            })
        );
        assert!(region.update_endpoint((14.25, 24.5)));
        assert_eq!(
            region.picker_measurement((0.0, 0.0)),
            Some(RegionPickerMeasurement::Size {
                width: 9,
                height: 8,
            })
        );
        assert_eq!(ocr_region(2.0).picker_measurement((10.25, 20.5)), None);
    }

    #[test]
    fn compositor_shift_sync_recomputes_capture_preview_without_changing_ownership() {
        let mut backend = Some(capture_region());
        let mut input = make_test_input_state();
        input.activate_region(RegionPurposeTag::CaptureDeliver, 1);

        assert!(sync_region_square_modifier_event(
            &mut backend,
            &mut input,
            true,
        ));
        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Touch,
            (10.8, 20.6),
        ));
        update_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Touch,
            (18.2, 35.8),
        );
        let square = backend.unwrap().selection_geometry().unwrap();
        assert_eq!(square.image_span().size(), (16, 16));
        assert_eq!(
            input.region_state().selection().unwrap(),
            square.display_selection()
        );
        assert!(input.region_selection_is_owned_by(RegionInputSource::Touch));

        assert!(sync_region_square_modifier_event(
            &mut backend,
            &mut input,
            false,
        ));
        let raw = backend.unwrap().selection_geometry().unwrap();
        assert_eq!(raw.image_span().size(), (9, 16));
        assert_eq!(
            input.region_state().selection().unwrap(),
            raw.display_selection()
        );
        assert!(input.region_selection_is_owned_by(RegionInputSource::Touch));
    }

    #[test]
    fn capture_owner_loss_rearms_without_releasing_backend_ownership() {
        let mut backend = Some(capture_region());
        let mut input = make_test_input_state();
        input.activate_region(RegionPurposeTag::CaptureDeliver, 1);
        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Stylus,
            (10.0, 20.0),
        ));
        assert!(
            backend.is_some_and(ActiveScreenRegion::legend_dismissed),
            "the first press permanently dismisses this picker's legend"
        );
        update_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Stylus,
            (30.0, 40.0),
        );

        assert_eq!(
            region_owner_lost_event(&mut backend, &mut input, RegionInputSource::Touch),
            RegionOwnerLoss::NotOwned
        );
        assert!(input.region_selection_is_owned_by(RegionInputSource::Stylus));

        assert_eq!(
            region_owner_lost_event(&mut backend, &mut input, RegionInputSource::Stylus),
            RegionOwnerLoss::RearmedCapture
        );
        assert!(matches!(
            input.region_state(),
            RegionSelectUiState::Armed {
                purpose: RegionPurposeTag::CaptureDeliver,
                generation: 1,
            }
        ));
        assert!(backend.is_some(), "capture source ownership was released");
        let backend = backend.unwrap();
        assert_eq!(backend.selection_geometry(), None);
        assert!(
            backend.legend_dismissed(),
            "rearming must not show the first-use legend again"
        );
    }

    #[test]
    fn ocr_owner_loss_requests_its_existing_terminal_cancel_path() {
        let mut backend = Some(ocr_region(1.0));
        let mut input = make_test_input_state();
        input.activate_region(RegionPurposeTag::Ocr, 1);
        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (10.0, 20.0),
        ));

        assert_eq!(
            region_owner_lost_event(&mut backend, &mut input, RegionInputSource::Pointer),
            RegionOwnerLoss::Cancel(RegionPurposeTag::Ocr)
        );
        assert!(
            input.region_selection_is_owned_by(RegionInputSource::Pointer),
            "the lifecycle owner must perform OCR cleanup"
        );
    }

    #[test]
    fn whole_image_is_available_only_to_capture_purposes() {
        let capture = capture_region();
        let RegionSelectionFinalize::Selected { purpose, rect } = capture
            .whole_image_selection()
            .expect("capture whole image")
        else {
            panic!("whole image must be a selected result")
        };
        assert_eq!(purpose, RegionPurposeTag::CaptureDeliver);
        assert_eq!((rect.x(), rect.y(), rect.size()), (0, 0, (100, 80)));
        assert_eq!(ocr_region(1.0).whole_image_selection(), None);
    }
}
