//! Painter for view-engine widget trees.
//!
//! Lives render-side so it can reach the private widget draw functions; it
//! is the only consumer of a tree's visual data. Hover is resolved here at
//! paint time against each node's drawn rect (matching the legacy renderer),
//! so pointer motion never rebuilds a tree.

use crate::backend::wayland::toolbar::view::{
    ButtonStyle, ShortcutBadgePlacement, WidgetKind, WidgetNode, WidgetTree,
};
use crate::ui_text::UiTextStyle;

use super::widgets::constants::{
    COLOR_ACCENT, COLOR_BADGE_BACKGROUND, COLOR_BADGE_BORDER, COLOR_ICON_DEFAULT, COLOR_LABEL_HINT,
    COLOR_SWATCH_HAIRLINE, COLOR_SWATCH_HAIRLINE_DARK, COLOR_TEXT_DISABLED, COLOR_TEXT_SECONDARY,
    COLOR_TRACK_BACKGROUND, COLOR_TRACK_KNOB, FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL,
    PRESET_SLOT_ICON_RATIO, PRESET_SLOT_SWATCH_INSET, PRESET_SLOT_SWATCH_RADIUS,
    PRESET_SLOT_SWATCH_RATIO, set_color,
};
use super::widgets::{
    draw_button, draw_checkbox, draw_destructive_button, draw_disabled_button,
    draw_divider_vertical, draw_drag_handle, draw_group_card, draw_label_center,
    draw_label_center_color, draw_label_left, draw_label_left_wrapped, draw_mini_checkbox,
    draw_minimize_button, draw_panel_background, draw_pin_button, draw_popover_panel,
    draw_round_rect, draw_segmented_control, ellipsize_to_width, point_in_rect, set_icon_color,
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
pub fn paint_tree(ctx: &cairo::Context, tree: &WidgetTree, hover: Option<(f64, f64)>) {
    for node in tree.nodes() {
        paint_node(ctx, node, hover);
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
/// the dark body, a white swatch on the accent body). Mirrors the built-in
/// side-palette swatch treatment and the GTK preset slot so black and white
/// presets both stay legible.
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
    let luminance = 0.299 * color.0 + 0.587 * color.1 + 0.114 * color.2;
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

fn paint_shortcut_badge(ctx: &cairo::Context, node: &WidgetNode) {
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

fn paint_node(ctx: &cairo::Context, node: &WidgetNode, hover: Option<(f64, f64)>) {
    let (x, y, w, h) = node.rect;
    let is_hover = hovered(node, hover) && node.interact.is_some();
    match &node.kind {
        WidgetKind::Panel => draw_panel_background(ctx, x, y, w, h),
        WidgetKind::Card => draw_group_card(ctx, x, y, w, h),
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
                ctx,
                text_style,
                &label.text,
                (w - TEXT_BUTTON_LABEL_INSET * 2.0).max(0.0),
            );
            if style.disabled {
                draw_label_center_color(ctx, text_style, x, y, w, h, &display, COLOR_TEXT_DISABLED);
            } else {
                draw_label_center(ctx, text_style, x, y, w, h, &display);
            }
        }
        WidgetKind::Icon { glyph } => {
            set_color(ctx, COLOR_ICON_DEFAULT);
            (glyph.0)(ctx, x, y, w.min(h));
        }
        WidgetKind::Label(label) => {
            let text_style = label_style(label.size, label.bold);
            if label.wrap {
                draw_label_left_wrapped(ctx, text_style, x, y, w, h, &label.text);
            } else {
                draw_label_left(ctx, text_style, x, y, w, h, &label.text);
            }
        }
        WidgetKind::MiniCheckbox { checked, label } => {
            draw_mini_checkbox(
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
        WidgetKind::Slider { t } => {
            // Track and knob share the side-palette slider treatment: a
            // rounded track with the accent knob riding the inset travel.
            let track_h = (h * 0.5).min(8.0);
            let track_y = y + (h - track_h) / 2.0;
            set_color(ctx, COLOR_TRACK_BACKGROUND);
            draw_round_rect(ctx, x, track_y, w, track_h, track_h / 2.0);
            let _ = ctx.fill();
            let knob_r = (h / 2.0).min(7.0);
            let knob_x = x + knob_r + t.clamp(0.0, 1.0) * (w - knob_r * 2.0);
            set_color(ctx, COLOR_TRACK_KNOB);
            ctx.arc(knob_x, y + h / 2.0, knob_r, 0.0, std::f64::consts::PI * 2.0);
            let _ = ctx.fill();
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
            // Subtle inner hairline keeps dark fills defined against the bar.
            set_color(ctx, COLOR_SWATCH_HAIRLINE);
            ctx.set_line_width(1.0);
            draw_round_rect(ctx, x + 1.5, y + 1.5, w - 3.0, h - 3.0, 4.5);
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
                // corner swatch instead (the side-palette convention).
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
                }
                // Empty slot: the 1-based slot number in the secondary text
                // color, inviting a save.
                None => {
                    draw_label_center_color(
                        ctx,
                        label_style(FONT_SIZE_LABEL, true),
                        x,
                        y,
                        w,
                        h,
                        label,
                        COLOR_TEXT_SECONDARY,
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
        WidgetKind::Popover { caret_x, caret_up } => {
            draw_popover_panel(ctx, x, y, w, h, *caret_x, *caret_up);
        }
        WidgetKind::VScrollbar { t, thumb } => {
            // Same treatment as the side palette's scrollbar: a soft track
            // with the theme's proportional slider thumb.
            set_color(
                ctx,
                crate::backend::wayland::toolbar::render::side_palette::COLOR_SCROLLBAR_TRACK,
            );
            draw_round_rect(ctx, x, y, w, h, w / 2.0);
            let _ = ctx.fill();
            let thumb_h = (h * thumb.clamp(0.0, 1.0)).max(w * 2.0).min(h);
            let thumb_y = y + (h - thumb_h) * t.clamp(0.0, 1.0);
            set_color(ctx, crate::ui::theme::toolbar::COLOR_SCROLLBAR_SLIDER);
            draw_round_rect(ctx, x, thumb_y, w, thumb_h, w / 2.0);
            let _ = ctx.fill();
        }
    }
    paint_shortcut_badge(ctx, node);
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
            paint_node(&ctx, &node, None);
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
            paint_node(&ctx, &node, None);
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
