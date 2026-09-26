//! Painter for view-engine widget trees.
//!
//! Lives render-side so it can reach the private widget draw functions; it
//! is the only consumer of a tree's visual data. Hover is resolved here at
//! paint time against each node's drawn rect (matching the legacy renderer),
//! so pointer motion never rebuilds a tree.

use crate::backend::wayland::toolbar::view::{
    ButtonStyle, ShortcutBadgePlacement, WidgetKind, WidgetNode, WidgetTree,
};
use crate::ui::theme::swatch::{chrome_rgb, swatch_edge_stroke};
use crate::ui_text::{UiTextEngine, UiTextStyle};

use super::widgets::constants::{
    COLOR_ACCENT, COLOR_BADGE_BACKGROUND, COLOR_BADGE_BORDER, COLOR_ICON_DEFAULT, COLOR_LABEL_HINT,
    COLOR_PANEL_BACKGROUND, COLOR_SWATCH_HAIRLINE, COLOR_SWATCH_HAIRLINE_DARK, COLOR_TEXT_DISABLED,
    COLOR_TEXT_SECONDARY, COLOR_TRACK_BACKGROUND, COLOR_TRACK_KNOB, FONT_FAMILY_DEFAULT,
    FONT_SIZE_LABEL, FONT_SIZE_SWATCH_KEY, PRESET_SLOT_ICON_RATIO, PRESET_SLOT_NUMBER_BOX,
    PRESET_SLOT_SWATCH_INSET, PRESET_SLOT_SWATCH_RADIUS, PRESET_SLOT_SWATCH_RATIO, set_color,
};
use super::widgets::{
    draw_button, draw_checkbox, draw_destructive_button, draw_disabled_button,
    draw_divider_vertical, draw_drag_handle, draw_label_center, draw_label_center_color,
    draw_label_left, draw_label_left_color, draw_label_left_wrapped, draw_meter_bar,
    draw_mini_checkbox, draw_minimize_button, draw_panel_background, draw_pin_button,
    draw_popover_panel, draw_restore_tab_body, draw_round_rect, draw_segmented_control,
    ellipsize_to_width, point_in_rect, set_icon_color,
};

/// Hover ring around an unselected swatch (dimmer sibling of the accent
/// selection ring).
const SWATCH_HOVER_RING: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.4);

/// Breathing room on each side of centered text-button labels. The painter
/// ellipsizes against the remaining width so labels never spill into adjacent
/// menu cells when a compact grid is narrower than their natural text.
const TEXT_BUTTON_LABEL_INSET: f64 = 6.0;

/// Paint every node of `tree` in order. `hover` is in the same logical space
/// as the tree's rects.
pub fn paint_tree(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    tree: &WidgetTree,
    hover: Option<(f64, f64)>,
) {
    for node in tree.nodes() {
        paint_node(engine, ctx, node, hover);
    }
}

fn hovered(node: &WidgetNode, hover: Option<(f64, f64)>) -> bool {
    hover
        .map(|(hx, hy)| point_in_rect(hx, hy, node.rect.0, node.rect.1, node.rect.2, node.rect.3))
        .unwrap_or(false)
}

fn label_style(size: f64, bold: bool) -> UiTextStyle<'static> {
    UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: if bold {
            cairo::FontWeight::Bold
        } else {
            cairo::FontWeight::Normal
        },
        size,
    }
}

fn paint_button_body(
    ctx: &cairo::Context,
    rect: (f64, f64, f64, f64),
    style: ButtonStyle,
    hover: bool,
) {
    let (x, y, w, h) = rect;
    if style.disabled {
        draw_disabled_button(ctx, x, y, w, h);
    } else if style.destructive {
        draw_destructive_button(ctx, x, y, w, h, hover);
    } else {
        draw_button(ctx, x, y, w, h, style.active, hover);
    }
}

