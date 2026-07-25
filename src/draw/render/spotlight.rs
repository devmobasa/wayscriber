//! The spotlight compositing pass.
//!
//! Unlike every other shape, a spotlight does not paint itself — it darkens
//! everything *around* itself. So it cannot be a per-shape draw call; it is one
//! pass over the whole canvas that runs after the background and before the
//! annotations, leaving anything drawn later at full brightness.

/// One elliptical opening left bright by the pass, in logical canvas coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpotlightRegion {
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
}

/// How strongly the surrounding canvas is dimmed and how soft each edge is.
#[derive(Clone, Copy, Debug)]
pub struct SpotlightPass {
    /// Alpha of the dim layer outside every opening.
    pub dim_opacity: f64,
    /// Fraction of each radius spent fading out. 0.0 is a hard edge.
    pub feather: f64,
}

/// Dims the current clip except inside `regions`.
///
/// The dim layer is built in its own group before the openings are punched out
/// of it. That isolation matters: punching directly onto the canvas would clear
/// the board background or captured backdrop underneath, leaving a transparent
/// hole instead of revealing what the spotlight is supposed to reveal.
pub fn render_spotlight_pass(
    ctx: &cairo::Context,
    regions: &[SpotlightRegion],
    pass: SpotlightPass,
) {
    if regions.is_empty() {
        return;
    }
    let dim = pass.dim_opacity.clamp(0.0, 1.0);
    if dim <= f64::EPSILON {
        return;
    }
    let feather = pass.feather.clamp(0.0, 0.9);

    let _ = ctx.save();
    ctx.push_group();

    ctx.set_source_rgba(0.0, 0.0, 0.0, dim);
    let _ = ctx.paint();

    ctx.set_operator(cairo::Operator::DestOut);
    for region in regions {
        punch_opening(ctx, *region, feather);
    }

    let _ = ctx.pop_group_to_source();
    ctx.set_operator(cairo::Operator::Over);
    let _ = ctx.paint();
    let _ = ctx.restore();
}

/// Erases one feathered ellipse from the dim layer currently being built.
fn punch_opening(ctx: &cairo::Context, region: SpotlightRegion, feather: f64) {
    let rx = region.rx.max(1.0);
    let ry = region.ry.max(1.0);

    let _ = ctx.save();
    ctx.translate(region.cx, region.cy);
    ctx.scale(rx, ry);

    // Radius 1.0 in the scaled space, so one gradient serves any aspect ratio.
    let gradient = cairo::RadialGradient::new(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    let solid_until = (1.0 - feather).clamp(0.0, 1.0);
    gradient.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 1.0);
    gradient.add_color_stop_rgba(solid_until, 0.0, 0.0, 0.0, 1.0);
    gradient.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    let _ = ctx.set_source(&gradient);

    ctx.new_sub_path();
    ctx.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
    let _ = ctx.fill();
    let _ = ctx.restore();
}

