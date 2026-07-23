use std::f64::consts::PI;

use super::super::primitives::{BADGE_STACK_GAP, BadgeAlign, draw_badge, draw_pill, measure_badge};
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
use crate::ui_text::{UiTextExtents, UiTextStyle, measure_text, text_layout};

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
    /// Tool + size chip: opens the radial menu at the pointer.
    Tool,
    /// Help hint chip: toggles the help overlay.
    Help,
    /// Hidden-toolbar hint chip (shown only while no toolbar surface is
    /// visible): restores the toolbar.
    Toolbar,
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

fn status_text_style(font_size: f64) -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: font_size,
    }
}

/// One pill content piece before positioning: a text chip or the color dot.
struct StatusHudPiece {
    /// `None` marks the color dot.
    text: Option<String>,
    kind: Option<StatusHudSegmentKind>,
    /// Optional display pieces are shed (last first) when the width budget
    /// binds.
    optional: bool,
    extents: Option<UiTextExtents>,
}

impl StatusHudPiece {
    fn text(text: String, kind: Option<StatusHudSegmentKind>, optional: bool) -> Self {
        Self {
            text: Some(text),
            kind,
            optional,
            extents: None,
        }
    }

    fn dot() -> Self {
        Self {
            text: None,
            kind: Some(StatusHudSegmentKind::Color),
            optional: false,
            extents: None,
        }
    }

    fn advance(&self, dot_diameter: f64) -> f64 {
        match &self.extents {
            Some(extents) => extents.x_advance(),
            None => dot_diameter,
        }
    }
}

/// Compute the status HUD layout headlessly (no rendering context; text goes
/// through the shared measurement cache, so rendering agrees exactly).
/// Callers gate on `show_status_bar`; this always lays out the visible HUD.
pub fn compute_status_hud_layout(
    input_state: &InputState,
    position: StatusPosition,
    style: &crate::config::StatusBarStyle,
    screen_width: u32,
    screen_height: u32,
) -> Option<StatusHudLayout> {
    let text_style = status_text_style(style.font_size);
    let dot_diameter = style.dot_radius * 2.0;
    let sep_extents = measure_text(text_style, SEGMENT_SEPARATOR, None)?;
    let sep_advance = sep_extents.x_advance();

    let mut pieces = build_cluster_pieces(input_state);
    for piece in &mut pieces {
        if let Some(text) = &piece.text {
            piece.extents = Some(measure_text(text_style, text, None)?);
        }
    }

    let prefix_text = build_prefix_text(input_state);

    // Degradation ladder while the width budget binds: shed optional display
    // pieces (last first), then truncate the board name progressively down
    // to the compact "Board i/N" form, then drop the help chip. The
    // unconditional backstop below clamps the pill to the budget regardless.
    let mut board_rungs = BOARD_NAME_DEGRADATION_RUNGS.iter().copied();
    let mut measurement = loop {
        let cluster_width = cluster_width(&pieces, sep_advance, dot_diameter);
        let line_metrics = cluster_line_metrics(&pieces, sep_extents);
        let measurement = measure_status_bar(
            style,
            prefix_text.as_deref().unwrap_or(""),
            cluster_width,
            line_metrics.height(),
            dot_diameter,
            screen_width,
        )?;
        if !measurement.overflow {
            break measurement;
        }
        if let Some(index) = pieces.iter().rposition(|piece| piece.optional) {
            pieces.remove(index);
            continue;
        }
        if let Some(limit) = board_rungs.next() {
            if let Some(piece) = pieces
                .iter_mut()
                .find(|piece| piece.kind == Some(StatusHudSegmentKind::Board))
            {
                let label = board_segment_label(input_state, limit);
                if piece.text.as_deref() != Some(label.as_str()) {
                    piece.extents = Some(measure_text(text_style, &label, None)?);
                    piece.text = Some(label);
                }
            }
            continue;
        }
        if let Some(index) = pieces
            .iter()
            .position(|piece| piece.kind == Some(StatusHudSegmentKind::Help))
        {
            pieces.remove(index);
            continue;
        }
        break measurement;
    };
    // Unconditional backstop: even a mandatory cluster that still overflows
    // never widens the pill past the budget. Rendering clips content to the
    // pill and hit rects are clamped inside it below.
    let max_pill_width = screen_width as f64 * STATUS_BAR_MAX_WIDTH_FRACTION;
    measurement.pill_width = measurement.pill_width.min(max_pill_width);
    let line_metrics = cluster_line_metrics(&pieces, sep_extents);

    let (pill_x, pill_y) = pill_origin(
        position,
        screen_width as f64,
        screen_height as f64,
        measurement.pill_width,
        measurement.pill_height,
    );
    let pill_height = measurement.pill_height;
    let line_baseline = pill_y + (pill_height - line_metrics.height()) / 2.0 + line_metrics.ascent;

    // Position runs and interactive segment rects.
    let mut runs = Vec::new();
    let mut segments = Vec::new();
    let mut cursor = pill_x + style.padding;

    let prefix = prefix_text.map(|text| {
        let prefix = StatusHudPrefix {
            text,
            x: cursor,
            wrap_budget: measurement.prefix_budget,
            height: measurement.prefix_height,
            y_bearing: measurement.prefix_bearing,
        };
        cursor += measurement.prefix_width;
        runs.push(StatusHudRun::Text {
            text: SEGMENT_SEPARATOR.to_string(),
            x: cursor,
            accent: false,
        });
        cursor += sep_advance;
        prefix
    });

    for (index, piece) in pieces.iter().enumerate() {
        if index > 0 {
            runs.push(StatusHudRun::Text {
                text: SEGMENT_SEPARATOR.to_string(),
                x: cursor,
                accent: false,
            });
            cursor += sep_advance;
        }
        let advance = piece.advance(dot_diameter);
        if let Some(kind) = piece.kind {
            // Hit target: the piece plus half a separator on each side, at
            // full pill height (>= MIN_INTERACTIVE_HEIGHT by construction),
            // clamped inside the (possibly budget-clamped) pill.
            let pill_right = pill_x + measurement.pill_width;
            let hit_x = (cursor - sep_advance * 0.5).clamp(pill_x, pill_right);
            let hit_right = (cursor + advance + sep_advance * 0.5).clamp(pill_x, pill_right);
            segments.push(StatusHudSegment {
                kind,
                x: hit_x,
                y: pill_y,
                width: (hit_right - hit_x).max(0.0),
                height: pill_height,
            });
        }
        match &piece.text {
            Some(text) => runs.push(StatusHudRun::Text {
                text: text.clone(),
                x: cursor,
                // The clickable-affordance underline must not advertise a
                // click that a display-only HUD
                // (`[ui] status_bar_interactive = false`) will reject; the
                // chip itself still shows the recovery binding there.
                accent: piece.kind == Some(StatusHudSegmentKind::Toolbar)
                    && input_state.status_bar_interactive,
            }),
            None => runs.push(StatusHudRun::Dot { x: cursor }),
        }
        cursor += advance;
    }

    widen_narrow_segments(&mut segments, pill_x, measurement.pill_width);

    let badges = layout_mode_badges(
        input_state,
        position,
        pill_x,
        pill_y,
        measurement.pill_width,
        pill_height,
    );

    let mut min_x = pill_x;
    let mut min_y = pill_y;
    let mut max_x = pill_x + measurement.pill_width;
    let mut max_y = pill_y + pill_height;
    for badge in &badges {
        min_x = min_x.min(badge.x);
        min_y = min_y.min(badge.y);
        max_x = max_x.max(badge.x + badge.width);
        max_y = max_y.max(badge.y + badge.height);
    }

    Some(StatusHudLayout {
        pill_x,
        pill_y,
        pill_width: measurement.pill_width,
        pill_height,
        prefix,
        runs,
        line_baseline,
        segments,
        badges,
        bounds: (min_x, min_y, max_x - min_x, max_y - min_y),
        screen_width,
        screen_height,
    })
}

