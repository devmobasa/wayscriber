//! Radial menu renderer: a cached static "wedge base" (rest-state rings)
//! with dynamic overlays (hover, sub-ring, size arc, center well) on top.

mod cache;

use std::f64::consts::PI;

use crate::config::{Action, action_meta, action_short_label};
use crate::input::state::{
    RADIAL_COMPASS_SLICES, RADIAL_TOOL_SEGMENT_COUNT, RadialMenuLayout, RadialMenuState,
    RadialParent, RadialRingSwatch, RadialSegmentId, RadialSlice, RadialSliceKind,
    SIZE_RING_ARC_SPAN, SIZE_RING_ARC_START, TextInputMode, size_ring_angle_for_value,
    sub_ring_children,
};
use crate::input::{DrawingState, InputState, Tool};
use crate::toolbar_icons::{
    MicroChipStyle, ToolbarIconPainter, draw_icon_note, draw_icon_shape_picker, draw_micro_chip,
    top_toolbar_icon_painter,
};
use crate::ui::primitives::{draw_keycap, keycap_size};
use crate::ui::theme::{self, Rgba, overlay, toolbar};
use crate::ui_text::{UiTextStyle, text_layout};

// ── File-local style values without a matching token in ui/theme.rs ──

/// Hover brighten overlay on color ring swatches.
const COLOR_HOVER_OVERLAY: Rgba = (1.0, 1.0, 1.0, 0.35);
/// Border of the active color swatch.
const COLOR_ACTIVE_BORDER: Rgba = (1.0, 1.0, 1.0, 0.9);
/// Border of inactive color swatches.
const COLOR_SWATCH_BORDER: Rgba = (0.0, 0.0, 0.0, 0.4);

/// Separation between adjacent wedges, in pixels of arc length.
const WEDGE_GAP_PX: f64 = 2.0;
/// Tool ring wedge label font size.
const TOOL_LABEL_SIZE: f64 = 12.0;
/// Sub-ring wedge label font size.
const SUB_LABEL_SIZE: f64 = 11.0;
/// Vertical lift of the wedge label when it has no glyph but a keycap hint
/// is shown below it.
const HINT_LABEL_LIFT: f64 = 6.0;
/// Drop of the keycap hint's top edge below the wedge midpoint when the
/// wedge has no glyph.
const HINT_LABEL_DROP: f64 = 8.0;

/// Render the radial menu overlay.
pub fn render_radial_menu(ctx: &cairo::Context, input_state: &InputState, width: u32, height: u32) {
    let (hover, expanded_sub_ring, size_dragging) = match &input_state.radial_menu_state {
        RadialMenuState::Open {
            hover,
            expanded_sub_ring,
            size_dragging,
            ..
        } => (*hover, *expanded_sub_ring, *size_dragging),
        RadialMenuState::Hidden => return,
    };

    let layout = match &input_state.radial_menu_layout {
        Some(l) => *l,
        None => return,
    };

    let theme = theme::current();
    let swatches = input_state.radial_ring_swatches();
    let _ = ctx.save();

    // Semi-transparent backdrop
    ctx.set_source_rgba(0.0, 0.0, 0.0, overlay::OVERLAY_DIM_LIGHT);
    ctx.rectangle(0.0, 0.0, width as f64, height as f64);
    let _ = ctx.fill();

    let cx = layout.center_x;
    let cy = layout.center_y;

    // ── Static wedge base (cached): rest-state color ring, compass ring,
    // size-ring track ──
    cache::paint_base(ctx, input_state, &layout, theme, &swatches);

    // ── Dynamic overlays ──
    match hover {
        Some(RadialSegmentId::Tool(idx)) => {
            draw_compass_hover(ctx, input_state, theme, cx, cy, &layout, idx);
        }
        Some(RadialSegmentId::Color(idx)) => {
            draw_color_hover(ctx, cx, cy, &layout, &swatches, idx);
        }
        _ => {}
    }

    if let Some(parent_idx) = expanded_sub_ring {
        draw_sub_ring(ctx, input_state, theme, cx, cy, &layout, parent_idx, hover);
    }

    draw_size_value(
        ctx,
        input_state,
        theme,
        cx,
        cy,
        &layout,
        hover == Some(RadialSegmentId::SizeRing) || size_dragging,
    );

    draw_center_well(ctx, input_state, cx, cy, &layout, hover);

    let _ = ctx.restore();
}

