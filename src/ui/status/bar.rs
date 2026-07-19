use std::f64::consts::PI;

use super::super::primitives::draw_rounded_rect;
use super::super::theme::overlay;
use crate::config::{Action, StatusPosition, action_display_label};
use crate::input::{BoardBackground, DrawingState, InputState, TextInputMode, Tool};
use crate::label_format::format_binding_labels;
use crate::ui::toolbar::bindings::action_for_tool;
use crate::ui_text::{UiTextLayout, UiTextStyle, text_layout};

// ============================================================================
// UI Layout Constants (not configurable)
// ============================================================================

/// Inset between the pill background and the screen edges
const STATUS_BAR_EDGE_INSET: f64 = overlay::SPACING_MD;
/// Corner radius of the pill background
const STATUS_BAR_CORNER_RADIUS: f64 = 11.0;
/// Horizontal gap between the color dot and its neighbouring text pieces
const STATUS_DOT_GAP: f64 = 6.0;
/// Maximum fraction of the screen width the whole pill (background including
/// padding) may occupy
const STATUS_BAR_MAX_WIDTH_FRACTION: f64 = 0.8;
/// Minimum share of the width budget reserved for the prefix when both text
/// pieces compete for space; the suffix is re-wrapped when this floor binds
const MIN_PREFIX_BUDGET_FRACTION: f64 = 0.25;
/// Separator between status segments
const SEGMENT_SEPARATOR: &str = " · ";

