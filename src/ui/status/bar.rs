use std::f64::consts::PI;

use super::super::primitives::{
    BADGE_STACK_GAP, BadgeAlign, draw_badge_with_engine, draw_pill, draw_rounded_rect,
    measure_badge_with_engine,
};
use super::super::theme::{self, overlay};
use super::badges::{
    EDITING_BADGE_FONT_SIZE, EDITING_BADGE_HINT, EDITING_BADGE_LABEL, EDITING_BADGE_TINT,
    FROZEN_BADGE_FONT_SIZE, FROZEN_BADGE_LABEL, FROZEN_BADGE_TINT, PAN_BADGE_FONT_SIZE,
    PAN_BADGE_TINT, ZOOM_BADGE_FONT_SIZE, ZOOM_BADGE_TINT, pan_badge_label, zoom_badge_label,
};
use crate::config::{Action, StatusPosition, action_display_label};
use crate::input::{BoardBackground, DrawingState, InputState, TextInputMode, Tool};
use crate::label_format::{format_binding_labels, join_binding_labels};
use crate::ui::toolbar::bindings::action_for_tool;
use crate::ui_text::{UiTextEngine, UiTextExtents, UiTextStyle, with_legacy_engine};

mod content;
mod helpers;
mod measurement;
mod render;

pub use content::compute_status_hud_layout;
pub(crate) use content::compute_status_hud_layout_with_engine;
pub(crate) use render::render_status_bar_with_resources;
pub use render::{render_status_bar, render_status_bar_with_theme};

#[cfg(test)]
use content::{build_cluster_pieces, build_prefix_text};
#[cfg(test)]
use helpers::pill_origin;
#[cfg(test)]
use measurement::{StatusBarMeasurement, measure_status_bar};

// ============================================================================
// UI Layout Constants (not configurable)
// ============================================================================

/// Inset between the pill background and the screen edges
const STATUS_BAR_EDGE_INSET: f64 = overlay::SPACING_MD;
/// Corner radius of the pill background (shared with the zoom chip so the two
/// bottom-anchored status pills can never drift apart).
const STATUS_BAR_CORNER_RADIUS: f64 = overlay::STATUS_PILL_RADIUS;
/// Maximum fraction of the screen width the whole pill (background including
/// padding) may occupy
const STATUS_BAR_MAX_WIDTH_FRACTION: f64 = 0.8;
/// Minimum share of the width budget reserved for the prefix when the prefix
/// and the segment cluster compete for space; optional display segments are
/// shed when this floor binds
const MIN_PREFIX_BUDGET_FRACTION: f64 = 0.25;
/// Separator between status segments
const SEGMENT_SEPARATOR: &str = " · ";
/// Minimum pill height so every interactive segment hit target is at least
/// this tall (the pill pads out vertically as needed)
const MIN_INTERACTIVE_HEIGHT: f64 = 28.0;
/// Minimum width of an interactive segment hit target; narrower natural
/// rects (e.g. the color dot with small user font/dot sizes) are widened to
/// this, centered on the natural rect and clamped inside the pill
const MIN_INTERACTIVE_WIDTH: f64 = 28.0;
/// Board name length before any degradation
const BOARD_NAME_MAX_CHARS: usize = 20;
/// Board-name degradation rungs applied (in order) when the width budget
/// still binds after optional pieces shed: progressively tighter truncation,
/// then `None` for the compact index-only "Board i/N" form
const BOARD_NAME_DEGRADATION_RUNGS: [Option<usize>; 3] = [Some(12), Some(6), None];

// ============================================================================
// Layout types (cached on `InputState`, consumed by rendering, hit-testing,
// and damage geometry)
// ============================================================================

