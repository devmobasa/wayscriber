use crate::input::InputState;

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

    let _ = ctx.save();
    ctx.set_source_rgba(0.08, 0.11, 0.17, 0.92);
    ctx.rectangle(
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
    );
    let _ = ctx.fill();

    ctx.set_source_rgba(0.18, 0.22, 0.3, 0.95);
    ctx.set_line_width(1.0);
    ctx.rectangle(
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
    );
    let _ = ctx.stroke();

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(title_font_size);
    if panel.multiple_selection {
        ctx.set_source_rgba(0.88, 0.91, 0.97, 1.0);
    } else {
        ctx.set_source_rgba(0.93, 0.95, 0.99, 1.0);
    }
    ctx.move_to(layout.label_x, layout.title_baseline_y);
    let _ = ctx.show_text(&panel.title);

    ctx.set_source_rgba(0.35, 0.4, 0.5, 0.9);
    ctx.move_to(layout.label_x, layout.title_baseline_y + 4.0);
    ctx.line_to(
        layout.origin_x + layout.width - layout.padding_x,
        layout.title_baseline_y + 4.0,
    );
    let _ = ctx.stroke();

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(body_font_size);
    ctx.set_source_rgba(0.86, 0.89, 0.95, 1.0);
    let mut text_y = layout.info_start_y;
    for line in &panel.lines {
        ctx.move_to(layout.label_x, text_y);
        let _ = ctx.show_text(line);
        text_y += line_height;
    }

    if !panel.entries.is_empty() {
        let active_index = panel.hover_index.or(panel.keyboard_focus);
        for (index, entry) in panel.entries.iter().enumerate() {
            let row_top = layout.entry_start_y + layout.entry_row_height * index as f64;
            let row_center = row_top + layout.entry_row_height * 0.5;

            if active_index == Some(index) && !entry.disabled {
                ctx.set_source_rgba(0.25, 0.32, 0.45, 0.9);
                ctx.rectangle(
                    layout.origin_x,
                    row_top,
                    layout.width,
                    layout.entry_row_height,
                );
                let _ = ctx.fill();
            }

            let (text_r, text_g, text_b, text_a) = if entry.disabled {
                (0.6, 0.64, 0.68, 0.5)
            } else {
                (0.9, 0.92, 0.97, 1.0)
            };
            ctx.set_source_rgba(text_r, text_g, text_b, text_a);
            ctx.move_to(layout.label_x, row_center + body_font_size * 0.35);
            let _ = ctx.show_text(&entry.label);

            ctx.set_source_rgba(0.7, 0.73, 0.78, text_a);
            ctx.move_to(layout.value_x, row_center + body_font_size * 0.35);
            let _ = ctx.show_text(&entry.value);
        }
    }

    let _ = ctx.restore();
}
