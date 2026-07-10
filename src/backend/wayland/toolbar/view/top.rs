//! Top-strip tree builder: a pixel-neutral port of the legacy render pass.
//!
//! Every rect here reproduces the coordinates the render code computed
//! historically (spec constants in `layout/spec/top.rs`); the visual regroup
//! happens in a later phase as a data-only change to this builder.

use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::config::{Action, action_label, action_short_label, toolbar_item_ids as ids};
use crate::input::Tool;
use crate::toolbar_icons;
use crate::ui::toolbar::bindings::{tool_label, tool_tooltip_label};
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot, model};

use super::node::{ButtonStyle, IconFn, Interaction, LabelSpec, WidgetKind, WidgetNode};
use super::tree::WidgetTree;

const TOP_LABEL_FONT_SIZE: f64 = 14.0;
const ICON_TOGGLE_FONT_SIZE: f64 = 12.0;
const MINI_LABEL_FONT_SIZE: f64 = 10.0; // FONT_SIZE_SMALL

/// Build the complete top-strip tree for the given logical surface size.
pub fn build_top_view(snapshot: &ToolbarSnapshot, width: f64, height: f64) -> WidgetTree {
    let spec = ToolbarLayoutSpec::new(snapshot);
    let gap = ToolbarLayoutSpec::TOP_GAP;
    let mut tree = WidgetTree::new((width, height));

    tree.push(WidgetNode::decor(
        "top.panel",
        (0.0, 0.0, width, height),
        WidgetKind::Panel,
    ));

    let mut x = ToolbarLayoutSpec::TOP_START_X;
    let handle_size = ToolbarLayoutSpec::TOP_HANDLE_SIZE;
    let handle_visible = model::toolbar_item_visible(snapshot, ids::TOP_CHROME_DRAG);
    if handle_visible {
        tree.push(WidgetNode::new(
            ids::TOP_CHROME_DRAG.as_str(),
            (x, ToolbarLayoutSpec::TOP_HANDLE_Y, handle_size, handle_size),
            WidgetKind::DragHandle,
            Some(Interaction {
                event: ToolbarEvent::MoveTopToolbar { x: 0.0, y: 0.0 },
                kind: HitKind::DragMoveTop,
                tooltip: Some("Drag toolbar".to_string()),
            }),
        ));
        x += handle_size + gap;
    }
    let picker_handle_w = if handle_visible { handle_size } else { 0.0 };

    let is_simple = snapshot.layout_mode == crate::config::ToolbarLayoutMode::Simple;
    let current_shape_tool =
        model::current_shape_tool(snapshot.active_tool, snapshot.tool_override);
    let fill_tool_active = model::fill_tool_active(snapshot.active_tool, snapshot.tool_override);

    let (btn_w, btn_h) = spec.top_button_size();
    let y = spec.top_button_y(height);

    // --- Tool row ---------------------------------------------------------
    let mut fill_anchor: Option<(f64, f64)> = None;
    let mut rect_x = None;
    let mut fill_end_x = None;
    for tool in model::visible_top_tool_buttons(is_simple, snapshot) {
        if model::is_fill_tool(tool) {
            if rect_x.is_none() {
                rect_x = Some(x);
            }
            fill_end_x = Some(x + btn_w);
        }
        let active = snapshot.active_tool == tool || snapshot.tool_override == Some(tool);
        tree.push(tool_button_node(
            snapshot,
            tool,
            model::toolbar_item_id_for_tool(tool).as_str(),
            (x, y, btn_w, btn_h),
            active,
        ));
        x += btn_w + gap;
    }

    // --- Shape/Polygon picker toggle --------------------------------------
    if model::top_shape_picker_visible(snapshot) {
        let (label, tooltip, icon_tool, picker_active) = if is_simple {
            let icon_tool = current_shape_tool.unwrap_or_else(model::default_shape_tool);
            let active = snapshot.shape_picker_open || current_shape_tool.is_some();
            ("Shapes", "Shapes", icon_tool, active)
        } else {
            let current_polygon = current_shape_tool.filter(|tool| model::is_polygon_tool(*tool));
            let icon_tool = current_polygon.unwrap_or_else(model::default_polygon_tool);
            let active = snapshot.shape_picker_open || current_polygon.is_some();
            ("Poly", "Polygons", icon_tool, active)
        };
        let interact = Interaction::click(
            ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
            Some(tooltip.to_string()),
        );
        let kind = if snapshot.use_icons {
            WidgetKind::IconButton {
                glyph: semantic_icon_fn(model::semantic_icon_for_tool(icon_tool)),
                icon_size: ToolbarLayoutSpec::TOP_ICON_SIZE,
                style: ButtonStyle::active(picker_active),
            }
        } else {
            WidgetKind::TextButton {
                label: LabelSpec::new(label, TOP_LABEL_FONT_SIZE, true),
                style: ButtonStyle::active(picker_active),
            }
        };
        tree.push(WidgetNode::new(
            ids::TOP_UTILITY_SHAPE_PICKER.as_str(),
            (x, y, btn_w, btn_h),
            kind,
            Some(interact),
        ));
        // Fill anchor bookkeeping (icon mode hangs the checkbox below):
        // under the picker button when it owns the fill-capable tool,
        // otherwise spanning the inline fill-capable tools.
        let picker_owns_fill = is_simple
            || current_shape_tool
                .filter(|tool| model::is_polygon_tool(*tool))
                .is_some();
        if picker_owns_fill {
            fill_anchor = Some((x, btn_w));
        } else if let (Some(rect_x), Some(fill_end_x)) = (rect_x, fill_end_x) {
            fill_anchor = Some((rect_x, fill_end_x - rect_x));
        }
        x += btn_w + gap;
    }

    // --- Fill toggle -------------------------------------------------------
    let fill_visible =
        fill_tool_active && !snapshot.shape_picker_open && model::top_fill_visible(snapshot);
    if fill_visible {
        let interact = Interaction::click(
            ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
            Some(format_binding_label(
                action_label(Action::ToggleFill),
                snapshot
                    .binding_hints
                    .binding_for_action(Action::ToggleFill),
            )),
        );
        if snapshot.use_icons {
            // Mini checkbox hanging below the fill-capable tool span.
            if let Some((fill_x, fill_w)) = fill_anchor {
                let fill_y = y + btn_h + ToolbarLayoutSpec::TOP_ICON_FILL_OFFSET;
                tree.push(WidgetNode::new(
                    ids::TOP_UTILITY_FILL.as_str(),
                    (
                        fill_x,
                        fill_y,
                        fill_w,
                        ToolbarLayoutSpec::TOP_ICON_FILL_HEIGHT,
                    ),
                    WidgetKind::MiniCheckbox {
                        checked: snapshot.fill_enabled,
                        label: LabelSpec::new(
                            action_short_label(Action::ToggleFill),
                            MINI_LABEL_FONT_SIZE,
                            false,
                        ),
                    },
                    Some(interact),
                ));
            }
        } else {
            // Inline checkbox between the picker and the utilities.
            let fill_w = ToolbarLayoutSpec::TOP_TEXT_FILL_W;
            tree.push(WidgetNode::new(
                ids::TOP_UTILITY_FILL.as_str(),
                (x, y, fill_w, btn_h),
                WidgetKind::Checkbox {
                    checked: snapshot.fill_enabled,
                    label: LabelSpec::new(
                        action_short_label(Action::ToggleFill),
                        TOP_LABEL_FONT_SIZE,
                        true,
                    ),
                },
                Some(interact),
            ));
            x += fill_w + gap;
        }
    }

    // --- Utility row --------------------------------------------------------
    for button in model::visible_top_utility_buttons(snapshot, is_simple, snapshot.use_icons) {
        match button {
            model::TopUtilityButton::Text => {
                tree.push(utility_node(
                    snapshot,
                    ids::TOP_UTILITY_TEXT.as_str(),
                    (x, y, btn_w, btn_h),
                    IconFn(toolbar_icons::draw_icon_text),
                    action_short_label(Action::EnterTextMode),
                    ButtonStyle::active(snapshot.text_active),
                    ToolbarEvent::EnterTextMode,
                    format_binding_label(
                        action_label(Action::EnterTextMode),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::EnterTextMode),
                    ),
                ));
                x += btn_w + gap;
            }
            model::TopUtilityButton::StickyNote => {
                tree.push(utility_node(
                    snapshot,
                    ids::TOP_UTILITY_STICKY_NOTE.as_str(),
                    (x, y, btn_w, btn_h),
                    IconFn(toolbar_icons::draw_icon_note),
                    action_short_label(Action::EnterStickyNoteMode),
                    ButtonStyle::active(snapshot.note_active),
                    ToolbarEvent::EnterStickyNoteMode,
                    format_binding_label(
                        action_label(Action::EnterStickyNoteMode),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::EnterStickyNoteMode),
                    ),
                ));
                x += btn_w + gap;
            }
            model::TopUtilityButton::Screenshot => {
                tree.push(utility_node(
                    snapshot,
                    ids::TOP_UTILITY_SCREENSHOT.as_str(),
                    (x, y, btn_w, btn_h),
                    IconFn(toolbar_icons::draw_icon_screenshot),
                    "Shot",
                    ButtonStyle::plain(),
                    ToolbarEvent::CaptureScreenshot,
                    format_binding_label(
                        action_label(Action::CaptureSelection),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::CaptureSelection),
                    ),
                ));
                x += btn_w + gap;
            }
            model::TopUtilityButton::ClearCanvas => {
                tree.push(utility_node(
                    snapshot,
                    ids::TOP_UTILITY_CLEAR_CANVAS.as_str(),
                    (x, y, btn_w, btn_h),
                    IconFn(toolbar_icons::draw_icon_clear),
                    action_short_label(Action::ClearCanvas),
                    ButtonStyle::destructive(),
                    ToolbarEvent::ClearCanvas,
                    format_binding_label(
                        action_label(Action::ClearCanvas),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::ClearCanvas),
                    ),
                ));
                x += btn_w + gap;
            }
            model::TopUtilityButton::Highlight => {
                tree.push(utility_node(
                    snapshot,
                    ids::TOP_UTILITY_HIGHLIGHT.as_str(),
                    (x, y, btn_w, btn_h),
                    IconFn(toolbar_icons::draw_icon_highlight),
                    "Highlight",
                    ButtonStyle::active(snapshot.any_highlight_active),
                    ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active),
                    format_binding_label(
                        action_label(Action::ToggleHighlightTool),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::ToggleHighlightTool),
                    ),
                ));
                if snapshot.highlight_tool_active && model::top_highlight_ring_visible(snapshot) {
                    let ring_y = y + btn_h + ToolbarLayoutSpec::TOP_ICON_FILL_OFFSET;
                    tree.push(WidgetNode::new(
                        ids::TOP_UTILITY_HIGHLIGHT_RING.as_str(),
                        (x, ring_y, btn_w, ToolbarLayoutSpec::TOP_ICON_FILL_HEIGHT),
                        WidgetKind::MiniCheckbox {
                            checked: snapshot.highlight_tool_ring_enabled,
                            label: LabelSpec::new("Ring", MINI_LABEL_FONT_SIZE, false),
                        },
                        Some(Interaction::click(
                            ToolbarEvent::ToggleHighlightToolRing(
                                !snapshot.highlight_tool_ring_enabled,
                            ),
                            Some("Highlight ring".to_string()),
                        )),
                    ));
                }
                x += btn_w + gap;
            }
            model::TopUtilityButton::IconMode => {}
        }
    }

    // --- Ico/Txt segmented toggle -------------------------------------------
    if model::top_icon_mode_toggle_visible(snapshot) {
        let toggle_w = ToolbarLayoutSpec::TOP_TOGGLE_WIDTH;
        let half_w = toggle_w / 2.0;
        tree.push(WidgetNode::decor(
            "top.utility.icon-mode",
            (x, y, toggle_w, btn_h),
            WidgetKind::SegmentedControl {
                left: LabelSpec::new("Ico", ICON_TOGGLE_FONT_SIZE, true),
                right: LabelSpec::new("Txt", ICON_TOGGLE_FONT_SIZE, true),
                active_right: !snapshot.use_icons,
            },
        ));
        tree.push(WidgetNode::new(
            ids::TOP_UTILITY_ICON_MODE_ICONS.as_str(),
            (x, y, half_w, btn_h),
            WidgetKind::HitArea,
            Some(Interaction::click(
                ToolbarEvent::ToggleIconMode(true),
                Some("Icons mode".to_string()),
            )),
        ));
        tree.push(WidgetNode::new(
            ids::TOP_UTILITY_ICON_MODE_TEXT.as_str(),
            (x + half_w, y, half_w, btn_h),
            WidgetKind::HitArea,
            Some(Interaction::click(
                ToolbarEvent::ToggleIconMode(false),
                Some("Text mode".to_string()),
            )),
        ));
    }

    // --- Divider between tools and utilities (icon mode only) ---------------
    // Drawn last in the legacy path order, but position depends on the tool
    // row end; recompute it the same way the icon renderer did.
    if snapshot.use_icons {
        let tool_count = model::visible_top_tool_buttons(is_simple, snapshot).count()
            + usize::from(model::top_shape_picker_visible(snapshot));
        if tool_count > 0 {
            let tools_end = ToolbarLayoutSpec::TOP_START_X
                + if handle_visible {
                    handle_size + gap
                } else {
                    0.0
                }
                + tool_count as f64 * (btn_w + gap);
            tree.push(WidgetNode::decor(
                "top.divider.tools",
                (tools_end - gap * 0.5, y + 6.0, 1.0, btn_h - 12.0),
                WidgetKind::Divider { vertical: true },
            ));
        }
    }

    // --- Shape picker rows ---------------------------------------------------
    if snapshot.shape_picker_open && model::top_shape_picker_visible(snapshot) {
        let mut row_y = y + btn_h + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
        for row in model::visible_shape_picker_rows(snapshot, is_simple) {
            let mut shape_x = ToolbarLayoutSpec::TOP_START_X + picker_handle_w + gap;
            for tool in row {
                if !model::tool_visible(snapshot, tool) {
                    continue;
                }
                let active = snapshot.active_tool == tool || snapshot.tool_override == Some(tool);
                tree.push(tool_button_node(
                    snapshot,
                    tool,
                    format!(
                        "top.picker.{}",
                        model::toolbar_item_id_for_tool(tool).as_str()
                    ),
                    (shape_x, row_y, btn_w, btn_h),
                    active,
                ));
                shape_x += btn_w + gap;
            }
            row_y += btn_h + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
        }
    }

    // --- Right-aligned chrome -------------------------------------------------
    let chrome_size = ToolbarLayoutSpec::TOP_PIN_BUTTON_SIZE;
    let chrome_y = spec.top_pin_button_y(height);
    let mut right_x = width - ToolbarLayoutSpec::TOP_PIN_BUTTON_MARGIN_RIGHT - chrome_size;
    if model::toolbar_item_visible(snapshot, ids::TOP_CHROME_CLOSE) {
        tree.push(WidgetNode::new(
            ids::TOP_CHROME_CLOSE.as_str(),
            (right_x, chrome_y, chrome_size, chrome_size),
            WidgetKind::CloseButton,
            Some(Interaction::click(
                ToolbarEvent::CloseTopToolbar,
                Some("Close".to_string()),
            )),
        ));
        right_x -= chrome_size + ToolbarLayoutSpec::TOP_PIN_BUTTON_GAP;
    }
    if model::toolbar_item_visible(snapshot, ids::TOP_CHROME_PIN) {
        tree.push(WidgetNode::new(
            ids::TOP_CHROME_PIN.as_str(),
            (right_x, chrome_y, chrome_size, chrome_size),
            WidgetKind::PinButton {
                pinned: snapshot.top_pinned,
            },
            Some(Interaction::click(
                ToolbarEvent::PinTopToolbar(!snapshot.top_pinned),
                Some(if snapshot.top_pinned {
                    "Pinned: opens at startup (click to disable)".to_string()
                } else {
                    "Pin: click to open at startup".to_string()
                }),
            )),
        ));
    }

    tree
}