/// Outline drawn for a selected spotlight, since the shape itself is invisible.
pub fn render_spotlight_outline(
    ctx: &cairo::Context,
    region: SpotlightRegion,
    color: crate::draw::Color,
    thick: f64,
) {
    let rx = region.rx.max(1.0);
    let ry = region.ry.max(1.0);

    let _ = ctx.save();
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.translate(region.cx, region.cy);
    ctx.scale(rx, ry);
    ctx.new_sub_path();
    ctx.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
    let _ = ctx.restore();
    let _ = ctx.stroke();
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{Context, ImageSurface};

    fn surface_with_context(width: i32, height: i32) -> (ImageSurface, Context) {
        let surface = ImageSurface::create(cairo::Format::ARgb32, width, height)
            .expect("test surface allocation");
        let ctx = Context::new(&surface).expect("test cairo context");
        (surface, ctx)
    }

    fn alpha_at(surface: &mut ImageSurface, x: i32, y: i32) -> u8 {
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().expect("surface data");
        data[y as usize * stride + x as usize * 4 + 3]
    }

    fn rgb_at(surface: &mut ImageSurface, x: i32, y: i32) -> (u8, u8, u8) {
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().expect("surface data");
        let idx = y as usize * stride + x as usize * 4;
        (data[idx + 2], data[idx + 1], data[idx])
    }

    const CENTERED: SpotlightRegion = SpotlightRegion {
        cx: 100.0,
        cy: 100.0,
        rx: 40.0,
        ry: 30.0,
    };

    #[test]
    fn no_regions_leaves_the_canvas_untouched() {
        let (mut surface, ctx) = surface_with_context(60, 60);
        render_spotlight_pass(
            &ctx,
            &[],
            SpotlightPass {
                dim_opacity: 0.6,
                feather: 0.3,
            },
        );
        drop(ctx);
        assert_eq!(
            alpha_at(&mut surface, 30, 30),
            0,
            "a frame with no spotlights must not dim anything"
        );
    }

    #[test]
    fn zero_dim_opacity_is_a_no_op() {
        let (mut surface, ctx) = surface_with_context(60, 60);
        render_spotlight_pass(
            &ctx,
            &[SpotlightRegion {
                cx: 30.0,
                cy: 30.0,
                rx: 10.0,
                ry: 10.0,
            }],
            SpotlightPass {
                dim_opacity: 0.0,
                feather: 0.3,
            },
        );
        drop(ctx);
        assert_eq!(alpha_at(&mut surface, 5, 5), 0);
    }

    #[test]
    fn opening_is_transparent_while_the_surround_is_dimmed() {
        let (mut surface, ctx) = surface_with_context(200, 200);
        render_spotlight_pass(
            &ctx,
            &[CENTERED],
            SpotlightPass {
                dim_opacity: 0.6,
                feather: 0.3,
            },
        );
        drop(ctx);

        assert_eq!(
            alpha_at(&mut surface, 100, 100),
            0,
            "the opening must be fully transparent so live content shows through"
        );
        let surround = alpha_at(&mut surface, 5, 5);
        assert!(
            surround > 140,
            "surround should carry the dim layer, got alpha {surround}"
        );
    }

    #[test]
    fn feathering_produces_a_gradual_edge() {
        let (mut surface, ctx) = surface_with_context(200, 200);
        render_spotlight_pass(
            &ctx,
            &[CENTERED],
            SpotlightPass {
                dim_opacity: 0.8,
                feather: 0.5,
            },
        );
        drop(ctx);

        // Walking out along +x from the centre, alpha must rise monotonically.
        let samples: Vec<u8> = (0..=40)
            .step_by(8)
            .map(|dx| alpha_at(&mut surface, 100 + dx, 100))
            .collect();
        for pair in samples.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "feathered edge should not brighten outward: {samples:?}"
            );
        }
        assert!(
            samples[0] < samples[samples.len() - 1],
            "edge should actually fade: {samples:?}"
        );
    }

    #[test]
    fn hard_edge_when_feather_is_zero() {
        let (mut surface, ctx) = surface_with_context(200, 200);
        render_spotlight_pass(
            &ctx,
            &[CENTERED],
            SpotlightPass {
                dim_opacity: 0.8,
                feather: 0.0,
            },
        );
        drop(ctx);

        assert_eq!(alpha_at(&mut surface, 100, 100), 0);
        // Just inside the 40px x-radius stays clear; just outside is fully dim.
        assert!(alpha_at(&mut surface, 135, 100) < 40);
        assert!(alpha_at(&mut surface, 145, 100) > 180);
    }

    #[test]
    fn background_beneath_the_opening_survives_the_pass() {
        // The board-mode and export case: the pass must not clear what is under it.
        let (mut surface, ctx) = surface_with_context(200, 200);
        ctx.set_source_rgb(0.0, 1.0, 0.0);
        let _ = ctx.paint();

        render_spotlight_pass(
            &ctx,
            &[CENTERED],
            SpotlightPass {
                dim_opacity: 0.6,
                feather: 0.0,
            },
        );
        drop(ctx);

        let inside = rgb_at(&mut surface, 100, 100);
        assert_eq!(
            inside,
            (0, 255, 0),
            "background under the opening must be untouched, got {inside:?}"
        );
        assert_eq!(
            alpha_at(&mut surface, 100, 100),
            255,
            "the opening must stay opaque where a background exists"
        );

        let outside = rgb_at(&mut surface, 5, 5);
        assert!(
            outside.1 < 130,
            "surround should be darkened, got {outside:?}"
        );
    }

    #[test]
    fn multiple_openings_are_all_punched() {
        let (mut surface, ctx) = surface_with_context(300, 120);
        render_spotlight_pass(
            &ctx,
            &[
                SpotlightRegion {
                    cx: 70.0,
                    cy: 60.0,
                    rx: 30.0,
                    ry: 30.0,
                },
                SpotlightRegion {
                    cx: 220.0,
                    cy: 60.0,
                    rx: 30.0,
                    ry: 30.0,
                },
            ],
            SpotlightPass {
                dim_opacity: 0.7,
                feather: 0.0,
            },
        );
        drop(ctx);

        assert_eq!(alpha_at(&mut surface, 70, 60), 0);
        assert_eq!(alpha_at(&mut surface, 220, 60), 0);
        assert!(
            alpha_at(&mut surface, 145, 60) > 150,
            "the gap between two spotlights stays dimmed"
        );
    }

    #[test]
    fn degenerate_radii_do_not_panic_or_dim_everything() {
        let (mut surface, ctx) = surface_with_context(80, 80);
        render_spotlight_pass(
            &ctx,
            &[SpotlightRegion {
                cx: 40.0,
                cy: 40.0,
                rx: 0.0,
                ry: 0.0,
            }],
            SpotlightPass {
                dim_opacity: 0.6,
                feather: 0.3,
            },
        );
        drop(ctx);
        assert!(alpha_at(&mut surface, 5, 5) > 140);
    }
}
