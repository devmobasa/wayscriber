use crate::backend::wayland::state::screen_image::ScreenSourceToken;
use crate::capture::{CutAxis, CutBand};
use crate::input::state::{RegionInputSource, RegionSelection};
use crate::screen_pixels::ImagePixelRect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland::state::region_capture) enum PreviewApply {
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
    pub(in crate::backend::wayland::state::region_capture) owner: RegionInputSource,
    pub(in crate::backend::wayland::state::region_capture) start: (f64, f64),
    pub(in crate::backend::wayland::state::region_capture) current: (f64, f64),
    pub(in crate::backend::wayland::state::region_capture) axis: Option<CutAxis>,
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::backend::wayland) struct RegionReviewCorrelation {
    pub(in crate::backend::wayland::state::region_capture) generation: u64,
    pub(in crate::backend::wayland::state::region_capture) source: ScreenSourceToken,
}

/// Board and overlay facts that distinguish two annotated region renders.
/// Fingerprint and snapshot construction share one of these so halo, Spotlight,
/// and board identity cannot describe different frames.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::backend::wayland::state::region_capture) struct RegionAnnotatedRenderContext {
    pub(in crate::backend::wayland::state::region_capture) board_id: String,
    pub(in crate::backend::wayland::state::region_capture) page_index: usize,
    pub(in crate::backend::wayland::state::region_capture) page_generation: u64,
    pub(in crate::backend::wayland::state::region_capture) canvas_content_generation: u64,
    pub(in crate::backend::wayland::state::region_capture) board_view_offset: (f64, f64),
    pub(in crate::backend::wayland::state::region_capture) text_halo_enabled: bool,
    pub(in crate::backend::wayland::state::region_capture) spotlight:
        crate::canvas_export::SpotlightPassSnapshot,
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
    pub(in crate::backend::wayland::state::region_capture) fn correlation(
        &self,
    ) -> &RegionReviewCorrelation {
        match self {
            Self::Raw { correlation, .. } | Self::Annotated { correlation, .. } => correlation,
        }
    }

    pub(in crate::backend::wayland::state::region_capture) fn source_rect(&self) -> ImagePixelRect {
        match self {
            Self::Raw { source_rect, .. } | Self::Annotated { source_rect, .. } => *source_rect,
        }
    }

    pub(in crate::backend::wayland::state::region_capture) fn include_drawings(&self) -> bool {
        matches!(self, Self::Annotated { .. })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::backend::wayland) struct CutPreviewKey {
    pub(in crate::backend::wayland::state::region_capture) fingerprint: RegionRenderFingerprint,
    pub(in crate::backend::wayland::state::region_capture) revision: u64,
    pub(in crate::backend::wayland::state::region_capture) cuts: Vec<CutBand>,
}

#[derive(Debug, Clone)]
pub(in crate::backend::wayland) struct RegionCutPreview {
    pub(in crate::backend::wayland::state::region_capture) key: CutPreviewKey,
    pub(in crate::backend::wayland::state::region_capture) pixels:
        std::sync::Arc<crate::screen_pixels::PackedArgb32>,
    pub(in crate::backend::wayland::state::region_capture) display: RegionSelection,
}

#[derive(Debug, Clone)]
pub(in crate::backend::wayland) struct RegionCutBase {
    pub(in crate::backend::wayland::state::region_capture) fingerprint: RegionRenderFingerprint,
    pub(in crate::backend::wayland::state::region_capture) pixels:
        std::sync::Arc<crate::screen_pixels::PackedArgb32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland::state::region_capture) enum CutCommit {
    None,
    Applied,
    RejectedFullAxis,
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct RegionReviewEdits {
    pub(in crate::backend::wayland::state::region_capture) correlation: RegionReviewCorrelation,
    pub(in crate::backend::wayland::state::region_capture) source_rect: ImagePixelRect,
    pub(in crate::backend::wayland::state::region_capture) mode: CutMode,
    pub(in crate::backend::wayland::state::region_capture) drag: Option<CutDrag>,
    pub(in crate::backend::wayland::state::region_capture) cuts: Vec<CutBand>,
    pub(in crate::backend::wayland::state::region_capture) redo: Vec<CutBand>,
    pub(in crate::backend::wayland::state::region_capture) revision: u64,
    pub(in crate::backend::wayland::state::region_capture) desired_preview: Option<CutPreviewKey>,
    pub(in crate::backend::wayland::state::region_capture) ready_preview: Option<RegionCutPreview>,
    pub(in crate::backend::wayland::state::region_capture) base_cache: Option<RegionCutBase>,
    pub(in crate::backend::wayland::state::region_capture) failed_revision: Option<u64>,
}