fn tool_button_node(
    snapshot: &ToolbarSnapshot,
    tool: Tool,
    id: impl Into<super::node::WidgetId>,
    rect: (f64, f64, f64, f64),
    active: bool,
) -> WidgetNode {
    let tooltip = tool_tooltip(snapshot, tool, tool_tooltip_label(tool));
    let kind = if snapshot.use_icons {
        WidgetKind::IconButton {
            glyph: semantic_icon_fn(model::semantic_icon_for_tool(tool)),
            icon_size: ToolbarLayoutSpec::TOP_ICON_SIZE,
            style: ButtonStyle::active(active),
        }
    } else {
        WidgetKind::TextButton {
            label: LabelSpec::new(tool_label(tool), TOP_LABEL_FONT_SIZE, true),
            style: ButtonStyle::active(active),
        }
    };
    WidgetNode::new(
        id,
        rect,
        kind,
        Some(Interaction::click(
            ToolbarEvent::SelectTool(tool),
            Some(tooltip),
        )),
    )
}

#[allow(clippy::too_many_arguments)]
fn utility_node(
    snapshot: &ToolbarSnapshot,
    id: &'static str,
    rect: (f64, f64, f64, f64),
    glyph: IconFn,
    label: &str,
    style: ButtonStyle,
    event: ToolbarEvent,
    tooltip: String,
) -> WidgetNode {
    let kind = if snapshot.use_icons {
        WidgetKind::IconButton {
            glyph,
            icon_size: ToolbarLayoutSpec::TOP_ICON_SIZE,
            style,
        }
    } else {
        WidgetKind::TextButton {
            label: LabelSpec::new(label, TOP_LABEL_FONT_SIZE, true),
            style,
        }
    };
    WidgetNode::new(
        id,
        rect,
        kind,
        Some(Interaction::click(event, Some(tooltip))),
    )
}