/// Board segment text (per the plan mock, e.g. "Overlay 1/6"): a named board
/// shows "{truncated-name} {i}/{N}", an unnamed board the compact
/// "Board i/N". `max_name_chars: None` forces the compact form (the last
/// degradation rung).
fn board_segment_label(input_state: &InputState, max_name_chars: Option<usize>) -> String {
    let index = input_state.boards.active_index() + 1;
    let count = input_state.boards.board_count().max(1);
    let name = max_name_chars
        .map(|limit| crate::util::truncate_with_ellipsis(input_state.board_name(), limit))
        .unwrap_or_default();
    if name.trim().is_empty() {
        format!("Board {index}/{count}")
    } else {
        format!("{name} {index}/{count}")
    }
}

/// Build the single-line segment pieces in display order.
fn build_cluster_pieces(input_state: &InputState) -> Vec<StatusHudPiece> {
    let mut pieces = Vec::new();

    if input_state.show_status_board_badge && input_state.boards.show_badge() {
        pieces.push(StatusHudPiece::text(
            board_segment_label(input_state, Some(BOARD_NAME_MAX_CHARS)),
            Some(StatusHudSegmentKind::Board),
            false,
        ));
    }

    if input_state.show_status_page_badge {
        pieces.push(StatusHudPiece::text(
            format!(
                "Page {}/{}",
                input_state.boards.active_page_index() + 1,
                input_state.boards.page_count().max(1)
            ),
            Some(StatusHudSegmentKind::Page),
            false,
        ));
    }

    pieces.push(StatusHudPiece::dot());

    let tool = input_state.active_tool();
    pieces.push(StatusHudPiece::text(
        format!(
            "{} · {}px",
            tool_display_name(input_state, tool),
            input_state.size_for_active_tool() as i32
        ),
        Some(StatusHudSegmentKind::Tool),
        false,
    ));

    if matches!(
        input_state.state,
        DrawingState::TextInput { .. } | DrawingState::PendingTextClick { .. }
    ) {
        pieces.push(StatusHudPiece::text(
            format!("Text {}px", input_state.current_font_size as i32),
            None,
            true,
        ));
    }
    if input_state.click_highlight_enabled() {
        pieces.push(StatusHudPiece::text(
            action_display_label(Action::ToggleClickHighlight).to_string(),
            None,
            true,
        ));
    }
    if input_state.highlight_tool_active() {
        pieces.push(StatusHudPiece::text(
            action_display_label(Action::SelectHighlightTool).to_string(),
            None,
            true,
        ));
    }

    // Hidden-toolbar hint: when every toolbar surface is gone (F9 toggle or
    // F2 cycle-hidden), point at the way back so an accidental hide is
    // recoverable from the status bar alone. Clicking the chip restores the
    // toolbar directly. Opt-out via `[ui] show_toolbar_hint = false` for
    // deliberate toolbar-less setups; suppressed while presenter mode owns
    // toolbar visibility (the toggle is a no-op there); shed first when the
    // width budget binds.
    if input_state.show_toolbar_hint
        && !(input_state.toolbar_visible()
            || input_state.presenter_mode && input_state.presenter_mode_config.hide_toolbars)
    {
        pieces.push(StatusHudPiece::text(
            toolbar_hint_label(input_state),
            Some(StatusHudSegmentKind::Toolbar),
            true,
        ));
    }

    let binding = help_binding_label(input_state);
    let help_label = if binding.is_empty() {
        action_display_label(Action::ToggleHelp).to_string()
    } else {
        format!("{} {}", binding, action_display_label(Action::ToggleHelp))
    };
    pieces.push(StatusHudPiece::text(
        help_label,
        Some(StatusHudSegmentKind::Help),
        false,
    ));

    pieces
}

