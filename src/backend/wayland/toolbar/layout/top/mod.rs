#![allow(dead_code)]

use super::super::events::HitKind;
use super::super::hit::HitRegion;
use super::spec::ToolbarLayoutSpec;
use crate::config::ToolbarLayoutMode;
use crate::input::Tool;
use crate::ui::toolbar::model;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

mod icons;
mod text;

pub fn build_top_hits(
    width: f64,
    height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
) {
    let spec = ToolbarLayoutSpec::new(snapshot);
    let is_simple = snapshot.layout_mode == ToolbarLayoutMode::Simple;
    let fill_tool_active = model::fill_tool_active(snapshot.active_tool, snapshot.tool_override);

    if spec.use_icons() {
        icons::build_hits(height, snapshot, &spec, is_simple, fill_tool_active, hits);
    } else {
        text::build_hits(height, snapshot, &spec, is_simple, fill_tool_active, hits);
    }

    let btn_size = ToolbarLayoutSpec::TOP_PIN_BUTTON_SIZE;
    let btn_y = spec.top_pin_button_y(height);

    let mut right_x = width - ToolbarLayoutSpec::TOP_PIN_BUTTON_MARGIN_RIGHT - btn_size;
    if model::toolbar_item_visible(snapshot, "top.chrome.close") {
        hits.push(HitRegion {
            rect: (right_x, btn_y, btn_size, btn_size),
            event: ToolbarEvent::CloseTopToolbar,
            kind: HitKind::Click,
            tooltip: Some("Close".to_string()),
        });
        right_x -= btn_size + ToolbarLayoutSpec::TOP_PIN_BUTTON_GAP;
    }

    if model::toolbar_item_visible(snapshot, "top.chrome.pin") {
        hits.push(HitRegion {
            rect: (right_x, btn_y, btn_size, btn_size),
            event: ToolbarEvent::PinTopToolbar(!snapshot.top_pinned),
            kind: HitKind::Click,
            tooltip: Some(if snapshot.top_pinned {
                "Unpin".to_string()
            } else {
                "Pin".to_string()
            }),
        });
    }
}

fn shape_buttons() -> &'static [Tool] {
    model::polygon_tools()
}