/// Interactive surface a status HUD segment activates on click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusHudSegmentKind {
    /// Board name/index chip: opens the board picker.
    Board,
    /// Page counter chip: opens the board picker (its page panel).
    Page,
    /// Color dot: opens the color picker popup.
    Color,
    /// Active tool chip: opens the radial menu at the pointer.
    Tool,
    /// Active tool size chip: opens the radial menu at the pointer.
    Size,
    /// Help hint chip: toggles the help overlay.
    Help,
    /// Hidden-toolbar hint chip (shown only while no toolbar surface is
    /// visible): restores the toolbar.
    Toolbar,
    /// Version chip: opens the About window. Informational and actionable at
    /// once, which is why it carries the version rather than a bare glyph.
    About,
}

/// One laid-out run of pill content on the shared single-line baseline.
#[derive(Debug, Clone)]
pub(crate) enum StatusHudRun {
    /// A text run whose left edge sits at absolute screen `x`. `accent`
    /// underlines the run so it reads as actionable rather than
    /// informational (the hidden-toolbar hint chip).
    Text { text: String, x: f64, accent: bool },
    /// The color dot; `x` is the left edge of its bounding square.
    Dot { x: f64 },
}

/// Clickable rect (absolute screen coordinates) mapped to an activation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StatusHudSegment {
    pub(crate) kind: StatusHudSegmentKind,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl StatusHudSegment {
    pub(crate) fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

/// A mode badge pill (FROZEN/ZOOM/PAN/EDITING) stacked on the HUD.
#[derive(Debug, Clone)]
pub(crate) struct StatusHudBadge {
    pub(crate) label: String,
    pub(crate) hint: Option<(&'static str, f64)>,
    pub(crate) font_size: f64,
    pub(crate) tint: [f64; 4],
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

/// Wrappable non-interactive info block (selection size, output label).
#[derive(Debug, Clone)]
pub(crate) struct StatusHudPrefix {
    pub(crate) text: String,
    pub(crate) x: f64,
    /// Wrap width the text was shaped with; rendering reuses it so the
    /// cached pango layout is shared between measurement and drawing.
    pub(crate) wrap_budget: f64,
    pub(crate) height: f64,
    pub(crate) y_bearing: f64,
}

/// Cached status HUD geometry for one frame: the segmented pill, its
/// interactive segment rects, and the stacked mode badges.
#[derive(Debug, Clone)]
pub struct StatusHudLayout {
    pub(crate) pill_x: f64,
    pub(crate) pill_y: f64,
    pub(crate) pill_width: f64,
    pub(crate) pill_height: f64,
    pub(crate) prefix: Option<StatusHudPrefix>,
    pub(crate) runs: Vec<StatusHudRun>,
    /// Absolute baseline y shared by all single-line text runs.
    pub(crate) line_baseline: f64,
    pub(crate) segments: Vec<StatusHudSegment>,
    pub(crate) badges: Vec<StatusHudBadge>,
    /// Union of pill + stacked badges (x, y, w, h) for damage tracking.
    pub(crate) bounds: (f64, f64, f64, f64),
    /// Screen size this layout was computed for.
    pub(crate) screen_width: u32,
    pub(crate) screen_height: u32,
}

impl StatusHudLayout {
    pub(crate) fn pill_contains(&self, x: f64, y: f64) -> bool {
        x >= self.pill_x
            && x <= self.pill_x + self.pill_width
            && y >= self.pill_y
            && y <= self.pill_y + self.pill_height
    }

    pub(crate) fn segment_at(&self, x: f64, y: f64) -> Option<StatusHudSegmentKind> {
        self.segments
            .iter()
            .find(|segment| segment.contains(x, y))
            .map(|segment| segment.kind)
    }
}

/// On-screen bounds (x, y, width, height) the status HUD occupies (pill plus
/// stacked mode badges), without rendering it. Used for damage tracking; the
/// bounds come from the same cached layout rendering consumes, so the two
/// always agree. Returns `None` when no HUD layout exists for this screen
/// size (bar hidden or UI suppressed).
pub fn status_hud_geometry(
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) -> Option<(f64, f64, f64, f64)> {
    let layout = input_state.status_hud_layout()?;
    (layout.screen_width == screen_width && layout.screen_height == screen_height)
        .then_some(layout.bounds)
}

// ============================================================================
// Layout computation
// ============================================================================
#[cfg(test)]
mod tests;