/// Wrappable non-interactive info before the segments (selection size,
/// output label), or `None` when nothing applies.
fn build_prefix_text(input_state: &InputState) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(bounds) = input_state.selection_bounds() {
        let count = input_state.selected_shape_ids().len();
        parts.push(if count == 1 {
            format!("{}×{}px", bounds.width, bounds.height)
        } else {
            format!("{} items: {}×{}px", count, bounds.width, bounds.height)
        });
    }
    if input_state.show_active_output_badge
        && let Some(label) = input_state.active_output_label.as_ref()
    {
        let label = crate::util::truncate_with_ellipsis(label, 28);
        parts.push(format!("Output: {label}"));
    }
    (!parts.is_empty()).then(|| parts.join(SEGMENT_SEPARATOR))
}

/// Widen interactive hit rects narrower than [`MIN_INTERACTIVE_WIDTH`] to
/// that floor (the color dot's natural target can drop below it with small
/// user font/dot sizes), centered on the natural rect and clamped inside the
/// pill. Neighboring segments cede the overlapped span — never dropping
/// below the floor themselves — so segment rects stay disjoint. Layout only;
/// rendering is unaffected.
fn widen_narrow_segments(segments: &mut [StatusHudSegment], pill_x: f64, pill_width: f64) {
    let pill_right = pill_x + pill_width;
    for index in 0..segments.len() {
        if segments[index].width >= MIN_INTERACTIVE_WIDTH {
            continue;
        }
        let center = segments[index].x + segments[index].width / 2.0;
        let half = MIN_INTERACTIVE_WIDTH / 2.0;
        let mut left = center - half;
        let mut right = center + half;
        // Slide the expanded rect back inside the pill.
        if left < pill_x {
            right += pill_x - left;
            left = pill_x;
        }
        if right > pill_right {
            left -= right - pill_right;
            right = pill_right;
        }
        left = left.max(pill_x);
        if index > 0 {
            let prev = &mut segments[index - 1];
            let prev_right = prev.x + prev.width;
            if prev_right > left {
                // The previous segment cedes down to its own floor; any
                // remainder pushes this rect's left edge back right.
                let boundary = left.max((prev.x + MIN_INTERACTIVE_WIDTH).min(prev_right));
                prev.width = (boundary - prev.x).max(0.0);
                left = boundary;
            }
        }
        if index + 1 < segments.len() {
            let next = &mut segments[index + 1];
            let next_right = next.x + next.width;
            if next.x < right {
                let boundary = right
                    .min((next_right - MIN_INTERACTIVE_WIDTH).max(next.x))
                    .max(left);
                next.width = (next_right - boundary).max(0.0);
                next.x = boundary;
                right = boundary;
            }
        }
        let segment = &mut segments[index];
        segment.x = left;
        segment.width = (right - left).max(0.0);
    }
}

fn cluster_width(pieces: &[StatusHudPiece], sep_advance: f64, dot_diameter: f64) -> f64 {
    let piece_widths: f64 = pieces.iter().map(|piece| piece.advance(dot_diameter)).sum();
    piece_widths + sep_advance * pieces.len().saturating_sub(1) as f64
}

/// Shared ascent/descent for the single-line cluster, so every text run can
/// sit on one baseline.
#[derive(Clone, Copy)]
struct ClusterLineMetrics {
    ascent: f64,
    descent: f64,
}

impl ClusterLineMetrics {
    fn height(self) -> f64 {
        self.ascent + self.descent
    }
}

fn cluster_line_metrics(
    pieces: &[StatusHudPiece],
    sep_extents: UiTextExtents,
) -> ClusterLineMetrics {
    let mut ascent = (-sep_extents.y_bearing()).max(0.0);
    let mut descent = (sep_extents.height() + sep_extents.y_bearing()).max(0.0);
    for extents in pieces.iter().filter_map(|piece| piece.extents.as_ref()) {
        ascent = ascent.max(-extents.y_bearing());
        descent = descent.max(extents.height() + extents.y_bearing());
    }
    ClusterLineMetrics { ascent, descent }
}

/// Measured pill geometry for the wrapped prefix + fixed-width segment
/// cluster.
struct StatusBarMeasurement {
    /// Wrap width offered to the prefix (0 when absent).
    prefix_budget: f64,
    prefix_width: f64,
    prefix_height: f64,
    prefix_bearing: f64,
    pill_width: f64,
    pill_height: f64,
    /// True when the width budget binds: the prefix floor is already
    /// consumed, so the cluster must shed optional pieces to fit.
    overflow: bool,
}

