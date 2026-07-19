//! Top-strip tree builder.
//!
//! The strip reads left to right as three detached pill islands. Island A
//! (tools): drag grip, pens (Select/Pen/Marker/Step/Eraser), shapes
//! (Line/Arrow/Shapes picker), annotations (Text/Note/Screenshot/Highlight),
//! and quick colors + the current color chip, with thin dividers between the
//! groups. Island B (history): Undo/Redo plus the overflow toggle whose menu
//! anchors the destructive Clear (red on hover) and any width-dropped items.
//! Island C (chrome): the quieter right-aligned pin and minimize buttons.
//! Blue is reserved for the active tool; disabled history buttons are dimmed
//! and not interactive.

use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::input::Tool;
use crate::ui::toolbar::{ToolbarSnapshot, model};

use super::tree::WidgetTree;

mod build;

const TOP_LABEL_FONT_SIZE: f64 = 14.0;
const MINI_LABEL_FONT_SIZE: f64 = 10.0; // FONT_SIZE_SMALL

/// Extra advance consumed by a group divider (the 1px line plus breathing
/// room on both sides, on top of the regular gap).
pub(crate) const TOP_DIVIDER_SPAN: f64 = 7.0;
/// Quick-color swatch size and gap.
pub(crate) const TOP_SWATCH_SIZE: f64 = 22.0;
pub(crate) const TOP_SWATCH_GAP: f64 = 4.0;
/// Current-color chip size (opens the full picker; never collapses).
pub(crate) const TOP_CHIP_SIZE: f64 = 28.0;
/// Maximum quick-color swatches when width allows.
#[cfg(test)]
pub(crate) const TOP_MAX_QUICK_COLORS: usize = TopStripPlan::MAX_QUICK_COLORS;
const TOP_COMPACT_BUTTON: f64 = 26.0;
const TOP_COMPACT_GAP: f64 = 1.0;
const TOP_COMPACT_CHROME: f64 = 18.0;
const TOP_COMPACT_MARGIN_RIGHT: f64 = 8.0;
/// Tightened island gap/padding for the last-resort compact presentation.
/// The pad shares its number with the GTK `.pill.compact` padding via
/// `theme::toolbar::COMPACT_ISLAND_PAD`.
const TOP_COMPACT_ISLAND_GAP: f64 = 4.0;
const TOP_COMPACT_ISLAND_PAD: f64 = crate::ui::theme::toolbar::COMPACT_ISLAND_PAD;

pub(crate) use model::TopStripPlan;

/// Degrade the strip until it fits the viewport: quick swatches shrink
/// 8→6→4→0 first, then droppable items move into the overflow menu.
pub fn plan_top_strip(snapshot: &ToolbarSnapshot) -> TopStripPlan {
    let mut plan = TopStripPlan::unconstrained();
    if snapshot.top_minimized || snapshot.top_micro_active() {
        return plan;
    }
    let Some(budget) = snapshot.top_viewport_max else {
        return plan;
    };
    let fits = |plan: &TopStripPlan| natural_width_planned(snapshot, plan) <= budget;
    if fits(&plan) {
        return plan;
    }
    for count in [6, 4, 0] {
        plan.swatch_count = count;
        if fits(&plan) {
            return plan;
        }
    }
    let visible_utilities = model::visible_top_utility_buttons(
        snapshot,
        snapshot.layout_mode == crate::config::ToolbarLayoutMode::Simple,
        snapshot.use_icons,
    );
    let visible_tools: Vec<_> = model::visible_top_tool_buttons(
        snapshot.layout_mode == crate::config::ToolbarLayoutMode::Simple,
        snapshot,
    )
    .collect();
    let utility_candidates = [
        model::TopUtilityButton::Screenshot,
        model::TopUtilityButton::Highlight,
        model::TopUtilityButton::StickyNote,
        model::TopUtilityButton::Text,
    ];
    for candidate in utility_candidates {
        if fits(&plan) {
            sort_dropped_items(&mut plan, &visible_tools, &visible_utilities);
            return plan;
        }
        if visible_utilities.contains(&candidate) {
            plan.dropped_utilities.push(candidate);
        }
    }
    for candidate in [Tool::Arrow, Tool::Line] {
        if fits(&plan) {
            sort_dropped_items(&mut plan, &visible_tools, &visible_utilities);
            return plan;
        }
        if visible_tools.contains(&candidate) {
            plan.dropped_tools.push(candidate);
        }
    }
    if fits(&plan) {
        sort_dropped_items(&mut plan, &visible_tools, &visible_utilities);
        return plan;
    }

    // Last-resort compact presentation keeps the protected core available
    // while switching text buttons to icons and tightening spacing.
    plan.compact = true;
    if fits(&plan) {
        sort_dropped_items(&mut plan, &visible_tools, &visible_utilities);
        return plan;
    }
    for candidate in [Tool::StepMarker, Tool::Marker, Tool::Select] {
        if fits(&plan) {
            break;
        }
        if visible_tools.contains(&candidate) {
            plan.dropped_tools.push(candidate);
        }
    }

    // The overflow preserves each configured group's visual order even
    // though candidates degrade by priority.
    sort_dropped_items(&mut plan, &visible_tools, &visible_utilities);
    plan
}