/// Render status bar showing current color, thickness, and tool
pub fn render_status_bar(
    ctx: &cairo::Context,
    input_state: &InputState,
    position: StatusPosition,
    style: &crate::config::StatusBarStyle,
    screen_width: u32,
    screen_height: u32,
) {
    let tool = input_state.active_tool();
    let color = input_state.color_for_tool(tool);
    let thickness = input_state.size_for_active_tool();

    let tool_name = tool_display_name(input_state, tool);
    let font_size = input_state.current_font_size;

    // Segments before the color dot, in display order.
    let mut pre_segments: Vec<String> = Vec::new();
    if input_state.frozen_active() {
        pre_segments.push("FROZEN".to_string());
    }
    if input_state.zoom_active() {
        let pct = (input_state.zoom_scale() * 100.0).round() as i32;
        pre_segments.push(if input_state.zoom_locked() {
            format!("ZOOM {}% LOCKED", pct)
        } else {
            format!("ZOOM {}%", pct)
        });
    }
    if input_state.boards.pan_enabled()
        && input_state.boards.show_pan_badge()
        && !input_state.board_is_transparent()
    {
        pre_segments.push(
            if input_state.boards.active_frame().view_offset() == (0, 0) {
                "PAN Space+Drag"
            } else {
                "PANNED Space+Drag"
            }
            .to_string(),
        );
    }
    if let Some(bounds) = input_state.selection_bounds() {
        let count = input_state.selected_shape_ids().len();
        pre_segments.push(if count == 1 {
            format!("{}×{}px", bounds.width, bounds.height)
        } else {
            format!("{} items: {}×{}px", count, bounds.width, bounds.height)
        });
    }
    if input_state.show_active_output_badge
        && let Some(label) = input_state.active_output_label.as_ref()
    {
        let label = crate::util::truncate_with_ellipsis(label, 28);
        pre_segments.push(format!("Output: {label}"));
    }
    if input_state.show_status_board_badge && input_state.boards.show_badge() {
        let board_index = input_state.boards.active_index() + 1;
        let board_count = input_state.boards.board_count();
        let board_name = crate::util::truncate_with_ellipsis(input_state.board_name(), 20);
        pre_segments.push(format!(
            "Board {}/{}: {}",
            board_index, board_count, board_name
        ));
    }
    if input_state.show_status_page_badge {
        let page_count = input_state.boards.page_count().max(1);
        let page_index = input_state.boards.active_page_index();
        let page_name = input_state
            .boards
            .board_states()
            .get(input_state.boards.active_index())
            .and_then(|board| board.pages.page_name(page_index))
            .map(|name| crate::util::truncate_with_ellipsis(name, 20));
        pre_segments.push(if let Some(name) = page_name {
            format!("Page {}/{}: {}", page_index + 1, page_count, name)
        } else {
            format!("Page {}/{}", page_index + 1, page_count)
        });
    }

    // Segments after the color dot, ending with the help hint.
    let mut post_segments: Vec<String> = vec![
        format!("{}px", thickness as i32),
        tool_name.to_string(),
        format!("Text {}px", font_size as i32),
    ];
    if input_state.click_highlight_enabled() {
        post_segments.push(action_display_label(Action::ToggleClickHighlight).to_string());
    }
    if input_state.highlight_tool_active() {
        post_segments.push(action_display_label(Action::SelectHighlightTool).to_string());
    }
    post_segments.push(format!(
        "{}={}",
        help_binding_label(input_state),
        action_display_label(Action::ToggleHelp)
    ));

    // The color dot replaces the color-name segment, so the separators around
    // it are rendered as part of the text pieces on either side.
    let prefix_text = if pre_segments.is_empty() {
        String::new()
    } else {
        format!("{} ·", pre_segments.join(SEGMENT_SEPARATOR))
    };
    let suffix_text = format!("· {}", post_segments.join(SEGMENT_SEPARATOR));

    log::debug!("Status bar font_size from config: {}", style.font_size);

    let measurement = measure_status_bar(ctx, style, &prefix_text, &suffix_text, screen_width);
    let pill_width = measurement.pill_width;
    let pill_height = measurement.pill_height;

    let (bx, by) = pill_origin(
        position,
        screen_width as f64,
        screen_height as f64,
        pill_width,
        pill_height,
    );

    let (bg_color, text_color) = match input_state.boards.active_background() {
        BoardBackground::Transparent => (style.bg_color, style.text_color),
        BoardBackground::Solid(color) => status_bar_palette_for_background(*color),
    };

    let [r, g, b, a] = bg_color;
    ctx.set_source_rgba(r, g, b, a);
    draw_rounded_rect(
        ctx,
        bx,
        by,
        pill_width,
        pill_height,
        STATUS_BAR_CORNER_RADIUS,
    );
    let _ = ctx.fill();

    let [r, g, b, a] = text_color;
    let mut cursor = bx + style.padding;
    if let Some(layout) = &measurement.prefix_layout {
        // Center the (possibly wrapped) prefix block within the pill so a
        // second line never spills past the background.
        let baseline =
            by + (pill_height - measurement.prefix_height) / 2.0 - measurement.prefix_bearing;
        ctx.set_source_rgba(r, g, b, a);
        layout.show_at_baseline(ctx, cursor, baseline);
        cursor += measurement.prefix_width + STATUS_DOT_GAP;
    }

    // Color dot: the sole indicator of the current draw color, centered in the pill.
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.arc(
        cursor + style.dot_radius,
        by + pill_height / 2.0,
        style.dot_radius,
        0.0,
        2.0 * PI,
    );
    let _ = ctx.fill();
    cursor += measurement.dot_diameter + STATUS_DOT_GAP;

    // Center the suffix within the pill as well, so it stays aligned with the
    // dot rather than sitting on the first baseline of a wrapped prefix.
    let baseline =
        by + (pill_height - measurement.suffix_height) / 2.0 - measurement.suffix_bearing;
    ctx.set_source_rgba(r, g, b, a);
    measurement
        .suffix_layout
        .show_at_baseline(ctx, cursor, baseline);
}

/// Measured geometry for the status bar pill and its text pieces.
struct StatusBarMeasurement {
    prefix_layout: Option<UiTextLayout>,
    suffix_layout: UiTextLayout,
    prefix_width: f64,
    prefix_height: f64,
    prefix_bearing: f64,
    suffix_height: f64,
    suffix_bearing: f64,
    dot_diameter: f64,
    pill_width: f64,
    pill_height: f64,
}