/// Shape the status bar so the whole pill (background including padding)
/// stays within `STATUS_BAR_MAX_WIDTH_FRACTION` of the screen width. The
/// segment cluster is fixed-width (single line); the prefix wraps within the
/// remaining budget, floored at `MIN_PREFIX_BUDGET_FRACTION` of the total
/// budget. When the floor binds, `overflow` asks the caller to shed optional
/// cluster pieces (the cluster cannot re-wrap the way the M0 suffix could).
fn measure_status_bar(
    style: &crate::config::StatusBarStyle,
    prefix_text: &str,
    cluster_width: f64,
    cluster_line_height: f64,
    dot_diameter: f64,
    screen_width: u32,
) -> Option<StatusBarMeasurement> {
    let max_width = screen_width as f64 * STATUS_BAR_MAX_WIDTH_FRACTION - style.padding * 2.0;
    let text_style = status_text_style(style.font_size);
    let sep_advance = measure_text(text_style, SEGMENT_SEPARATOR, None)?.x_advance();

    let has_prefix = !prefix_text.is_empty();
    let (prefix_budget, prefix_width, prefix_height, prefix_bearing, prefix_advance, overflow) =
        if has_prefix {
            let min_prefix_budget = (max_width * MIN_PREFIX_BUDGET_FRACTION).min(max_width);
            let available = max_width - cluster_width - sep_advance;
            let prefix_budget = available.max(min_prefix_budget).max(1.0);
            // The floor binds when the cluster leaves less room than the
            // prefix is guaranteed; the caller sheds optional pieces then.
            let overflow = prefix_budget > available;
            let extents = measure_text(text_style, prefix_text, Some(prefix_budget))?;
            let width = extents.width().min(prefix_budget);
            (
                prefix_budget,
                width,
                extents.height(),
                extents.y_bearing(),
                width + sep_advance,
                overflow,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0, 0.0, cluster_width > max_width)
        };

    let content_width = prefix_advance + cluster_width;
    let content_height = prefix_height.max(cluster_line_height).max(dot_diameter);
    let v_pad = style.padding * 0.5;
    Some(StatusBarMeasurement {
        prefix_budget,
        prefix_width,
        prefix_height,
        prefix_bearing,
        pill_width: content_width + style.padding * 2.0,
        pill_height: (content_height + v_pad * 2.0).max(MIN_INTERACTIVE_HEIGHT),
        overflow,
    })
}

