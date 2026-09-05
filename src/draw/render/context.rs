use super::blur::{BlurRenderCache, render_blur_rect_with_cache};
use super::image::ImageSurfaceCache;
use super::{BlurRectParams, EraserReplayContext};
use crate::draw::Shape;

/// Decoded image and sampled backdrop resources for one rendering owner.
///
/// Reuse across passes to retain cached surfaces. Independent backdrop generation
/// namespaces must use separate owners. Construction allocates no Cairo resources;
/// entries are populated on demand. Images retain at most 32 entries and 64 MiB
/// of decoded pixels; blur retains at most 8 entries and 64 MiB.
#[derive(Default)]
pub struct RenderCaches {
    images: ImageSurfaceCache,
    blur: BlurRenderCache,
}

/// A short drawing pass borrowing its target and persistent resources.
pub struct RenderCtx<'c, 'r> {
    pub cairo: &'c cairo::Context,
    pub caches: &'r mut RenderCaches,
}

impl<'c, 'r> RenderCtx<'c, 'r> {
    pub fn new(cairo: &'c cairo::Context, caches: &'r mut RenderCaches) -> Self {
        Self { cairo, caches }
    }

    pub fn render_shape(&mut self, shape: &Shape) {
        let measurer = crate::draw::TextMeasurer::default();
        self.render_shape_with_measurer(&measurer, shape);
    }

    pub fn render_shape_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        shape: &Shape,
    ) {
        self.render_shape_with_halo_with_measurer(measurer, shape, true);
    }

    pub fn render_shape_with_halo(&mut self, shape: &Shape, text_halo_enabled: bool) {
        let measurer = crate::draw::TextMeasurer::default();
        self.render_shape_with_halo_with_measurer(&measurer, shape, text_halo_enabled);
    }

    pub fn render_shape_with_halo_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        shape: &Shape,
        text_halo_enabled: bool,
    ) {
        self.render_shape_over_with_halo_with_measurer(measurer, shape, None, text_halo_enabled);
    }

    pub fn render_shape_over(&mut self, shape: &Shape, known_background_luminance: Option<f64>) {
        let measurer = crate::draw::TextMeasurer::default();
        self.render_shape_over_with_measurer(&measurer, shape, known_background_luminance);
    }

    pub fn render_shape_over_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        shape: &Shape,
        known_background_luminance: Option<f64>,
    ) {
        self.render_shape_over_with_halo_with_measurer(
            measurer,
            shape,
            known_background_luminance,
            true,
        );
    }

    pub fn render_shape_over_with_halo(
        &mut self,
        shape: &Shape,
        known_background_luminance: Option<f64>,
        text_halo_enabled: bool,
    ) {
        let measurer = crate::draw::TextMeasurer::default();
        self.render_shape_over_with_halo_with_measurer(
            &measurer,
            shape,
            known_background_luminance,
            text_halo_enabled,
        );
    }

    pub fn render_shape_over_with_halo_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        shape: &Shape,
        known_background_luminance: Option<f64>,
        text_halo_enabled: bool,
    ) {
        super::shapes::render_shape_with_cache(
            measurer,
            &mut self.caches.images,
            self.cairo,
            shape,
            known_background_luminance,
            text_halo_enabled,
        );
    }

    pub fn render_blur_rect(
        &mut self,
        params: BlurRectParams,
        replay_ctx: &EraserReplayContext<'_>,
    ) {
        render_blur_rect_with_cache(&mut self.caches.blur, self.cairo, params, replay_ctx);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod text_tests;
