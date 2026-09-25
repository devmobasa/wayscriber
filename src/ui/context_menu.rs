use crate::input::InputState;
use crate::input::state::{ContextMenuEntry, ContextMenuLayout, ContextMenuState, SubmenuSide};
use crate::ui::primitives::draw_rounded_rect;
use crate::ui::theme::Rgba;
use crate::ui_text::{UiTextEngine, UiTextStyle};

use super::constants::{
    self, BG_EXPANDED, BG_HOVER, BORDER_FOCUS, DIVIDER_LIGHT, FOCUS_RING_WIDTH, ICON_SUBMENU_ARROW,
    RADIUS_PANEL, RADIUS_SM, SHADOW, TEXT_DISABLED, TEXT_HINT, TEXT_PRIMARY,
};

/// Footer hint text: slightly brighter than TEXT_TERTIARY so the key hints
/// stay legible inside the menu (kept from pre-theme literals).
const HINT_FOOTER_TEXT: Rgba = (0.65, 0.68, 0.75, 1.0);
/// Accent bar on the parent row of an open submenu.
const EXPANDED_BAR_WIDTH: f64 = 3.0;
/// The submenu's shadow: a few layers stepping outward stand in for a blur.
const SHADOW_LAYERS: u32 = 3;
const SHADOW_SPREAD: f64 = 2.0;
const SHADOW_OFFSET_Y: f64 = 2.0;
/// How far the submenu's shadow reaches past its pane.
const SHADOW_EXTENT: f64 = SHADOW_LAYERS as f64 * SHADOW_SPREAD + SHADOW_OFFSET_Y;

/// Renders a floating context menu for shape or canvas actions.
pub fn render_context_menu(ctx: &cairo::Context, input_state: &InputState) {
    render_context_menu_with_engine(&UiTextEngine::default(), ctx, input_state);
}

pub(crate) fn render_context_menu_with_engine(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    input_state: &InputState,
) {
    let ContextMenuState::Open {
        hover_index,
        keyboard_focus,
        submenu,
        ..
    } = input_state.context_menu.state()
    else {
        return;
    };

    let entries = input_state.context_menu_entries();
    if entries.is_empty() {
        return;
    }

    let Some(layout) = input_state.context_menu_layout().copied() else {
        return;
    };

    let _ = ctx.save();
    let side = input_state.context_submenu_side();
    draw_menu(
        engine,
        ctx,
        &layout,
        &entries,
        MenuStyle {
            surface: crate::ui::theme::popup::bg_context_menu(),
            arrow_side: side,
            shadow: false,
        },
        RowHighlight {
            hover: *hover_index,
            focus: *keyboard_focus,
            expanded: submenu.map(|submenu| submenu.parent_index),
        },
    );
    draw_hint_footer(engine, ctx, &layout, input_state.context_menu_footer_hint());

    // An open submenu paints above the menu it opens from.
    let submenu_entries = input_state.context_submenu_entries();
    if let (Some(submenu), Some(pane)) = (submenu, input_state.context_submenu_layout())
        && !submenu_entries.is_empty()
    {
        draw_menu(
            engine,
            ctx,
            pane,
            &submenu_entries,
            MenuStyle {
                surface: crate::ui::theme::popup::bg_context_submenu(),
                arrow_side: side,
                shadow: true,
            },
            RowHighlight {
                hover: submenu.hover_index,
                focus: submenu.keyboard_focus,
                expanded: None,
            },
        );
    }

    let _ = ctx.restore();
}

/// Bounds of the open menu, hint footer included, as laid out for this frame.
pub(crate) fn context_menu_visual_geometry(
    input_state: &InputState,
) -> Option<(f64, f64, f64, f64)> {
    let layout = input_state.context_menu_layout()?;
    Some((
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
    ))
}

/// Bounds of the open submenu and its shadow, as laid out for this frame.
pub(crate) fn context_submenu_visual_geometry(
    input_state: &InputState,
) -> Option<(f64, f64, f64, f64)> {
    let pane = input_state.context_submenu_layout()?;
    Some((
        pane.origin_x - SHADOW_EXTENT,
        pane.origin_y - SHADOW_EXTENT,
        pane.width + SHADOW_EXTENT * 2.0,
        pane.height + SHADOW_EXTENT * 2.0,
    ))
}

