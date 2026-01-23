use crate::input::InputState;
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::constants::{
    self, BG_HOVER, BORDER_FOCUS, BORDER_PROPERTIES, DIVIDER, EMPTY_PROPERTIES, FOCUS_RING_WIDTH,
    PANEL_BG_PROPERTIES, TEXT_DISABLED, TEXT_HINT, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY,
};

pub fn render_properties_panel(
    ctx: &cairo::Context,
    input_state: &InputState,
    _screen_width: u32,
    _screen_height: u32,
) {
    let panel = match input_state.properties_panel() {
        Some(panel) => panel,
        None => return,
    };
    let layout = match input_state.properties_panel_layout() {
        Some(layout) => layout,
        None => return,
    };

    let title_font_size = 15.0;
    let body_font_size = 13.0;
    let line_height = 18.0;
    let title_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: title_font_size,
    };
    let body_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: body_font_size,
    };

    let _ = ctx.save();
    constants::set_color(ctx, PANEL_BG_PROPERTIES);
    ctx.rectangle(
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
    );
    let _ = ctx.fill();

    constants::set_color(ctx, BORDER_PROPERTIES);
    ctx.set_line_width(1.0);
    ctx.rectangle(
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
    );
    let _ = ctx.stroke();

    if panel.multiple_selection {
        constants::set_color(ctx, TEXT_SECONDARY);
    } else {
        constants::set_color(ctx, TEXT_PRIMARY);
    }
    draw_text_baseline(
        ctx,
        title_style,
        &panel.title,
        layout.label_x,
        layout.title_baseline_y,
        None,
    );

    constants::set_color(ctx, DIVIDER);
    ctx.move_to(layout.label_x, layout.title_baseline_y + 4.0);
    ctx.line_to(
        layout.origin_x + layout.width - layout.padding_x,
        layout.title_baseline_y + 4.0,
    );
    let _ = ctx.stroke();

    // Show empty state if no content
    if panel.lines.is_empty() && panel.entries.is_empty() {
        let empty_style = UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Italic,
            weight: cairo::FontWeight::Normal,
            size: body_font_size,
        };
        constants::set_color(ctx, TEXT_TERTIARY);
        let empty_y = layout.info_start_y + line_height;
        draw_text_baseline(
            ctx,
            empty_style,
            EMPTY_PROPERTIES,
            layout.label_x,
            empty_y,
            None,
        );
        let _ = ctx.restore();
        return;
    }

    constants::set_color(ctx, TEXT_SECONDARY);
    let mut text_y = layout.info_start_y;
    for line in &panel.lines {
        draw_text_baseline(ctx, body_style, line, layout.label_x, text_y, None);
        text_y += line_height;
    }

    if !panel.entries.is_empty() {
        for (index, entry) in panel.entries.iter().enumerate() {
            let row_top = layout.entry_start_y + layout.entry_row_height * index as f64;
            let row_center = row_top + layout.entry_row_height * 0.5;

            // Distinguish hover from keyboard focus
            let is_hovered = panel.hover_index == Some(index) && !entry.disabled;
            let is_focused = panel.keyboard_focus == Some(index) && !entry.disabled;

            if is_hovered {
                constants::set_color(ctx, BG_HOVER);
                ctx.rectangle(
                    layout.origin_x,
                    row_top,
                    layout.width,
                    layout.entry_row_height,
                );
                let _ = ctx.fill();
            }

            if is_focused && !is_hovered {
                // Draw focus ring for keyboard navigation
                constants::set_color(ctx, BORDER_FOCUS);
                ctx.set_line_width(FOCUS_RING_WIDTH);
                ctx.rectangle(
                    layout.origin_x + 2.0,
                    row_top + 1.0,
                    layout.width - 4.0,
                    layout.entry_row_height - 2.0,
                );
                let _ = ctx.stroke();
            }

            let (text_r, text_g, text_b, text_a) = if entry.disabled {
                TEXT_DISABLED
            } else {
                TEXT_PRIMARY
            };
            ctx.set_source_rgba(text_r, text_g, text_b, text_a);
            draw_text_baseline(
                ctx,
                body_style,
                &entry.label,
                layout.label_x,
                row_center + body_font_size * 0.35,
                None,
            );

            let value_color = constants::with_alpha(TEXT_HINT, text_a);
            constants::set_color(ctx, value_color);
            draw_text_baseline(
                ctx,
                body_style,
                &entry.value,
                layout.value_x,
                row_center + body_font_size * 0.35,
                None,
            );
        }
    }

    let _ = ctx.restore();
}
