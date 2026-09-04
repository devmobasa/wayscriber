use crate::draw::{
    EraserReplayContext, SpotlightMagnifierScratch, SpotlightMagnifierSource, SpotlightPass,
    render_eraser_stroke, render_spotlight_magnification_pass, render_spotlight_pass,
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

pub(super) fn render_page_content(args: PageContentArgs<'_, '_, '_>) {
    let PageContentArgs {
        render,
        frame,
        background,
        x,
        y,
        width,
        height,
        screen_width,
        screen_height,
        text_halo_enabled,
    } = args;
    let ctx = render.cairo;
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
    render_frame_shapes(
        render,
        frame,
        background,
        screen_width,
        screen_height,
        text_halo_enabled,
    );
    let _ = ctx.restore();
    let _ = ctx.restore();
}

fn render_frame_shapes(
    render: &mut crate::draw::RenderCtx<'_, '_>,
    frame: &crate::draw::Frame,
    background: &BoardBackground,
    target_width: u32,
    target_height: u32,
    text_halo_enabled: bool,
) {
    let ctx = render.cairo;
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
                render.render_shape_with_halo(&drawn.shape, text_halo_enabled);
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
    use crate::draw::{Color, FontDescriptor, Frame, Shape};

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
                render: &mut crate::draw::RenderCtx::new(
                    &ctx,
                    &mut crate::draw::RenderCaches::default(),
                ),
                frame: &frame,
                background,
                x: 0.0,
                y: 0.0,
                width: 120.0,
                height: 90.0,
                screen_width: 400,
                screen_height: 300,
                text_halo_enabled: true,
            });
        }
        let mut surface = surface;
        surface.flush();
        surface.data().expect("thumbnail pixels").to_vec()
    }

    fn text_thumbnail_pixels(text_halo_enabled: bool) -> Vec<u8> {
        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, 120, 90).expect("thumbnail surface");
        {
            let ctx = cairo::Context::new(&surface).expect("thumbnail context");
            let mut frame = Frame::new();
            frame.add_shape(Shape::Text {
                x: 60,
                y: 160,
                text: "Read me".to_string(),
                color: Color::new(0.96, 0.2, 0.25, 1.0),
                size: 48.0,
                font_descriptor: FontDescriptor::default(),
                background_enabled: false,
                wrap_width: None,
            });
            render_page_content(PageContentArgs {
                render: &mut crate::draw::RenderCtx::new(
                    &ctx,
                    &mut crate::draw::RenderCaches::default(),
                ),
                frame: &frame,
                background: &BoardBackground::Solid(Color::new(1.0, 1.0, 1.0, 1.0)),
                x: 0.0,
                y: 0.0,
                width: 120.0,
                height: 90.0,
                screen_width: 400,
                screen_height: 300,
                text_halo_enabled,
            });
        }
        surface.flush();
        surface.data().expect("thumbnail pixels").to_vec()
    }

    #[test]
    fn a_page_thumbnail_honours_the_text_halo_setting() {
        assert!(
            text_thumbnail_pixels(true) != text_thumbnail_pixels(false),
            "thumbnail text must forward the halo setting to the shape renderer",
        );
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

    #[test]
    fn thumbnail_owner_reuses_images_across_frames_with_pixel_parity() {
        use crate::draw::{EmbeddedImage, RenderCaches, RenderCtx};
        use std::sync::Arc;
        let image = cairo::ImageSurface::create(cairo::Format::ARgb32, 2, 2).unwrap();
        let ctx = cairo::Context::new(&image).unwrap();
        ctx.set_source_rgb(1.0, 0.0, 0.0);
        ctx.paint().unwrap();
        let mut png = Vec::new();
        image.write_to_png(&mut png).unwrap();
        let bytes: Arc<[u8]> = png.into();
        let mut frame = Frame::new();
        frame.add_shape(Shape::Image {
            x: 8,
            y: 8,
            w: 24,
            h: 24,
            data: EmbeddedImage {
                mime_type: "image/png".into(),
                width: 2,
                height: 2,
                bytes: Arc::clone(&bytes),
            },
        });
        frame.add_shape(Shape::Text {
            x: 40,
            y: 36,
            text: "Image".into(),
            color: crate::draw::BLACK,
            size: 20.0,
            font_descriptor: Default::default(),
            background_enabled: false,
            wrap_width: None,
        });
        let paint = |caches: &mut RenderCaches| {
            let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 124, 64).unwrap();
            {
                let ctx = cairo::Context::new(&surface).unwrap();
                render_page_content(PageContentArgs {
                    render: &mut RenderCtx::new(&ctx, caches),
                    frame: &frame,
                    background: &BoardBackground::Solid(crate::draw::WHITE),
                    x: 0.0,
                    y: 0.0,
                    width: 124.0,
                    height: 64.0,
                    screen_width: 120,
                    screen_height: 60,
                    text_halo_enabled: false,
                });
            }
            surface.flush();
            surface.data().unwrap().to_vec()
        };
        let baseline = Arc::strong_count(&bytes);
        let mut caches = RenderCaches::default();
        let first = paint(&mut caches);
        let retained = Arc::strong_count(&bytes);
        assert!(
            retained > baseline,
            "thumbnail must retain its decoded image in the supplied owner"
        );
        assert_eq!(paint(&mut caches), first);
        assert_eq!(Arc::strong_count(&bytes), retained);
        assert_eq!(paint(&mut RenderCaches::default()), first);
        let offset = (20 * 124 + 20) * 4;
        assert_eq!(
            u32::from_ne_bytes(first[offset..offset + 4].try_into().unwrap()),
            0xffff0000
        );
        let dark = (8..42)
            .flat_map(|y| (40..115).map(move |x| (y * 124 + x) * 4))
            .filter(|&offset| {
                u32::from_ne_bytes(first[offset..offset + 4].try_into().unwrap()) & 0x00ffffff
                    < 0x00404040
            })
            .count();
        assert!(dark > 30, "thumbnail must contain text beside the image");
        drop(caches);
        assert_eq!(
            Arc::strong_count(&bytes),
            baseline,
            "owner drop releases the retained payload"
        );
    }
}
