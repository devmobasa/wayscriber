use crate::input::InputState;
use crate::input::state::ContextMenuState;
use crate::ui::primitives::draw_rounded_rect;
use crate::ui::theme::Rgba;
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::constants::{
    self, BG_HOVER, BORDER_CONTEXT_MENU, BORDER_FOCUS, FOCUS_RING_WIDTH, ICON_SUBMENU_ARROW,
    NAV_HINT_MENU, PANEL_BG_CONTEXT_MENU, RADIUS_PANEL, RADIUS_SM, RADIUS_STD, TEXT_DISABLED,
    TEXT_HINT, TEXT_PRIMARY,
};

/// Footer strip below the menu: darker than the menu surface so the hint reads
/// as an attachment (no matching theme token; kept from pre-theme literals).
const HINT_FOOTER_BG: Rgba = (0.08, 0.10, 0.14, 0.9);
/// Footer hint text: slightly brighter than TEXT_TERTIARY for legibility on
/// the darker strip (kept from pre-theme literals).
const HINT_FOOTER_TEXT: Rgba = (0.65, 0.68, 0.75, 1.0);

/// Renders a floating context menu for shape or canvas actions.
pub fn render_context_menu(
    ctx: &cairo::Context,
    input_state: &InputState,
    _screen_width: u32,
    _screen_height: u32,
) {
    let (hover_index, focus_index) = match &input_state.context_menu_state {
        ContextMenuState::Open {
            hover_index,
            keyboard_focus,
            ..
        } => (*hover_index, *keyboard_focus),
        ContextMenuState::Hidden => return,
    };

    let entries = input_state.context_menu_entries();
    if entries.is_empty() {
        return;
    }

    let layout = match input_state.context_menu_layout() {
        Some(layout) => *layout,
        None => return,
    };

    let _ = ctx.save();
    let text_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: layout.font_size,
    };

    // Background and hairline border (popover radius, matching the other
    // overlay popups)
    draw_rounded_rect(
        ctx,
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
        RADIUS_PANEL,
    );
    constants::set_color(ctx, PANEL_BG_CONTEXT_MENU);
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, BORDER_CONTEXT_MENU);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    for (index, entry) in entries.iter().enumerate() {
        let row_top = layout.origin_y + layout.padding_y + layout.row_height * index as f64;
        let row_center = row_top + layout.row_height * 0.5;

        // Distinguish hover (filled background) from keyboard focus (border ring)
        let is_hovered = hover_index == Some(index) && !entry.disabled;
        let is_focused = focus_index == Some(index) && !entry.disabled;

        if is_hovered {
            constants::set_color(ctx, BG_HOVER);
            draw_rounded_rect(
                ctx,
                layout.origin_x + 4.0,
                row_top,
                layout.width - 8.0,
                layout.row_height,
                RADIUS_SM,
            );
            let _ = ctx.fill();
        }

        if is_focused && !is_hovered {
            // Draw focus ring (outline) when keyboard navigating
            constants::set_color(ctx, BORDER_FOCUS);
            ctx.set_line_width(FOCUS_RING_WIDTH);
            draw_rounded_rect(
                ctx,
                layout.origin_x + 2.0,
                row_top + 1.0,
                layout.width - 4.0,
                layout.row_height - 2.0,
                RADIUS_SM,
            );
            let _ = ctx.stroke();
        }

        let text_color = if entry.disabled {
            TEXT_DISABLED
        } else {
            TEXT_PRIMARY
        };
        let text_a = text_color.3;

        constants::set_color(ctx, text_color);
        draw_text_baseline(
            ctx,
            text_style,
            &entry.label,
            layout.origin_x + layout.padding_x,
            row_center + layout.font_size * 0.35,
            None,
        );

        if let Some(shortcut) = &entry.shortcut {
            let shortcut_color = constants::with_alpha(TEXT_HINT, text_a);
            constants::set_color(ctx, shortcut_color);
            let shortcut_x = layout.origin_x + layout.width
                - layout.padding_x
                - layout.arrow_width
                - layout.shortcut_width;
            draw_text_baseline(
                ctx,
                text_style,
                shortcut,
                shortcut_x,
                row_center + layout.font_size * 0.35,
                None,
            );
        }

        if entry.has_submenu {
            let arrow_x =
                layout.origin_x + layout.width - layout.padding_x - layout.arrow_width * 0.6;
            let arrow_y = row_center;
            constants::set_color(ctx, constants::with_alpha(ICON_SUBMENU_ARROW, text_a));
            ctx.move_to(arrow_x, arrow_y - 5.0);
            ctx.line_to(arrow_x + 6.0, arrow_y);
            ctx.line_to(arrow_x, arrow_y + 5.0);
            let _ = ctx.fill();
        }
    }

    // Navigation hint footer with background for visibility
    let hint_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: layout.font_size * 0.8,
    };
    let hint_padding = 6.0;
    let hint_height = layout.font_size * 0.8 + hint_padding * 2.0;
    let hint_y = layout.origin_y + layout.height + 4.0;

    // Draw hint background
    constants::set_color(ctx, HINT_FOOTER_BG);
    draw_rounded_rect(
        ctx,
        layout.origin_x,
        hint_y,
        layout.width,
        hint_height,
        RADIUS_STD,
    );
    let _ = ctx.fill();

    // Draw hint text
    constants::set_color(ctx, HINT_FOOTER_TEXT);
    draw_text_baseline(
        ctx,
        hint_style,
        NAV_HINT_MENU,
        layout.origin_x + layout.padding_x,
        hint_y + hint_padding + layout.font_size * 0.65,
        None,
    );

    let _ = ctx.restore();
}