/// Tooltip text for a tool button: label plus binding and/or drag hint.
fn tool_tooltip(snapshot: &ToolbarSnapshot, tool: Tool, label: &str) -> String {
    let default_hint = model::default_drag_hint(tool);
    let binding = match (snapshot.binding_hints.for_tool(tool), default_hint) {
        (Some(binding), Some(fallback)) => Some(format!("{}, {}", binding, fallback)),
        (Some(binding), None) => Some(binding.to_string()),
        (None, Some(fallback)) => Some(fallback.to_string()),
        (None, None) => None,
    };
    format_binding_label(label, binding.as_deref())
}

/// Glyph function for a semantic tool icon.
pub(crate) fn semantic_icon_fn(icon: model::SemanticToolIcon) -> IconFn {
    use model::SemanticToolIcon as S;
    IconFn(match icon {
        S::Select => toolbar_icons::draw_icon_select,
        S::Pen => toolbar_icons::draw_icon_pen,
        S::Line => toolbar_icons::draw_icon_line,
        S::Rect => toolbar_icons::draw_icon_rect,
        S::Circle => toolbar_icons::draw_icon_circle,
        S::Triangle => toolbar_icons::draw_icon_triangle,
        S::Parallelogram => toolbar_icons::draw_icon_parallelogram,
        S::Rhombus => toolbar_icons::draw_icon_rhombus,
        S::Polygon => toolbar_icons::draw_icon_polygon,
        S::FreeformPolygon => toolbar_icons::draw_icon_freeform_polygon,
        S::Arrow => toolbar_icons::draw_icon_arrow,
        S::Blur => toolbar_icons::draw_icon_blur,
        S::Marker => toolbar_icons::draw_icon_marker,
        S::Highlight => toolbar_icons::draw_icon_highlight,
        S::StepMarker => toolbar_icons::draw_icon_step_marker,
        S::Eraser => toolbar_icons::draw_icon_eraser,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::toolbar::layout::top_size;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarBindingHints;

    fn snapshot() -> ToolbarSnapshot {
        let state = make_test_input_state();
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default())
    }

    fn build(snapshot: &ToolbarSnapshot) -> WidgetTree {
        let (w, h) = top_size(snapshot);
        build_top_view(snapshot, w as f64, h as f64)
    }

    fn node_id_list(tree: &WidgetTree) -> Vec<&str> {
        tree.nodes().iter().map(|node| node.id.as_str()).collect()
    }

    #[test]
    fn default_icon_strip_matches_legacy_geometry() {
        let snapshot = snapshot();
        let tree = build(&snapshot);

        // Drag handle at the historical spot.
        let drag = tree
            .node_by_id(&"top.chrome.drag".into())
            .expect("drag handle");
        assert_eq!(drag.rect, (19.0, 27.0, 18.0, 18.0));

        // First tool button starts after the handle; 46px at y=8.
        let select = tree
            .node_by_id(&"top.tool.select".into())
            .expect("select tool");
        assert_eq!(select.rect, (42.0, 8.0, 46.0, 46.0));

        // Second tool advances by button + 5px gap.
        let pen = tree.node_by_id(&"top.tool.pen".into()).expect("pen tool");
        assert_eq!(pen.rect, (93.0, 8.0, 46.0, 46.0));

        // Screenshot is hidden by default.
        assert!(tree.node_by_id(&"top.utility.screenshot".into()).is_none());

        // Chrome hangs on the right edge of the declared width.
        let (w, _) = tree.size();
        let close = tree
            .node_by_id(&"top.chrome.close".into())
            .expect("close button");
        assert_eq!(close.rect, (w - 15.0 - 24.0, 24.0, 24.0, 24.0));
        let pin = tree.node_by_id(&"top.chrome.pin".into()).expect("pin");
        assert_eq!(pin.rect.0, close.rect.0 - 24.0 - 6.0);

        // All interactive rects lie within the surface bounds.
        for node in tree.nodes() {
            if node.interact.is_some() {
                assert!(node.rect.0 >= 0.0 && node.rect.1 >= 0.0, "{:?}", node.id);
                assert!(
                    node.rect.0 + node.rect.2 <= w + 0.5,
                    "{:?} exceeds width",
                    node.id
                );
            }
        }
    }

    #[test]
    fn icon_strip_contains_expected_items_in_order() {
        let snapshot = snapshot();
        let tree = build(&snapshot);
        let ids = node_id_list(&tree);

        let expected_prefix = [
            "top.panel",
            "top.chrome.drag",
            "top.tool.select",
            "top.tool.pen",
            "top.tool.marker",
            "top.tool.step-marker",
            "top.tool.eraser",
            "top.tool.line",
            "top.tool.rect",
            "top.tool.ellipse",
            "top.tool.arrow",
            "top.tool.blur",
            "top.utility.shape-picker",
        ];
        assert_eq!(&ids[..expected_prefix.len()], &expected_prefix);
        assert!(ids.contains(&"top.utility.icon-mode"));
        assert!(ids.contains(&"top.chrome.pin"));
        assert!(ids.contains(&"top.chrome.close"));
    }

    #[test]
    fn text_mode_centers_buttons_and_swaps_kinds() {
        let mut snapshot = snapshot();
        snapshot.use_icons = false;
        let tree = build(&snapshot);

        let pen = tree.node_by_id(&"top.tool.pen".into()).expect("pen tool");
        assert!(matches!(pen.kind, WidgetKind::TextButton { .. }));
        // 60x36 buttons vertically centered in the 60px bar.
        assert_eq!(pen.rect.2, 60.0);
        assert_eq!(pen.rect.3, 36.0);
        assert_eq!(pen.rect.1, 12.0);

        // Highlight utility exists only in icon mode.
        assert!(tree.node_by_id(&"top.utility.highlight".into()).is_none());
    }

    #[test]
    fn shape_picker_rows_expand_below_the_bar() {
        let mut state = make_test_input_state();
        state.toolbar_shapes_expanded = true;
        let snapshot =
            ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
        assert!(snapshot.shape_picker_open);

        let tree = build(&snapshot);
        let picker_ids: Vec<&str> = tree
            .nodes()
            .iter()
            .map(|node| node.id.as_str())
            .filter(|id| id.starts_with("top.picker."))
            .collect();
        assert!(!picker_ids.is_empty(), "picker rows populated");

        // Picker rows sit below the tool row.
        let first = tree
            .node_by_id(&picker_ids[0].to_string().into())
            .expect("picker node");
        assert_eq!(first.rect.1, 8.0 + 46.0 + 6.0);
        // Row is indented past the drag handle.
        assert_eq!(first.rect.0, 19.0 + 18.0 + 5.0);
    }

    #[test]
    fn segmented_toggle_emits_two_half_width_hits() {
        let snapshot = snapshot();
        let tree = build(&snapshot);

        let seg = tree
            .node_by_id(&"top.utility.icon-mode".into())
            .expect("segment body");
        let icons_half = tree
            .node_by_id(&"top.utility.icon-mode-icons".into())
            .expect("icons half");
        let text_half = tree
            .node_by_id(&"top.utility.icon-mode-text".into())
            .expect("text half");

        assert_eq!(seg.rect.2, 84.0);
        assert_eq!(icons_half.rect.2, 42.0);
        assert_eq!(text_half.rect.0, icons_half.rect.0 + 42.0);
        assert!(matches!(
            icons_half.interact.as_ref().unwrap().event,
            ToolbarEvent::ToggleIconMode(true)
        ));
        assert!(matches!(
            text_half.interact.as_ref().unwrap().event,
            ToolbarEvent::ToggleIconMode(false)
        ));
    }

    #[test]
    fn hidden_items_produce_no_nodes() {
        let mut snapshot = snapshot();
        snapshot.resolved_toolbar_items = crate::config::ToolbarItemsConfig {
            hidden: vec![ids::TOP_TOOL_PEN.as_str().to_string()],
            shown: Vec::new(),
            order: crate::config::ToolbarItemOrderConfig::default(),
        }
        .resolved();

        let tree = build(&snapshot);
        assert!(tree.node_by_id(&"top.tool.pen".into()).is_none());
        assert!(tree.node_by_id(&"top.tool.marker".into()).is_some());
    }
}