// ── Static base (shared by the cache renderer and the uncached fallback) ──

/// Draw the rest-state rings around (cx, cy): every color swatch, every
/// compass wedge (active state included — it is part of the cache key), and
/// the size-ring track. Hover, sub-ring, size value arc, and the center well
/// are dynamic and drawn on top by the caller.
fn draw_static_base(
    ctx: &cairo::Context,
    input_state: &InputState,
    theme: &theme::Theme,
    cx: f64,
    cy: f64,
    layout: &RadialMenuLayout,
    swatches: &[RadialRingSwatch],
) {
    let active_tool = input_state.active_tool();
    let active_color = input_state.color_for_tool(active_tool);

    // Color ring
    let (color_seg_angle, color_offset, color_gap) = color_ring_geometry(layout, swatches.len());
    for (i, swatch) in swatches.iter().enumerate() {
        let start = color_offset + i as f64 * color_seg_angle;
        let end = start + color_seg_angle;
        let c = swatch.color;
        let is_active = colors_match(&active_color, &c);

        color_swatch_path(
            ctx,
            cx,
            cy,
            layout,
            swatch.recent,
            start + color_gap,
            end - color_gap,
        );
        ctx.set_source_rgba(c.r, c.g, c.b, c.a);
        let _ = ctx.fill_preserve();

        if is_active {
            theme::set_color(ctx, COLOR_ACTIVE_BORDER);
            ctx.set_line_width(2.5);
        } else {
            theme::set_color(ctx, COLOR_SWATCH_BORDER);
            ctx.set_line_width(1.0);
        }
        let _ = ctx.stroke();
    }

    // Primary compass ring
    let (seg_angle, offset, tool_gap) = compass_geometry(layout);
    for (i, slice) in RADIAL_COMPASS_SLICES.iter().enumerate() {
        let start = offset + i as f64 * seg_angle;
        let end = start + seg_angle;
        let is_active = slice_matches_active(slice, active_tool, input_state);

        draw_annular_sector(
            ctx,
            cx,
            cy,
            layout.tool_inner,
            layout.tool_outer,
            start + tool_gap,
            end - tool_gap,
        );
        fill_wedge(ctx, theme, theme.surface_pill, false, is_active);
        stroke_wedge_border(ctx, theme, is_active);

        // Content: glyph + short label + live keycap hint, all resolved
        // through the ActionMeta registry.
        let (lx, ly) = compass_wedge_midpoint(layout, cx, cy, start, seg_angle);
        let color = wedge_content_color(theme, false, is_active);
        let hint =
            slice_action(slice).and_then(|action| input_state.action_binding_primary_label(action));
        draw_wedge_content(
            ctx,
            lx,
            ly,
            slice_label(slice),
            slice_icon(slice),
            hint.as_deref(),
            color,
            TOOL_LABEL_SIZE,
            true,
        );
    }

    // Size-ring track
    let (track_r, track_w) = size_ring_track_geometry(layout);
    let _ = ctx.save();
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.new_path();
    theme::set_color(ctx, theme.surface_pill);
    ctx.set_line_width(track_w);
    ctx.arc(
        cx,
        cy,
        track_r,
        SIZE_RING_ARC_START,
        SIZE_RING_ARC_START + SIZE_RING_ARC_SPAN,
    );
    let _ = ctx.stroke();
    let _ = ctx.restore();
}

// ── Dynamic overlays ──