/// Draw a filled preset slot's color as a small rounded swatch tucked in the
/// bottom-right corner. A luminance-driven hairline keeps the swatch defined
/// against the slot body regardless of the preset color (a black swatch on
/// the dark body, a white swatch on the accent body). It matches the GTK
/// preset slot so black and white presets both stay legible.
///
/// A translucent preset paints at its own alpha over the checkerboard, so the
/// slot previews the color the preset will actually apply.
fn paint_preset_color_swatch(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: (f64, f64, f64, f64),
) {
    let size = (w.min(h) * PRESET_SLOT_SWATCH_RATIO).round();
    let sx = x + w - size - PRESET_SLOT_SWATCH_INSET;
    let sy = y + h - size - PRESET_SLOT_SWATCH_INSET;
    let swatch_path =
        |ctx: &cairo::Context| draw_round_rect(ctx, sx, sy, size, size, PRESET_SLOT_SWATCH_RADIUS);
    crate::ui::checkerboard_behind(ctx, color.3, swatch_path);
    ctx.set_source_rgba(color.0, color.1, color.2, color.3);
    swatch_path(ctx);
    let _ = ctx.fill();
    let luminance = crate::draw::perceived_luminance(color.0, color.1, color.2);
    set_color(
        ctx,
        if luminance < 0.3 {
            COLOR_SWATCH_HAIRLINE_DARK
        } else {
            COLOR_SWATCH_HAIRLINE
        },
    );
    ctx.set_line_width(1.0);
    draw_round_rect(ctx, sx, sy, size, size, PRESET_SLOT_SWATCH_RADIUS);
    let _ = ctx.stroke();
}