/// Pre-layout description of one mode badge pill.
struct StatusHudBadgeSpec {
    label: String,
    hint: Option<(&'static str, f64)>,
    font_size: f64,
    tint: [f64; 4],
}

/// Mode badges (FROZEN/ZOOM/PAN/EDITING) stacked directly above the HUD, or
/// below it for top positions, aligned to the pill's near screen edge.
fn layout_mode_badges(
    input_state: &InputState,
    position: StatusPosition,
    pill_x: f64,
    pill_y: f64,
    pill_width: f64,
    pill_height: f64,
) -> Vec<StatusHudBadge> {
    // Labels, font sizes, and the EDITING hint are the shared specs from
    // `badges.rs`, so the stacked pills and the top-corner badges cannot
    // drift apart.
    let mut specs: Vec<StatusHudBadgeSpec> = Vec::new();
    if input_state.frozen_active() {
        // Literal red safety state; never abstracted behind the theme.
        specs.push(StatusHudBadgeSpec {
            label: FROZEN_BADGE_LABEL.to_string(),
            hint: None,
            font_size: FROZEN_BADGE_FONT_SIZE,
            tint: FROZEN_BADGE_TINT,
        });
    }
    // Reconciliation (M8): when the bottom-right zoom chip is effectively
    // visible it is the canonical zoom indicator/control, so the HUD-stacked
    // ZOOM badge is suppressed to avoid showing the zoom percentage twice.
    // With the chip absent (zoom actions off, or hidden at runtime via
    // ToggleZoomChip) the badge remains the HUD's zoom indicator, keeping
    // exactly one indicator in every state.
    if input_state.zoom_active() && !input_state.zoom_chip_enabled() {
        specs.push(StatusHudBadgeSpec {
            label: zoom_badge_label(input_state.zoom_scale(), input_state.zoom_locked()),
            hint: None,
            font_size: ZOOM_BADGE_FONT_SIZE,
            tint: ZOOM_BADGE_TINT,
        });
    }
    if input_state.boards.pan_enabled()
        && input_state.boards.show_pan_badge()
        && !input_state.board_is_transparent()
    {
        let panned = input_state.boards.active_frame().view_offset() != (0, 0);
        specs.push(StatusHudBadgeSpec {
            label: pan_badge_label(panned).to_string(),
            hint: None,
            font_size: PAN_BADGE_FONT_SIZE,
            tint: PAN_BADGE_TINT,
        });
    }
    if matches!(input_state.state, DrawingState::TextInput { .. })
        && input_state.text_edit_target.is_some()
    {
        specs.push(StatusHudBadgeSpec {
            label: EDITING_BADGE_LABEL.to_string(),
            hint: Some(EDITING_BADGE_HINT),
            font_size: EDITING_BADGE_FONT_SIZE,
            tint: EDITING_BADGE_TINT,
        });
    }

    let stack_down = matches!(position, StatusPosition::TopLeft | StatusPosition::TopRight);
    let align_left = matches!(
        position,
        StatusPosition::TopLeft | StatusPosition::BottomLeft
    );

    let mut badges = Vec::new();
    let mut offset = BADGE_STACK_GAP;
    for spec in specs {
        let Some((width, height)) = measure_badge(&spec.label, spec.font_size, spec.hint) else {
            continue;
        };
        let x = if align_left {
            pill_x
        } else {
            pill_x + pill_width - width
        };
        let y = if stack_down {
            pill_y + pill_height + offset
        } else {
            pill_y - offset - height
        };
        offset += height + BADGE_STACK_GAP;
        badges.push(StatusHudBadge {
            label: spec.label,
            hint: spec.hint,
            font_size: spec.font_size,
            tint: spec.tint,
            x,
            y,
            width,
            height,
        });
    }
    badges
}

// ============================================================================
// Rendering
// ============================================================================

/// Render the status HUD (segmented pill plus stacked mode badges) from the
/// layout cached on `InputState` by `update_status_hud_layout`.
pub fn render_status_bar(
    ctx: &cairo::Context,
    input_state: &InputState,
    style: &crate::config::StatusBarStyle,
    screen_width: u32,
    screen_height: u32,
) {
    let Some(layout) = input_state.status_hud_layout() else {
        return;
    };
    if layout.screen_width != screen_width || layout.screen_height != screen_height {
        return;
    }

    let (bg_color, text_color) = match input_state.boards.active_background() {
        BoardBackground::Transparent => (style.bg_color, style.text_color),
        BoardBackground::Solid(color) => {
            theme::Theme::status_palette_for_background(color.r, color.g, color.b)
        }
    };

    draw_pill(
        ctx,
        layout.pill_x,
        layout.pill_y,
        layout.pill_width,
        layout.pill_height,
        STATUS_BAR_CORNER_RADIUS,
        (bg_color[0], bg_color[1], bg_color[2], bg_color[3]),
        theme::current().border_hairline,
        None,
    );

    let text_style = status_text_style(style.font_size);
    let [r, g, b, a] = text_color;

    // Clip content to the pill: when the unconditional width backstop binds,
    // overflowing runs must never paint past the pill background.
    let _ = ctx.save();
    ctx.rectangle(
        layout.pill_x,
        layout.pill_y,
        layout.pill_width,
        layout.pill_height,
    );
    ctx.clip();

    if let Some(prefix) = &layout.prefix {
        // Center the (possibly wrapped) prefix block within the pill so a
        // second line never spills past the background.
        let pango = text_layout(ctx, text_style, &prefix.text, Some(prefix.wrap_budget));
        let baseline =
            layout.pill_y + (layout.pill_height - prefix.height) / 2.0 - prefix.y_bearing;
        ctx.set_source_rgba(r, g, b, a);
        pango.show_at_baseline(ctx, prefix.x, baseline);
    }

    let tool = input_state.active_tool();
    let dot_color = input_state.color_for_tool(tool);
    for run in &layout.runs {
        match run {
            StatusHudRun::Text { text, x, accent } => {
                ctx.set_source_rgba(r, g, b, a);
                text_layout(ctx, text_style, text, None).show_at_baseline(
                    ctx,
                    *x,
                    layout.line_baseline,
                );
                if *accent && let Some(extents) = measure_text(text_style, text, None) {
                    // Underline the actionable hint run so it reads as
                    // clickable against the informational runs. Follows the
                    // palette text color, so it holds up on any board
                    // background.
                    ctx.set_source_rgba(r, g, b, a * 0.55);
                    ctx.rectangle(*x, layout.line_baseline + 2.0, extents.x_advance(), 1.0);
                    let _ = ctx.fill();
                }
            }
            StatusHudRun::Dot { x } => {
                // Color dot: the sole indicator of the current draw color.
                ctx.set_source_rgba(dot_color.r, dot_color.g, dot_color.b, dot_color.a);
                ctx.arc(
                    x + style.dot_radius,
                    layout.pill_y + layout.pill_height / 2.0,
                    style.dot_radius,
                    0.0,
                    2.0 * PI,
                );
                let _ = ctx.fill();
            }
        }
    }

    let _ = ctx.restore();

    for badge in &layout.badges {
        draw_badge(
            ctx,
            badge.x,
            badge.y,
            BadgeAlign::Left,
            &badge.label,
            badge.font_size,
            badge.hint,
            badge.tint,
        );
    }
}

// ============================================================================
// Helpers
// ============================================================================

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

/// Hidden-toolbar hint chip text, styled like the help chip: "{binding}
/// Toolbar" (e.g. "F9 Toolbar"), or bare "Toolbar" when the toggle is
/// unbound (the chip stays clickable either way).
fn toolbar_hint_label(input_state: &InputState) -> String {
    let labels = input_state.action_binding_labels(Action::ToggleToolbar);
    match join_binding_labels(&labels) {
        Some(binding) => format!("{binding} Toolbar"),
        None => "Toolbar".to_string(),
    }
}

fn tool_action_label(tool: Tool) -> &'static str {
    action_for_tool(tool)
        .map(action_display_label)
        .unwrap_or("Select")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig, StatusBarStyle};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};

    /// Worst-case prefix: selection info plus a long output label on a
    /// narrow screen.
    const LONG_PREFIX: &str = "12 items: 1920×1080px · Output: DP-3 Dell UltraSharp U2723QE… · \
         And an implausibly long tail of extra selection detail text that must wrap";
    /// Realistic cluster width (board + page + dot + tool + hints + help).
    const CLUSTER_WIDTH: f64 = 400.0;
    const CLUSTER_LINE_HEIGHT: f64 = 21.0;
    const DOT_DIAMETER: f64 = 12.0;

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        )
    }

    fn measure(
        style: &StatusBarStyle,
        prefix: &str,
        cluster_width: f64,
        screen_width: u32,
    ) -> StatusBarMeasurement {
        measure_status_bar(
            style,
            prefix,
            cluster_width,
            CLUSTER_LINE_HEIGHT,
            DOT_DIAMETER,
            screen_width,
        )
        .expect("measurement")
    }

    const LONG_BOARD_NAME: &str = "An Implausibly Long Board Name That Keeps Going And Going";

    /// Worst-case HUD content: an implausibly long board name, the longest
    /// tool label ("Freeform Polygon"), and a long wrappable prefix.
    fn make_worst_case_state() -> InputState {
        let mut state = make_state();
        let active = state.boards.active_index();
        state.boards.board_states_mut()[active].spec.name = LONG_BOARD_NAME.to_string();
        state.state = DrawingState::BuildingPolygon {
            points: vec![(0, 0)],
            preview: None,
            fill: false,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 4.0,
        };
        state.show_active_output_badge = true;
        state.active_output_label = Some("DP-3 Dell UltraSharp U2723QE 3840x2160@60".to_string());
        state
    }

    /// The M0 unconditional cap: even worst-case mandatory content on small
    /// screens must never widen the pill past the width budget, at any
    /// position, and every hit rect stays clamped inside the pill.
    #[test]
    fn pill_width_never_exceeds_max_fraction_of_screen() {
        let state = make_worst_case_state();
        let style = StatusBarStyle::default();

        for (screen_width, screen_height) in [(640_u32, 480_u32), (800, 600), (1280, 720)] {
            for position in [
                StatusPosition::TopLeft,
                StatusPosition::TopRight,
                StatusPosition::BottomLeft,
                StatusPosition::BottomRight,
            ] {
                let layout = compute_status_hud_layout(
                    &state,
                    position,
                    &style,
                    screen_width,
                    screen_height,
                )
                .expect("layout");
                let max_pill_width = screen_width as f64 * STATUS_BAR_MAX_WIDTH_FRACTION;
                assert!(
                    layout.pill_width <= max_pill_width + 1e-6,
                    "pill width {} exceeds cap {} on {}x{} at {:?}",
                    layout.pill_width,
                    max_pill_width,
                    screen_width,
                    screen_height,
                    position
                );
                assert!(layout.pill_x >= 0.0, "pill off-screen at {:?}", position);
                assert!(
                    layout.pill_x + layout.pill_width <= screen_width as f64 + 1e-6,
                    "pill runs off a {}px screen at {:?}",
                    screen_width,
                    position
                );
                for segment in &layout.segments {
                    assert!(
                        segment.x >= layout.pill_x - 1e-6,
                        "{:?} left of pill",
                        segment.kind
                    );
                    assert!(
                        segment.x + segment.width <= layout.pill_x + layout.pill_width + 1e-6,
                        "{:?} hit rect outside the pill",
                        segment.kind
                    );
                }
            }
        }
    }

    /// The measurement stays within the budget for realistic content on
    /// common screens (the pre-ladder fast path).
    #[test]
    fn measured_pill_width_stays_within_budget_for_realistic_cluster() {
        let style = StatusBarStyle::default();

        for screen_width in [1280_u32, 1366, 1920] {
            let measurement = measure(&style, LONG_PREFIX, CLUSTER_WIDTH, screen_width);
            let max_pill_width = screen_width as f64 * STATUS_BAR_MAX_WIDTH_FRACTION;
            assert!(
                !measurement.overflow,
                "cluster {} should fit {}px screen",
                CLUSTER_WIDTH, screen_width
            );
            assert!(
                measurement.pill_width <= max_pill_width + 1e-6,
                "pill width {} exceeds cap {} on {}px screen",
                measurement.pill_width,
                max_pill_width,
                screen_width
            );
        }
    }

    /// Degradation ladder order: a comfortable screen keeps the plan-mock
    /// "{name} {i}/{N}" board label; a tight budget degrades it down to the
    /// compact "Board i/N" and drops the help chip before the unconditional
    /// backstop clamps the pill.
    #[test]
    fn width_budget_degrades_board_label_then_drops_help() {
        let mut state = make_state();
        let active = state.boards.active_index();
        state.boards.board_states_mut()[active].spec.name = LONG_BOARD_NAME.to_string();
        let style = StatusBarStyle::default();
        let index = state.boards.active_index() + 1;
        let count = state.boards.board_count().max(1);

        let first_text = |layout: &StatusHudLayout| match &layout.runs[0] {
            StatusHudRun::Text { text, .. } => text.clone(),
            StatusHudRun::Dot { .. } => panic!("expected the board text run first"),
        };

        // Comfortable budget: full 20-char truncation, name then index/count.
        let wide =
            compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 3840, 2160)
                .expect("wide layout");
        assert_eq!(
            first_text(&wide),
            format!(
                "{} {}/{}",
                crate::util::truncate_with_ellipsis(LONG_BOARD_NAME, BOARD_NAME_MAX_CHARS),
                index,
                count
            )
        );
        assert!(
            wide.segments
                .iter()
                .any(|s| s.kind == StatusHudSegmentKind::Help)
        );

        // Tight budget: the board label reaches the compact form and the
        // help chip is dropped, yet the cap still holds.
        let narrow =
            compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 400, 300)
                .expect("narrow layout");
        assert_eq!(first_text(&narrow), format!("Board {}/{}", index, count));
        assert!(
            !narrow
                .segments
                .iter()
                .any(|s| s.kind == StatusHudSegmentKind::Help),
            "help chip should shed under a tight budget"
        );
        assert!(narrow.pill_width <= 400.0 * STATUS_BAR_MAX_WIDTH_FRACTION + 1e-6);
    }

    /// The hidden-toolbar hint chip appears only while no toolbar surface is
    /// visible, carries the toggle binding, and never shows while presenter
    /// mode owns toolbar visibility (the toggle is a no-op there).
    #[test]
    fn toolbar_hint_chip_appears_only_while_toolbar_hidden() {
        let style = StatusBarStyle::default();
        let mut state = make_state();

        let has_chip = |state: &InputState| {
            compute_status_hud_layout(state, StatusPosition::BottomLeft, &style, 1920, 1080)
                .expect("layout")
                .segments
                .iter()
                .any(|s| s.kind == StatusHudSegmentKind::Toolbar)
        };

        // Toolbars visible: no hint.
        assert!(!has_chip(&state));

        // All toolbar surfaces hidden: the hint appears with the F9 binding.
        state.set_toolbar_visible(false);
        assert!(has_chip(&state));
        let layout =
            compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
                .expect("layout");
        assert!(
            layout.runs.iter().any(|run| matches!(
                run,
                StatusHudRun::Text { text, .. } if text == "F9 Toolbar"
            )),
            "expected the default-binding hint text"
        );

        // Presenter mode with hide_toolbars: the hint must not tease a
        // toggle that presenter mode suppresses.
        state.presenter_mode = true;
        assert!(!has_chip(&state));
        state.presenter_mode = false;

        // `[ui] show_toolbar_hint = false` opts deliberate toolbar-less
        // setups out entirely.
        state.show_toolbar_hint = false;
        assert!(!has_chip(&state));
    }

    #[test]
    fn pill_width_stays_capped_without_prefix() {
        let style = StatusBarStyle::default();

        let measurement = measure(&style, "", CLUSTER_WIDTH, 1280);
        assert_eq!(measurement.prefix_width, 0.0);
        assert!(measurement.pill_width <= 1280.0 * STATUS_BAR_MAX_WIDTH_FRACTION + 1e-6);
    }

    #[test]
    fn oversized_cluster_reports_overflow_for_piece_shedding() {
        let style = StatusBarStyle::default();
        let max_width = 1280.0 * STATUS_BAR_MAX_WIDTH_FRACTION - style.padding * 2.0;

        // Without a prefix the budget binds only when the cluster itself
        // exceeds it.
        assert!(measure(&style, "", max_width + 1.0, 1280).overflow);
        // With a prefix, the reserved prefix floor binds earlier.
        let floor = max_width * MIN_PREFIX_BUDGET_FRACTION;
        assert!(measure(&style, LONG_PREFIX, max_width - floor + 1.0, 1280).overflow);
        assert!(!measure(&style, LONG_PREFIX, 200.0, 1280).overflow);
    }

    #[test]
    fn wrapped_prefix_grows_pill_height() {
        let style = StatusBarStyle::default();

        let narrow = measure(&style, LONG_PREFIX, CLUSTER_WIDTH, 1280);
        let wide = measure(&style, "FROZEN", CLUSTER_WIDTH, 3840);
        // The wrapped prefix block must be accounted for in the pill height so
        // extra lines never spill past the background.
        assert!(narrow.prefix_height >= CLUSTER_LINE_HEIGHT);
        assert!(narrow.pill_height >= narrow.prefix_height + style.padding);
        assert!(narrow.pill_height > wide.pill_height);
    }

    #[test]
    fn pill_height_covers_min_interactive_hit_target() {
        let style = StatusBarStyle {
            font_size: 8.0,
            padding: 2.0,
            dot_radius: 2.0,
            ..StatusBarStyle::default()
        };

        let measurement = measure_status_bar(&style, "", 100.0, 9.0, 4.0, 1920).unwrap();
        assert!(measurement.pill_height >= MIN_INTERACTIVE_HEIGHT);
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

    #[test]
    fn layout_places_core_segments_inside_the_pill_with_min_hit_height() {
        let state = make_state();
        let style = StatusBarStyle::default();
        let layout =
            compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
                .expect("layout");

        for kind in [
            StatusHudSegmentKind::Color,
            StatusHudSegmentKind::Tool,
            StatusHudSegmentKind::Help,
        ] {
            let segment = layout
                .segments
                .iter()
                .find(|segment| segment.kind == kind)
                .unwrap_or_else(|| panic!("segment {kind:?} missing"));
            assert!(
                segment.height >= MIN_INTERACTIVE_HEIGHT,
                "{kind:?} too short"
            );
            assert!(
                segment.width >= MIN_INTERACTIVE_WIDTH,
                "{kind:?} too narrow"
            );
            assert!(segment.x >= layout.pill_x);
            assert!(segment.x + segment.width <= layout.pill_x + layout.pill_width + 1e-6);
            assert!(
                layout.pill_contains(
                    segment.x + segment.width / 2.0,
                    segment.y + segment.height / 2.0
                ),
                "{kind:?} center outside pill"
            );
        }

        // Segments are laid out left-to-right without overlap.
        for pair in layout.segments.windows(2) {
            assert!(
                pair[0].x + pair[0].width <= pair[1].x + 1e-6,
                "segments {:?} and {:?} overlap",
                pair[0].kind,
                pair[1].kind
            );
        }

        let max_pill_width = 1920.0 * STATUS_BAR_MAX_WIDTH_FRACTION;
        assert!(layout.pill_width <= max_pill_width + 1e-6);
    }

    /// With small user font/dot sizes the color dot's natural target drops
    /// below the floor; the layout widens it to `MIN_INTERACTIVE_WIDTH`
    /// (centered, clamped inside the pill) while rects stay disjoint.
    #[test]
    fn narrow_hit_targets_widen_to_min_width() {
        let state = make_state();
        let style = StatusBarStyle {
            font_size: 9.0,
            padding: 4.0,
            dot_radius: 2.0,
            ..StatusBarStyle::default()
        };
        let layout =
            compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
                .expect("layout");

        for segment in &layout.segments {
            assert!(
                segment.width >= MIN_INTERACTIVE_WIDTH - 1e-6,
                "{:?} hit rect narrower than the floor: {}",
                segment.kind,
                segment.width
            );
            assert!(segment.x >= layout.pill_x - 1e-6);
            assert!(segment.x + segment.width <= layout.pill_x + layout.pill_width + 1e-6);
        }
        // Widening cedes neighbor space instead of overlapping it.
        for pair in layout.segments.windows(2) {
            assert!(
                pair[0].x + pair[0].width <= pair[1].x + 1e-6,
                "segments {:?} and {:?} overlap after widening",
                pair[0].kind,
                pair[1].kind
            );
        }
    }

    #[test]
    fn segment_at_maps_hits_and_misses() {
        let state = make_state();
        let style = StatusBarStyle::default();
        let layout =
            compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
                .expect("layout");

        let tool = layout
            .segments
            .iter()
            .find(|segment| segment.kind == StatusHudSegmentKind::Tool)
            .expect("tool segment");
        assert_eq!(
            layout.segment_at(tool.x + tool.width / 2.0, tool.y + tool.height / 2.0),
            Some(StatusHudSegmentKind::Tool)
        );
        assert_eq!(
            layout.segment_at(layout.pill_x - 10.0, layout.pill_y - 10.0),
            None
        );
    }

    #[test]
    fn mode_badges_stack_above_bottom_hud_and_below_top_hud() {
        let mut state = make_state();
        state.set_frozen_active(true);
        state.set_zoom_status(true, false, 2.5, (0.0, 0.0));
        // Zoom actions off: the HUD-stacked ZOOM badge is the zoom indicator
        // here (with zoom actions on the bottom-right chip owns it instead).
        state.show_zoom_actions = false;
        let style = StatusBarStyle::default();

        let bottom =
            compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
                .expect("bottom layout");
        assert_eq!(bottom.badges.len(), 2);
        assert_eq!(bottom.badges[0].label, "FROZEN");
        assert_eq!(bottom.badges[1].label, "ZOOM 250%");
        // Stacked upward, closest badge first, no overlap with the pill.
        assert!(bottom.badges[0].y + bottom.badges[0].height <= bottom.pill_y);
        assert!(bottom.badges[1].y + bottom.badges[1].height <= bottom.badges[0].y);
        // Union bounds cover the badges.
        let (bx, by, bw, bh) = bottom.bounds;
        assert!(by <= bottom.badges[1].y);
        assert!(bx <= bottom.pill_x && bw >= bottom.pill_width);
        assert!(by + bh >= bottom.pill_y + bottom.pill_height);

        let top = compute_status_hud_layout(&state, StatusPosition::TopRight, &style, 1920, 1080)
            .expect("top layout");
        assert!(top.badges[0].y >= top.pill_y + top.pill_height);
        assert!(top.badges[1].y >= top.badges[0].y + top.badges[0].height);
        // Right-aligned to the pill edge.
        for badge in &top.badges {
            assert!((badge.x + badge.width - (top.pill_x + top.pill_width)).abs() < 1e-6);
        }
    }

    /// Reconciliation (M8): with zoom actions enabled the HUD-stacked ZOOM
    /// badge is suppressed (the bottom-right zoom chip is the canonical zoom
    /// indicator), so the percentage never shows in two places at once. Other
    /// mode badges are unaffected.
    #[test]
    fn zoom_badge_suppressed_when_zoom_actions_enabled() {
        let mut state = make_state();
        state.set_frozen_active(true);
        state.set_zoom_status(true, false, 2.5, (0.0, 0.0));
        assert!(state.show_zoom_actions, "default enables zoom actions");
        let style = StatusBarStyle::default();

        let layout =
            compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
                .expect("layout");
        assert!(
            !layout
                .badges
                .iter()
                .any(|badge| badge.label.contains("ZOOM")),
            "ZOOM badge must be suppressed when the zoom chip owns the display"
        );
        // The unrelated FROZEN badge still stacks normally.
        assert!(layout.badges.iter().any(|badge| badge.label == "FROZEN"));

        // Hiding the chip at runtime (ToggleZoomChip) while zoom actions
        // stay on must hand the display back to the HUD badge — otherwise a
        // zoomed session with a visible status bar has NO zoom indicator.
        state.show_zoom_chip = false;
        let chip_hidden =
            compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
                .expect("layout");
        assert!(
            chip_hidden
                .badges
                .iter()
                .any(|badge| badge.label == "ZOOM 250%"),
            "HUD ZOOM badge must return when the chip is runtime-hidden"
        );
        state.show_zoom_chip = true;

        // With zoom actions off the badge returns.
        state.show_zoom_actions = false;
        let restored =
            compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
                .expect("layout");
        assert!(
            restored
                .badges
                .iter()
                .any(|badge| badge.label == "ZOOM 250%")
        );
    }

    #[test]
    fn frozen_badge_keeps_literal_red_tint() {
        let mut state = make_state();
        state.set_frozen_active(true);
        let style = StatusBarStyle::default();
        let layout =
            compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
                .expect("layout");
        let frozen = layout
            .badges
            .iter()
            .find(|badge| badge.label == "FROZEN")
            .expect("frozen badge");
        assert_eq!(frozen.tint, FROZEN_BADGE_TINT);
        assert_eq!(FROZEN_BADGE_TINT, [0.82, 0.22, 0.2, 0.9]);
    }

    #[test]
    fn status_hud_geometry_reads_the_cached_layout_for_matching_screens() {
        let mut state = make_state();
        assert_eq!(status_hud_geometry(&state, 1920, 1080), None);

        state.update_status_hud_layout(
            StatusPosition::BottomLeft,
            &StatusBarStyle::default(),
            1920,
            1080,
        );
        let bounds = status_hud_geometry(&state, 1920, 1080).expect("bounds");
        assert!(bounds.2 > 0.0 && bounds.3 > 0.0);
        // A stale layout for another screen size is not reported.
        assert_eq!(status_hud_geometry(&state, 1280, 720), None);

        state.clear_status_hud_layout();
        assert_eq!(status_hud_geometry(&state, 1920, 1080), None);
    }
}
