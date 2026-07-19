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
    COLOR_SWATCH_HAIRLINE, COLOR_TEXT_DISABLED, FONT_FAMILY_DEFAULT, set_color,
};
use super::widgets::{
    draw_button, draw_checkbox, draw_destructive_button, draw_disabled_button,
    draw_divider_vertical, draw_drag_handle, draw_group_card, draw_label_center,
    draw_label_center_color, draw_label_left, draw_mini_checkbox, draw_minimize_button,
    draw_panel_background, draw_pin_button, draw_popover_panel, draw_round_rect,
    draw_segmented_control, point_in_rect, set_icon_color,
};

/// Hover ring around an unselected swatch (dimmer sibling of the accent
/// selection ring).
const SWATCH_HOVER_RING: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.4);

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
        ShortcutBadgePlacement::Above | ShortcutBadgePlacement::Below => x + (w - badge_w) / 2.0,
    };
    let badge_y = match badge.placement {
        ShortcutBadgePlacement::Corner => y + 2.0,
        ShortcutBadgePlacement::Above => (y - badge_h - 1.0).max(1.0),
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
    // Above/Below key letters read as unboxed 9px captions in the secondary
    // text color; corner badges keep the boxed 8px icon-color treatment.
    let (font_size, label_color) = match badge.placement {
        ShortcutBadgePlacement::Corner => (8.0, COLOR_ICON_DEFAULT),
        ShortcutBadgePlacement::Above | ShortcutBadgePlacement::Below => (9.0, COLOR_LABEL_HINT),
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
            if style.disabled {
                draw_label_center_color(
                    ctx,
                    text_style,
                    x,
                    y,
                    w,
                    h,
                    &label.text,
                    COLOR_TEXT_DISABLED,
                );
            } else {
                draw_label_center(ctx, text_style, x, y, w, h, &label.text);
            }
        }
        WidgetKind::Icon { glyph } => {
            set_color(ctx, COLOR_ICON_DEFAULT);
            (glyph.0)(ctx, x, y, w.min(h));
        }
        WidgetKind::Label(label) => {
            let text_style = label_style(label.size, label.bold);
            draw_label_left(ctx, text_style, x, y, w, h, &label.text);
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
        WidgetKind::Swatch { color, selected } => {
            // Rounded square inset one pixel so the accent selection ring
            // (2px stroke, ~2px gap) stays clear of the neighbouring swatch.
            ctx.set_source_rgba(color.0, color.1, color.2, color.3);
            draw_round_rect(ctx, x + 1.0, y + 1.0, w - 2.0, h - 2.0, 5.0);
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
    }
    paint_shortcut_badge(ctx, node);
}