/// Shape the status bar text so the whole pill (background including padding)
/// never exceeds `STATUS_BAR_MAX_WIDTH_FRACTION` of the screen width. The
/// suffix is shaped first; the prefix wraps within the remaining budget,
/// floored at `MIN_PREFIX_BUDGET_FRACTION` of the total width budget (the
/// suffix is re-wrapped with what the floor leaves when it binds).
fn measure_status_bar(
    ctx: &cairo::Context,
    style: &crate::config::StatusBarStyle,
    prefix_text: &str,
    suffix_text: &str,
    screen_width: u32,
) -> StatusBarMeasurement {
    let max_width = screen_width as f64 * STATUS_BAR_MAX_WIDTH_FRACTION - style.padding * 2.0;
    let dot_diameter = style.dot_radius * 2.0;
    let has_prefix = !prefix_text.is_empty();
    // Width consumed by the dot and its gaps; the rest is shared by the text.
    let dot_span = if has_prefix {
        dot_diameter + STATUS_DOT_GAP * 2.0
    } else {
        dot_diameter + STATUS_DOT_GAP
    };
    let text_budget = (max_width - dot_span).max(1.0);

    let text_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: style.font_size,
    };

    let mut suffix_budget = text_budget;
    let mut suffix_layout = text_layout(ctx, text_style, suffix_text, Some(suffix_budget));
    let mut suffix_width = suffix_layout.ink_extents().width().min(suffix_budget);

    let prefix_layout = has_prefix.then(|| {
        let min_prefix_budget = (max_width * MIN_PREFIX_BUDGET_FRACTION).min(text_budget);
        let prefix_budget = (text_budget - suffix_width).max(min_prefix_budget);
        if suffix_width > text_budget - prefix_budget {
            // The prefix floor binds: re-wrap the suffix within what remains.
            suffix_budget = (text_budget - prefix_budget).max(1.0);
            suffix_layout = text_layout(ctx, text_style, suffix_text, Some(suffix_budget));
            suffix_width = suffix_layout.ink_extents().width().min(suffix_budget);
        }
        let layout = text_layout(ctx, text_style, prefix_text, Some(prefix_budget));
        (layout, prefix_budget)
    });

    let suffix_extents = suffix_layout.ink_extents();
    let (prefix_layout, prefix_width, prefix_height, prefix_bearing) = match prefix_layout {
        Some((layout, budget)) => {
            let extents = layout.ink_extents();
            (
                Some(layout),
                extents.width().min(budget),
                extents.height(),
                extents.y_bearing(),
            )
        }
        None => (None, 0.0, 0.0, 0.0),
    };

    let prefix_advance = if prefix_layout.is_some() {
        prefix_width + STATUS_DOT_GAP
    } else {
        0.0
    };
    let content_width = prefix_advance + dot_diameter + STATUS_DOT_GAP + suffix_width;
    let content_height = prefix_height.max(suffix_extents.height()).max(dot_diameter);

    let v_pad = style.padding * 0.5;
    StatusBarMeasurement {
        prefix_layout,
        suffix_layout,
        prefix_width,
        prefix_height,
        prefix_bearing,
        suffix_height: suffix_extents.height(),
        suffix_bearing: suffix_extents.y_bearing(),
        dot_diameter,
        pill_width: content_width + style.padding * 2.0,
        pill_height: content_height + v_pad * 2.0,
    }
}

/// Top-left corner of the pill for `position`, clamped so the pill never
/// leaves the screen even when it is as wide as the budget allows.
fn pill_origin(
    position: StatusPosition,
    screen_width: f64,
    screen_height: f64,
    pill_width: f64,
    pill_height: f64,
) -> (f64, f64) {
    let inset = STATUS_BAR_EDGE_INSET;
    let (bx, by) = match position {
        StatusPosition::TopLeft => (inset, inset),
        StatusPosition::TopRight => (screen_width - inset - pill_width, inset),
        StatusPosition::BottomLeft => (inset, screen_height - inset - pill_height),
        StatusPosition::BottomRight => (
            screen_width - inset - pill_width,
            screen_height - inset - pill_height,
        ),
    };
    (
        bx.clamp(inset, (screen_width - inset - pill_width).max(inset)),
        by.clamp(inset, (screen_height - inset - pill_height).max(inset)),
    )
}

fn tool_display_name(input_state: &InputState, tool: Tool) -> &'static str {
    match &input_state.state {
        DrawingState::TextInput { .. } => match input_state.text_input_mode {
            TextInputMode::Plain => action_display_label(Action::EnterTextMode),
            TextInputMode::StickyNote => action_display_label(Action::EnterStickyNoteMode),
        },
        DrawingState::Drawing { tool, .. } => tool_action_label(*tool),
        DrawingState::BuildingPolygon { .. } => "Freeform Polygon",
        DrawingState::MovingSelection { .. } => "Move",
        DrawingState::Selecting { .. } => "Select",
        DrawingState::ResizingText { .. } | DrawingState::ResizingSelection { .. } => "Resize",
        DrawingState::PendingTextClick { .. } | DrawingState::Idle => tool_action_label(tool),
    }
}