/// The accent knob riding a slider's inset travel at `t` in `[0, 1]`.
fn draw_slider_knob(ctx: &cairo::Context, rect: (f64, f64, f64, f64), t: f64) {
    let (x, y, w, h) = rect;
    let knob_r = (h / 2.0).min(7.0);
    let knob_x = x + knob_r + t.clamp(0.0, 1.0) * (w - knob_r * 2.0);
    set_color(ctx, COLOR_TRACK_KNOB);
    ctx.arc(knob_x, y + h / 2.0, knob_r, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();
}

fn paint_shortcut_badge(engine: &UiTextEngine, ctx: &cairo::Context, node: &WidgetNode) {
    let Some(badge) = &node.shortcut_badge else {
        return;
    };
    let (x, y, w, h) = node.rect;
    let badge_h = 10.0;
    let badge_w = (badge.label.chars().count() as f64 * 5.0 + 4.0)
        .max(10.0)
        .min(w.max(10.0));
    let badge_x = match badge.placement {
        ShortcutBadgePlacement::Corner => x + w - badge_w - 2.0,
        ShortcutBadgePlacement::Below => x + (w - badge_w) / 2.0,
    };
    let badge_y = match badge.placement {
        ShortcutBadgePlacement::Corner => y + 2.0,
        ShortcutBadgePlacement::Below => y + h - badge_h - 2.0,
    };

    if badge.placement == ShortcutBadgePlacement::Corner {
        set_color(ctx, COLOR_BADGE_BACKGROUND);
        draw_round_rect(ctx, badge_x, badge_y, badge_w, badge_h, 3.0);
        let _ = ctx.fill();
        set_color(ctx, COLOR_BADGE_BORDER);
        ctx.set_line_width(1.0);
        draw_round_rect(ctx, badge_x, badge_y, badge_w, badge_h, 3.0);
        let _ = ctx.stroke();
    }
    // Below key letters read as unboxed 9px captions in the secondary text
    // color; corner badges keep the boxed 8px icon-color treatment.
    let (font_size, label_color) = match badge.placement {
        ShortcutBadgePlacement::Corner => (8.0, COLOR_ICON_DEFAULT),
        ShortcutBadgePlacement::Below => (9.0, COLOR_LABEL_HINT),
    };
    draw_label_center_color(
        engine,
        ctx,
        label_style(font_size, true),
        badge_x,
        badge_y,
        badge_w,
        badge_h,
        &badge.label,
        label_color,
    );
}

fn paint_node(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    node: &WidgetNode,
    hover: Option<(f64, f64)>,
) {
    let (x, y, w, h) = node.rect;
    let is_hover = hovered(node, hover) && node.interact.is_some();
    match &node.kind {
        WidgetKind::Panel => draw_panel_background(ctx, x, y, w, h),
        WidgetKind::Divider { vertical } => {
            if *vertical {
                draw_divider_vertical(ctx, x, y, h);
            } else {
                // Horizontal divider: same treatment rotated.
                set_color(ctx, super::widgets::constants::COLOR_DIVIDER);
                ctx.set_line_width(1.0);
                ctx.move_to(x, y + 0.5);
                ctx.line_to(x + w, y + 0.5);
                let _ = ctx.stroke();
            }
        }
        WidgetKind::DragHandle => draw_drag_handle(ctx, x, y, w, h, is_hover),
        WidgetKind::IconButton {
            glyph,
            icon_size,
            style,
        } => {
            paint_button_body(ctx, node.rect, *style, is_hover);
            if style.disabled {
                set_color(ctx, COLOR_TEXT_DISABLED);
            } else {
                set_icon_color(ctx, is_hover);
            }
            // A caption under the icon shares the tile: lift the icon a few
            // pixels so both fit inside the unchanged button rect.
            let caption_lift = if matches!(
                node.shortcut_badge.as_ref().map(|badge| badge.placement),
                Some(ShortcutBadgePlacement::Below)
            ) {
                4.0
            } else {
                0.0
            };
            let icon_x = x + (w - icon_size) / 2.0;
            let icon_y = y + (h - icon_size) / 2.0 - caption_lift;
            (glyph.0)(ctx, icon_x, icon_y, *icon_size);
        }
        WidgetKind::TextButton { label, style } => {
            paint_button_body(ctx, node.rect, *style, is_hover);
            let text_style = label_style(label.size, label.bold);
            let display = ellipsize_to_width(
                engine,
                ctx,
                text_style,
                &label.text,
                (w - TEXT_BUTTON_LABEL_INSET * 2.0).max(0.0),
            );
            if style.disabled {
                draw_label_center_color(
                    engine,
                    ctx,
                    text_style,
                    x,
                    y,
                    w,
                    h,
                    &display,
                    COLOR_TEXT_DISABLED,
                );
            } else {
                draw_label_center(engine, ctx, text_style, x, y, w, h, &display);
            }
        }
        WidgetKind::Label(label) => {
            let text_style = label_style(label.size, label.bold);
            if label.caption {
                // Captions name the control beside them; they sit in the hint
                // tone so the value they label stays the brightest text.
                draw_label_left_color(
                    engine,
                    ctx,
                    text_style,
                    (x, y, h),
                    &label.text,
                    COLOR_LABEL_HINT,
                );
            } else if label.centered {
                draw_label_center(engine, ctx, text_style, x, y, w, h, &label.text);
            } else if label.wrap {
                draw_label_left_wrapped(engine, ctx, text_style, x, y, w, h, &label.text);
            } else {
                draw_label_left(engine, ctx, text_style, x, y, w, h, &label.text);
            }
        }
        WidgetKind::MiniCheckbox { checked, label } => {
            draw_mini_checkbox(
                engine,
                ctx,
                x,
                y,
                w,
                h,
                *checked,
                is_hover,
                label_style(label.size, label.bold),
                &label.text,
            );
        }
        WidgetKind::Checkbox { checked, label } => {
            draw_checkbox(
                engine,
                ctx,
                x,
                y,
                w,
                h,
                *checked,
                is_hover,
                label_style(label.size, label.bold),
                &label.text,
            );
        }
        WidgetKind::SegmentedControl {
            left,
            right,
            active_right,
        } => {
            let active = usize::from(*active_right);
            let seg_hover = hover.and_then(|(hx, hy)| {
                if point_in_rect(hx, hy, x, y, w, h) {
                    Some(if hx < x + w / 2.0 { 0 } else { 1 })
                } else {
                    None
                }
            });
            draw_segmented_control(
                engine,
                ctx,
                x,
                y,
                w,
                h,
                (&left.text, &right.text),
                active,
                seg_hover,
                label_style(left.size, left.bold),
            );
        }
        WidgetKind::HitArea => {}
        WidgetKind::MeterBar { filled, enabled } => {
            draw_meter_bar(ctx, node.rect, *filled, is_hover, *enabled);
        }
        WidgetKind::SmoothingPreview { level } => {
            crate::toolbar_icons::draw_smoothing_preview(ctx, node.rect, *level);
        }
        WidgetKind::Slider { t } => {
            // Track and knob: a rounded track with the accent knob riding the
            // inset travel.
            let track_h = (h * 0.5).min(8.0);
            let track_y = y + (h - track_h) / 2.0;
            set_color(ctx, COLOR_TRACK_BACKGROUND);
            draw_round_rect(ctx, x, track_y, w, track_h, track_h / 2.0);
            let _ = ctx.fill();
            draw_slider_knob(ctx, node.rect, *t);
        }
        WidgetKind::OpacitySlider { t, paint } => {
            let track_h = (h * 0.5).min(8.0);
            crate::toolbar_icons::draw_opacity_track(
                ctx,
                (x, y + (h - track_h) / 2.0, w, track_h),
                paint.rgb,
                paint.alpha_range,
            );
            draw_slider_knob(ctx, node.rect, *t);
        }
        WidgetKind::OpacitySwatch { paint } => {
            crate::toolbar_icons::draw_opacity_swatch(
                ctx,
                node.rect,
                paint.rgb,
                paint.stroke_alpha,
            );
        }
        WidgetKind::Swatch { color, selected } => {
            // Rounded square inset one pixel so the accent selection ring
            // (2px stroke, ~2px gap) stays clear of the neighbouring swatch.
            // Translucent colors sit on the checkerboard, so a low-alpha
            // swatch reads as see-through rather than as the bar behind it.
            let swatch_path = |ctx: &cairo::Context| {
                draw_round_rect(ctx, x + 1.0, y + 1.0, w - 2.0, h - 2.0, 5.0)
            };
            crate::ui::checkerboard_behind(ctx, color.3, swatch_path);
            ctx.set_source_rgba(color.0, color.1, color.2, color.3);
            swatch_path(ctx);
            let _ = ctx.fill();
            // The inner edge: a subtle hairline, or a contrast ring when the
            // fill would vanish into the bar (the palette's black).
            let (edge, edge_width) = swatch_edge_stroke(
                *color,
                chrome_rgb(COLOR_PANEL_BACKGROUND),
                COLOR_SWATCH_HAIRLINE,
                1.0,
            );
            let inset = 1.0 + edge_width / 2.0;
            set_color(ctx, edge);
            ctx.set_line_width(edge_width);
            draw_round_rect(
                ctx,
                x + inset,
                y + inset,
                w - inset * 2.0,
                h - inset * 2.0,
                6.0 - inset,
            );
            let _ = ctx.stroke();
            if *selected {
                set_color(ctx, COLOR_ACCENT);
                ctx.set_line_width(2.0);
                draw_round_rect(ctx, x - 2.0, y - 2.0, w + 4.0, h + 4.0, 7.0);
                let _ = ctx.stroke();
            } else if is_hover {
                set_color(ctx, SWATCH_HOVER_RING);
                ctx.set_line_width(1.5);
                draw_round_rect(ctx, x - 2.0, y - 2.0, w + 4.0, h + 4.0, 7.0);
                let _ = ctx.stroke();
            }
        }
        WidgetKind::PresetSlot {
            glyph,
            color,
            label,
            active,
        } => {
            paint_button_body(ctx, node.rect, ButtonStyle::active(*active), is_hover);
            match glyph {
                // Filled slot: the saved tool glyph in the neutral foreground
                // so a dark preset color never renders it invisible against
                // the slot body; the preset color rides along as a separate
                // corner swatch instead.
                Some(glyph) => {
                    let icon_size = (w.min(h) * PRESET_SLOT_ICON_RATIO).round();
                    set_color(ctx, COLOR_TEXT_SECONDARY);
                    (glyph.0)(
                        ctx,
                        x + (w - icon_size) / 2.0,
                        y + (h - icon_size) / 2.0,
                        icon_size,
                    );
                    paint_preset_color_swatch(ctx, x, y, w, h, *color);
                    // The slot number stays readable as a small caption in the
                    // corner opposite the color, like the key captions under
                    // the tool icons.
                    let number = PRESET_SLOT_NUMBER_BOX;
                    draw_label_center_color(
                        engine,
                        ctx,
                        label_style(FONT_SIZE_SWATCH_KEY, true),
                        x + PRESET_SLOT_SWATCH_INSET,
                        y + h - number - PRESET_SLOT_SWATCH_INSET,
                        number,
                        number,
                        label,
                        COLOR_LABEL_HINT,
                    );
                }
                // Empty slot: the 1-based slot number, muted so a filled slot
                // reads as the one holding something; hover brings it up.
                None => {
                    draw_label_center_color(
                        engine,
                        ctx,
                        label_style(FONT_SIZE_LABEL, true),
                        x,
                        y,
                        w,
                        h,
                        label,
                        if is_hover {
                            COLOR_TEXT_SECONDARY
                        } else {
                            COLOR_LABEL_HINT
                        },
                    );
                }
            }
        }
        WidgetKind::MicroChip {
            glyph,
            ring_color,
            ring_width,
        } => {
            crate::toolbar_icons::draw_micro_chip(
                ctx,
                x,
                y,
                w.min(h),
                glyph.0,
                &crate::toolbar_icons::MicroChipStyle {
                    ring_color: *ring_color,
                    ring_width: *ring_width,
                    icon_color: if is_hover {
                        super::widgets::constants::COLOR_ICON_HOVER
                    } else {
                        COLOR_ICON_DEFAULT
                    },
                    hovered: is_hover,
                },
            );
        }
        WidgetKind::PinButton { pinned } => {
            draw_pin_button(ctx, x, y, w, *pinned, is_hover);
        }
        WidgetKind::MinimizeButton => draw_minimize_button(ctx, x, y, w, is_hover),
        WidgetKind::RestoreTab { glyph, label } => {
            draw_restore_tab_body(ctx, x, y, w, h, is_hover);
            set_icon_color(ctx, is_hover);
            let icon = (h * 0.56).min(18.0);
            let pad = (h - icon) / 2.0 + 2.0;
            (glyph.0)(ctx, x + pad, y + (h - icon) / 2.0, icon);
            let text_x = x + pad + icon + 6.0;
            draw_label_left(
                engine,
                ctx,
                label_style(label.size, label.bold),
                text_x,
                y,
                (x + w - text_x).max(0.0),
                h,
                &label.text,
            );
        }
        WidgetKind::Popover { caret_x, caret_up } => {
            draw_popover_panel(ctx, x, y, w, h, *caret_x, *caret_up);
        }
        WidgetKind::VScrollbar { t, thumb } => {
            // A soft track with the theme's proportional slider thumb.
            set_color(ctx, crate::ui::theme::toolbar::COLOR_SCROLLBAR_TRACK);
            draw_round_rect(ctx, x, y, w, h, w / 2.0);
            let _ = ctx.fill();
            let thumb_h = (h * thumb.clamp(0.0, 1.0)).max(w * 2.0).min(h);
            let thumb_y = y + (h - thumb_h) * t.clamp(0.0, 1.0);
            set_color(ctx, crate::ui::theme::toolbar::COLOR_SCROLLBAR_SLIDER);
            draw_round_rect(ctx, x, thumb_y, w, thumb_h, w / 2.0);
            let _ = ctx.fill();
        }
    }
    paint_shortcut_badge(engine, ctx, node);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::toolbar::view::node::LabelSpec;
    use cairo::{Context, Format, ImageSurface};

    /// `(r, g, b)` of one pixel, 0-255. Rgb24 packs pixels as native-endian
    /// 32-bit words, so on little-endian the byte order is B, G, R, unused.
    fn pixel_at(surface: &mut ImageSurface, x: i32, y: i32) -> (u8, u8, u8) {
        let stride = surface.stride() as usize;
        let offset = y as usize * stride + x as usize * 4;
        let data = surface.data().expect("pixel data");
        (data[offset + 2], data[offset + 1], data[offset])
    }

    /// Paint a preset slot's corner swatch on white and sample its middle.
    fn preset_swatch_center(color: (f64, f64, f64, f64)) -> (u8, u8, u8) {
        let slot = 40.0;
        let surface = ImageSurface::create(Format::Rgb24, 48, 48).expect("surface");
        {
            let ctx = Context::new(&surface).expect("context");
            ctx.set_source_rgb(1.0, 1.0, 1.0);
            let _ = ctx.paint();
            paint_preset_color_swatch(&ctx, 0.0, 0.0, slot, slot, color);
        }
        let size = (slot * PRESET_SLOT_SWATCH_RATIO).round();
        let center = (slot - size - PRESET_SLOT_SWATCH_INSET + size / 2.0).round() as i32;
        let mut surface = surface;
        pixel_at(&mut surface, center, center)
    }

    /// Paint one top-bar quick-color swatch node on white and sample its middle.
    fn swatch_node_center(color: (f64, f64, f64, f64)) -> (u8, u8, u8) {
        let size = 24.0;
        let surface = ImageSurface::create(Format::Rgb24, 32, 32).expect("surface");
        {
            let ctx = Context::new(&surface).expect("context");
            ctx.set_source_rgb(1.0, 1.0, 1.0);
            let _ = ctx.paint();
            let node = WidgetNode::decor(
                "test.swatch",
                (4.0, 4.0, size, size),
                WidgetKind::Swatch {
                    color,
                    selected: false,
                },
            );
            paint_node(&UiTextEngine::default(), &ctx, &node, None);
        }
        let mut surface = surface;
        pixel_at(&mut surface, 4 + size as i32 / 2, 4 + size as i32 / 2)
    }

    #[test]
    fn text_button_label_does_not_paint_outside_its_rect() {
        const WIDTH: i32 = 240;
        const HEIGHT: i32 = 80;
        const RECT: (f64, f64, f64, f64) = (80.0, 20.0, 80.0, 40.0);
        let surface = ImageSurface::create(Format::Rgb24, WIDTH, HEIGHT).expect("surface");
        {
            let ctx = Context::new(&surface).expect("context");
            ctx.set_source_rgb(1.0, 0.0, 0.0);
            let _ = ctx.paint();
            let node = WidgetNode::decor(
                "test.long-text-button",
                RECT,
                WidgetKind::TextButton {
                    label: LabelSpec::new("Status bar contents and customization", 16.0, true),
                    style: ButtonStyle::plain(),
                },
            );
            paint_node(&UiTextEngine::default(), &ctx, &node, None);
        }

        let mut surface = surface;
        let stride = surface.stride() as usize;
        let data = surface.data().expect("pixel data");
        let painted_outside = (RECT.1 as i32..(RECT.1 + RECT.3) as i32).any(|y| {
            (0..WIDTH).any(|x| {
                let outside = x < RECT.0 as i32 - 2 || x >= (RECT.0 + RECT.2) as i32 + 2;
                if !outside {
                    return false;
                }
                let offset = y as usize * stride + x as usize * 4;
                data[offset] > 5 || data[offset + 1] > 5 || data[offset + 2] < 250
            })
        });
        assert!(
            !painted_outside,
            "a long built-in button label escaped its declared cell"
        );
    }

    #[test]
    fn a_translucent_top_bar_swatch_shows_its_transparency() {
        assert_eq!(
            swatch_node_center((1.0, 0.0, 0.0, 1.0)),
            (255, 0, 0),
            "opaque swatches stay exact"
        );

        // A fully transparent swatch used to show only the bar behind it, so
        // "no color" and "transparent" looked identical.
        let (r, g, b) = swatch_node_center((1.0, 0.0, 0.0, 0.0));
        assert!(
            r < 255 && r == g && g == b,
            "a fully transparent swatch should read as bare checkerboard: ({r}, {g}, {b})"
        );

        let (r, g, b) = swatch_node_center((1.0, 0.0, 0.0, 0.5));
        assert!(r < 255, "translucent swatch painted at full red: {r}");
        assert!(
            g > 0 && b > 0,
            "checkerboard did not show through: ({r}, {g}, {b})"
        );
    }

    /// Paint one quick-color swatch node on the toolbar panel color and sample
    /// the pixel on its inner edge, halfway down the left side.
    fn swatch_edge_on_panel(color: (f64, f64, f64, f64)) -> (u8, u8, u8) {
        let surface = ImageSurface::create(Format::Rgb24, 32, 32).expect("surface");
        {
            let ctx = Context::new(&surface).expect("context");
            let panel = COLOR_PANEL_BACKGROUND;
            ctx.set_source_rgb(panel.0, panel.1, panel.2);
            let _ = ctx.paint();
            let node = WidgetNode::decor(
                "test.swatch",
                (4.0, 4.0, 24.0, 24.0),
                WidgetKind::Swatch {
                    color,
                    selected: false,
                },
            );
            paint_node(&UiTextEngine::default(), &ctx, &node, None);
        }
        let mut surface = surface;
        pixel_at(&mut surface, 5, 16)
    }

    #[test]
    fn a_dark_swatch_gets_a_light_ring_against_the_dark_bar() {
        let black = crate::domain::color::PALETTE_BLACK;
        let (r, g, b) = swatch_edge_on_panel((black.r, black.g, black.b, black.a));
        let edge = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;
        assert!(
            edge >= 120,
            "the black swatch's edge should read against the bar: ({r}, {g}, {b})"
        );

        // A swatch that already stands out keeps the quiet hairline.
        let red = crate::domain::color::PALETTE_RED;
        let (r, g, b) = swatch_edge_on_panel((red.r, red.g, red.b, red.a));
        assert!(
            r > 200 && g < 120 && b < 120,
            "red keeps its own edge: ({r}, {g}, {b})"
        );
    }

    /// Paint one 46px preset slot on black and return its pixels as luma.
    fn preset_slot_luma(filled: bool, label: &str, hover: Option<(f64, f64)>) -> Vec<Vec<u32>> {
        const SIZE: i32 = 46;
        let surface = ImageSurface::create(Format::Rgb24, SIZE, SIZE).expect("surface");
        {
            let ctx = Context::new(&surface).expect("context");
            ctx.set_source_rgb(0.0, 0.0, 0.0);
            let _ = ctx.paint();
            let glyph = filled.then(|| {
                crate::backend::wayland::toolbar::view::node::IconFn(
                    crate::toolbar_icons::top_toolbar_icon_painter(
                        crate::ui::toolbar::model::TopToolbarIcon::Tool(
                            crate::ui::toolbar::model::SemanticToolIcon::Pen,
                        ),
                    ),
                )
            });
            let node = WidgetNode::new(
                "test.preset",
                (0.0, 0.0, SIZE as f64, SIZE as f64),
                WidgetKind::PresetSlot {
                    glyph,
                    color: (1.0, 0.0, 0.0, 1.0),
                    label: label.to_string(),
                    active: false,
                },
                Some(
                    crate::backend::wayland::toolbar::view::node::Interaction::click(
                        crate::ui::toolbar::ToolbarEvent::SavePreset(1),
                        None,
                    ),
                ),
            );
            paint_node(&UiTextEngine::default(), &ctx, &node, hover);
        }
        let mut surface = surface;
        (0..SIZE)
            .map(|y| {
                (0..SIZE)
                    .map(|x| {
                        let (r, g, b) = pixel_at(&mut surface, x, y);
                        (u32::from(r) + u32::from(g) + u32::from(b)) / 3
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_filled_preset_slot_keeps_its_number_in_the_corner() {
        let with_number = preset_slot_luma(true, "1", None);
        let without = preset_slot_luma(true, "", None);

        // The number sits in the bottom-left box, clear of the color swatch
        // in the opposite corner.
        let inset = PRESET_SLOT_SWATCH_INSET as usize;
        let box_size = PRESET_SLOT_NUMBER_BOX as usize;
        let rows = 46 - inset - box_size..46 - inset;
        let columns = inset..inset + box_size;
        let changed = rows
            .flat_map(|y| columns.clone().map(move |x| (x, y)))
            .filter(|&(x, y)| with_number[y][x] != without[y][x])
            .count();
        assert!(
            changed > 4,
            "the slot number should paint in its corner box"
        );
    }

    #[test]
    fn an_empty_preset_slot_is_muted_until_hovered() {
        let brightest = |pixels: &[Vec<u32>]| pixels.iter().flatten().copied().max().unwrap_or(0);
        let resting = brightest(&preset_slot_luma(false, "1", None));
        let hovered = brightest(&preset_slot_luma(false, "1", Some((23.0, 23.0))));

        assert!(
            resting + 30 < hovered,
            "the empty slot's number rests muted ({resting}) and lifts on hover ({hovered})"
        );
    }

    #[test]
    fn a_translucent_preset_swatch_shows_its_transparency() {
        assert_eq!(
            preset_swatch_center((1.0, 0.0, 0.0, 1.0)),
            (255, 0, 0),
            "opaque presets stay exact"
        );

        // Half-alpha red over the checkerboard lets the gray through, so the
        // slot cannot be mistaken for an opaque preset.
        let (r, g, b) = preset_swatch_center((1.0, 0.0, 0.0, 0.5));
        assert!(r < 255, "translucent preset painted at full red: {r}");
        assert!(
            g > 0 && b > 0,
            "checkerboard did not show through: ({r}, {g}, {b})"
        );
    }
}
