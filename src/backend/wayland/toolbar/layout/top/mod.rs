#![allow(dead_code)]

use super::super::events::HitKind;
use super::super::hit::HitRegion;
use super::spec::ToolbarLayoutSpec;
use crate::config::ToolbarLayoutMode;
use crate::input::Tool;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

mod icons;
mod text;

const TOOL_BUTTONS_SIMPLE: &[(Tool, &str)] = &[
    (Tool::Select, "Select"),
    (Tool::Pen, "Pen"),
    (Tool::Marker, "Marker"),
    (Tool::Eraser, "Eraser"),
];
const TOOL_BUTTONS_FULL: &[(Tool, &str)] = &[
    (Tool::Select, "Select"),
    (Tool::Pen, "Pen"),
    (Tool::Marker, "Marker"),
    (Tool::Eraser, "Eraser"),
    (Tool::Line, "Line"),
    (Tool::Rect, "Rect"),
    (Tool::Ellipse, "Circle"),
    (Tool::Arrow, "Arrow"),
];
const SHAPE_BUTTONS: &[(Tool, &str)] = &[
    (Tool::Line, "Line"),
    (Tool::Rect, "Rect"),
    (Tool::Ellipse, "Circle"),
    (Tool::Arrow, "Arrow"),
];

pub fn build_top_hits(
    width: f64,
    height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
) {
    let spec = ToolbarLayoutSpec::new(snapshot);
    let is_simple = snapshot.layout_mode == ToolbarLayoutMode::Simple;
    let fill_tool_active = matches!(snapshot.tool_override, Some(Tool::Rect | Tool::Ellipse))
        || matches!(snapshot.active_tool, Tool::Rect | Tool::Ellipse);

    if spec.use_icons() {
        icons::build_hits(height, snapshot, &spec, is_simple, fill_tool_active, hits);
    } else {
        text::build_hits(height, snapshot, &spec, is_simple, fill_tool_active, hits);
    }

    let btn_size = ToolbarLayoutSpec::TOP_PIN_BUTTON_SIZE;
    let btn_y = spec.top_pin_button_y(height);

    let pin_x = spec.top_pin_x(width);
    hits.push(HitRegion {
        rect: (pin_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::PinTopToolbar(!snapshot.top_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.top_pinned {
            "Unpin".to_string()
        } else {
            "Pin".to_string()
        }),
    });

    let close_x = spec.top_close_x(width);
    hits.push(HitRegion {
        rect: (close_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::CloseTopToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });
}

fn tool_buttons(is_simple: bool) -> &'static [(Tool, &'static str)] {
    if is_simple {
        TOOL_BUTTONS_SIMPLE
    } else {
        TOOL_BUTTONS_FULL
    }
}

fn shape_buttons() -> &'static [(Tool, &'static str)] {
    SHAPE_BUTTONS
}