/// Hover overlay for a compass wedge: the state-ladder wash over the base
/// wedge, then its glyph + label repainted in the primary content color (the
/// keycap hint is color-independent, so the base copy stays).
fn draw_compass_hover(
    ctx: &cairo::Context,
    input_state: &InputState,
    theme: &theme::Theme,
    cx: f64,
    cy: f64,
    layout: &RadialMenuLayout,
    idx: u8,
) {
    let Some(slice) = RADIAL_COMPASS_SLICES.get(idx as usize) else {
        return;
    };
    let (seg_angle, offset, tool_gap) = compass_geometry(layout);
    let start = offset + idx as f64 * seg_angle;
    let end = start + seg_angle;
    draw_annular_sector(
        ctx,
        cx,
        cy,
        layout.tool_inner,
        layout.tool_outer,
        start + tool_gap,
        end - tool_gap,
    );
    theme::set_color(ctx, overlay::BG_HOVER_WASH);
    let _ = ctx.fill();

    let (lx, ly) = compass_wedge_midpoint(layout, cx, cy, start, seg_angle);
    let hint =
        slice_action(slice).and_then(|action| input_state.action_binding_primary_label(action));
    draw_wedge_content(
        ctx,
        lx,
        ly,
        slice_label(slice),
        slice_icon(slice),
        hint.as_deref(),
        theme.text_primary,
        TOOL_LABEL_SIZE,
        false,
    );
}

/// Hover overlay for a color swatch: brighten wash over the base swatch.
fn draw_color_hover(
    ctx: &cairo::Context,
    cx: f64,
    cy: f64,
    layout: &RadialMenuLayout,
    swatches: &[RadialRingSwatch],
    idx: u8,
) {
    let Some(swatch) = swatches.get(idx as usize) else {
        return;
    };
    let (seg_angle, offset, gap) = color_ring_geometry(layout, swatches.len());
    let start = offset + idx as f64 * seg_angle;
    let end = start + seg_angle;
    color_swatch_path(ctx, cx, cy, layout, swatch.recent, start + gap, end - gap);
    theme::set_color(ctx, COLOR_HOVER_OVERLAY);
    let _ = ctx.fill();
}

/// Expanded sub-ring (hover-dependent, drawn fully dynamically).
#[allow(clippy::too_many_arguments)]
fn draw_sub_ring(
    ctx: &cairo::Context,
    input_state: &InputState,
    theme: &theme::Theme,
    cx: f64,
    cy: f64,
    layout: &RadialMenuLayout,
    parent_idx: u8,
    hover: Option<RadialSegmentId>,
) {
    let children = sub_ring_children(parent_idx);
    if children.is_empty() {
        return;
    }
    let active_tool = input_state.active_tool();
    let (seg_angle, offset, _) = compass_geometry(layout);
    let parent_start = offset + parent_idx as f64 * seg_angle;
    let child_angle = seg_angle / children.len() as f64;
    let sub_gap = gap_half_angle((layout.sub_inner + layout.sub_outer) / 2.0);
    let mid_r = (layout.sub_inner + layout.sub_outer) / 2.0;
    // Narrow sub wedges render glyph-only; wide ones get the full
    // glyph + label + keycap stack.
    let show_labels = child_angle * mid_r >= overlay::RADIAL_SUB_LABEL_MIN_ARC;

    for (ci, action) in children.iter().enumerate() {
        let start = parent_start + ci as f64 * child_angle;
        let end = start + child_angle;
        let is_hovered = hover == Some(RadialSegmentId::SubTool(parent_idx, ci as u8));
        let is_active = action_is_active(*action, active_tool, input_state);

        draw_annular_sector(
            ctx,
            cx,
            cy,
            layout.sub_inner,
            layout.sub_outer,
            start + sub_gap,
            end - sub_gap,
        );
        fill_wedge(ctx, theme, theme.surface_popover, is_hovered, is_active);
        stroke_wedge_border(ctx, theme, is_active);

        // Content
        let mid_angle = start + child_angle / 2.0;
        let lx = cx + mid_r * mid_angle.cos();
        let ly = cy + mid_r * mid_angle.sin();
        let color = wedge_content_color(theme, is_hovered, is_active);
        let icon = action_meta(*action).and_then(|meta| meta.icon);
        let label = action_short_label(*action);
        match icon {
            Some(_) if show_labels => {
                let hint = input_state.action_binding_primary_label(*action);
                draw_wedge_content(
                    ctx,
                    lx,
                    ly,
                    label,
                    icon,
                    hint.as_deref(),
                    color,
                    SUB_LABEL_SIZE,
                    true,
                );
            }
            Some(paint) => {
                let size = overlay::RADIAL_WEDGE_ICON_SIZE;
                theme::set_color(ctx, color);
                paint(ctx, lx - size / 2.0, ly - size / 2.0, size);
            }
            None => draw_centered_label(ctx, lx, ly, label, SUB_LABEL_SIZE, color),
        }
    }
}

