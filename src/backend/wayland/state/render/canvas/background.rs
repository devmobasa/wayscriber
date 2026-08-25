use std::hash::{Hash, Hasher};

use super::super::super::*;
use crate::backend::wayland::state::screen_image::{
    ScreenImageKind, ScreenSourceToken, current_screen_source_token, displayed_screen_image,
};
use crate::draw::Color;

pub(super) struct CanvasEraserContext {
    surface: Option<cairo::ImageSurface>,
    pattern: Option<cairo::SurfacePattern>,
    backdrop_cache_key: Option<u64>,
    bg_color: Option<Color>,
    logical_to_image_scale_x: f64,
    logical_to_image_scale_y: f64,
    /// Resolved once for the frame by [`resolve_backdrop_provenance`], in the
    /// same call that decided whether the capture below could be painted at
    /// all, and carried here so the pixels on the surface and the availability
    /// the loupe and the toolbar report cannot come from different facts.
    magnifier_source: crate::draw::SpotlightMagnifierSource,
}

impl CanvasEraserContext {
    pub(super) fn replay_context(&self) -> crate::draw::EraserReplayContext<'_> {
        crate::draw::EraserReplayContext {
            pattern: self.pattern.as_ref().map(|p| p as &cairo::Pattern),
            surface: self.surface.as_ref(),
            backdrop_cache_key: self.backdrop_cache_key,
            bg_color: self.bg_color,
            logical_to_image_scale_x: self.logical_to_image_scale_x,
            logical_to_image_scale_y: self.logical_to_image_scale_y,
            logical_image_origin_x: 0.0,
            logical_image_origin_y: 0.0,
        }
    }

    pub(super) fn magnifier_source(&self) -> crate::draw::SpotlightMagnifierSource {
        self.magnifier_source
    }
}

/// Opaque provenance identity for the captured pixels behind the canvas.
///
/// Built from the capture's output, layout generation, kind, and image
/// generation, so a recapture, a Freeze/Zoom swap, or an output-layout change
/// all yield a different id. It is what makes the availability descriptor name
/// a specific capture rather than merely "some raster".
fn raster_source_id(token: &ScreenSourceToken) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    token.output_id.hash(&mut hasher);
    token.output_layout_generation.hash(&mut hasher);
    match token.kind {
        ScreenImageKind::Zoom => 1u8,
        ScreenImageKind::Frozen => 0u8,
    }
    .hash(&mut hasher);
    token.image_generation.hash(&mut hasher);
    token.image_size.hash(&mut hasher);
    token.stride.hash(&mut hasher);
    token.surface.hash(&mut hasher);
    token.output_scale.hash(&mut hasher);
    hasher.finish()
}

/// The one decision behind both the painted backdrop and the loupe's source.
///
/// A capture whose provenance no longer validates is dropped, not stretched:
/// CONTRIBUTING.md requires captured pixels to stay exact or fail visibly,
/// never shift, stretch, or reuse another output's image. Painting it anyway
/// and merely disabling magnification would leave the two describing different
/// sources, which the spec forbids — the descriptor must be derived from the
/// background actually rendered.
///
/// Returns whether the displayed capture may be painted, and the availability
/// that follows from what will be on the surface.
fn resolve_backdrop_provenance(
    raster_token: Option<&ScreenSourceToken>,
    board_is_transparent: bool,
) -> (bool, crate::draw::SpotlightMagnifierSource) {
    let source = crate::draw::SpotlightMagnifierSource::from_backdrop(
        raster_token.map(raster_source_id),
        !board_is_transparent,
    );
    (raster_token.is_some(), source)
}

impl WaylandState {
    /// Provenance token for the capture currently displayed, if it still
    /// validates against the active output and surface.
    fn current_screen_source_token(&self) -> Option<ScreenSourceToken> {
        displayed_screen_image(
            &self.zoom,
            &self.frozen,
            self.input_state.board_is_transparent(),
        )
        .and_then(|source| {
            current_screen_source_token(
                &source,
                &self.zoom,
                &self.frozen,
                (self.surface.width(), self.surface.height()),
            )
        })
    }

