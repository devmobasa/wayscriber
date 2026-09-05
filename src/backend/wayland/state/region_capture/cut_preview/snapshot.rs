use super::super::ActiveScreenRegion;
use super::super::cut_review::{
    RegionAnnotatedRenderContext, RegionRenderFingerprint, RegionReviewCorrelation,
};
use super::super::render::RegionPixelSource;
use crate::backend::wayland::state::WaylandState;
use crate::backend::wayland::state::screen_image::{
    displayed_screen_image, screen_source_is, shared_displayed_screen_image,
};
use crate::canvas_export::{CanvasExportRect, CanvasRegionExportSnapshot, CanvasRegionSource};
use crate::capture::CaptureError;
use crate::screen_pixels::ImagePixelRect;

pub(in crate::backend::wayland::state::region_capture) struct RegionRenderSnapshot {
    pub source: RegionPixelSource,
    pub fingerprint: RegionRenderFingerprint,
}

pub(super) fn capture_ready_correlation(
    region: Option<ActiveScreenRegion>,
) -> Result<RegionReviewCorrelation, CaptureError> {
    match region {
        Some(ActiveScreenRegion::Ready {
            purpose,
            generation,
            source,
            ..
        }) if purpose.is_capture() => Ok(RegionReviewCorrelation { generation, source }),
        _ => Err(CaptureError::ImageError(
            "Region capture is not active.".to_string(),
        )),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum CutPreviewSnapshotClass {
    Ready,
    Cancelled,
    Failed { message: String },
}

pub(super) fn classify_cut_preview_snapshot(
    desired: &RegionRenderFingerprint,
    live: Result<&RegionRenderFingerprint, &CaptureError>,
) -> CutPreviewSnapshotClass {
    match live {
        Ok(fingerprint) if fingerprint == desired => CutPreviewSnapshotClass::Ready,
        Ok(fingerprint) if fingerprint.correlation().source != desired.correlation().source => {
            CutPreviewSnapshotClass::Cancelled
        }
        Ok(_) => CutPreviewSnapshotClass::Failed {
            message: "The capture source no longer matches the cut preview.".to_string(),
        },
        Err(CaptureError::Cancelled(_)) => CutPreviewSnapshotClass::Cancelled,
        Err(error) => CutPreviewSnapshotClass::Failed {
            message: error.to_string(),
        },
    }
}

impl WaylandState {
    fn annotated_render_context(&self) -> RegionAnnotatedRenderContext {
        RegionAnnotatedRenderContext {
            board_id: self.input_state.boards.active_board_id().to_string(),
            page_index: self.input_state.boards.active_page_index(),
            page_generation: self.input_state.boards.active_page_generation(),
            canvas_content_generation: self.input_state.canvas_content_generation(),
            board_view_offset: self.board_view_offset(),
            text_halo_enabled: self.config.drawing.text_halo_enabled,
            spotlight: crate::canvas_export::SpotlightPassSnapshot {
                dim_opacity: self.input_state.style.spotlight_dim_opacity,
                feather: self.input_state.style.spotlight_feather,
            },
        }
    }

    fn fingerprint_for_live_render(
        &self,
        correlation: RegionReviewCorrelation,
        rect: ImagePixelRect,
        include_drawings: bool,
    ) -> RegionRenderFingerprint {
        if include_drawings {
            RegionRenderFingerprint::Annotated {
                correlation,
                source_rect: rect,
                context: self.annotated_render_context(),
            }
        } else {
            RegionRenderFingerprint::Raw {
                correlation,
                source_rect: rect,
            }
        }
    }

    fn retain_current_capture_image(
        &self,
        token: &crate::backend::wayland::state::screen_image::ScreenSourceToken,
    ) -> Result<std::sync::Arc<crate::screen_pixels::ScreenImage>, CaptureError> {
        let Some(source) = displayed_screen_image(
            &self.zoom,
            &self.frozen,
            self.input_state.board_is_transparent(),
        ) else {
            return Err(CaptureError::Cancelled(
                "The captured screen image is no longer available.".to_string(),
            ));
        };
        if !screen_source_is(
            token,
            &source,
            &self.zoom,
            &self.frozen,
            (self.surface.width(), self.surface.height()),
        ) {
            return Err(CaptureError::Cancelled(
                "The captured screen image changed.".to_string(),
            ));
        }
        shared_displayed_screen_image(&self.zoom, &self.frozen, source.kind).ok_or_else(|| {
            CaptureError::ImageError("Could not retain the captured screen image.".to_string())
        })
    }

    fn region_pixel_source(
        &self,
        fingerprint: &RegionRenderFingerprint,
        shared_image: std::sync::Arc<crate::screen_pixels::ScreenImage>,
    ) -> Result<RegionPixelSource, CaptureError> {
        match fingerprint {
            RegionRenderFingerprint::Raw { source_rect, .. } => Ok(RegionPixelSource::Raw {
                image: shared_image,
                selection: *source_rect,
            }),
            RegionRenderFingerprint::Annotated {
                source_rect,
                context,
                correlation,
            } => {
                let logical_bounds = CanvasExportRect::new(
                    context.board_view_offset.0,
                    context.board_view_offset.1,
                    f64::from(correlation.source.surface.0),
                    f64::from(correlation.source.surface.1),
                )
                .ok_or_else(|| {
                    CaptureError::ImageError("Could not map the selected drawings.".to_string())
                })?;
                Ok(RegionPixelSource::Annotated(Box::new(
                    CanvasRegionExportSnapshot {
                        source: CanvasRegionSource {
                            image: shared_image,
                            logical_bounds,
                        },
                        selection: *source_rect,
                        frame: self
                            .input_state
                            .boards
                            .active_frame()
                            .clone_without_history(),
                        text_halo_enabled: context.text_halo_enabled,
                        spotlight: context.spotlight,
                    },
                )))
            }
        }
    }

    pub(in crate::backend::wayland::state::region_capture) fn snapshot_region_render(
        &self,
        rect: ImagePixelRect,
        include_drawings: bool,
    ) -> Result<RegionRenderSnapshot, CaptureError> {
        let correlation = capture_ready_correlation(self.region_capture.active())?;
        let shared_image = self.retain_current_capture_image(&correlation.source)?;
        let fingerprint = self.fingerprint_for_live_render(correlation, rect, include_drawings);
        let source = self.region_pixel_source(&fingerprint, shared_image)?;
        Ok(RegionRenderSnapshot {
            source,
            fingerprint,
        })
    }

    pub(in crate::backend::wayland::state::region_capture) fn current_region_fingerprint(
        &self,
    ) -> Option<RegionRenderFingerprint> {
        let rect = self.region_review_rect()?;
        let include_drawings = self.region_picker_include_drawings();
        let correlation = capture_ready_correlation(self.region_capture.active()).ok()?;
        if self
            .retain_current_capture_image(&correlation.source)
            .is_err()
        {
            return None;
        }
        Some(self.fingerprint_for_live_render(correlation, rect, include_drawings))
    }
}
