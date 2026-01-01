use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::draw::{Color, ORANGE};
use crate::input::Tool;
use crate::input::state::PresetFeedbackKind;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;

use super::super::super::widgets::*;
use super::SidePaletteLayout;
use super::format::{preset_tooltip_text, truncate_label};
use super::widgets::{draw_keycap, draw_preset_name_tag};

pub(super) struct PresetSlotLayout {
    pub(super) slot_size: f64,
    pub(super) slot_gap: f64,
    pub(super) slot_row_y: f64,
    pub(super) action_row_y: f64,
    pub(super) action_h: f64,
    pub(super) action_gap: f64,
    pub(super) action_w: f64,
    pub(super) action_icon: f64,
    pub(super) icon_size: f64,
    pub(super) swatch_size: f64,
    pub(super) number_box: f64,
    pub(super) keycap_pad: f64,
    pub(super) keycap_radius: f64,
    pub(super) card_x: f64,
    pub(super) card_w: f64,
    pub(super) section_y: f64,
}

pub(super) fn draw_preset_slot(
    layout: &mut SidePaletteLayout,
    layout_spec: &PresetSlotLayout,
    slot_index: usize,
) -> Option<Color> {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let x = layout.x;

    let slot = slot_index + 1;
    let slot_x = x + slot_index as f64 * (layout_spec.slot_size + layout_spec.slot_gap);
    let preset = snapshot
        .presets
        .get(slot_index)
        .and_then(|preset| preset.as_ref());
    let preset_exists = preset.is_some();
    let slot_hover = hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                slot_x,
                layout_spec.slot_row_y,
                layout_spec.slot_size,
                layout_spec.slot_size,
            )
        })
        .unwrap_or(false)
        && preset_exists;
    draw_button(
        ctx,
        slot_x,
        layout_spec.slot_row_y,
        layout_spec.slot_size,
        layout_spec.slot_size,
        false,
        slot_hover,
    );
    if let Some(preset) = preset {
        ctx.set_source_rgba(preset.color.r, preset.color.g, preset.color.b, 0.12);
        draw_round_rect(
            ctx,
            slot_x + 1.0,
            layout_spec.slot_row_y + 1.0,
            layout_spec.slot_size - 2.0,
            layout_spec.slot_size - 2.0,
            6.0,
        );
        let _ = ctx.fill();
        ctx.set_source_rgba(preset.color.r, preset.color.g, preset.color.b, 0.35);
        ctx.set_line_width(1.0);
        draw_round_rect(
            ctx,
            slot_x + 1.0,
            layout_spec.slot_row_y + 1.0,
            layout_spec.slot_size - 2.0,
            layout_spec.slot_size - 2.0,
            6.0,
        );
        let _ = ctx.stroke();
    } else {
        ctx.set_source_rgba(0.05, 0.05, 0.07, 0.35);
        draw_round_rect(
            ctx,
            slot_x + 1.0,
            layout_spec.slot_row_y + 1.0,
            layout_spec.slot_size - 2.0,
            layout_spec.slot_size - 2.0,
            6.0,
        );
        let _ = ctx.fill();
    }

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
        match preset.tool {
            Tool::Select => {
                toolbar_icons::draw_icon_select(ctx, icon_x, icon_y, layout_spec.icon_size)
            }
            Tool::Pen => toolbar_icons::draw_icon_pen(ctx, icon_x, icon_y, layout_spec.icon_size),
            Tool::Line => toolbar_icons::draw_icon_line(ctx, icon_x, icon_y, layout_spec.icon_size),
            Tool::Rect => toolbar_icons::draw_icon_rect(ctx, icon_x, icon_y, layout_spec.icon_size),
            Tool::Ellipse => {
                toolbar_icons::draw_icon_circle(ctx, icon_x, icon_y, layout_spec.icon_size)
            }
            Tool::Arrow => {
                toolbar_icons::draw_icon_arrow(ctx, icon_x, icon_y, layout_spec.icon_size)
            }
            Tool::Marker => {
                toolbar_icons::draw_icon_marker(ctx, icon_x, icon_y, layout_spec.icon_size)
            }
            Tool::Highlight => {
                toolbar_icons::draw_icon_highlight(ctx, icon_x, icon_y, layout_spec.icon_size)
            }
            Tool::Eraser => {
                toolbar_icons::draw_icon_eraser(ctx, icon_x, icon_y, layout_spec.icon_size)
            }
        }

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

    let key_x = slot_x + layout_spec.keycap_pad;
    let key_y = layout_spec.slot_row_y + layout_spec.keycap_pad;
    draw_keycap(
        ctx,
        key_x,
        key_y,
        layout_spec.number_box,
        layout_spec.keycap_radius,
        &slot.to_string(),
        preset_exists,
    );

    if let Some(feedback) = snapshot
        .preset_feedback
        .get(slot_index)
        .and_then(|feedback| feedback.as_ref())
    {
        let fade = (1.0 - feedback.progress as f64).clamp(0.0, 1.0);
        if fade > 0.0 {
            let (r, g, b) = match feedback.kind {
                PresetFeedbackKind::Apply => (0.35, 0.55, 0.95),
                PresetFeedbackKind::Save => (0.25, 0.75, 0.4),
                PresetFeedbackKind::Clear => (0.9, 0.3, 0.3),
            };
            ctx.set_source_rgba(r, g, b, 0.35 * fade);
            draw_round_rect(
                ctx,
                slot_x + 1.0,
                layout_spec.slot_row_y + 1.0,
                layout_spec.slot_size - 2.0,
                layout_spec.slot_size - 2.0,
                6.0,
            );
            let _ = ctx.fill();
        }
    }
    if preset_exists && snapshot.active_preset_slot == Some(slot) {
        ctx.set_source_rgba(ORANGE.r, ORANGE.g, ORANGE.b, 0.95);
        ctx.set_line_width(2.0);
        draw_round_rect(
            ctx,
            slot_x + 1.0,
            layout_spec.slot_row_y + 1.0,
            layout_spec.slot_size - 2.0,
            layout_spec.slot_size - 2.0,
            7.0,
        );
        let _ = ctx.stroke();
    }

    let save_hover = hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                slot_x,
                layout_spec.action_row_y,
                layout_spec.action_w,
                layout_spec.action_h,
            )
        })
        .unwrap_or(false);
    draw_button(
        ctx,
        slot_x,
        layout_spec.action_row_y,
        layout_spec.action_w,
        layout_spec.action_h,
        false,
        save_hover,
    );
    set_icon_color(ctx, save_hover);
    toolbar_icons::draw_icon_save(
        ctx,
        slot_x + (layout_spec.action_w - layout_spec.action_icon) / 2.0,
        layout_spec.action_row_y + (layout_spec.action_h - layout_spec.action_icon) / 2.0,
        layout_spec.action_icon,
    );
    hits.push(HitRegion {
        rect: (
            slot_x,
            layout_spec.action_row_y,
            layout_spec.action_w,
            layout_spec.action_h,
        ),
        event: ToolbarEvent::SavePreset(slot),
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            &format!("Save preset {}", slot),
            snapshot.binding_hints.save_preset(slot),
        )),
    });

    let clear_x = slot_x + layout_spec.action_w + layout_spec.action_gap;
    let clear_y = layout_spec.action_row_y;
    let clear_hover = hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                clear_x,
                clear_y,
                layout_spec.action_w,
                layout_spec.action_h,
            )
        })
        .unwrap_or(false)
        && preset_exists;
    draw_button(
        ctx,
        clear_x,
        clear_y,
        layout_spec.action_w,
        layout_spec.action_h,
        false,
        clear_hover,
    );
    if preset_exists {
        set_icon_color(ctx, clear_hover);
    } else {
        ctx.set_source_rgba(0.7, 0.7, 0.7, 0.6);
    }
    toolbar_icons::draw_icon_clear(
        ctx,
        clear_x + (layout_spec.action_w - layout_spec.action_icon) / 2.0,
        clear_y + (layout_spec.action_h - layout_spec.action_icon) / 2.0,
        layout_spec.action_icon,
    );
    if preset_exists {
        hits.push(HitRegion {
            rect: (clear_x, clear_y, layout_spec.action_w, layout_spec.action_h),
            event: ToolbarEvent::ClearPreset(slot),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                &format!("Clear preset {}", slot),
                snapshot.binding_hints.clear_preset(slot),
            )),
        });
    }

    hover_preset_color
}
