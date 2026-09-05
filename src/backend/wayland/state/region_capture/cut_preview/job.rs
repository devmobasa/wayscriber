use super::super::cut_review::CutPreviewKey;
use super::super::render::{
    RegionPixelSource, compose_shared_region_pixels, render_region_base_pixels,
};
use crate::screen_pixels::PackedArgb32;
use std::sync::Arc;

pub(super) enum CutPreviewInput {
    CachedBase(Arc<PackedArgb32>),
    RenderSource(RegionPixelSource),
}

pub(super) struct CutPreviewJob {
    pub key: CutPreviewKey,
    pub input: CutPreviewInput,
}

pub(in crate::backend::wayland) enum CutPreviewOutcome {
    Success {
        key: CutPreviewKey,
        base: Arc<PackedArgb32>,
        composed: Arc<PackedArgb32>,
    },
    Failed {
        key: CutPreviewKey,
        message: String,
    },
}

pub(super) fn run_cut_preview(job: CutPreviewJob) -> CutPreviewOutcome {
    let key = job.key;
    let base = match job.input {
        CutPreviewInput::CachedBase(base) => base,
        CutPreviewInput::RenderSource(source) => match render_region_base_pixels(source) {
            Ok(pixels) => Arc::new(pixels),
            Err(error) => {
                return CutPreviewOutcome::Failed {
                    key,
                    message: error.to_string(),
                };
            }
        },
    };
    match compose_shared_region_pixels(&base, &key.cuts) {
        Ok(composed) => CutPreviewOutcome::Success {
            key,
            base,
            composed,
        },
        Err(error) => CutPreviewOutcome::Failed {
            key,
            message: error.to_string(),
        },
    }
}
