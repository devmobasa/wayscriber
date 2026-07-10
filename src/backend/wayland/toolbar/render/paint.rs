//! Painter for view-engine widget trees.
//!
//! Lives render-side so it can reach the private widget draw functions; it
//! is the only consumer of a tree's visual data. Hover is resolved here at
//! paint time against each node's drawn rect (matching the legacy renderer),
//! so pointer motion never rebuilds a tree.

use crate::backend::wayland::toolbar::view::{ButtonStyle, WidgetKind, WidgetNode, WidgetTree};
use crate::ui_text::UiTextStyle;

use super::widgets::constants::{
    COLOR_ICON_DEFAULT, COLOR_TEXT_DISABLED, FONT_FAMILY_DEFAULT, set_color,
};
use super::widgets::{
    draw_button, draw_checkbox, draw_destructive_button, draw_disabled_button,
    draw_divider_vertical, draw_drag_handle, draw_group_card, draw_label_center,
    draw_label_center_color, draw_label_left, draw_mini_checkbox, draw_minimize_button,
    draw_panel_background, draw_pin_button, draw_popover_panel, draw_round_rect,
    draw_segmented_control, point_in_rect, set_icon_color,
};

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

fn paint_node(ctx: &cairo::Context, node: &WidgetNode, hover: Option<(f64, f64)>) {
    let (x, y, w, h) = node.rect;
    let is_hover = hovered(node, hover) && node.interact.is_some();
    match &node.kind {
        WidgetKind::Panel => draw_panel_background(ctx, w, h),
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
            let icon_x = x + (w - icon_size) / 2.0;
            let icon_y = y + (h - icon_size) / 2.0;
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
            ctx.set_source_rgba(color.0, color.1, color.2, color.3);
            draw_round_rect(ctx, x, y, w, h, 4.0);
            let _ = ctx.fill();
            if *selected || is_hover {
                ctx.set_source_rgba(1.0, 1.0, 1.0, if *selected { 0.95 } else { 0.5 });
                ctx.set_line_width(2.0);
                draw_round_rect(ctx, x - 1.0, y - 1.0, w + 2.0, h + 2.0, 5.0);
                let _ = ctx.stroke();
            }
        }
        WidgetKind::PinButton { pinned } => {
            draw_pin_button(ctx, x, y, w, *pinned, is_hover);
        }
        WidgetKind::MinimizeButton => draw_minimize_button(ctx, x, y, w, is_hover),
        WidgetKind::Popover { caret_x, caret_up } => {
            draw_popover_panel(ctx, x, y, w, h, *caret_x, *caret_up);
        }
    }
}