/// Accent value arc (and knob) of the size ring, up to the current thickness.
/// When emphasized (hover or drag), the whole track gets the state-ladder
/// wash first.
fn draw_size_value(
    ctx: &cairo::Context,
    input_state: &InputState,
    theme: &theme::Theme,
    cx: f64,
    cy: f64,
    layout: &RadialMenuLayout,
    emphasized: bool,
) {
    let (track_r, track_w) = size_ring_track_geometry(layout);
    let value_angle = size_ring_angle_for_value(input_state.size_for_active_tool());
    let _ = ctx.save();
    ctx.set_line_cap(cairo::LineCap::Round);

    if emphasized {
        ctx.new_path();
        theme::set_color(ctx, overlay::BG_HOVER_WASH);
        ctx.set_line_width(track_w);
        ctx.arc(
            cx,
            cy,
            track_r,
            SIZE_RING_ARC_START,
            SIZE_RING_ARC_START + SIZE_RING_ARC_SPAN,
        );
        let _ = ctx.stroke();
    }

    if value_angle > SIZE_RING_ARC_START + 1e-6 {
        ctx.new_path();
        theme::set_color(ctx, theme.accent);
        ctx.set_line_width(track_w);
        ctx.arc(cx, cy, track_r, SIZE_RING_ARC_START, value_angle);
        let _ = ctx.stroke();
    }

    // Value knob
    let knob_r = track_w / 2.0 + if emphasized { 1.5 } else { 0.5 };
    ctx.new_path();
    theme::set_color(ctx, theme.accent_bright);
    ctx.arc(
        cx + track_r * value_angle.cos(),
        cy + track_r * value_angle.sin(),
        knob_r,
        0.0,
        2.0 * PI,
    );
    let _ = ctx.fill();
    let _ = ctx.restore();
}

/// Center well: HUD micro-chip echo + thickness numeral keycap.
fn draw_center_well(
    ctx: &cairo::Context,
    input_state: &InputState,
    cx: f64,
    cy: f64,
    layout: &RadialMenuLayout,
    hover: Option<RadialSegmentId>,
) {
    let active_tool = input_state.active_tool();
    let active_color = input_state.color_for_tool(active_tool);
    let is_center_hovered = hover == Some(RadialSegmentId::Center);
    let size = input_state.size_for_active_tool();
    draw_micro_chip(
        ctx,
        cx - layout.center_radius,
        cy - layout.center_radius,
        layout.center_radius * 2.0,
        center_glyph(input_state, active_tool),
        &MicroChipStyle {
            ring_color: (
                active_color.r,
                active_color.g,
                active_color.b,
                active_color.a,
            ),
            ring_width: crate::ui::toolbar::model::micro_ring_width(size),
            icon_color: if is_center_hovered {
                toolbar::COLOR_ICON_HOVER
            } else {
                toolbar::COLOR_ICON_DEFAULT
            },
            hovered: is_center_hovered,
        },
    );
    let numeral = format!("{size:.0}px");
    let (numeral_w, numeral_h) = keycap_size(ctx, &numeral, toolbar::FONT_SIZE_SWATCH_KEY);
    draw_keycap(
        ctx,
        cx - numeral_w / 2.0,
        cy + overlay::RADIAL_CENTER_NUMERAL_DROP - numeral_h / 2.0,
        &numeral,
        toolbar::FONT_SIZE_SWATCH_KEY,
        toolbar::COLOR_BADGE_BACKGROUND,
        toolbar::COLOR_BADGE_TEXT,
    );
}

