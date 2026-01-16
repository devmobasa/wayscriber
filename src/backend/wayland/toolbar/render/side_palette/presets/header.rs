use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui_text::{UiTextStyle, text_layout};

use super::super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
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
    let label_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_LABEL,
    };

    draw_section_label(
        ctx,
        label_style,
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
        let hint_style = UiTextStyle {
            family: FONT_FAMILY_DEFAULT,
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Normal,
            size: 10.0,
        };
        let layout = text_layout(ctx, hint_style, &hint, None);
        let ext = layout.ink_extents();
        let hint_x = card_x + card_w - ext.width() - 8.0 - ext.x_bearing();
        let hint_y = section_y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y;
        ctx.set_source_rgba(0.7, 0.7, 0.75, 0.8);
        layout.show_at_baseline(ctx, hint_x, hint_y);
    }
}
