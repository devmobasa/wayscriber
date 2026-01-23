mod icons;
mod text;

use anyhow::Result;

use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::input::Tool;
use crate::ui::toolbar::ToolbarSnapshot;

use super::super::events::HitKind;
use super::widgets::{
    draw_close_button, draw_drag_handle, draw_panel_background, draw_pin_button, draw_tooltip,
    point_in_rect,
};
use crate::ui::toolbar::ToolbarEvent;

pub(super) const TOP_LABEL_FONT_SIZE: f64 = 14.0;
pub(super) const ICON_TOGGLE_FONT_SIZE: f64 = 12.0;

pub(super) struct TopStripLayout<'a> {
    pub(super) ctx: &'a cairo::Context,
    pub(super) height: f64,
    pub(super) snapshot: &'a ToolbarSnapshot,
    pub(super) hits: &'a mut Vec<HitRegion>,
    pub(super) hover: Option<(f64, f64)>,
    pub(super) spec: ToolbarLayoutSpec,
    pub(super) gap: f64,
}

impl<'a> TopStripLayout<'a> {
    fn new(
        ctx: &'a cairo::Context,
        height: f64,
        snapshot: &'a ToolbarSnapshot,
        hits: &'a mut Vec<HitRegion>,
        hover: Option<(f64, f64)>,
    ) -> Self {
        let spec = ToolbarLayoutSpec::new(snapshot);
        let gap = ToolbarLayoutSpec::TOP_GAP;
        Self {
            ctx,
            height,
            snapshot,
            hits,
            hover,
            spec,
            gap,
        }
    }

    pub(super) fn tool_tooltip(&self, tool: Tool, label: &str) -> String {
        let default_hint = match tool {
            Tool::Line => Some("Shift+Drag"),
            Tool::Rect => Some("Ctrl+Drag"),
            Tool::Ellipse => Some("Tab+Drag"),
            Tool::Arrow => Some("Ctrl+Shift+Drag"),
            _ => None,
        };
        let binding = match (self.snapshot.binding_hints.for_tool(tool), default_hint) {
            (Some(binding), Some(fallback)) => Some(format!("{}, {}", binding, fallback)),
            (Some(binding), None) => Some(binding.to_string()),
            (None, Some(fallback)) => Some(fallback.to_string()),
            (None, None) => None,
        };
        format_binding_label(label, binding.as_deref())
    }
}

pub fn render_top_strip(
    ctx: &cairo::Context,
    width: f64,
    height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
) -> Result<()> {
    draw_panel_background(ctx, width, height);

    let mut layout = TopStripLayout::new(ctx, height, snapshot, hits, hover);

    let mut x = ToolbarLayoutSpec::TOP_START_X;
    let handle_w = ToolbarLayoutSpec::TOP_HANDLE_SIZE;
    let handle_h = ToolbarLayoutSpec::TOP_HANDLE_SIZE;
    let handle_y = ToolbarLayoutSpec::TOP_HANDLE_Y;
    let handle_hover = layout
        .hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, handle_y, handle_w, handle_h))
        .unwrap_or(false);
    draw_drag_handle(ctx, x, handle_y, handle_w, handle_h, handle_hover);
    layout.hits.push(HitRegion {
        rect: (x, handle_y, handle_w, handle_h),
        event: ToolbarEvent::MoveTopToolbar { x: 0.0, y: 0.0 },
        kind: HitKind::DragMoveTop,
        tooltip: Some("Drag toolbar".to_string()),
    });
    x += handle_w + layout.gap;

    let is_simple = snapshot.layout_mode == crate::config::ToolbarLayoutMode::Simple;
    let current_shape_tool = match snapshot.tool_override {
        Some(Tool::Line) => Some(Tool::Line),
        Some(Tool::Rect) => Some(Tool::Rect),
        Some(Tool::Ellipse) => Some(Tool::Ellipse),
        Some(Tool::Arrow) => Some(Tool::Arrow),
        _ => match snapshot.active_tool {
            Tool::Line => Some(Tool::Line),
            Tool::Rect => Some(Tool::Rect),
            Tool::Ellipse => Some(Tool::Ellipse),
            Tool::Arrow => Some(Tool::Arrow),
            _ => None,
        },
    };
    let shape_icon_tool = current_shape_tool.unwrap_or(Tool::Rect);
    let fill_tool_active = matches!(snapshot.tool_override, Some(Tool::Rect | Tool::Ellipse))
        || matches!(snapshot.active_tool, Tool::Rect | Tool::Ellipse);

    if snapshot.use_icons {
        icons::draw_icon_strip(
            &mut layout,
            x,
            handle_w,
            is_simple,
            current_shape_tool,
            shape_icon_tool,
            fill_tool_active,
        );
    } else {
        text::draw_text_strip(
            &mut layout,
            x,
            handle_w,
            is_simple,
            current_shape_tool,
            fill_tool_active,
        );
    }

    let btn_size = ToolbarLayoutSpec::TOP_PIN_BUTTON_SIZE;
    let btn_y = layout.spec.top_pin_button_y(height);
    let pin_x = layout.spec.top_pin_x(width);
    let pin_hover = layout
        .hover
        .map(|(hx, hy)| point_in_rect(hx, hy, pin_x, btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_pin_button(ctx, pin_x, btn_y, btn_size, snapshot.top_pinned, pin_hover);
    layout.hits.push(HitRegion {
        rect: (pin_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::PinTopToolbar(!snapshot.top_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.top_pinned {
            "Unpin".to_string()
        } else {
            "Pin".to_string()
        }),
    });

    let close_x = layout.spec.top_close_x(width);
    let close_hover = layout
        .hover
        .map(|(hx, hy)| point_in_rect(hx, hy, close_x, btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_close_button(ctx, close_x, btn_y, btn_size, close_hover);
    layout.hits.push(HitRegion {
        rect: (close_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::CloseTopToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });

    draw_tooltip(ctx, layout.hits, layout.hover, width, height, false);
    Ok(())
}