// ── Ring geometry ──

/// Compass-ring geometry: (segment angle, angle of segment 0's leading edge,
/// per-edge gap inset). Segment 0 (compass N) is centered on straight-up.
fn compass_geometry(layout: &RadialMenuLayout) -> (f64, f64, f64) {
    let seg_angle = 2.0 * PI / RADIAL_TOOL_SEGMENT_COUNT as f64;
    let offset = -PI / 2.0 - seg_angle / 2.0;
    let gap = gap_half_angle((layout.tool_inner + layout.tool_outer) / 2.0);
    (seg_angle, offset, gap)
}

/// Color-ring geometry for a swatch count: (segment angle, angle of segment
/// 0's leading edge, per-edge gap inset).
fn color_ring_geometry(layout: &RadialMenuLayout, count: usize) -> (f64, f64, f64) {
    if count == 0 {
        return (0.0, 0.0, 0.0);
    }
    let seg_angle = 2.0 * PI / count as f64;
    let offset = -PI / 2.0 - seg_angle / 2.0;
    let gap = if count > 1 {
        gap_half_angle((layout.color_inner + layout.color_outer) / 2.0)
    } else {
        0.0
    };
    (seg_angle, offset, gap)
}

/// Path of one color swatch. Recent swatches are radially inset at the outer
/// edge so the recents arc reads as a separate, thinner band.
fn color_swatch_path(
    ctx: &cairo::Context,
    cx: f64,
    cy: f64,
    layout: &RadialMenuLayout,
    recent: bool,
    start: f64,
    end: f64,
) {
    let outer = if recent {
        layout.color_outer - overlay::RADIAL_RECENT_OUTER_INSET
    } else {
        layout.color_outer
    };
    draw_annular_sector(ctx, cx, cy, layout.color_inner, outer, start, end);
}

/// Midpoint of a compass wedge's content stack.
fn compass_wedge_midpoint(
    layout: &RadialMenuLayout,
    cx: f64,
    cy: f64,
    start: f64,
    seg_angle: f64,
) -> (f64, f64) {
    let mid_angle = start + seg_angle / 2.0;
    let mid_r = (layout.tool_inner + layout.tool_outer) / 2.0;
    (cx + mid_r * mid_angle.cos(), cy + mid_r * mid_angle.sin())
}

/// Size-ring track stroke geometry: (arc radius, stroke width).
fn size_ring_track_geometry(layout: &RadialMenuLayout) -> (f64, f64) {
    let mid_r = (layout.size_inner + layout.size_outer) / 2.0;
    let width = (layout.size_outer - layout.size_inner) - 2.0 * overlay::RADIAL_SIZE_TRACK_INSET;
    (mid_r, width.max(1.0))
}

/// Angular inset that yields half of [`WEDGE_GAP_PX`] of arc length at the
/// given radius, so adjacent wedges are separated by a full gap. Render-only:
/// hit-testing in the input layer still assigns the full segment angle.
fn gap_half_angle(radius: f64) -> f64 {
    (WEDGE_GAP_PX / 2.0) / radius.max(1.0)
}

// ── Wedge painting primitives ──

