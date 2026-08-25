use crate::draw::{
    EraserReplayContext, SpotlightMagnifierScratch, SpotlightMagnifierSource, SpotlightPass,
    render_eraser_stroke, render_shape, render_spotlight_magnification_pass, render_spotlight_pass,
    spotlight_regions_for_frame,
};
use crate::input::BoardBackground;
use crate::input::state::{PAGE_NAME_HEIGHT, PAGE_NAME_PADDING};
use crate::ui::constants::{
    self, PANEL_BG_CONTEXT_MENU, RADIUS_STD, TEXT_HINT, TEXT_PRIMARY, TEXT_TERTIARY,
};
use crate::ui::primitives::{draw_rounded_rect, text_extents_for};
use crate::ui::theme::Rgba;
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::types::PageContentArgs;

/// Transparent-board thumbnail backdrop: faint white tint plus a cross-hatch
/// stroke (no matching theme token; kept from the pre-theme literals).
/// Spotlight dimming for thumbnails: enough to read as a spotlight at preview
/// size without turning the card black.
const THUMBNAIL_SPOTLIGHT_DIM: f64 = 0.55;
const THUMBNAIL_SPOTLIGHT_FEATHER: f64 = 0.35;

const TRANSPARENT_TINT: Rgba = (1.0, 1.0, 1.0, 0.06);
const TRANSPARENT_CROSS: Rgba = (1.0, 1.0, 1.0, 0.08);

pub(super) fn render_page_content(args: PageContentArgs<'_>) {
    let PageContentArgs {
        ctx,
        frame,
        background,
        x,
        y,
        width,
        height,
        screen_width,
        screen_height,
    } = args;
    let radius = RADIUS_STD;
    let _ = ctx.save();
    draw_rounded_rect(ctx, x, y, width, height, radius);
    ctx.clip();

    match background {
        BoardBackground::Solid(color) => {
            ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
            ctx.rectangle(x, y, width, height);
            let _ = ctx.fill();
        }
        BoardBackground::Transparent => {
            constants::set_color(ctx, TRANSPARENT_TINT);
            ctx.rectangle(x, y, width, height);
            let _ = ctx.fill();
            constants::set_color(ctx, TRANSPARENT_CROSS);
            ctx.set_line_width(1.0);
            ctx.move_to(x, y);
            ctx.line_to(x + width, y + height);
            ctx.move_to(x + width, y);
            ctx.line_to(x, y + height);
            let _ = ctx.stroke();
        }
    }

    let inset = 2.0;
    let content_w = (width - inset * 2.0).max(1.0);
    let content_h = (height - inset * 2.0).max(1.0);
    let scale = (content_w / screen_width as f64).min(content_h / screen_height as f64);
    let offset_x = (content_w - screen_width as f64 * scale) * 0.5;
    let offset_y = (content_h - screen_height as f64 * scale) * 0.5;

    let _ = ctx.save();
    ctx.translate(x + inset + offset_x, y + inset + offset_y);
    ctx.scale(scale, scale);
    render_frame_shapes(ctx, frame, background, screen_width, screen_height);
    let _ = ctx.restore();
    let _ = ctx.restore();
}

fn render_frame_shapes(
    ctx: &cairo::Context,
    frame: &crate::draw::Frame,
    background: &BoardBackground,
    target_width: u32,
    target_height: u32,
) {
    let eraser_ctx = EraserReplayContext {
        pattern: None,
        surface: None,
        backdrop_cache_key: None,
        bg_color: match background {
            BoardBackground::Solid(color) => Some(*color),
            BoardBackground::Transparent => None,
        },
        logical_to_image_scale_x: 1.0,
        logical_to_image_scale_y: 1.0,
        logical_image_origin_x: 0.0,
        logical_image_origin_y: 0.0,
    };

    for drawn in &frame.shapes {
        match &drawn.shape {
            crate::draw::Shape::EraserStroke { points, brush } => {
                render_eraser_stroke(ctx, points, brush, &eraser_ctx);
            }
            _ => {
                render_shape(ctx, &drawn.shape);
            }
        }
    }

    // Spotlights paint nothing per-shape, so without this pass a page holding
    // only spotlights would thumbnail as an empty page. Runs after the shapes to
    // match the canvas and export ordering.
    //
    // The thumbnail chain carries no config, and a strong configured dim would
    // render a postage-stamp preview almost black, so previews use a fixed,
    // gentler appearance rather than the live values.
    let regions = spotlight_regions_for_frame(frame);
    // A thumbnail has no captured desktop behind it, so an opaque board colour
    // is the only complete source it can offer. Which of those counts as
    // complete is decided by the shared rule, not restated here, so the preview
    // and the canvas can never disagree about when a loupe is possible.
    let magnifier_source =
        SpotlightMagnifierSource::from_backdrop(None, eraser_ctx.bg_color.is_some());
    if magnifier_source.is_complete() {
        let mut scratch = SpotlightMagnifierScratch::default();
        let _ = render_spotlight_magnification_pass(
            ctx,
            &regions,
            THUMBNAIL_SPOTLIGHT_FEATHER,
            magnifier_source,
            Some((target_width, target_height)),
            &mut scratch,
        );
    }
    render_spotlight_pass(
        ctx,
        &regions,
        SpotlightPass {
            dim_opacity: THUMBNAIL_SPOTLIGHT_DIM,
            feather: THUMBNAIL_SPOTLIGHT_FEATHER,
        },
    );
    if !magnifier_source.is_complete() {
        render_unavailable_magnification_labels(ctx, &regions);
    }
}

