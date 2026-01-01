use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;

use super::super::super::widgets::draw_section_label;
use super::SidePaletteLayout;

pub(super) fn draw_presets_header(
    layout: &mut SidePaletteLayout,
    section_y: f64,
    card_x: f64,
    card_w: f64,
    slot_count: usize,
) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let x = layout.x;

    draw_section_label(
        ctx,
        x,
        section_y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
        "Presets",
    );

    let apply_hint = {
        let mut uses_digit_bindings = true;
        for slot in 1..=slot_count {
            let expected = slot.to_string();
            if snapshot.binding_hints.apply_preset(slot) != Some(expected.as_str()) {
                uses_digit_bindings = false;
                break;
            }
        }
        if uses_digit_bindings {
            Some(format!("Keys 1-{} apply", slot_count))
        } else {
            Some("Keys apply presets".to_string())
        }
    };
    if let Some(hint) = apply_hint {
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(10.0);
        if let Ok(ext) = ctx.text_extents(&hint) {
            let hint_x = card_x + card_w - ext.width() - 8.0 - ext.x_bearing();
            let hint_y = section_y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y;
            ctx.set_source_rgba(0.7, 0.7, 0.75, 0.8);
            ctx.move_to(hint_x, hint_y);
            let _ = ctx.show_text(&hint);
        }
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(13.0);
    }
}