fn help_binding_label(input_state: &InputState) -> String {
    let mut labels = input_state.action_binding_labels(Action::ToggleHelp);
    if labels.iter().any(|label| label == "F1") {
        // Prefer showing F1 in the status bar when both defaults are bound.
        labels.retain(|label| label != "F10");
    }
    format_binding_labels(&labels)
}

fn tool_action_label(tool: Tool) -> &'static str {
    action_for_tool(tool)
        .map(action_display_label)
        .unwrap_or("Select")
}

fn status_bar_palette_for_background(color: crate::draw::Color) -> ([f64; 4], [f64; 4]) {
    let luminance = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
    if luminance > 0.5 {
        ([0.15, 0.15, 0.15, 0.85], [1.0, 1.0, 1.0, 1.0])
    } else {
        ([0.85, 0.85, 0.85, 0.85], [0.0, 0.0, 0.0, 1.0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatusBarStyle;

    /// Worst-case prefix: every badge active with long names on a narrow screen.
    const LONG_PREFIX: &str = "FROZEN · ZOOM 250% LOCKED · PANNED Space+Drag · \
         12 items: 1920×1080px · Output: DP-3 Dell UltraSharp U2723QE… · \
         Board 12/24: Retrospective sketches… · Page 37/64: Architecture over… ·";
    const LONG_SUFFIX: &str = "· 12px · Freeform Polygon · Text 48px · Click Highlight · \
         Highlighter · F1=Help";

    fn measurement_context() -> cairo::Context {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).unwrap();
        cairo::Context::new(&surface).unwrap()
    }

    #[test]
    fn pill_width_never_exceeds_max_fraction_of_screen() {
        let ctx = measurement_context();
        let style = StatusBarStyle::default();

        for screen_width in [1280_u32, 1366, 1920] {
            let measurement =
                measure_status_bar(&ctx, &style, LONG_PREFIX, LONG_SUFFIX, screen_width);
            let max_pill_width = screen_width as f64 * STATUS_BAR_MAX_WIDTH_FRACTION;
            assert!(
                measurement.pill_width <= max_pill_width + 1e-6,
                "pill width {} exceeds cap {} on {}px screen",
                measurement.pill_width,
                max_pill_width,
                screen_width
            );
        }
    }

    #[test]
    fn pill_width_stays_capped_without_prefix() {
        let ctx = measurement_context();
        let style = StatusBarStyle::default();

        let measurement = measure_status_bar(&ctx, &style, "", LONG_SUFFIX, 1280);
        assert!(measurement.prefix_layout.is_none());
        assert!(measurement.pill_width <= 1280.0 * STATUS_BAR_MAX_WIDTH_FRACTION + 1e-6);
    }

    #[test]
    fn wrapped_prefix_grows_pill_height() {
        let ctx = measurement_context();
        let style = StatusBarStyle::default();

        let narrow = measure_status_bar(&ctx, &style, LONG_PREFIX, LONG_SUFFIX, 1280);
        let wide = measure_status_bar(&ctx, &style, "FROZEN ·", LONG_SUFFIX, 3840);
        // The wrapped prefix block must be accounted for in the pill height so
        // extra lines never spill past the background.
        assert!(narrow.prefix_height >= narrow.suffix_height);
        assert!(narrow.pill_height >= narrow.prefix_height + style.padding);
        assert!(narrow.pill_height > wide.pill_height);
    }

    #[test]
    fn pill_origin_stays_on_screen_for_all_corners() {
        let inset = STATUS_BAR_EDGE_INSET;
        let (screen_width, screen_height) = (1280.0, 720.0);
        // Wider than the screen: right-aligned corners would go negative
        // without clamping.
        let (pill_width, pill_height) = (1500.0, 60.0);

        for position in [
            StatusPosition::TopLeft,
            StatusPosition::TopRight,
            StatusPosition::BottomLeft,
            StatusPosition::BottomRight,
        ] {
            let (bx, by) = pill_origin(
                position,
                screen_width,
                screen_height,
                pill_width,
                pill_height,
            );
            assert!(bx >= inset, "bx {} below inset for {:?}", bx, position);
            assert!(by >= inset, "by {} below inset for {:?}", by, position);
            assert!(by <= screen_height - inset - pill_height);
        }

        // A pill that fits keeps its requested corner alignment.
        let (bx, by) = pill_origin(
            StatusPosition::BottomRight,
            screen_width,
            screen_height,
            400.0,
            60.0,
        );
        assert_eq!(bx, screen_width - inset - 400.0);
        assert_eq!(by, screen_height - inset - 60.0);
    }
}