/// The highlighted rows of one menu.
#[derive(Clone, Copy)]
struct RowHighlight {
    hover: Option<usize>,
    focus: Option<usize>,
    /// The row whose submenu is open.
    expanded: Option<usize>,
}

/// How one menu pane is dressed.
#[derive(Clone, Copy)]
struct MenuStyle {
    surface: Rgba,
    /// Where submenu arrows point.
    arrow_side: SubmenuSide,
    /// A drop shadow lifts a pane above the menu it opens from.
    shadow: bool,
}

fn draw_menu(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    layout: &ContextMenuLayout,
    entries: &[ContextMenuEntry],
    style: MenuStyle,
    highlight: RowHighlight,
) {
    let text_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: layout.font_size,
    };

    if style.shadow {
        draw_shadow(ctx, layout);
    }

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
    constants::set_color(ctx, style.surface);
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, crate::ui::theme::popup::border_context_menu());
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    for (index, entry) in entries.iter().enumerate() {
        let row_top = layout.origin_y + layout.padding_y + layout.row_height * index as f64;
        let row_center = row_top + layout.row_height * 0.5;

        // Hover fills the row, keyboard focus rings it, and the parent of an
        // open submenu keeps a quieter fill with an accent bar.
        let is_hovered = highlight.hover == Some(index) && !entry.disabled;
        let is_expanded = highlight.expanded == Some(index) && !entry.disabled;
        let is_focused = highlight.focus == Some(index) && !entry.disabled;

        if is_hovered || is_expanded {
            constants::set_color(ctx, if is_hovered { BG_HOVER } else { BG_EXPANDED });
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
        if is_expanded {
            constants::set_color(ctx, BORDER_FOCUS);
            let bar_x = match style.arrow_side {
                SubmenuSide::Right => layout.origin_x + layout.width - 4.0 - EXPANDED_BAR_WIDTH,
                SubmenuSide::Left => layout.origin_x + 4.0,
            };
            ctx.rectangle(
                bar_x,
                row_top + 3.0,
                EXPANDED_BAR_WIDTH,
                layout.row_height - 6.0,
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

        if entry.submenu.is_some() {
            let arrow_x =
                layout.origin_x + layout.width - layout.padding_x - layout.arrow_width * 0.6;
            let arrow_y = row_center;
            let arrow_color = if is_expanded {
                BORDER_FOCUS
            } else {
                ICON_SUBMENU_ARROW
            };
            constants::set_color(ctx, constants::with_alpha(arrow_color, text_a));
            // The arrow points to the side the pane opens on.
            let (base_x, tip_x) = match style.arrow_side {
                SubmenuSide::Right => (arrow_x, arrow_x + 6.0),
                SubmenuSide::Left => (arrow_x + 6.0, arrow_x),
            };
            ctx.move_to(base_x, arrow_y - 5.0);
            ctx.line_to(tip_x, arrow_y);
            ctx.line_to(base_x, arrow_y + 5.0);
            let _ = ctx.fill();
        }
    }

    // Group dividers go on top of the row fills so hover never hides them.
    for (index, entry) in entries.iter().enumerate() {
        if index == 0 || !entry.separator_before {
            continue;
        }
        let y =
            (layout.origin_y + layout.padding_y + layout.row_height * index as f64).round() + 0.5;
        draw_divider(ctx, layout, y);
    }
}

/// A hairline across the menu between its horizontal paddings.
fn draw_divider(ctx: &cairo::Context, layout: &ContextMenuLayout, y: f64) {
    constants::set_color(ctx, DIVIDER_LIGHT);
    ctx.set_line_width(1.0);
    ctx.move_to(layout.origin_x + layout.padding_x, y);
    ctx.line_to(layout.origin_x + layout.width - layout.padding_x, y);
    let _ = ctx.stroke();
}

/// A soft shadow under a pane: stacked translucent rects widening outward.
fn draw_shadow(ctx: &cairo::Context, layout: &ContextMenuLayout) {
    let alpha = SHADOW.3 / SHADOW_LAYERS as f64;
    for layer in 1..=SHADOW_LAYERS {
        let spread = SHADOW_SPREAD * layer as f64;
        constants::set_color(ctx, constants::with_alpha(SHADOW, alpha));
        draw_rounded_rect(
            ctx,
            layout.origin_x - spread,
            layout.origin_y - spread + SHADOW_OFFSET_Y,
            layout.width + spread * 2.0,
            layout.height + spread * 2.0,
            RADIUS_PANEL + spread,
        );
        let _ = ctx.fill();
    }
}

/// Key-hint footer inside the bottom of the menu, below a divider. The menu
/// was measured wide enough for every hint it can show.
fn draw_hint_footer(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    layout: &ContextMenuLayout,
    hint: &str,
) {
    if layout.footer_height <= 0.0 {
        return;
    }
    let hint_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: layout.footer_font_size,
    };
    let footer_top = layout.origin_y + layout.height - layout.padding_y - layout.footer_height;

    draw_divider(ctx, layout, footer_top.round() + 0.5);
    constants::set_color(ctx, HINT_FOOTER_TEXT);
    engine.draw_baseline(
        ctx,
        hint_style,
        hint,
        layout.origin_x + layout.padding_x,
        footer_top + layout.footer_height * 0.5 + layout.footer_font_size * 0.4,
        None,
    );
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
            render_context_menu_with_engine(engine, &ctx, state);
        }
        surface.data().unwrap().to_vec()
    }

    // Runs separately in tools/lint-and-test.sh under both feature configurations.
    // Parallel font rendering has triggered a native Cairo/FreeType crash; the
    // related upstream race is tracked at cairo/cairo!81. Process isolation keeps
    // these assertions covered without claiming the native race is fixed.
    #[test]
    #[ignore = "isolated by tools/lint-and-test.sh; Cairo race: https://gitlab.freedesktop.org/cairo/cairo/-/merge_requests/81"]
    fn retained_context_menu_owner_preserves_layout_pixels_and_row_hits() {
        let engine = UiTextEngine::default();
        let mut state = crate::input::state::test_support::make_test_input_state();
        for (kind, density) in [
            (ContextMenuKind::Canvas, 1),
            (ContextMenuKind::Zoom, 2),
            (ContextMenuKind::Canvas, 1),
        ] {
            state.open_context_menu((620, 460), Vec::new(), kind, None);
            state.update_context_menu_layout_with_engine(&engine, 640, 480);
            let layout = *state.context_menu_layout().unwrap();
            let actual = paint(&engine, &state, density);
            assert!(actual.iter().any(|&byte| byte != 0));
            state.update_context_menu_layout_with_engine(&UiTextEngine::default(), 640, 480);
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

        // A submenu paints with its menu and hit-tests its own entries. It sits
        // beside the menu, or over it when 640px leaves no room on either side.
        state.open_context_menu((20, 20), Vec::new(), ContextMenuKind::Canvas, None);
        let boards = state
            .context_menu_entries()
            .iter()
            .position(|entry| entry.label == "Boards")
            .unwrap();
        assert!(state.open_context_submenu(boards, false));
        state.update_context_menu_layout_with_engine(&engine, 640, 480);
        let pane = *state.context_submenu_layout().unwrap();
        assert!(pane.origin_x >= 6.0 && pane.origin_x + pane.width <= 634.0);
        let actual = paint(&engine, &state, 1);
        assert!(actual == paint(&UiTextEngine::default(), &state, 1));
        for index in 0..state.context_submenu_entries().len() {
            let x = (pane.origin_x + pane.padding_x) as i32;
            let y =
                (pane.origin_y + pane.padding_y + pane.row_height * (index as f64 + 0.5)) as i32;
            assert_eq!(state.context_submenu_index_at(x, y), Some(index));
        }
    }
}