    /// Live Spotlight source availability for callers outside the render pass:
    /// the toolbar snapshot and the action-time warning.
    ///
    /// The render pass does not go through here — it calls
    /// [`resolve_backdrop_provenance`] directly, because it needs the paint
    /// decision from the same answer. Both end at that one resolver, so no
    /// caller can disagree about whether a loupe can preview.
    pub(in crate::backend::wayland::state) fn current_spotlight_magnifier_source(
        &self,
    ) -> crate::draw::SpotlightMagnifierSource {
        resolve_backdrop_provenance(
            self.current_screen_source_token().as_ref(),
            self.input_state.board_is_transparent(),
        )
        .1
    }

    pub(super) fn render_canvas_background(
        &mut self,
        ctx: &cairo::Context,
        scale: i32,
        phys_width: u32,
        phys_height: u32,
    ) -> Result<CanvasEraserContext> {
        let mut eraser_surface: Option<cairo::ImageSurface> = None;
        let mut eraser_pattern: Option<cairo::SurfacePattern> = None;
        let mut backdrop_cache_key: Option<u64> = None;
        let mut eraser_bg_color: Option<Color> = None;
        let mut logical_to_image_scale_x = 1.0;
        let mut logical_to_image_scale_y = 1.0;

        // One provenance answer decides both what is painted and what the loupe
        // may sample, so the pixels on screen and the availability reported can
        // never describe different sources.
        let (backdrop_is_paintable, magnifier_source) = resolve_backdrop_provenance(
            self.current_screen_source_token().as_ref(),
            self.input_state.board_is_transparent(),
        );

        let background_image = displayed_screen_image(
            &self.zoom,
            &self.frozen,
            self.input_state.board_is_transparent(),
        )
        .filter(|_| backdrop_is_paintable)
        .map(|source| {
            let cache_key = match source.kind {
                ScreenImageKind::Zoom => (self.zoom.image_generation() << 1) | 1,
                ScreenImageKind::Frozen => self.frozen.image_generation() << 1,
            };
            (source.image, cache_key, source.zoom_transformed)
        });

        if let Some((image, cache_key, zoom_render_active)) = background_image {
            // SAFETY: Cairo borrows `image.data` for this surface. The buffer
            // is owned by `image` and stays alive until the surface is dropped
            // (before Wayland commit). The API wants `*mut u8` even though this
            // path only reads pixels; we never write through the pointer, and
            // no other alias mutates the buffer while Cairo holds it.
            let surface = unsafe {
                cairo::ImageSurface::create_for_data_unsafe(
                    image.data.as_ptr() as *mut u8,
                    cairo::Format::ARgb32,
                    image.width as i32,
                    image.height as i32,
                    image.stride,
                )
            }
            .context("Failed to create frozen image surface")?;

            let scale_x = if image.width > 0 {
                phys_width as f64 / image.width as f64
            } else {
                1.0
            };
            let scale_y = if image.height > 0 {
                phys_height as f64 / image.height as f64
            } else {
                1.0
            };
            logical_to_image_scale_x = (scale as f64) / scale_x.max(f64::MIN_POSITIVE);
            logical_to_image_scale_y = (scale as f64) / scale_y.max(f64::MIN_POSITIVE);
            let _ = ctx.save();
            if zoom_render_active {
                let scale_x_safe = scale_x.max(f64::MIN_POSITIVE);
                let scale_y_safe = scale_y.max(f64::MIN_POSITIVE);
                let offset_x = self.zoom.view_offset.0 * (scale as f64) / scale_x_safe;
                let offset_y = self.zoom.view_offset.1 * (scale as f64) / scale_y_safe;
                ctx.scale(scale_x * self.zoom.scale, scale_y * self.zoom.scale);
                ctx.translate(-offset_x, -offset_y);
            } else if (scale_x - 1.0).abs() > f64::EPSILON || (scale_y - 1.0).abs() > f64::EPSILON {
                ctx.scale(scale_x, scale_y);
            }

            if let Err(err) = ctx.set_source_surface(&surface, 0.0, 0.0) {
                warn!("Failed to set frozen background surface: {}", err);
            } else if let Err(err) = ctx.paint() {
                warn!("Failed to paint frozen background: {}", err);
            }
            let _ = ctx.restore();

            let pattern = cairo::SurfacePattern::create(&surface);
            pattern.set_extend(cairo::Extend::Pad);
            let mut matrix = cairo::Matrix::identity();
            let scale_x_inv = 1.0 / (scale as f64 * scale_x.max(f64::MIN_POSITIVE));
            let scale_y_inv = 1.0 / (scale as f64 * scale_y.max(f64::MIN_POSITIVE));
            matrix.scale(scale_x_inv, scale_y_inv);
            pattern.set_matrix(matrix);
            eraser_surface = Some(surface);
            eraser_pattern = Some(pattern);
            backdrop_cache_key = Some(cache_key);
        } else {
            match self.input_state.boards.active_background() {
                crate::input::BoardBackground::Solid(color) => {
                    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
                    let _ = ctx.paint();
                    eraser_bg_color = Some(*color);
                }
                crate::input::BoardBackground::Transparent => {}
            }
        }

        Ok(CanvasEraserContext {
            surface: eraser_surface,
            pattern: eraser_pattern,
            backdrop_cache_key,
            bg_color: eraser_bg_color,
            logical_to_image_scale_x,
            logical_to_image_scale_y,
            magnifier_source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wayland_client::protocol::wl_output;

    fn token(
        kind: ScreenImageKind,
        layout_generation: u64,
        image_generation: u64,
    ) -> ScreenSourceToken {
        ScreenSourceToken {
            output_id: 1,
            output_layout_generation: layout_generation,
            kind,
            image_generation,
            image_size: (1920, 1080),
            stride: 1920 * 4,
            surface: (1920, 1080),
            output_scale: 1,
            output_transform: wl_output::Transform::Normal,
            zoom_transformed: false,
            zoom_scale: 1.0,
            zoom_view_offset: (0.0, 0.0),
        }
    }

    #[test]
    fn the_same_capture_keeps_the_same_source_id() {
        let frozen = token(ScreenImageKind::Frozen, 4, 9);
        assert_eq!(raster_source_id(&frozen), raster_source_id(&frozen));
    }

    #[test]
    fn a_recapture_or_layout_change_is_a_new_source_id() {
        let base = token(ScreenImageKind::Frozen, 4, 9);
        // Freeze taken again on the same output.
        assert_ne!(
            raster_source_id(&base),
            raster_source_id(&token(ScreenImageKind::Frozen, 4, 10))
        );
        // Outputs rearranged under the same capture generation.
        assert_ne!(
            raster_source_id(&base),
            raster_source_id(&token(ScreenImageKind::Frozen, 5, 9))
        );
        // Zoom pixels are not Freeze pixels, even at matching generations.
        assert_ne!(
            raster_source_id(&base),
            raster_source_id(&token(ScreenImageKind::Zoom, 4, 9))
        );
    }

    #[test]
    fn a_capture_that_fails_provenance_is_not_painted_at_all() {
        // The layout moved under a Freeze: `current_screen_source_token`
        // returns `None`, and the stale image must be dropped rather than
        // stretched onto the new geometry. Failing visibly is the contract;
        // painting it while merely disabling the loupe is not.
        let (paint, source) = resolve_backdrop_provenance(None, true);
        assert!(!paint, "a stale capture must not reach the surface");
        assert_eq!(
            source,
            crate::draw::SpotlightMagnifierSource::IncompleteTransparent
        );

        // A valid token paints and magnifies, and the descriptor names that
        // exact capture rather than merely reporting "some raster".
        let live = token(ScreenImageKind::Frozen, 3, 7);
        let (paint, source) = resolve_backdrop_provenance(Some(&live), true);
        assert!(paint);
        assert_eq!(source.raster_token(), Some(raster_source_id(&live)));
    }

    #[test]
    fn an_opaque_board_still_magnifies_when_its_capture_is_dropped() {
        // Nothing captured is paintable, but the board colour fills every
        // pixel itself, so the loupe keeps a complete source.
        let (paint, source) = resolve_backdrop_provenance(None, false);
        assert!(!paint);
        assert_eq!(source, crate::draw::SpotlightMagnifierSource::CompleteSolid);
    }

    #[test]
    fn a_transparent_board_without_valid_pixels_is_incomplete() {
        assert_eq!(
            crate::draw::SpotlightMagnifierSource::from_backdrop(None, false),
            crate::draw::SpotlightMagnifierSource::IncompleteTransparent
        );
        // Same board, valid capture: the loupe has a source again without the
        // shape's requested factor ever being rewritten.
        let live = raster_source_id(&token(ScreenImageKind::Frozen, 1, 1));
        assert!(
            crate::draw::SpotlightMagnifierSource::from_backdrop(Some(live), false).is_complete()
        );
    }
}
