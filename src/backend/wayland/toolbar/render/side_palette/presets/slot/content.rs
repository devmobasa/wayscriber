use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::draw::Color;
use crate::input::Tool;
use crate::toolbar_icons;
use crate::ui::toolbar::{PresetSlotSnapshot, ToolbarEvent, ToolbarSnapshot};

use super::super::super::super::widgets::{draw_round_rect, draw_swatch};
use super::super::format::{preset_tooltip_text, truncate_label};
use super::super::widgets::draw_preset_name_tag;
use super::PresetSlotLayout;

pub(super) fn draw_preset_content(
    ctx: &cairo::Context,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
    layout_spec: &PresetSlotLayout,
    slot_x: f64,
    slot: usize,
    slot_hover: bool,
    preset: Option<&PresetSlotSnapshot>,
) -> Option<Color> {
    let mut hover_preset_color = None;

    if let Some(preset) = preset {
        if slot_hover {
            hover_preset_color = Some(preset.color);
        }
        let preset_name = preset
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty());
        let tooltip = preset_tooltip_text(preset, slot, snapshot.binding_hints.apply_preset(slot));
        hits.push(HitRegion {
            rect: (
                slot_x,
                layout_spec.slot_row_y,
                layout_spec.slot_size,
                layout_spec.slot_size,
            ),
            event: ToolbarEvent::ApplyPreset(slot),
            kind: HitKind::Click,
            tooltip: Some(tooltip),
        });

        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        let icon_x = slot_x + (layout_spec.slot_size - layout_spec.icon_size) / 2.0;
        let icon_y = layout_spec.slot_row_y + (layout_spec.slot_size - layout_spec.icon_size) / 2.0;
        draw_preset_icon(ctx, preset.tool, icon_x, icon_y, layout_spec.icon_size);

        let preview_thickness = (preset.size / 50.0 * 6.0).clamp(1.0, 6.0);
        let preview_y = layout_spec.slot_row_y + layout_spec.slot_size - 6.0;
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.8);
        ctx.set_line_width(preview_thickness);
        ctx.move_to(slot_x + 4.0, preview_y);
        ctx.line_to(slot_x + layout_spec.slot_size - 4.0, preview_y);
        let _ = ctx.stroke();

        let swatch_x = slot_x + layout_spec.slot_size - layout_spec.swatch_size - 4.0;
        let swatch_y =
            layout_spec.slot_row_y + layout_spec.slot_size - layout_spec.swatch_size - 4.0;
        draw_swatch(
            ctx,
            swatch_x,
            swatch_y,
            layout_spec.swatch_size,
            preset.color,
            false,
        );
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.75);
        ctx.set_line_width(1.0);
        draw_round_rect(
            ctx,
            swatch_x,
            swatch_y,
            layout_spec.swatch_size,
            layout_spec.swatch_size,
            4.0,
        );
        let _ = ctx.stroke();

        if slot_hover && let Some(name) = preset_name {
            let display_name = truncate_label(name, 12);
            draw_preset_name_tag(
                ctx,
                &display_name,
                slot_x,
                layout_spec.slot_row_y,
                layout_spec.slot_size,
                layout_spec.card_x,
                layout_spec.card_w,
                layout_spec.section_y,
            );
        }
    } else {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.35);
        ctx.set_line_width(1.0);
        ctx.set_dash(&[3.0, 2.0], 0.0);
        draw_round_rect(
            ctx,
            slot_x + 1.0,
            layout_spec.slot_row_y + 1.0,
            layout_spec.slot_size - 2.0,
            layout_spec.slot_size - 2.0,
            6.0,
        );
        let _ = ctx.stroke();
        ctx.set_dash(&[], 0.0);
    }

    hover_preset_color
}

fn draw_preset_icon(ctx: &cairo::Context, tool: Tool, x: f64, y: f64, size: f64) {
    match tool {
        Tool::Select => toolbar_icons::draw_icon_select(ctx, x, y, size),
        Tool::Pen => toolbar_icons::draw_icon_pen(ctx, x, y, size),
        Tool::Line => toolbar_icons::draw_icon_line(ctx, x, y, size),
        Tool::Rect => toolbar_icons::draw_icon_rect(ctx, x, y, size),
        Tool::Ellipse => toolbar_icons::draw_icon_circle(ctx, x, y, size),
        Tool::Arrow => toolbar_icons::draw_icon_arrow(ctx, x, y, size),
        Tool::Marker => toolbar_icons::draw_icon_marker(ctx, x, y, size),
        Tool::Highlight => toolbar_icons::draw_icon_highlight(ctx, x, y, size),
        Tool::Eraser => toolbar_icons::draw_icon_eraser(ctx, x, y, size),
    }
}
