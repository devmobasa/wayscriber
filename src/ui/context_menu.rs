use crate::input::InputState;
use crate::input::state::ContextMenuState;
use crate::ui_text::{UiTextStyle, draw_text_baseline};

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

    // Background and border
    ctx.set_source_rgba(0.1, 0.13, 0.17, 0.95);
    ctx.rectangle(
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
    );
    let _ = ctx.fill();

    ctx.set_source_rgba(0.18, 0.22, 0.28, 0.9);
    ctx.set_line_width(1.0);
    ctx.rectangle(
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
    );
    let _ = ctx.stroke();

    let active_index = hover_index.or(focus_index);

    for (index, entry) in entries.iter().enumerate() {
        let row_top = layout.origin_y + layout.padding_y + layout.row_height * index as f64;
        let row_center = row_top + layout.row_height * 0.5;

        if active_index == Some(index) && !entry.disabled {
            ctx.set_source_rgba(0.25, 0.32, 0.45, 0.9);
            ctx.rectangle(layout.origin_x, row_top, layout.width, layout.row_height);
            let _ = ctx.fill();
        }

        let (text_r, text_g, text_b, text_a) = if entry.disabled {
            (0.6, 0.64, 0.68, 0.5)
        } else {
            (0.9, 0.92, 0.97, 1.0)
        };

        ctx.set_source_rgba(text_r, text_g, text_b, text_a);
        draw_text_baseline(
            ctx,
            text_style,
            &entry.label,
            layout.origin_x + layout.padding_x,
            row_center + layout.font_size * 0.35,
            None,
        );

        if let Some(shortcut) = &entry.shortcut {
            ctx.set_source_rgba(0.7, 0.73, 0.78, text_a);
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
            ctx.set_source_rgba(0.75, 0.78, 0.84, text_a);
            ctx.move_to(arrow_x, arrow_y - 5.0);
            ctx.line_to(arrow_x + 6.0, arrow_y);
            ctx.line_to(arrow_x, arrow_y + 5.0);
            let _ = ctx.fill();
        }
    }

    let _ = ctx.restore();
}
