use crate::input::InputState;
use crate::input::state::ContextMenuState;
use crate::ui::primitives::draw_rounded_rect;
use crate::ui::theme::Rgba;
use crate::ui_text::{UiTextEngine, UiTextStyle};

use super::constants::{
    self, BG_HOVER, BORDER_FOCUS, FOCUS_RING_WIDTH, ICON_SUBMENU_ARROW, NAV_HINT_MENU,
    RADIUS_PANEL, RADIUS_SM, RADIUS_STD, TEXT_DISABLED, TEXT_HINT, TEXT_PRIMARY,
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
    render_context_menu_with_engine(
        &UiTextEngine::default(),
        ctx,
        input_state,
        _screen_width,
        _screen_height,
    );
}

pub(crate) fn render_context_menu_with_engine(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    input_state: &InputState,
    _screen_width: u32,
    _screen_height: u32,
) {
    let (hover_index, focus_index) = match input_state.context_menu.state() {
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
    constants::set_color(ctx, crate::ui::theme::popup::bg_context_menu());
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, crate::ui::theme::popup::border_context_menu());
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
        engine.draw_baseline(
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
            engine.draw_baseline(
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
    engine.draw_baseline(
        ctx,
        hint_style,
        NAV_HINT_MENU,
        layout.origin_x + layout.padding_x,
        hint_y + hint_padding + layout.font_size * 0.65,
        None,
    );

    let _ = ctx.restore();
}

#[cfg(test)]
mod engine_tests {
    use super::*;
    use crate::input::state::ContextMenuKind;

    fn paint(engine: &UiTextEngine, state: &InputState, density: i32) -> Vec<u8> {
        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, 640 * density, 480 * density)
                .unwrap();
        {
            let ctx = cairo::Context::new(&surface).unwrap();
            ctx.scale(f64::from(density), f64::from(density));
            render_context_menu_with_engine(engine, &ctx, state, 640, 480);
        }
        surface.data().unwrap().to_vec()
    }

    #[test]
    fn retained_context_menu_owner_preserves_layout_pixels_and_row_hits() {
        let engine = UiTextEngine::default();
        let mut state = crate::input::state::test_support::make_test_input_state();
        for (kind, density) in [
            (ContextMenuKind::Canvas, 1),
            (ContextMenuKind::Zoom, 2),
            (ContextMenuKind::Canvas, 1),
        ] {
            state.open_context_menu((620, 460), Vec::new(), kind, None);
            let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 640, 480).unwrap();
            let ctx = cairo::Context::new(&surface).unwrap();
            state.update_context_menu_layout_with_engine(&engine, &ctx, 640, 480);
            let layout = *state.context_menu_layout().unwrap();
            let actual = paint(&engine, &state, density);
            assert!(actual.iter().any(|&byte| byte != 0));
            state.update_context_menu_layout_with_engine(&UiTextEngine::default(), &ctx, 640, 480);
            let fresh = state.context_menu_layout().unwrap();
            assert_eq!(
                (fresh.origin_x, fresh.origin_y, fresh.width, fresh.height),
                (
                    layout.origin_x,
                    layout.origin_y,
                    layout.width,
                    layout.height
                )
            );
            assert_eq!(
                (fresh.row_height, fresh.shortcut_width, fresh.arrow_width),
                (layout.row_height, layout.shortcut_width, layout.arrow_width)
            );
            assert!(actual == paint(&UiTextEngine::default(), &state, density));
            for index in 0..state.context_menu_entries().len() {
                let x = (layout.origin_x + layout.padding_x) as i32;
                let y = (layout.origin_y
                    + layout.padding_y
                    + layout.row_height * (index as f64 + 0.5)) as i32;
                assert_eq!(state.context_menu_index_at(x, y), Some(index));
            }
        }
    }
}
