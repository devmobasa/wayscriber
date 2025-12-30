#![allow(clippy::too_many_arguments)]

use anyhow::Result;

use crate::backend::wayland::toolbar::format_binding_label;
use crate::draw::{
    BLACK, BLUE, Color, EraserKind, FontDescriptor, GREEN, ORANGE, PINK, RED, WHITE, YELLOW,
};
use crate::input::state::PresetFeedbackKind;
use crate::input::{EraserMode, Tool};
use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};
use crate::util::color_to_name;

use super::super::events::{HitKind, delay_secs_from_t, delay_t_from_ms};
use super::super::hit::HitRegion;
use super::super::layout::ToolbarLayoutSpec;
use super::widgets::*;

pub fn render_side_palette(
    ctx: &cairo::Context,
    width: f64,
    _height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
) -> Result<()> {
    draw_panel_background(ctx, width, _height);

    let spec = ToolbarLayoutSpec::new(snapshot);
    let mut y = ToolbarLayoutSpec::SIDE_TOP_PADDING;
    let x = ToolbarLayoutSpec::SIDE_START_X;
    let use_icons = snapshot.use_icons;
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(13.0);

    let btn_size = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_SIZE;
    let handle_w = ToolbarLayoutSpec::SIDE_HEADER_HANDLE_SIZE;
    let handle_h = ToolbarLayoutSpec::SIDE_HEADER_HANDLE_SIZE;

    // Place handle above the header row to avoid widening the palette.
    let handle_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, y, handle_w, handle_h))
        .unwrap_or(false);
    draw_drag_handle(ctx, x, y, handle_w, handle_h, handle_hover);
    hits.push(HitRegion {
        rect: (x, y, handle_w, handle_h),
        event: ToolbarEvent::MoveSideToolbar { x: 0.0, y: 0.0 },
        kind: HitKind::DragMoveSide,
        tooltip: Some("Drag toolbar".to_string()),
    });
    let header_y = spec.side_header_y();

    let icons_w = ToolbarLayoutSpec::SIDE_HEADER_TOGGLE_WIDTH;
    let icons_h = btn_size;
    let icons_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, header_y, icons_w, icons_h))
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        header_y,
        icons_w,
        icons_h,
        use_icons,
        icons_hover,
        "Icons",
    );
    hits.push(HitRegion {
        rect: (x, header_y, icons_w, icons_h),
        event: ToolbarEvent::ToggleIconMode(!use_icons),
        kind: HitKind::Click,
        tooltip: None,
    });

    let mode_w = ToolbarLayoutSpec::SIDE_HEADER_MODE_WIDTH;
    let mode_x = x + icons_w + ToolbarLayoutSpec::SIDE_HEADER_MODE_GAP;
    let mode_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, mode_x, header_y, mode_w, icons_h))
        .unwrap_or(false);
    draw_button(ctx, mode_x, header_y, mode_w, icons_h, false, mode_hover);
    let mode_label = match snapshot.layout_mode {
        crate::config::ToolbarLayoutMode::Simple => "Mode: S",
        crate::config::ToolbarLayoutMode::Regular => "Mode: R",
        crate::config::ToolbarLayoutMode::Advanced => "Mode: A",
    };
    draw_label_center(ctx, mode_x, header_y, mode_w, icons_h, mode_label);
    let next_mode = snapshot.layout_mode.next();
    let mode_tooltip = format!(
        "Mode: S/R/A = {}/{}/{}",
        crate::config::ToolbarLayoutMode::Simple.label(),
        crate::config::ToolbarLayoutMode::Regular.label(),
        crate::config::ToolbarLayoutMode::Advanced.label(),
    );
    hits.push(HitRegion {
        rect: (mode_x, header_y, mode_w, icons_h),
        event: ToolbarEvent::SetToolbarLayoutMode(next_mode),
        kind: HitKind::Click,
        tooltip: Some(mode_tooltip),
    });

    let (pin_x, close_x, header_btn_y) = spec.side_header_button_positions(width);
    let pin_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, pin_x, header_btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_pin_button(
        ctx,
        pin_x,
        header_btn_y,
        btn_size,
        snapshot.side_pinned,
        pin_hover,
    );
    hits.push(HitRegion {
        rect: (pin_x, header_btn_y, btn_size, btn_size),
        event: ToolbarEvent::PinSideToolbar(!snapshot.side_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.side_pinned {
            "Unpin".to_string()
        } else {
            "Pin".to_string()
        }),
    });

    let close_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, close_x, header_btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_close_button(ctx, close_x, header_btn_y, btn_size, close_hover);
    hits.push(HitRegion {
        rect: (close_x, header_btn_y, btn_size, btn_size),
        event: ToolbarEvent::CloseSideToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });

    y = spec.side_content_start_y();

    let card_x = spec.side_card_x();
    let card_w = spec.side_card_width(width);
    let content_width = spec.side_content_width(width);
    let section_gap = ToolbarLayoutSpec::SIDE_SECTION_GAP;
    let mut hover_preset_color: Option<Color> = None;
    let show_text_controls =
        snapshot.text_active || snapshot.note_active || snapshot.show_text_controls;

    let basic_colors: &[(Color, &str)] = &[
        (RED, "Red"),
        (GREEN, "Green"),
        (BLUE, "Blue"),
        (YELLOW, "Yellow"),
        (WHITE, "White"),
        (BLACK, "Black"),
    ];
    let extended_colors: &[(Color, &str)] = &[
        (ORANGE, "Orange"),
        (PINK, "Pink"),
        (
            Color {
                r: 0.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            "Cyan",
        ),
        (
            Color {
                r: 0.6,
                g: 0.4,
                b: 0.8,
                a: 1.0,
            },
            "Purple",
        ),
        (
            Color {
                r: 0.4,
                g: 0.4,
                b: 0.4,
                a: 1.0,
            },
            "Gray",
        ),
    ];

    let swatch = ToolbarLayoutSpec::SIDE_COLOR_SWATCH;
    let swatch_gap = ToolbarLayoutSpec::SIDE_COLOR_SWATCH_GAP;
    let picker_h = ToolbarLayoutSpec::SIDE_COLOR_PICKER_INPUT_HEIGHT;
    let colors_card_h = spec.side_colors_height(snapshot);

    draw_group_card(ctx, card_x, y, card_w, colors_card_h);
    draw_section_label(
        ctx,
        x,
        y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
        "Colors",
    );

    let picker_y = y + ToolbarLayoutSpec::SIDE_COLOR_PICKER_OFFSET_Y;
    let picker_w = content_width;
    draw_color_picker(ctx, x, picker_y, picker_w, picker_h);
    hits.push(HitRegion {
        rect: (x, picker_y, picker_w, picker_h),
        event: ToolbarEvent::SetColor(snapshot.color),
        kind: HitKind::PickColor {
            x,
            y: picker_y,
            w: picker_w,
            h: picker_h + if snapshot.show_more_colors { 30.0 } else { 0.0 },
        },
        tooltip: None,
    });

    let mut cx = x;
    let mut row_y = picker_y + picker_h + 8.0;
    for (color, _name) in basic_colors {
        draw_swatch(ctx, cx, row_y, swatch, *color, *color == snapshot.color);
        hits.push(HitRegion {
            rect: (cx, row_y, swatch, swatch),
            event: ToolbarEvent::SetColor(*color),
            kind: HitKind::Click,
            tooltip: None,
        });
        cx += swatch + swatch_gap;
    }

    if !snapshot.show_more_colors {
        let plus_btn_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, cx, row_y, swatch, swatch))
            .unwrap_or(false);
        draw_button(ctx, cx, row_y, swatch, swatch, false, plus_btn_hover);
        set_icon_color(ctx, plus_btn_hover);
        toolbar_icons::draw_icon_plus(
            ctx,
            cx + (swatch - 14.0) / 2.0,
            row_y + (swatch - 14.0) / 2.0,
            14.0,
        );
        hits.push(HitRegion {
            rect: (cx, row_y, swatch, swatch),
            event: ToolbarEvent::ToggleMoreColors(true),
            kind: HitKind::Click,
            tooltip: Some("More colors".to_string()),
        });
    }
    row_y += swatch + swatch_gap;

    if snapshot.show_more_colors {
        cx = x;
        for (color, _name) in extended_colors {
            draw_swatch(ctx, cx, row_y, swatch, *color, *color == snapshot.color);
            hits.push(HitRegion {
                rect: (cx, row_y, swatch, swatch),
                event: ToolbarEvent::SetColor(*color),
                kind: HitKind::Click,
                tooltip: None,
            });
            cx += swatch + swatch_gap;
        }

        let minus_btn_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, cx, row_y, swatch, swatch))
            .unwrap_or(false);
        draw_button(ctx, cx, row_y, swatch, swatch, false, minus_btn_hover);
        set_icon_color(ctx, minus_btn_hover);
        toolbar_icons::draw_icon_minus(
            ctx,
            cx + (swatch - 14.0) / 2.0,
            row_y + (swatch - 14.0) / 2.0,
            14.0,
        );
        hits.push(HitRegion {
            rect: (cx, row_y, swatch, swatch),
            event: ToolbarEvent::ToggleMoreColors(false),
            kind: HitKind::Click,
            tooltip: Some("Hide colors".to_string()),
        });
    }

    y += colors_card_h + section_gap;

    let slot_count = snapshot.preset_slot_count.min(snapshot.presets.len());
    if snapshot.show_presets && slot_count > 0 {
        let presets_card_h = ToolbarLayoutSpec::SIDE_PRESET_CARD_HEIGHT;
        draw_group_card(ctx, card_x, y, card_w, presets_card_h);
        draw_section_label(
            ctx,
            x,
            y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
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
                let hint_y = y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y;
                ctx.set_source_rgba(0.7, 0.7, 0.75, 0.8);
                ctx.move_to(hint_x, hint_y);
                let _ = ctx.show_text(&hint);
            }
            ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            ctx.set_font_size(13.0);
        }

        let slot_size = ToolbarLayoutSpec::SIDE_PRESET_SLOT_SIZE;
        let slot_gap = ToolbarLayoutSpec::SIDE_PRESET_SLOT_GAP;
        let slot_row_y = y + ToolbarLayoutSpec::SIDE_PRESET_ROW_OFFSET_Y;
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
                    Tool::Ellipse => {
                        toolbar_icons::draw_icon_circle(ctx, icon_x, icon_y, icon_size)
                    }
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
                    ctx.select_font_face(
                        "Sans",
                        cairo::FontSlant::Normal,
                        cairo::FontWeight::Normal,
                    );
                    ctx.set_font_size(10.0);
                    if let Ok(extents) = ctx.text_extents(&display_name) {
                        let pad_x = 5.0;
                        let pad_y = 2.0;
                        let label_w = extents.width() + pad_x * 2.0;
                        let label_h = extents.height() + pad_y * 2.0;
                        let mut label_x = slot_x + (slot_size - label_w) / 2.0;
                        let label_y = (slot_row_y - label_h - 2.0).max(y + 2.0);
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

        y += presets_card_h + section_gap;
    }

    if let Some(color) = hover_preset_color {
        ctx.set_source_rgba(color.r, color.g, color.b, 0.85);
        ctx.set_line_width(2.0);
        draw_round_rect(
            ctx,
            x - 2.0,
            picker_y - 2.0,
            picker_w + 4.0,
            picker_h + 4.0,
            6.0,
        );
        let _ = ctx.stroke();
    }

    let slider_card_h = ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT;
    draw_group_card(ctx, card_x, y, card_w, slider_card_h);
    let thickness_label = if snapshot.thickness_targets_eraser {
        "Eraser size"
    } else {
        "Thickness"
    };
    draw_section_label(
        ctx,
        x,
        y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
        thickness_label,
    );

    let btn_size = ToolbarLayoutSpec::SIDE_NUDGE_SIZE;
    let nudge_icon_size = ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE;
    let value_w = ToolbarLayoutSpec::SIDE_SLIDER_VALUE_WIDTH;
    let thickness_slider_row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
    let track_h = ToolbarLayoutSpec::SIDE_TRACK_HEIGHT;
    let knob_r = ToolbarLayoutSpec::SIDE_TRACK_KNOB_RADIUS;
    let (min_thick, max_thick, nudge_step) = (1.0, 50.0, 1.0);

    let minus_x = x;
    draw_button(
        ctx,
        minus_x,
        thickness_slider_row_y,
        btn_size,
        btn_size,
        false,
        false,
    );
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    toolbar_icons::draw_icon_minus(
        ctx,
        minus_x + (btn_size - nudge_icon_size) / 2.0,
        thickness_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (minus_x, thickness_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::NudgeThickness(-nudge_step),
        kind: HitKind::Click,
        tooltip: None,
    });

    let plus_x = width - x - btn_size - value_w - 4.0;
    draw_button(
        ctx,
        plus_x,
        thickness_slider_row_y,
        btn_size,
        btn_size,
        false,
        false,
    );
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    toolbar_icons::draw_icon_plus(
        ctx,
        plus_x + (btn_size - nudge_icon_size) / 2.0,
        thickness_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (plus_x, thickness_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::NudgeThickness(nudge_step),
        kind: HitKind::Click,
        tooltip: None,
    });

    let track_x = minus_x + btn_size + 6.0;
    let track_w = plus_x - track_x - 6.0;
    let thickness_track_y = thickness_slider_row_y + (btn_size - track_h) / 2.0;
    let t = ((snapshot.thickness - min_thick) / (max_thick - min_thick)).clamp(0.0, 1.0);
    let knob_x = track_x + t * (track_w - knob_r * 2.0) + knob_r;

    ctx.set_source_rgba(0.5, 0.5, 0.6, 0.6);
    draw_round_rect(ctx, track_x, thickness_track_y, track_w, track_h, 4.0);
    let _ = ctx.fill();
    ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
    ctx.arc(
        knob_x,
        thickness_track_y + track_h / 2.0,
        knob_r,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();

    hits.push(HitRegion {
        rect: (track_x, thickness_track_y - 6.0, track_w, track_h + 12.0),
        event: ToolbarEvent::SetThickness(snapshot.thickness),
        kind: HitKind::DragSetThickness {
            min: min_thick,
            max: max_thick,
        },
        tooltip: None,
    });

    let thickness_text = format!("{:.0}px", snapshot.thickness);
    let value_x = width - x - value_w;
    draw_label_center(
        ctx,
        value_x,
        thickness_slider_row_y,
        value_w,
        btn_size,
        &thickness_text,
    );
    y += slider_card_h + section_gap;

    if snapshot.thickness_targets_eraser {
        let eraser_card_h = ToolbarLayoutSpec::SIDE_ERASER_MODE_CARD_HEIGHT;
        let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
        let toggle_w = content_width;
        draw_group_card(ctx, card_x, y, card_w, eraser_card_h);
        draw_section_label(
            ctx,
            x,
            y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
            "Eraser mode",
        );

        let toggle_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
        let toggle_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, toggle_y, toggle_w, toggle_h))
            .unwrap_or(false);
        let stroke_active = snapshot.eraser_mode == EraserMode::Stroke;
        draw_checkbox(
            ctx,
            x,
            toggle_y,
            toggle_w,
            toggle_h,
            stroke_active,
            toggle_hover,
            "Erase by stroke",
        );
        let toggle_tooltip = format_binding_label(
            "Erase by stroke",
            snapshot.binding_hints.toggle_eraser_mode.as_deref(),
        );
        hits.push(HitRegion {
            rect: (x, toggle_y, toggle_w, toggle_h),
            event: ToolbarEvent::SetEraserMode(if stroke_active {
                EraserMode::Brush
            } else {
                EraserMode::Stroke
            }),
            kind: HitKind::Click,
            tooltip: Some(toggle_tooltip),
        });
        y += eraser_card_h + section_gap;
    }

    let show_marker_opacity =
        snapshot.show_marker_opacity_section || snapshot.thickness_targets_marker;
    if show_marker_opacity {
        let marker_slider_row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
        draw_group_card(ctx, card_x, y, card_w, slider_card_h);
        draw_section_label(
            ctx,
            x,
            y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
            "Marker opacity",
        );

        let minus_x = x;
        draw_button(
            ctx,
            minus_x,
            marker_slider_row_y,
            btn_size,
            btn_size,
            false,
            false,
        );
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        toolbar_icons::draw_icon_minus(
            ctx,
            minus_x + (btn_size - nudge_icon_size) / 2.0,
            marker_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
            nudge_icon_size,
        );
        hits.push(HitRegion {
            rect: (minus_x, marker_slider_row_y, btn_size, btn_size),
            event: ToolbarEvent::NudgeMarkerOpacity(-0.05),
            kind: HitKind::Click,
            tooltip: None,
        });

        let plus_x = width - x - btn_size - value_w - 4.0;
        draw_button(
            ctx,
            plus_x,
            marker_slider_row_y,
            btn_size,
            btn_size,
            false,
            false,
        );
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        toolbar_icons::draw_icon_plus(
            ctx,
            plus_x + (btn_size - nudge_icon_size) / 2.0,
            marker_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
            nudge_icon_size,
        );
        hits.push(HitRegion {
            rect: (plus_x, marker_slider_row_y, btn_size, btn_size),
            event: ToolbarEvent::NudgeMarkerOpacity(0.05),
            kind: HitKind::Click,
            tooltip: None,
        });

        let track_x = minus_x + btn_size + 6.0;
        let track_w = plus_x - track_x - 6.0;
        let marker_track_y = marker_slider_row_y + (btn_size - track_h) / 2.0;
        let min_opacity = 0.05;
        let max_opacity = 0.9;
        let t =
            ((snapshot.marker_opacity - min_opacity) / (max_opacity - min_opacity)).clamp(0.0, 1.0);
        let knob_x = track_x + t * (track_w - knob_r * 2.0) + knob_r;

        ctx.set_source_rgba(0.5, 0.5, 0.6, 0.6);
        draw_round_rect(ctx, track_x, marker_track_y, track_w, track_h, 4.0);
        let _ = ctx.fill();
        ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
        ctx.arc(
            knob_x,
            marker_track_y + track_h / 2.0,
            knob_r,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        let _ = ctx.fill();

        hits.push(HitRegion {
            rect: (track_x, marker_track_y - 6.0, track_w, track_h + 12.0),
            event: ToolbarEvent::SetMarkerOpacity(snapshot.marker_opacity),
            kind: HitKind::DragSetMarkerOpacity {
                min: min_opacity,
                max: max_opacity,
            },
            tooltip: None,
        });

        let opacity_text = format!("{:.0}%", snapshot.marker_opacity * 100.0);
        draw_label_center(
            ctx,
            value_x,
            marker_slider_row_y,
            value_w,
            btn_size,
            &opacity_text,
        );

        y += slider_card_h + section_gap;
    }

    if show_text_controls {
        draw_group_card(ctx, card_x, y, card_w, slider_card_h);
        draw_section_label(
            ctx,
            x,
            y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
            "Text size",
        );

        let fs_min = 8.0;
        let fs_max = 72.0;
        let fs_slider_row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;

        let fs_minus_x = x;
        draw_button(
            ctx,
            fs_minus_x,
            fs_slider_row_y,
            btn_size,
            btn_size,
            false,
            false,
        );
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        toolbar_icons::draw_icon_minus(
            ctx,
            fs_minus_x + (btn_size - nudge_icon_size) / 2.0,
            fs_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
            nudge_icon_size,
        );
        hits.push(HitRegion {
            rect: (fs_minus_x, fs_slider_row_y, btn_size, btn_size),
            event: ToolbarEvent::SetFontSize((snapshot.font_size - 2.0).max(fs_min)),
            kind: HitKind::Click,
            tooltip: None,
        });

        let fs_plus_x = width - x - btn_size - value_w - 4.0;
        draw_button(
            ctx,
            fs_plus_x,
            fs_slider_row_y,
            btn_size,
            btn_size,
            false,
            false,
        );
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        toolbar_icons::draw_icon_plus(
            ctx,
            fs_plus_x + (btn_size - nudge_icon_size) / 2.0,
            fs_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
            nudge_icon_size,
        );
        hits.push(HitRegion {
            rect: (fs_plus_x, fs_slider_row_y, btn_size, btn_size),
            event: ToolbarEvent::SetFontSize((snapshot.font_size + 2.0).min(fs_max)),
            kind: HitKind::Click,
            tooltip: None,
        });

        let fs_track_x = fs_minus_x + btn_size + 6.0;
        let fs_track_w = fs_plus_x - fs_track_x - 6.0;
        let fs_track_y = fs_slider_row_y + (btn_size - track_h) / 2.0;
        let fs_t = ((snapshot.font_size - fs_min) / (fs_max - fs_min)).clamp(0.0, 1.0);
        let fs_knob_x = fs_track_x + fs_t * (fs_track_w - knob_r * 2.0) + knob_r;

        ctx.set_source_rgba(0.5, 0.5, 0.6, 0.6);
        draw_round_rect(ctx, fs_track_x, fs_track_y, fs_track_w, track_h, 4.0);
        let _ = ctx.fill();
        ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
        ctx.arc(
            fs_knob_x,
            fs_track_y + track_h / 2.0,
            knob_r,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        let _ = ctx.fill();

        hits.push(HitRegion {
            rect: (fs_track_x, fs_track_y - 6.0, fs_track_w, track_h + 12.0),
            event: ToolbarEvent::SetFontSize(snapshot.font_size),
            kind: HitKind::DragSetFontSize,
            tooltip: None,
        });

        let fs_text = format!("{:.0}pt", snapshot.font_size);
        draw_label_center(
            ctx,
            width - x - value_w,
            fs_slider_row_y,
            value_w,
            btn_size,
            &fs_text,
        );

        y += slider_card_h + section_gap;

        let font_card_h = ToolbarLayoutSpec::SIDE_FONT_CARD_HEIGHT;
        draw_group_card(ctx, card_x, y, card_w, font_card_h);
        draw_section_label(
            ctx,
            x,
            y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
            "Font",
        );

        let font_btn_h = ToolbarLayoutSpec::SIDE_FONT_BUTTON_HEIGHT;
        let font_gap = ToolbarLayoutSpec::SIDE_FONT_BUTTON_GAP;
        let font_btn_w = (content_width - font_gap) / 2.0;
        let fonts = [
            FontDescriptor::new("Sans".to_string(), "bold".to_string(), "normal".to_string()),
            FontDescriptor::new(
                "Monospace".to_string(),
                "normal".to_string(),
                "normal".to_string(),
            ),
        ];
        let mut fx = x;
        let fy = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
        for font in fonts {
            let is_active = font.family == snapshot.font.family;
            let font_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, fx, fy, font_btn_w, font_btn_h))
                .unwrap_or(false);
            draw_button(ctx, fx, fy, font_btn_w, font_btn_h, is_active, font_hover);
            draw_label_left(ctx, fx + 8.0, fy, font_btn_w, font_btn_h, &font.family);
            hits.push(HitRegion {
                rect: (fx, fy, font_btn_w, font_btn_h),
                event: ToolbarEvent::SetFont(font.clone()),
                kind: HitKind::Click,
                tooltip: None,
            });
            fx += font_btn_w + font_gap;
        }

        y += font_card_h + section_gap;
    }

    let show_actions = snapshot.show_actions_section || snapshot.show_actions_advanced;
    if show_actions {
        let actions_card_h = spec.side_actions_height(snapshot);
        draw_group_card(ctx, card_x, y, card_w, actions_card_h);
        draw_section_label(
            ctx,
            x,
            y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
            "Actions",
        );

        let mut actions_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
        type IconFn = fn(&cairo::Context, f64, f64, f64);
        let basic_actions: &[(ToolbarEvent, IconFn, &str, bool)] = &[
            (
                ToolbarEvent::Undo,
                toolbar_icons::draw_icon_undo as IconFn,
                "Undo",
                snapshot.undo_available,
            ),
            (
                ToolbarEvent::Redo,
                toolbar_icons::draw_icon_redo as IconFn,
                "Redo",
                snapshot.redo_available,
            ),
            (
                ToolbarEvent::ClearCanvas,
                toolbar_icons::draw_icon_clear as IconFn,
                "Clear",
                true,
            ),
        ];
        let lock_label = if snapshot.zoom_locked {
            "Unlock Zoom"
        } else {
            "Lock Zoom"
        };
        let show_delay_actions = snapshot.show_step_section && snapshot.show_delay_sliders;
        let mut advanced_actions: Vec<(ToolbarEvent, IconFn, &str, bool)> = Vec::new();
        advanced_actions.push((
            ToolbarEvent::UndoAll,
            toolbar_icons::draw_icon_undo_all as IconFn,
            "Undo All",
            snapshot.undo_available,
        ));
        advanced_actions.push((
            ToolbarEvent::RedoAll,
            toolbar_icons::draw_icon_redo_all as IconFn,
            "Redo All",
            snapshot.redo_available,
        ));
        if show_delay_actions {
            advanced_actions.push((
                ToolbarEvent::UndoAllDelayed,
                toolbar_icons::draw_icon_undo_all_delay as IconFn,
                "Undo All Delay",
                snapshot.undo_available,
            ));
            advanced_actions.push((
                ToolbarEvent::RedoAllDelayed,
                toolbar_icons::draw_icon_redo_all_delay as IconFn,
                "Redo All Delay",
                snapshot.redo_available,
            ));
        }
        advanced_actions.push((
            ToolbarEvent::ToggleFreeze,
            if snapshot.frozen_active {
                toolbar_icons::draw_icon_unfreeze as IconFn
            } else {
                toolbar_icons::draw_icon_freeze as IconFn
            },
            if snapshot.frozen_active {
                "Unfreeze"
            } else {
                "Freeze"
            },
            true,
        ));
        advanced_actions.push((
            ToolbarEvent::ZoomIn,
            toolbar_icons::draw_icon_zoom_in as IconFn,
            "Zoom In",
            true,
        ));
        advanced_actions.push((
            ToolbarEvent::ZoomOut,
            toolbar_icons::draw_icon_zoom_out as IconFn,
            "Zoom Out",
            true,
        ));
        advanced_actions.push((
            ToolbarEvent::ResetZoom,
            toolbar_icons::draw_icon_zoom_reset as IconFn,
            "Reset Zoom",
            snapshot.zoom_active,
        ));
        advanced_actions.push((
            ToolbarEvent::ToggleZoomLock,
            if snapshot.zoom_locked {
                toolbar_icons::draw_icon_lock as IconFn
            } else {
                toolbar_icons::draw_icon_unlock as IconFn
            },
            lock_label,
            snapshot.zoom_active,
        ));

        if use_icons {
            let icon_btn_size = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
            let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
            let mut actions: Vec<(ToolbarEvent, IconFn, &str, bool)> = Vec::new();
            if snapshot.show_actions_section {
                for (evt, icon_fn, label, enabled) in basic_actions {
                    actions.push((evt.clone(), *icon_fn, *label, *enabled));
                }
            }
            if snapshot.show_actions_advanced {
                for (evt, icon_fn, label, enabled) in &advanced_actions {
                    actions.push((evt.clone(), *icon_fn, *label, *enabled));
                }
            }
            let icons_per_row = 6usize;
            let total_icons = actions.len();
            let rows = if total_icons > 0 {
                total_icons.div_ceil(icons_per_row)
            } else {
                0
            };
            for row in 0..rows {
                let row_start = row * icons_per_row;
                let row_end = (row_start + icons_per_row).min(total_icons);
                let icons_in_row = row_end - row_start;
                let row_width =
                    icons_in_row as f64 * icon_btn_size + (icons_in_row as f64 - 1.0) * icon_gap;
                let row_x = x + (content_width - row_width) / 2.0;
                for col in 0..icons_in_row {
                    let idx = row_start + col;
                    let (evt, icon_fn, label, enabled) = &actions[idx];
                    let bx = row_x + (icon_btn_size + icon_gap) * col as f64;
                    let by = actions_y + (icon_btn_size + icon_gap) * row as f64;
                    let is_hover = hover
                        .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, icon_btn_size, icon_btn_size))
                        .unwrap_or(false);
                    if *enabled {
                        draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, is_hover);
                        set_icon_color(ctx, is_hover);
                    } else {
                        draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, false);
                        ctx.set_source_rgba(0.5, 0.5, 0.55, 0.5);
                    }
                    let icon_x = bx + (icon_btn_size - icon_size) / 2.0;
                    let icon_y = by + (icon_btn_size - icon_size) / 2.0;
                    icon_fn(ctx, icon_x, icon_y, icon_size);
                    hits.push(HitRegion {
                        rect: (bx, by, icon_btn_size, icon_btn_size),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            label,
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
            }
        } else {
            if snapshot.show_actions_section {
                let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
                let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
                for (idx, (evt, _icon, label, enabled)) in basic_actions.iter().enumerate() {
                    let by = actions_y + (action_h + action_gap) * idx as f64;
                    let is_hover = hover
                        .map(|(hx, hy)| point_in_rect(hx, hy, x, by, content_width, action_h))
                        .unwrap_or(false);
                    draw_button(ctx, x, by, content_width, action_h, *enabled, is_hover);
                    draw_label_center(ctx, x, by, content_width, action_h, label);
                    if *enabled {
                        hits.push(HitRegion {
                            rect: (x, by, content_width, action_h),
                            event: evt.clone(),
                            kind: HitKind::Click,
                            tooltip: Some(format_binding_label(
                                label,
                                snapshot.binding_hints.binding_for_event(evt),
                            )),
                        });
                    }
                }
                actions_y += action_h * basic_actions.len() as f64
                    + action_gap * (basic_actions.len() as f64 - 1.0);
            }

            if snapshot.show_actions_section && snapshot.show_actions_advanced {
                actions_y += ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            }

            if snapshot.show_actions_advanced {
                let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
                let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
                let action_col_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
                let action_w = (content_width - action_col_gap) / 2.0;
                for (idx, (evt, _icon, label, enabled)) in advanced_actions.iter().enumerate() {
                    let row = idx / 2;
                    let col = idx % 2;
                    let bx = x + (action_w + action_col_gap) * col as f64;
                    let by = actions_y + (action_h + action_gap) * row as f64;
                    let is_hover = hover
                        .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, action_w, action_h))
                        .unwrap_or(false);
                    draw_button(ctx, bx, by, action_w, action_h, *enabled, is_hover);
                    draw_label_center(ctx, bx, by, action_w, action_h, label);
                    if *enabled {
                        hits.push(HitRegion {
                            rect: (bx, by, action_w, action_h),
                            event: evt.clone(),
                            kind: HitKind::Click,
                            tooltip: Some(format_binding_label(
                                label,
                                snapshot.binding_hints.binding_for_event(evt),
                            )),
                        });
                    }
                }
            }
        }

        y += actions_card_h + section_gap;
    }

    if snapshot.show_actions_advanced {
        let pages_card_h = spec.side_pages_height(snapshot);
        draw_group_card(ctx, card_x, y, card_w, pages_card_h);
        draw_section_label(
            ctx,
            x,
            y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
            "Pages",
        );

        let pages_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
        let btn_h = if use_icons {
            ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON
        } else {
            ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT
        };
        let btn_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        let btn_w = (content_width - btn_gap * 4.0) / 5.0;
        let can_prev = snapshot.page_index > 0;
        let can_next = snapshot.page_index + 1 < snapshot.page_count;
        let buttons = [
            (
                ToolbarEvent::PagePrev,
                "Prev",
                toolbar_icons::draw_icon_undo as fn(&cairo::Context, f64, f64, f64),
                can_prev,
            ),
            (
                ToolbarEvent::PageNext,
                "Next",
                toolbar_icons::draw_icon_redo as fn(&cairo::Context, f64, f64, f64),
                can_next,
            ),
            (
                ToolbarEvent::PageNew,
                "New",
                toolbar_icons::draw_icon_plus as fn(&cairo::Context, f64, f64, f64),
                true,
            ),
            (
                ToolbarEvent::PageDuplicate,
                "Dup",
                toolbar_icons::draw_icon_save as fn(&cairo::Context, f64, f64, f64),
                true,
            ),
            (
                ToolbarEvent::PageDelete,
                "Del",
                toolbar_icons::draw_icon_clear as fn(&cairo::Context, f64, f64, f64),
                true,
            ),
        ];

        for (idx, (evt, label, icon_fn, enabled)) in buttons.iter().enumerate() {
            let bx = x + (btn_w + btn_gap) * idx as f64;
            let is_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, bx, pages_y, btn_w, btn_h))
                .unwrap_or(false);
            draw_button(ctx, bx, pages_y, btn_w, btn_h, *enabled, is_hover);
            if use_icons {
                if *enabled {
                    set_icon_color(ctx, is_hover);
                } else {
                    ctx.set_source_rgba(0.5, 0.5, 0.55, 0.5);
                }
                let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
                let icon_x = bx + (btn_w - icon_size) / 2.0;
                let icon_y = pages_y + (btn_h - icon_size) / 2.0;
                icon_fn(ctx, icon_x, icon_y, icon_size);
            } else {
                draw_label_center(ctx, bx, pages_y, btn_w, btn_h, label);
            }
            if *enabled {
                hits.push(HitRegion {
                    rect: (bx, pages_y, btn_w, btn_h),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        label,
                        snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
        }

        y += pages_card_h + section_gap;
    }

    if snapshot.show_step_section {
        let custom_toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
        let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
        let toggles_h = custom_toggle_h * 2.0 + toggle_gap;
        let custom_content_h = if snapshot.custom_section_enabled {
            ToolbarLayoutSpec::SIDE_CUSTOM_SECTION_HEIGHT
        } else {
            0.0
        };
        let delay_sliders_h = if snapshot.show_delay_sliders {
            ToolbarLayoutSpec::SIDE_DELAY_SECTION_HEIGHT
        } else {
            0.0
        };
        let custom_card_h = ToolbarLayoutSpec::SIDE_STEP_HEADER_HEIGHT
            + toggles_h
            + custom_content_h
            + delay_sliders_h;
        draw_group_card(ctx, card_x, y, card_w, custom_card_h);
        draw_section_label(
            ctx,
            x,
            y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
            "Step Undo/Redo",
        );

        let custom_toggle_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
        let toggle_w = content_width;

        let step_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, custom_toggle_y, toggle_w, custom_toggle_h))
            .unwrap_or(false);
        draw_checkbox(
            ctx,
            x,
            custom_toggle_y,
            toggle_w,
            custom_toggle_h,
            snapshot.custom_section_enabled,
            step_hover,
            "Step controls",
        );
        hits.push(HitRegion {
            rect: (x, custom_toggle_y, toggle_w, custom_toggle_h),
            event: ToolbarEvent::ToggleCustomSection(!snapshot.custom_section_enabled),
            kind: HitKind::Click,
            tooltip: Some("Step controls: multi-step undo/redo.".to_string()),
        });

        let delay_toggle_y = custom_toggle_y + custom_toggle_h + toggle_gap;
        let delay_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, delay_toggle_y, toggle_w, custom_toggle_h))
            .unwrap_or(false);
        draw_checkbox(
            ctx,
            x,
            delay_toggle_y,
            toggle_w,
            custom_toggle_h,
            snapshot.show_delay_sliders,
            delay_hover,
            "Delay sliders",
        );
        hits.push(HitRegion {
            rect: (x, delay_toggle_y, toggle_w, custom_toggle_h),
            event: ToolbarEvent::ToggleDelaySliders(!snapshot.show_delay_sliders),
            kind: HitKind::Click,
            tooltip: Some("Delay sliders: undo/redo delays.".to_string()),
        });

        let mut custom_y = delay_toggle_y + custom_toggle_h + toggle_gap;

        if snapshot.custom_section_enabled {
            let render_custom_row =
                |ctx: &cairo::Context,
                 hits: &mut Vec<HitRegion>,
                 x: f64,
                 y: f64,
                 w: f64,
                 snapshot: &ToolbarSnapshot,
                 is_undo: bool,
                 hover: Option<(f64, f64)>| {
                    let row_h = 26.0;
                    let btn_w = if snapshot.use_icons { 42.0 } else { 90.0 };
                    let steps_btn_w = 26.0;
                    let gap = 6.0;
                    let label = if is_undo { "Step Undo" } else { "Step Redo" };
                    let steps = if is_undo {
                        snapshot.custom_undo_steps
                    } else {
                        snapshot.custom_redo_steps
                    };
                    let delay_ms = if is_undo {
                        snapshot.custom_undo_delay_ms
                    } else {
                        snapshot.custom_redo_delay_ms
                    };

                    let btn_hover = hover
                        .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, row_h))
                        .unwrap_or(false);

                    if snapshot.use_icons {
                        let icon_size = 20.0;
                        draw_button(ctx, x, y, btn_w, row_h, false, btn_hover);
                        set_icon_color(ctx, btn_hover);
                        if is_undo {
                            toolbar_icons::draw_icon_step_undo(
                                ctx,
                                x + (btn_w - icon_size) / 2.0,
                                y + (row_h - icon_size) / 2.0,
                                icon_size,
                            );
                        } else {
                            toolbar_icons::draw_icon_step_redo(
                                ctx,
                                x + (btn_w - icon_size) / 2.0,
                                y + (row_h - icon_size) / 2.0,
                                icon_size,
                            );
                        }
                    } else {
                        draw_button(ctx, x, y, btn_w, row_h, false, btn_hover);
                        draw_label_left(ctx, x + 10.0, y, btn_w - 20.0, row_h, label);
                    }
                    hits.push(HitRegion {
                        rect: (x, y, btn_w, row_h),
                        event: if is_undo {
                            ToolbarEvent::CustomUndo
                        } else {
                            ToolbarEvent::CustomRedo
                        },
                        kind: HitKind::Click,
                        tooltip: Some(if is_undo {
                            "Step undo".to_string()
                        } else {
                            "Step redo".to_string()
                        }),
                    });

                    let steps_x = x + btn_w + gap;
                    let minus_hover = hover
                        .map(|(hx, hy)| point_in_rect(hx, hy, steps_x, y, steps_btn_w, row_h))
                        .unwrap_or(false);
                    draw_button(ctx, steps_x, y, steps_btn_w, row_h, false, minus_hover);
                    set_icon_color(ctx, minus_hover);
                    toolbar_icons::draw_icon_minus(
                        ctx,
                        steps_x + (steps_btn_w - ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE) / 2.0,
                        y + (row_h - ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE) / 2.0,
                        ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE,
                    );
                    hits.push(HitRegion {
                        rect: (steps_x, y, steps_btn_w, row_h),
                        event: if is_undo {
                            ToolbarEvent::SetCustomUndoSteps(steps.saturating_sub(1).max(1))
                        } else {
                            ToolbarEvent::SetCustomRedoSteps(steps.saturating_sub(1).max(1))
                        },
                        kind: HitKind::Click,
                        tooltip: Some(if is_undo {
                            "Decrease undo steps".to_string()
                        } else {
                            "Decrease redo steps".to_string()
                        }),
                    });

                    let steps_val_x = steps_x + steps_btn_w + 4.0;
                    draw_label_center(
                        ctx,
                        steps_val_x,
                        y,
                        54.0,
                        row_h,
                        &format!("{} steps", steps),
                    );

                    let steps_plus_x = steps_val_x + 58.0;
                    let plus_hover = hover
                        .map(|(hx, hy)| point_in_rect(hx, hy, steps_plus_x, y, steps_btn_w, row_h))
                        .unwrap_or(false);
                    draw_button(ctx, steps_plus_x, y, steps_btn_w, row_h, false, plus_hover);
                    set_icon_color(ctx, plus_hover);
                    toolbar_icons::draw_icon_plus(
                        ctx,
                        steps_plus_x
                            + (steps_btn_w - ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE) / 2.0,
                        y + (row_h - ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE) / 2.0,
                        ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE,
                    );
                    hits.push(HitRegion {
                        rect: (steps_plus_x, y, steps_btn_w, row_h),
                        event: if is_undo {
                            ToolbarEvent::SetCustomUndoSteps(steps.saturating_add(1))
                        } else {
                            ToolbarEvent::SetCustomRedoSteps(steps.saturating_add(1))
                        },
                        kind: HitKind::Click,
                        tooltip: Some(if is_undo {
                            "Increase undo steps".to_string()
                        } else {
                            "Increase redo steps".to_string()
                        }),
                    });

                    let slider_y = y + row_h + 8.0;
                    let slider_h = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HEIGHT;
                    let slider_r = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_KNOB_RADIUS;
                    let slider_w = w - ToolbarLayoutSpec::SIDE_CARD_INSET * 2.0;
                    ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
                    draw_round_rect(ctx, x, slider_y, slider_w, slider_h, 3.0);
                    let _ = ctx.fill();
                    let t = delay_t_from_ms(delay_ms);
                    let knob_x = x + t * (slider_w - slider_r * 2.0) + slider_r;
                    ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
                    ctx.arc(
                        knob_x,
                        slider_y + slider_h / 2.0,
                        slider_r,
                        0.0,
                        std::f64::consts::PI * 2.0,
                    );
                    let _ = ctx.fill();
                    let hit_pad = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING;
                    hits.push(HitRegion {
                        rect: (x, slider_y - hit_pad, slider_w, slider_h + hit_pad * 2.0),
                        event: if is_undo {
                            ToolbarEvent::SetCustomUndoDelay(delay_secs_from_t(t))
                        } else {
                            ToolbarEvent::SetCustomRedoDelay(delay_secs_from_t(t))
                        },
                        kind: if is_undo {
                            HitKind::DragCustomUndoDelay
                        } else {
                            HitKind::DragCustomRedoDelay
                        },
                        tooltip: Some(if is_undo {
                            format!("Undo step delay: {:.1}s (drag)", delay_ms as f64 / 1000.0)
                        } else {
                            format!("Redo step delay: {:.1}s (drag)", delay_ms as f64 / 1000.0)
                        }),
                    });

                    slider_y + slider_h + 10.0 - y
                };

            let undo_row_h =
                render_custom_row(ctx, hits, x, custom_y, card_w, snapshot, true, hover);
            custom_y += undo_row_h + 8.0;
            let _redo_row_h =
                render_custom_row(ctx, hits, x, custom_y, card_w, snapshot, false, hover);
        }

        if snapshot.show_delay_sliders {
            let sliders_w = content_width;
            let slider_h = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HEIGHT;
            let slider_knob_r = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_KNOB_RADIUS;
            let slider_start_y = y
                + ToolbarLayoutSpec::SIDE_STEP_HEADER_HEIGHT
                + toggles_h
                + custom_content_h
                + ToolbarLayoutSpec::SIDE_STEP_SLIDER_TOP_PADDING;

            let undo_label = format!(
                "Undo delay: {:.1}s",
                snapshot.undo_all_delay_ms as f64 / 1000.0
            );
            ctx.set_source_rgba(0.7, 0.7, 0.75, 0.9);
            ctx.set_font_size(11.0);
            ctx.move_to(x, slider_start_y + 10.0);
            let _ = ctx.show_text(&undo_label);

            let undo_slider_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_UNDO_OFFSET_Y;
            ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
            draw_round_rect(ctx, x, undo_slider_y, sliders_w, slider_h, 3.0);
            let _ = ctx.fill();
            let undo_t = delay_t_from_ms(snapshot.undo_all_delay_ms);
            let undo_knob_x = x + undo_t * (sliders_w - slider_knob_r * 2.0) + slider_knob_r;
            ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
            ctx.arc(
                undo_knob_x,
                undo_slider_y + slider_h / 2.0,
                slider_knob_r,
                0.0,
                std::f64::consts::PI * 2.0,
            );
            let _ = ctx.fill();
            let hit_pad = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING;
            hits.push(HitRegion {
                rect: (
                    x,
                    undo_slider_y - hit_pad,
                    sliders_w,
                    slider_h + hit_pad * 2.0,
                ),
                event: ToolbarEvent::SetUndoDelay(delay_secs_from_t(undo_t)),
                kind: HitKind::DragUndoDelay,
                tooltip: Some(format!(
                    "Undo-all delay: {:.1}s (drag)",
                    snapshot.undo_all_delay_ms as f64 / 1000.0
                )),
            });

            let redo_label = format!(
                "Redo delay: {:.1}s",
                snapshot.redo_all_delay_ms as f64 / 1000.0
            );
            ctx.set_source_rgba(0.7, 0.7, 0.75, 0.9);
            ctx.move_to(x + sliders_w / 2.0 + 10.0, slider_start_y + 10.0);
            let _ = ctx.show_text(&redo_label);

            let redo_slider_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_REDO_OFFSET_Y;
            ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
            draw_round_rect(ctx, x, redo_slider_y, sliders_w, slider_h, 3.0);
            let _ = ctx.fill();
            let redo_t = delay_t_from_ms(snapshot.redo_all_delay_ms);
            let redo_knob_x = x + redo_t * (sliders_w - slider_knob_r * 2.0) + slider_knob_r;
            ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
            ctx.arc(
                redo_knob_x,
                redo_slider_y + slider_h / 2.0,
                slider_knob_r,
                0.0,
                std::f64::consts::PI * 2.0,
            );
            let _ = ctx.fill();
            hits.push(HitRegion {
                rect: (
                    x,
                    redo_slider_y - hit_pad,
                    sliders_w,
                    slider_h + hit_pad * 2.0,
                ),
                event: ToolbarEvent::SetRedoDelay(delay_secs_from_t(redo_t)),
                kind: HitKind::DragRedoDelay,
                tooltip: Some(format!(
                    "Redo-all delay: {:.1}s (drag)",
                    snapshot.redo_all_delay_ms as f64 / 1000.0
                )),
            });
        }
        y += custom_card_h + section_gap;
    }

    if snapshot.show_settings_section {
        let settings_card_h = spec.side_settings_height(snapshot);
        draw_group_card(ctx, card_x, y, card_w, settings_card_h);
        draw_section_label(
            ctx,
            x,
            y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
            "Settings",
        );

        let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
        let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
        let mut toggles: Vec<(&str, bool, ToolbarEvent, Option<&str>)> = vec![
            (
                "Tool preview",
                snapshot.show_tool_preview,
                ToolbarEvent::ToggleToolPreview(!snapshot.show_tool_preview),
                Some("Tool preview: cursor bubble."),
            ),
            (
                "Preset toasts",
                snapshot.show_preset_toasts,
                ToolbarEvent::TogglePresetToasts(!snapshot.show_preset_toasts),
                Some("Preset toasts: apply/save/clear."),
            ),
        ];
        if snapshot.layout_mode == crate::config::ToolbarLayoutMode::Advanced {
            toggles.extend_from_slice(&[
                (
                    "Show presets",
                    snapshot.show_presets,
                    ToolbarEvent::TogglePresets(!snapshot.show_presets),
                    Some("Presets: quick slots."),
                ),
                (
                    "Show actions",
                    snapshot.show_actions_section,
                    ToolbarEvent::ToggleActionsSection(!snapshot.show_actions_section),
                    Some("Actions: undo/redo/clear."),
                ),
                (
                    "Adv. Actions",
                    snapshot.show_actions_advanced,
                    ToolbarEvent::ToggleActionsAdvanced(!snapshot.show_actions_advanced),
                    Some("Advanced: undo-all/delay/zoom."),
                ),
                (
                    "Step controls",
                    snapshot.show_step_section,
                    ToolbarEvent::ToggleStepSection(!snapshot.show_step_section),
                    Some("Step: step undo/redo."),
                ),
                (
                    "Text controls",
                    snapshot.show_text_controls,
                    ToolbarEvent::ToggleTextControls(!snapshot.show_text_controls),
                    Some("Text: font size/family."),
                ),
            ]);
        }

        let mut toggle_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
        let toggle_col_gap = toggle_gap;
        let toggle_col_w = (content_width - toggle_col_gap) / 2.0;
        ctx.set_font_size(12.0);
        for row in 0..toggles.len().div_ceil(2) {
            for col in 0..2 {
                let idx = row * 2 + col;
                if idx >= toggles.len() {
                    break;
                }
                let (label, value, event, tooltip) = &toggles[idx];
                let toggle_x = x + col as f64 * (toggle_col_w + toggle_col_gap);
                let toggle_hover = hover
                    .map(|(hx, hy)| {
                        point_in_rect(hx, hy, toggle_x, toggle_y, toggle_col_w, toggle_h)
                    })
                    .unwrap_or(false);
                draw_checkbox(
                    ctx,
                    toggle_x,
                    toggle_y,
                    toggle_col_w,
                    toggle_h,
                    *value,
                    toggle_hover,
                    label,
                );
                hits.push(HitRegion {
                    rect: (toggle_x, toggle_y, toggle_col_w, toggle_h),
                    event: event.clone(),
                    kind: HitKind::Click,
                    tooltip: tooltip.map(|text| text.to_string()),
                });
            }
            if row + 1 < toggles.len().div_ceil(2) {
                toggle_y += toggle_h + toggle_gap;
            } else {
                toggle_y += toggle_h;
            }
        }
        ctx.set_font_size(13.0);

        let buttons_y = toggle_y + toggle_gap;
        let button_h = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_HEIGHT;
        let button_gap = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_GAP;
        let button_w = (content_width - button_gap) / 2.0;
        let icon_size = 16.0;

        let config_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, buttons_y, button_w, button_h))
            .unwrap_or(false);
        draw_button(ctx, x, buttons_y, button_w, button_h, false, config_hover);
        if use_icons {
            set_icon_color(ctx, config_hover);
            toolbar_icons::draw_icon_settings(
                ctx,
                x + (button_w - icon_size) / 2.0,
                buttons_y + (button_h - icon_size) / 2.0,
                icon_size,
            );
        } else {
            draw_label_center(ctx, x, buttons_y, button_w, button_h, "Config UI");
        }
        hits.push(HitRegion {
            rect: (x, buttons_y, button_w, button_h),
            event: ToolbarEvent::OpenConfigurator,
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                "Config UI",
                snapshot.binding_hints.open_configurator.as_deref(),
            )),
        });

        let file_x = x + button_w + button_gap;
        let file_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, file_x, buttons_y, button_w, button_h))
            .unwrap_or(false);
        draw_button(
            ctx, file_x, buttons_y, button_w, button_h, false, file_hover,
        );
        if use_icons {
            set_icon_color(ctx, file_hover);
            toolbar_icons::draw_icon_file(
                ctx,
                file_x + (button_w - icon_size) / 2.0,
                buttons_y + (button_h - icon_size) / 2.0,
                icon_size,
            );
        } else {
            draw_label_center(ctx, file_x, buttons_y, button_w, button_h, "Config file");
        }
        hits.push(HitRegion {
            rect: (file_x, buttons_y, button_w, button_h),
            event: ToolbarEvent::OpenConfigFile,
            kind: HitKind::Click,
            tooltip: Some("Config file".to_string()),
        });
    }

    draw_tooltip(ctx, hits, hover, width, false);
    Ok(())
}