fn sort_dropped_items(
    plan: &mut TopStripPlan,
    visible_tools: &[Tool],
    visible_utilities: &[model::TopUtilityButton],
) {
    plan.dropped_tools.sort_by_key(|tool| {
        visible_tools
            .iter()
            .position(|candidate| candidate == tool)
            .unwrap_or(usize::MAX)
    });
    plan.dropped_utilities.sort_by_key(|utility| {
        visible_utilities
            .iter()
            .position(|candidate| candidate == utility)
            .unwrap_or(usize::MAX)
    });
}

fn planned_use_icons(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> bool {
    snapshot.use_icons || plan.compact
}

fn planned_gap(plan: &TopStripPlan) -> f64 {
    if plan.compact {
        TOP_COMPACT_GAP
    } else {
        ToolbarLayoutSpec::TOP_GAP
    }
}

/// `(island_gap, island_pad)` for the plan: the clear space between pill
/// islands and the inner padding between a pill edge and its content.
fn planned_island_metrics(plan: &TopStripPlan) -> (f64, f64) {
    if plan.compact {
        (TOP_COMPACT_ISLAND_GAP, TOP_COMPACT_ISLAND_PAD)
    } else {
        (
            ToolbarLayoutSpec::TOP_ISLAND_GAP,
            ToolbarLayoutSpec::TOP_ISLAND_PAD,
        )
    }
}

fn planned_button_size(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> (f64, f64) {
    if plan.compact {
        (TOP_COMPACT_BUTTON, TOP_COMPACT_BUTTON)
    } else {
        ToolbarLayoutSpec::new(snapshot).top_button_size()
    }
}

fn base_bar_height(snapshot: &ToolbarSnapshot) -> f64 {
    if snapshot.use_icons {
        ToolbarLayoutSpec::TOP_SIZE_ICONS.1 as f64
    } else {
        ToolbarLayoutSpec::TOP_SIZE_TEXT.1 as f64
    }
}

fn bar_band_height(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> f64 {
    base_bar_height(snapshot) + build::ring_row_height_planned(snapshot, plan)
}

/// Build the complete top-strip tree for the given logical surface size.
pub fn build_top_view(snapshot: &ToolbarSnapshot, width: f64, height: f64) -> WidgetTree {
    let plan = plan_top_strip(snapshot);
    build::build_top_view_planned(snapshot, &plan, width, height)
}

/// Input rects for the top surface in tree-logical coordinates, or None
/// when the whole surface should accept input (the minimized restore tab
/// and the micro chip). The full strip always restricts input to the
/// island pills — plus the popover panels while one is open — so the
/// transparent inter-island gaps consistently stay click-through to the
/// canvas whether or not a popover is up.
pub fn top_input_rects(
    snapshot: &ToolbarSnapshot,
    width: f64,
    height: f64,
) -> Option<Vec<(f64, f64, f64, f64)>> {
    if snapshot.top_minimized || snapshot.top_micro_active() {
        return None;
    }
    let plan = plan_top_strip(snapshot);
    let bar_h = bar_band_height(snapshot, &plan);
    let tree = build_top_view(snapshot, width, height);
    let mut rects: Vec<_> = tree
        .nodes()
        .iter()
        .filter(|node| node.id.as_str().starts_with("top.island."))
        .map(|node| node.rect)
        .collect();
    if rects.is_empty() {
        rects.push((0.0, 0.0, width, bar_h));
    }
    for id in ["top.shapes.panel", "top.overflow.panel"] {
        if let Some(node) = tree.node_by_id(&id.to_string().into()) {
            let (x, y, w, h) = node.rect;
            // Cover the caret and the anchor gap above the panel.
            rects.push((x, (y - 8.0).max(0.0), w, h + 10.0));
        }
    }
    Some(rects)
}

/// Everything that grows the surface below the base bar: the shapes/options
/// popover, the contextual highlight-ring row, and the overflow popover.
pub fn top_extra_height(snapshot: &ToolbarSnapshot) -> f64 {
    if snapshot.top_minimized || snapshot.top_micro_active() {
        return 0.0;
    }
    build::shape_popover_height(snapshot)
        + build::ring_row_height(snapshot)
        + build::overflow_height(snapshot)
}

/// Natural width of the strip: the left-to-right content walk plus the
/// right-aligned chrome block. Computed from a build against a sentinel
/// width so the size math and the builder can never drift apart.
pub fn top_natural_width(snapshot: &ToolbarSnapshot, height: f64) -> f64 {
    let plan = plan_top_strip(snapshot);
    natural_width_planned_at(snapshot, &plan, height)
}

fn natural_width_planned(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> f64 {
    let base_height = base_bar_height(snapshot);
    natural_width_planned_at(snapshot, plan, base_height)
}

fn natural_width_planned_at(snapshot: &ToolbarSnapshot, plan: &TopStripPlan, height: f64) -> f64 {
    let tree = build::build_top_view_planned(snapshot, plan, 0.0, height);
    // The tools/history island cards already include their trailing padding,
    // so the max right edge of the left-hand content is the pill edge. The
    // right-anchored chrome (island card and buttons) is excluded because it
    // is positioned from the sentinel width.
    let left_end = tree
        .nodes()
        .iter()
        .filter(|node| {
            let id = node.id.as_str();
            id != "top.panel"
                && id != "top.island.chrome"
                && !id.starts_with("top.chrome.pin")
                && !id.starts_with("top.chrome.close")
                && !id.starts_with("top.overflow.")
        })
        .map(|node| node.rect.0 + node.rect.2)
        .fold(0.0_f64, f64::max);

    let chrome_count = model::TopToolbarSpec::chrome_control_count(snapshot, plan);
    if chrome_count == 0 {
        return left_end;
    }
    let (island_gap, island_pad) = planned_island_metrics(plan);
    let chrome_size = if plan.compact {
        TOP_COMPACT_CHROME
    } else {
        ToolbarLayoutSpec::TOP_PIN_BUTTON_SIZE
    };
    let chrome_gap = if plan.compact {
        TOP_COMPACT_GAP
    } else {
        ToolbarLayoutSpec::TOP_PIN_BUTTON_GAP
    };
    let chrome = chrome_size * chrome_count as f64
        + chrome_gap * chrome_count.saturating_sub(1) as f64
        + if plan.compact {
            TOP_COMPACT_MARGIN_RIGHT
        } else {
            ToolbarLayoutSpec::TOP_PIN_BUTTON_MARGIN_RIGHT
        };
    // Gap to the chrome pill, its leading padding, then the chrome block
    // (which carries its own trailing margin inside the pill).
    left_end + island_gap + island_pad + chrome
}

#[cfg(test)]
mod tests;
