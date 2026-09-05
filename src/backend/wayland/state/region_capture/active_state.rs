use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland::state) enum FreezeOwnership {
    PreExisting,
    PickerOwned { image_generation: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland::state) enum RegionInteractionPhase {
    Armed,
    Selecting { owner: RegionInputSource },
    Review { owner: Option<RegionInputSource> },
    Measured,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::backend::wayland::state) enum ActiveScreenRegion {
    Measure {
        generation: u64,
        bounds: (u32, u32),
        anchor: Option<(f64, f64)>,
        edge: Option<(f64, f64)>,
        phase: RegionInteractionPhase,
    },
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
        include_drawings: bool,
        /// The grip a device is dragging in Review, if any. Mutually exclusive
        /// with `logical_anchor`, which owns a Review move-drag.
        review_resize: Option<ReviewResizeGrip>,
        phase: RegionInteractionPhase,
    },
}

impl ActiveScreenRegion {
    pub(super) fn ui_state(self) -> RegionSelectUiState {
        let purpose = self.purpose();
        let generation = self.generation();
        match self {
            Self::PendingFrozen { .. } => RegionSelectUiState::PendingCapture {
                purpose,
                generation,
                source: ScreenCaptureSource::Frozen,
            },
            Self::PendingZoom { .. } => RegionSelectUiState::PendingCapture {
                purpose,
                generation,
                source: ScreenCaptureSource::Zoom,
            },
            Self::Measure { phase, .. } | Self::Ready { phase, .. } => match phase {
                RegionInteractionPhase::Armed => RegionSelectUiState::Armed {
                    purpose,
                    generation,
                },
                RegionInteractionPhase::Selecting { owner } => {
                    let selection = self
                        .display_selection()
                        .expect("selecting screen region must own geometry");
                    RegionSelectUiState::Selecting {
                        purpose,
                        generation,
                        owner,
                        start: selection.start,
                        current: selection.end,
                    }
                }
                RegionInteractionPhase::Review { owner } => {
                    let display = self
                        .review_geometry()
                        .map(RegionSelectionGeometry::display_selection)
                        .expect("reviewing screen region must own geometry");
                    RegionSelectUiState::Review {
                        purpose,
                        generation,
                        display,
                        move_owner: owner,
                    }
                }
                RegionInteractionPhase::Measured => {
                    let display = self
                        .measure_selection()
                        .expect("completed measurement must own geometry");
                    RegionSelectUiState::Measured {
                        purpose,
                        generation,
                        display,
                    }
                }
            },
        }
    }

    pub(super) const fn phase(self) -> Option<RegionInteractionPhase> {
        match self {
            Self::Measure { phase, .. } | Self::Ready { phase, .. } => Some(phase),
            Self::PendingFrozen { .. } | Self::PendingZoom { .. } => None,
        }
    }

    pub(super) fn set_phase(&mut self, next: RegionInteractionPhase) -> bool {
        let phase = match self {
            Self::Measure { phase, .. } | Self::Ready { phase, .. } => phase,
            Self::PendingFrozen { .. } | Self::PendingZoom { .. } => return false,
        };
        if *phase == next {
            return false;
        }
        *phase = next;
        true
    }

    pub(super) fn selection_owner(self) -> Option<RegionInputSource> {
        match self.phase() {
            Some(RegionInteractionPhase::Selecting { owner })
            | Some(RegionInteractionPhase::Review { owner: Some(owner) }) => Some(owner),
            Some(
                RegionInteractionPhase::Armed
                | RegionInteractionPhase::Review { owner: None }
                | RegionInteractionPhase::Measured,
            )
            | None => None,
        }
    }

    pub const fn purpose(self) -> RegionPurposeTag {
        match self {
            Self::Measure { .. } => RegionPurposeTag::Measure,
            Self::PendingFrozen { purpose, .. }
            | Self::PendingZoom { purpose, .. }
            | Self::Ready { purpose, .. } => purpose,
        }
    }

    pub const fn generation(self) -> u64 {
        match self {
            Self::Measure { generation, .. }
            | Self::PendingFrozen { generation, .. }
            | Self::PendingZoom { generation, .. }
            | Self::Ready { generation, .. } => generation,
        }
    }

    pub const fn pending_acquisition(self) -> Option<ScreenAcquisitionId> {
        match self {
            Self::PendingFrozen { acquisition, .. } => Some(acquisition),
            Self::Measure { .. } | Self::PendingZoom { .. } | Self::Ready { .. } => None,
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
            | Self::Measure { .. }
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

    pub const fn include_drawings(self) -> bool {
        matches!(
            self,
            Self::Ready {
                include_drawings: true,
                ..
            }
        )
    }

    pub(super) fn toggle_include_drawings(&mut self) -> Option<bool> {
        let Self::Ready {
            purpose: RegionPurposeTag::CaptureInteractive,
            include_drawings,
            ..
        } = self
        else {
            return None;
        };
        *include_drawings = !*include_drawings;
        Some(*include_drawings)
    }

    pub(super) fn measure_selection(self) -> Option<RegionSelection> {
        let Self::Measure {
            anchor: Some(start),
            edge: Some(end),
            ..
        } = self
        else {
            return None;
        };
        Some(RegionSelection { start, end })
    }

    pub(super) fn display_selection(self) -> Option<RegionSelection> {
        self.measure_selection().or_else(|| {
            self.selection_geometry()
                .map(RegionSelectionGeometry::display_selection)
        })
    }

    pub(super) fn selection_rect(self) -> Option<ImagePixelRect> {
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

    pub(super) fn selection_geometry(self) -> Option<RegionSelectionGeometry> {
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

    pub(super) fn review_geometry(self) -> Option<RegionSelectionGeometry> {
        let Self::Ready {
            purpose, source, ..
        } = self
        else {
            return None;
        };
        let image_rect = self.stored_review_rect()?;
        let display = crate::backend::wayland::state::screen_image::screen_rect_for_image_rect(
            &source, image_rect,
        );
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

    pub(super) fn stored_review_rect(self) -> Option<ImagePixelRect> {
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

    pub(super) fn store_review_rect(&mut self, rect: ImagePixelRect) -> bool {
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

    pub(super) fn set_square_modifier(&mut self, active: bool) -> bool {
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

    pub(super) fn whole_image_selection(self) -> Option<RegionSelectionFinalize> {
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

    pub(super) fn picker_measurement(self, pointer: (f64, f64)) -> Option<RegionPickerMeasurement> {
        if let Self::Measure { bounds, .. } = self {
            if let Some(selection) = self.measure_selection() {
                return Some(RegionPickerMeasurement::Size {
                    width: (selection.end.0 - selection.start.0).abs() as u32,
                    height: (selection.end.1 - selection.start.1).abs() as u32,
                });
            }
            let point = measure_anchor(pointer, bounds)?;
            return Some(RegionPickerMeasurement::Point {
                x: point.0.max(0.0) as u32,
                y: point.1.max(0.0) as u32,
            });
        }
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
}
