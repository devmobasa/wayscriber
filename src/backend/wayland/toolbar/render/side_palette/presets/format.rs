use crate::draw::EraserKind;
use crate::input::{EraserMode, Tool};
use crate::ui::toolbar::PresetSlotSnapshot;
use crate::util::color_to_name;

pub(super) fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "Select",
        Tool::Pen => "Pen",
        Tool::Line => "Line",
        Tool::Rect => "Rect",
        Tool::Ellipse => "Circle",
        Tool::Arrow => "Arrow",
        Tool::Marker => "Marker",
        Tool::Highlight => "Highlight",
        Tool::Eraser => "Eraser",
    }
}

pub(super) fn px_label(value: f64) -> String {
    if (value - value.round()).abs() < 0.05 {
        format!("{:.0}px", value)
    } else {
        format!("{:.1}px", value)
    }
}

pub(super) fn angle_label(value: f64) -> String {
    if (value - value.round()).abs() < 0.05 {
        format!("{:.0}deg", value)
    } else {
        format!("{:.1}deg", value)
    }
}

pub(super) fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

pub(super) fn eraser_kind_label(kind: EraserKind) -> &'static str {
    match kind {
        EraserKind::Circle => "circle",
        EraserKind::Rect => "rect",
    }
}

pub(super) fn eraser_mode_label(mode: EraserMode) -> &'static str {
    match mode {
        EraserMode::Brush => "brush",
        EraserMode::Stroke => "stroke",
    }
}

pub(super) fn truncate_label(value: &str, max_chars: usize) -> String {
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
}

pub(super) fn preset_tooltip_text(
    preset: &PresetSlotSnapshot,
    slot: usize,
    binding: Option<&str>,
) -> String {
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
    match binding {
        Some(binding) => format!("{label} (key: {binding})"),
        None => label,
    }
}
