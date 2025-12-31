use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::draw::{Color, EraserKind, ORANGE};
use crate::input::state::PresetFeedbackKind;
use crate::input::{EraserMode, Tool};
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::util::color_to_name;

use super::super::widgets::*;

pub(super) fn draw_presets_section(layout: &mut SidePaletteLayout, y: &mut f64) -> Option<Color> {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let x = layout.x;
    let card_x = layout.card_x;
    let card_w = layout.card_w;
    let section_gap = layout.section_gap;

    let slot_count = snapshot.preset_slot_count.min(snapshot.presets.len());
    if !snapshot.show_presets || slot_count == 0 {
        return None;
    }

    let mut hover_preset_color: Option<Color> = None;

    let presets_card_h = ToolbarLayoutSpec::SIDE_PRESET_CARD_HEIGHT;
    draw_group_card(ctx, card_x, *y, card_w, presets_card_h);
    draw_section_label(
        ctx,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
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
            let hint_y = *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y;
            ctx.set_source_rgba(0.7, 0.7, 0.75, 0.8);
            ctx.move_to(hint_x, hint_y);
            let _ = ctx.show_text(&hint);
        }
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(13.0);
    }

    let slot_size = ToolbarLayoutSpec::SIDE_PRESET_SLOT_SIZE;
    let slot_gap = ToolbarLayoutSpec::SIDE_PRESET_SLOT_GAP;
    let slot_row_y = *y + ToolbarLayoutSpec::SIDE_PRESET_ROW_OFFSET_Y;
    let action_row_y = slot_row_y + slot_size + ToolbarLayoutSpec::SIDE_PRESET_ACTION_GAP;
    let action_h = ToolbarLayoutSpec::SIDE_PRESET_ACTION_HEIGHT;
    let action_gap = ToolbarLayoutSpec::SIDE_PRESET_ACTION_BUTTON_GAP;
    let action_w = (slot_size - action_gap) / 2.0;
    let action_icon = (action_h * 0.6).round();
    let icon_size = (slot_size * 0.45).round();
    let swatch_size = (slot_size * 0.35).round();
    let number_box = (slot_size * 0.4).round();
    let keycap_pad = (slot_size * 0.1).round().max(3.0);
    let keycap_radius = (number_box * 0.25).max(3.0);
    let draw_keycap = |ctx: &cairo::Context, key_x: f64, key_y: f64, label: &str, active| {
        let (bg_alpha, border_alpha, text_alpha) = if active {
            (0.75, 0.55, 0.95)
        } else {
            (0.4, 0.35, 0.6)
        };
        ctx.set_source_rgba(0.12, 0.12, 0.18, bg_alpha);
        draw_round_rect(ctx, key_x, key_y, number_box, number_box, keycap_radius);
        let _ = ctx.fill();
        ctx.set_source_rgba(1.0, 1.0, 1.0, border_alpha);
        ctx.set_line_width(1.0);
        draw_round_rect(ctx, key_x, key_y, number_box, number_box, keycap_radius);
        let _ = ctx.stroke();
        ctx.set_font_size(11.0);
        draw_label_center_color(
            ctx,
            key_x,
            key_y,
            number_box,
            number_box,
            label,
            (1.0, 1.0, 1.0, text_alpha),
        );
        ctx.set_font_size(13.0);
    };
    let tool_label = |tool: Tool| match tool {
        Tool::Select => "Select",
        Tool::Pen => "Pen",
        Tool::Line => "Line",
        Tool::Rect => "Rect",
        Tool::Ellipse => "Circle",
        Tool::Arrow => "Arrow",
        Tool::Marker => "Marker",
        Tool::Highlight => "Highlight",
        Tool::Eraser => "Eraser",
    };
    let px_label = |value: f64| {
        if (value - value.round()).abs() < 0.05 {
            format!("{:.0}px", value)
        } else {
            format!("{:.1}px", value)
        }
    };
    let angle_label = |value: f64| {
        if (value - value.round()).abs() < 0.05 {
            format!("{:.0}deg", value)
        } else {
            format!("{:.1}deg", value)
        }
    };
    let on_off = |value: bool| if value { "on" } else { "off" };
    let eraser_kind_label = |kind: EraserKind| match kind {
        EraserKind::Circle => "circle",
        EraserKind::Rect => "rect",
    };
    let eraser_mode_label = |mode: EraserMode| match mode {
        EraserMode::Brush => "brush",
        EraserMode::Stroke => "stroke",
    };
    let truncate_label = |value: &str, max_chars: usize| {
        if value.chars().count() <= max_chars {
            value.to_string()
        } else {
            let mut truncated = value
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>();
            truncated.push_str("...");
            truncated
        }
    };

    for slot_index in 0..slot_count {
        let slot = slot_index + 1;
        let slot_x = x + slot_index as f64 * (slot_size + slot_gap);
        let preset = snapshot
            .presets
            .get(slot_index)
            .and_then(|preset| preset.as_ref());
        let preset_exists = preset.is_some();
        let slot_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, slot_x, slot_row_y, slot_size, slot_size))
            .unwrap_or(false)
            && preset_exists;
        draw_button(
            ctx, slot_x, slot_row_y, slot_size, slot_size, false, slot_hover,
        );
        if let Some(preset) = preset {
            if slot_hover {
                hover_preset_color = Some(preset.color);
            }
            ctx.set_source_rgba(preset.color.r, preset.color.g, preset.color.b, 0.12);
            draw_round_rect(
                ctx,
                slot_x + 1.0,
                slot_row_y + 1.0,
                slot_size - 2.0,
                slot_size - 2.0,
                6.0,
            );
            let _ = ctx.fill();
            ctx.set_source_rgba(preset.color.r, preset.color.g, preset.color.b, 0.35);
            ctx.set_line_width(1.0);
            draw_round_rect(
                ctx,
                slot_x + 1.0,
                slot_row_y + 1.0,
                slot_size - 2.0,
                slot_size - 2.0,
                6.0,
            );
            let _ = ctx.stroke();
        } else {
            ctx.set_source_rgba(0.05, 0.05, 0.07, 0.35);
            draw_round_rect(
                ctx,
                slot_x + 1.0,
                slot_row_y + 1.0,
                slot_size - 2.0,
                slot_size - 2.0,
                6.0,
            );
            let _ = ctx.fill();
        }

        if let Some(preset) = preset {
            let preset_name = preset
                .name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty());
            let mut extra_details = Vec::new();
            if let Some(fill) = preset.fill_enabled {
                extra_details.push(format!("fill:{}", on_off(fill)));
            }
            if let Some(opacity) = preset.marker_opacity {
                let percent = (opacity * 100.0).round() as i32;
                extra_details.push(format!("opacity:{}%", percent));
            }
            if let Some(kind) = preset.eraser_kind {
                extra_details.push(format!("eraser:{}", eraser_kind_label(kind)));
            }
            if let Some(mode) = preset.eraser_mode {
                extra_details.push(format!("mode:{}", eraser_mode_label(mode)));
            }
            if let Some(font_size) = preset.font_size {
                extra_details.push(format!("font:{}", px_label(font_size)));
            }
            if let Some(text_bg) = preset.text_background_enabled {
                extra_details.push(format!("text bg:{}", on_off(text_bg)));
            }
            let mut arrow_bits = Vec::new();
            if let Some(length) = preset.arrow_length {
                arrow_bits.push(format!("len {}", px_label(length)));
            }
            if let Some(angle) = preset.arrow_angle {
                arrow_bits.push(format!("ang {}", angle_label(angle)));
            }
            if let Some(head_at_end) = preset.arrow_head_at_end {
                let head = if head_at_end { "end" } else { "start" };
                arrow_bits.push(format!("head {}", head));
            }
            if !arrow_bits.is_empty() {
                extra_details.push(format!("arrow:{}", arrow_bits.join(", ")));
            }
            if let Some(show_status_bar) = preset.show_status_bar {
                extra_details.push(format!("status:{}", on_off(show_status_bar)));
            }

            let base_summary = format!(
                "{}, {}, {}",
                tool_label(preset.tool),
                color_to_name(&preset.color),
                px_label(preset.size)
            );
            let summary = if extra_details.is_empty() {
                base_summary
            } else {
                format!("{}; {}", base_summary, extra_details.join("; "))
            };
            let label = if let Some(name) = preset_name {
                format!("Apply preset {}: {} ({})", slot, name, summary)
            } else {
                format!("Apply preset {} ({})", slot, summary)
            };
            let tooltip = match snapshot.binding_hints.apply_preset(slot) {
                Some(binding) => format!("{label} (key: {binding})"),
                None => label,
            };
            hits.push(HitRegion {
                rect: (slot_x, slot_row_y, slot_size, slot_size),
                event: ToolbarEvent::ApplyPreset(slot),
                kind: HitKind::Click,
                tooltip: Some(tooltip),
            });

            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
            let icon_x = slot_x + (slot_size - icon_size) / 2.0;
            let icon_y = slot_row_y + (slot_size - icon_size) / 2.0;
            match preset.tool {
                Tool::Select => toolbar_icons::draw_icon_select(ctx, icon_x, icon_y, icon_size),
                Tool::Pen => toolbar_icons::draw_icon_pen(ctx, icon_x, icon_y, icon_size),
                Tool::Line => toolbar_icons::draw_icon_line(ctx, icon_x, icon_y, icon_size),
                Tool::Rect => toolbar_icons::draw_icon_rect(ctx, icon_x, icon_y, icon_size),
                Tool::Ellipse => toolbar_icons::draw_icon_circle(ctx, icon_x, icon_y, icon_size),
                Tool::Arrow => toolbar_icons::draw_icon_arrow(ctx, icon_x, icon_y, icon_size),
                Tool::Marker => toolbar_icons::draw_icon_marker(ctx, icon_x, icon_y, icon_size),
                Tool::Highlight => {
                    toolbar_icons::draw_icon_highlight(ctx, icon_x, icon_y, icon_size)
                }
                Tool::Eraser => toolbar_icons::draw_icon_eraser(ctx, icon_x, icon_y, icon_size),
            }

            let preview_thickness = (preset.size / 50.0 * 6.0).clamp(1.0, 6.0);
            let preview_y = slot_row_y + slot_size - 6.0;
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.8);
            ctx.set_line_width(preview_thickness);
            ctx.move_to(slot_x + 4.0, preview_y);
            ctx.line_to(slot_x + slot_size - 4.0, preview_y);
            let _ = ctx.stroke();

            let swatch_x = slot_x + slot_size - swatch_size - 4.0;
            let swatch_y = slot_row_y + slot_size - swatch_size - 4.0;
            draw_swatch(ctx, swatch_x, swatch_y, swatch_size, preset.color, false);
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.75);
            ctx.set_line_width(1.0);
            draw_round_rect(ctx, swatch_x, swatch_y, swatch_size, swatch_size, 4.0);
            let _ = ctx.stroke();

            if slot_hover && let Some(name) = preset_name {
                let display_name = truncate_label(name, 12);
                ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                ctx.set_font_size(10.0);
                if let Ok(extents) = ctx.text_extents(&display_name) {
                    let pad_x = 5.0;
                    let pad_y = 2.0;
                    let label_w = extents.width() + pad_x * 2.0;
                    let label_h = extents.height() + pad_y * 2.0;
                    let mut label_x = slot_x + (slot_size - label_w) / 2.0;
                    let label_y = (slot_row_y - label_h - 2.0).max(*y + 2.0);
                    let min_x = card_x + 2.0;
                    let max_x = card_x + card_w - label_w - 2.0;
                    if label_x < min_x {
                        label_x = min_x;
                    }
                    if label_x > max_x {
                        label_x = max_x;
                    }
                    ctx.set_source_rgba(0.12, 0.12, 0.18, 0.92);
                    draw_round_rect(ctx, label_x, label_y, label_w, label_h, 4.0);
                    let _ = ctx.fill();
                    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
                    ctx.move_to(
                        label_x + pad_x - extents.x_bearing(),
                        label_y + pad_y - extents.y_bearing(),
                    );
                    let _ = ctx.show_text(&display_name);
                }
            }
        } else {
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.35);
            ctx.set_line_width(1.0);
            ctx.set_dash(&[3.0, 2.0], 0.0);
            draw_round_rect(
                ctx,
                slot_x + 1.0,
                slot_row_y + 1.0,
                slot_size - 2.0,
                slot_size - 2.0,
                6.0,
            );
            let _ = ctx.stroke();
            ctx.set_dash(&[], 0.0);
        }

        let key_x = slot_x + keycap_pad;
        let key_y = slot_row_y + keycap_pad;
        draw_keycap(ctx, key_x, key_y, &slot.to_string(), preset_exists);

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
                    slot_row_y + 1.0,
                    slot_size - 2.0,
                    slot_size - 2.0,
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
                slot_row_y + 1.0,
                slot_size - 2.0,
                slot_size - 2.0,
                7.0,
            );
            let _ = ctx.stroke();
        }

        let save_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, slot_x, action_row_y, action_w, action_h))
            .unwrap_or(false);
        draw_button(
            ctx,
            slot_x,
            action_row_y,
            action_w,
            action_h,
            false,
            save_hover,
        );
        set_icon_color(ctx, save_hover);
        toolbar_icons::draw_icon_save(
            ctx,
            slot_x + (action_w - action_icon) / 2.0,
            action_row_y + (action_h - action_icon) / 2.0,
            action_icon,
        );
        hits.push(HitRegion {
            rect: (slot_x, action_row_y, action_w, action_h),
            event: ToolbarEvent::SavePreset(slot),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                &format!("Save preset {}", slot),
                snapshot.binding_hints.save_preset(slot),
            )),
        });

        let clear_x = slot_x + action_w + action_gap;
        let clear_y = action_row_y;
        let clear_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, clear_x, clear_y, action_w, action_h))
            .unwrap_or(false)
            && preset_exists;
        draw_button(
            ctx,
            clear_x,
            clear_y,
            action_w,
            action_h,
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
            clear_x + (action_w - action_icon) / 2.0,
            clear_y + (action_h - action_icon) / 2.0,
            action_icon,
        );
        if preset_exists {
            hits.push(HitRegion {
                rect: (clear_x, clear_y, action_w, action_h),
                event: ToolbarEvent::ClearPreset(slot),
                kind: HitKind::Click,
                tooltip: Some(format_binding_label(
                    &format!("Clear preset {}", slot),
                    snapshot.binding_hints.clear_preset(slot),
                )),
            });
        }
    }

    *y += presets_card_h + section_gap;

    hover_preset_color
}