fn render_unavailable_magnification_labels(
    ctx: &cairo::Context,
    regions: &[crate::draw::SpotlightRegion],
) {
    for region in regions
        .iter()
        .filter(|region| crate::draw::spotlight_magnification_is_active(region.magnification))
    {
        let label = crate::draw::format_spotlight_magnification(region.magnification);
        let font_size = (region.ry.abs() * 0.35).clamp(18.0, 56.0);
        let style = UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Bold,
            size: font_size,
        };
        let _ = ctx.save();
        let extents = text_extents_for(
            ctx,
            style.family,
            style.slant,
            style.weight,
            style.size,
            &label,
        );
        let pad = font_size * 0.28;
        let x = region.cx - extents.width() * 0.5 - pad;
        let y = region.cy - extents.height() * 0.5 - pad;
        draw_rounded_rect(
            ctx,
            x,
            y,
            extents.width() + pad * 2.0,
            extents.height() + pad * 2.0,
            font_size * 0.25,
        );
        constants::set_color(ctx, PANEL_BG_CONTEXT_MENU);
        let _ = ctx.fill();
        constants::set_color(ctx, TEXT_PRIMARY);
        draw_text_baseline(
            ctx,
            style,
            &label,
            region.cx - extents.width() * 0.5 - extents.x_bearing(),
            region.cy - extents.height() * 0.5 - extents.y_bearing(),
            None,
        );
        let _ = ctx.restore();
    }
}

pub(super) fn render_page_name_label(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    name: Option<&str>,
    is_hovered: bool,
) {
    let label = match name {
        Some(value) => value,
        None if is_hovered => "Add name",
        None => return,
    };
    let max_w = width - 4.0;
    let label_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 10.5,
    };
    let label_x = x + 2.0;
    let label_y = y + height + PAGE_NAME_PADDING + PAGE_NAME_HEIGHT * 0.8;
    let color = if name.is_some() {
        TEXT_TERTIARY
    } else {
        TEXT_HINT
    };
    constants::set_color(ctx, constants::with_alpha(color, 0.85));
    let _ = ctx.save();
    ctx.rectangle(
        label_x,
        y + height + PAGE_NAME_PADDING,
        max_w,
        PAGE_NAME_HEIGHT,
    );
    ctx.clip();
    draw_text_baseline(ctx, label_style, label, label_x, label_y, None);
    let _ = ctx.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{Color, Frame, Shape};

    fn thumbnail_pixels(background: &BoardBackground, magnification: f64) -> Vec<u8> {
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, 120, 90).expect("thumbnail surface");
        {
            let ctx = cairo::Context::new(&surface).expect("thumbnail context");
            let mut frame = Frame::new();
            // Something for the loupe to magnify: a bare solid board is
            // uniform, so magnifying it could not change a single pixel.
            frame.add_shape(Shape::Rect {
                x: 180,
                y: 140,
                w: 40,
                h: 20,
                fill: true,
                color: Color {
                    r: 1.0,
                    g: 1.0,
                    b: 0.0,
                    a: 1.0,
                },
                thick: 2.0,
            });
            frame.add_shape(Shape::Spotlight {
                cx: 200,
                cy: 150,
                rx: 90,
                ry: 70,
                magnification,
            });
            render_page_content(PageContentArgs {
                ctx: &ctx,
                frame: &frame,
                background,
                x: 0.0,
                y: 0.0,
                width: 120.0,
                height: 90.0,
                screen_width: 400,
                screen_height: 300,
            });
        }
        let mut surface = surface;
        surface.flush();
        surface.data().expect("thumbnail pixels").to_vec()
    }

    #[test]
    fn a_solid_board_thumbnail_magnifies_its_spotlight() {
        let background = BoardBackground::Solid(Color {
            r: 0.2,
            g: 0.4,
            b: 0.8,
            a: 1.0,
        });
        assert_ne!(
            thumbnail_pixels(&background, 1.0),
            thumbnail_pixels(&background, 3.0),
            "a magnified spotlight must change what the preview shows"
        );
    }

    #[test]
    fn a_transparent_board_thumbnail_marks_the_factor_instead_of_faking_a_loupe() {
        // No captured desktop backs a transparent thumbnail, so the preview
        // shows the requested factor as a readout rather than magnifying an
        // incomplete source.
        assert_ne!(
            thumbnail_pixels(&BoardBackground::Transparent, 1.0),
            thumbnail_pixels(&BoardBackground::Transparent, 3.0),
            "an unavailable loupe must still say what it was asked for"
        );
    }
}