/// Fill the current wedge path following the state ladder: rest surface,
/// hover white 8% wash, selected accent.
fn fill_wedge(
    ctx: &cairo::Context,
    theme: &theme::Theme,
    rest: Rgba,
    is_hovered: bool,
    is_active: bool,
) {
    if is_active {
        theme::set_color(ctx, theme.accent);
    } else {
        theme::set_color(ctx, rest);
    }
    let _ = ctx.fill_preserve();
    if is_hovered {
        theme::set_color(ctx, overlay::BG_HOVER_WASH);
        let _ = ctx.fill_preserve();
    }
}

/// Stroke the current wedge path border: hairline at rest, bright accent on
/// the selected wedge.
fn stroke_wedge_border(ctx: &cairo::Context, theme: &theme::Theme, is_active: bool) {
    if is_active {
        theme::set_color(ctx, theme.accent_bright);
        ctx.set_line_width(2.0);
    } else {
        theme::set_color(ctx, theme.border_hairline);
        ctx.set_line_width(1.0);
    }
    let _ = ctx.stroke();
}

/// Label/glyph color for a wedge's content.
fn wedge_content_color(theme: &theme::Theme, is_hovered: bool, is_active: bool) -> Rgba {
    if is_hovered || is_active {
        theme.text_primary
    } else {
        theme.text_secondary
    }
}

/// Draw a wedge's content stack centered on the wedge midpoint: glyph above
/// a short label with the primary bound shortcut as a keycap below, falling
/// back to label-only layouts when the glyph or hint is missing. With
/// `paint_hint` false the keycap is left to the layer below (hover repaints
/// only the color-dependent glyph/label); the hint still shapes the layout
/// so both layers agree on positions.
#[allow(clippy::too_many_arguments)]
fn draw_wedge_content(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    label: &str,
    icon: Option<ToolbarIconPainter>,
    hint: Option<&str>,
    color: Rgba,
    label_size: f64,
    paint_hint: bool,
) {
    match icon {
        Some(paint) => {
            let size = overlay::RADIAL_WEDGE_ICON_SIZE;
            theme::set_color(ctx, color);
            paint(
                ctx,
                x - size / 2.0,
                y - overlay::RADIAL_WEDGE_ICON_LIFT - size / 2.0,
                size,
            );
            draw_centered_label(
                ctx,
                x,
                y + overlay::RADIAL_WEDGE_LABEL_DROP,
                label,
                label_size,
                color,
            );
            if paint_hint && let Some(hint) = hint {
                draw_hint_keycap(ctx, x, y + overlay::RADIAL_WEDGE_HINT_DROP, hint);
            }
        }
        None => match hint {
            Some(hint) => {
                draw_centered_label(ctx, x, y - HINT_LABEL_LIFT, label, label_size, color);
                if paint_hint {
                    draw_hint_keycap(ctx, x, y + HINT_LABEL_DROP, hint);
                }
            }
            None => draw_centered_label(ctx, x, y, label, label_size, color),
        },
    }
}

/// Draw a keycap hint horizontally centered on `center_x` with its top edge
/// at `top_y`, in the shared keycap language.
fn draw_hint_keycap(ctx: &cairo::Context, center_x: f64, top_y: f64, label: &str) {
    let (width, _height) = keycap_size(ctx, label, toolbar::FONT_SIZE_SWATCH_KEY);
    draw_keycap(
        ctx,
        center_x - width / 2.0,
        top_y,
        label,
        toolbar::FONT_SIZE_SWATCH_KEY,
        toolbar::COLOR_BADGE_BACKGROUND,
        toolbar::COLOR_BADGE_TEXT,
    );
}

/// Draw an annular (ring) sector path.
fn draw_annular_sector(
    ctx: &cairo::Context,
    cx: f64,
    cy: f64,
    r_inner: f64,
    r_outer: f64,
    start_angle: f64,
    end_angle: f64,
) {
    ctx.new_path();
    ctx.arc(cx, cy, r_outer, start_angle, end_angle);
    ctx.arc_negative(cx, cy, r_inner, end_angle, start_angle);
    ctx.close_path();
}

