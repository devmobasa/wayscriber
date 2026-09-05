use crate::capture::CutAxis;
use crate::input::SelectionHandle;
use crate::input::state::RegionSelection;
use crate::screen_pixels::PackedArgb32;
use crate::ui::region_action_bar::{
    RegionAction, RegionActionAvailability, RegionActionBar, RegionCutStatus,
};
use crate::ui::region_resize_handles::RegionResizeHandles;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionCaptureWindowVisual<'a> {
    pub available: bool,
    pub active: bool,
    pub targets: &'a [RegionSelection],
    /// The pointer-hovered or keyboard-focused window candidate.
    pub highlighted_target: Option<usize>,
}

impl RegionCaptureWindowVisual<'_> {
    #[cfg(test)]
    pub(crate) const fn disabled() -> Self {
        Self {
            available: false,
            active: false,
            targets: &[],
            highlighted_target: None,
        }
    }

    pub(super) fn highlighted_selection(self) -> Option<RegionSelection> {
        self.active
            .then(|| {
                self.highlighted_target
                    .and_then(|index| self.targets.get(index).copied())
            })
            .flatten()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionCapturePickerVisual<'a> {
    pub selection: Option<RegionSelection>,
    pub pointer: (f64, f64),
    /// Authoritative pixel coordinates or size supplied by the picker owner.
    pub measurement: Option<&'a str>,
    pub show_scrim: bool,
    pub show_legend: bool,
    /// The selection is committed and awaiting a destination choice. Review
    /// drops the targeting chrome: no crosshair, and the size badge anchors to
    /// the rectangle rather than following the pointer.
    pub review: bool,
    /// Resize grips on the reviewed rectangle. Present only in Review, where
    /// they replace the corner arms the targeting frame draws.
    pub resize_handles: Option<RegionResizeHandles>,
    pub hovered_handle: Option<SelectionHandle>,
    pub loupe: Option<RegionCaptureLoupeVisual>,
    pub action_bar: Option<RegionActionBar>,
    pub hovered_action: Option<RegionAction>,
    pub include_drawings: bool,
    pub cut: RegionCaptureCutVisual<'a>,
    pub window: RegionCaptureWindowVisual<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionCutPreviewVisual<'a> {
    pub pixels: &'a PackedArgb32,
    pub display: RegionSelection,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionCutDragVisual {
    pub axis: CutAxis,
    pub band: RegionSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct RegionCaptureCutVisual<'a> {
    pub preview: Option<RegionCutPreviewVisual<'a>>,
    pub drag: Option<RegionCutDragVisual>,
    pub availability: RegionActionAvailability,
    pub cut_armed: bool,
    pub status: Option<RegionCutStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionCaptureLoupeVisual {
    pub pointer: (f64, f64),
    pub image_center: (f64, f64),
}

impl RegionCaptureLoupeVisual {
    pub(crate) fn when_enabled(
        show_loupe: bool,
        pointer: (f64, f64),
        image_center: (f64, f64),
    ) -> Option<Self> {
        show_loupe.then_some(Self {
            pointer,
            image_center,
        })
    }
}