/// Draw a centered text label at the given position.
fn draw_centered_label(ctx: &cairo::Context, x: f64, y: f64, text: &str, size: f64, color: Rgba) {
    let style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size,
    };
    let layout = text_layout(ctx, style, text, None);
    let extents = layout.ink_extents();
    let tx = x - extents.width() / 2.0 - extents.x_bearing();
    let ty = y - extents.height() / 2.0 - extents.y_bearing();
    theme::set_color(ctx, color);
    layout.show_at_baseline(ctx, tx, ty);
}

// ── Slice content resolution (ActionMeta registry) ──

/// Short label of a compass slice, resolved through the ActionMeta registry
/// for action slices.
fn slice_label(slice: &RadialSlice) -> &'static str {
    match slice.kind {
        RadialSliceKind::Action(action) => action_short_label(action),
        RadialSliceKind::Parent(parent) => parent.label(),
    }
}

/// Glyph of a compass slice: the registry icon for action slices, the shared
/// family glyphs for parents.
fn slice_icon(slice: &RadialSlice) -> Option<ToolbarIconPainter> {
    match slice.kind {
        RadialSliceKind::Action(action) => action_meta(action).and_then(|meta| meta.icon),
        RadialSliceKind::Parent(RadialParent::Shapes) => Some(draw_icon_shape_picker),
        RadialSliceKind::Parent(RadialParent::Notes) => Some(draw_icon_note),
    }
}

/// Action a slice dispatches (its live keycap hint source). Parent wedges
/// expand a sub-ring instead of dispatching, so they show no hint.
fn slice_action(slice: &RadialSlice) -> Option<Action> {
    match slice.kind {
        RadialSliceKind::Action(action) => Some(action),
        RadialSliceKind::Parent(_) => None,
    }
}

/// Whether a compass slice reads as selected: action slices match the state
/// their action would (re)enter; parents match when any child does.
fn slice_matches_active(slice: &RadialSlice, active_tool: Tool, input_state: &InputState) -> bool {
    match slice.kind {
        RadialSliceKind::Action(action) => action_is_active(action, active_tool, input_state),
        RadialSliceKind::Parent(parent) => parent
            .children()
            .iter()
            .any(|action| action_is_active(*action, active_tool, input_state)),
    }
}

/// Whether an action slice/child corresponds to the current active state.
fn action_is_active(action: Action, active_tool: Tool, input_state: &InputState) -> bool {
    match action {
        Action::EnterTextMode => {
            matches!(input_state.state, DrawingState::TextInput { .. })
                && matches!(input_state.text_input_mode, TextInputMode::Plain)
        }
        Action::EnterStickyNoteMode => {
            matches!(input_state.state, DrawingState::TextInput { .. })
                && matches!(input_state.text_input_mode, TextInputMode::StickyNote)
        }
        _ => Tool::from_select_action(action) == Some(active_tool),
    }
}

/// Glyph shown in the center well: the active text mode while typing,
/// otherwise the active tool's semantic toolbar glyph (the HUD echo).
fn center_glyph(input_state: &InputState, active_tool: Tool) -> ToolbarIconPainter {
    use crate::ui::toolbar::model::{TopToolbarIcon, semantic_icon_for_tool};

    if matches!(input_state.state, DrawingState::TextInput { .. }) {
        let icon = match input_state.text_input_mode {
            TextInputMode::StickyNote => TopToolbarIcon::StickyNote,
            _ => TopToolbarIcon::Text,
        };
        return top_toolbar_icon_painter(icon);
    }
    top_toolbar_icon_painter(TopToolbarIcon::Tool(semantic_icon_for_tool(active_tool)))
}

/// Check whether two colors are approximately equal.
fn colors_match(a: &crate::draw::Color, b: &crate::draw::Color) -> bool {
    (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
}
