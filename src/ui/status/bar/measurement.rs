/// Shared ascent/descent for the single-line cluster, so every text run can
/// sit on one baseline.
#[derive(Clone, Copy)]
pub(super) struct ClusterLineMetrics {
    pub(super) ascent: f64,
    descent: f64,
}

impl ClusterLineMetrics {
    pub(super) fn height(self) -> f64 {
        self.ascent + self.descent
    }
}

pub(super) fn cluster_line_metrics(
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
pub(super) struct StatusBarMeasurement {
    /// Wrap width offered to the prefix (0 when absent).
    pub(super) prefix_budget: f64,
    pub(super) prefix_width: f64,
    pub(super) prefix_height: f64,
    pub(super) prefix_bearing: f64,
    pub(super) pill_width: f64,
    pub(super) pill_height: f64,
    /// True when the width budget binds: the prefix floor is already
    /// consumed, so the cluster must shed optional pieces to fit.
    pub(super) overflow: bool,
}

/// Shape the status bar so the whole pill (background including padding)
/// stays within `STATUS_BAR_MAX_WIDTH_FRACTION` of the screen width. The
/// segment cluster is fixed-width (single line); the prefix wraps within the
/// remaining budget, floored at `MIN_PREFIX_BUDGET_FRACTION` of the total
/// budget. When the floor binds, `overflow` asks the caller to shed optional
/// cluster pieces (the cluster cannot re-wrap the way the M0 suffix could).
pub(super) fn measure_status_bar(
    engine: &UiTextEngine,
    style: &crate::config::StatusBarStyle,
    prefix_text: &str,
    cluster_width: f64,
    cluster_line_height: f64,
    dot_diameter: f64,
    screen_width: u32,
) -> Option<StatusBarMeasurement> {
    let max_width = screen_width as f64 * STATUS_BAR_MAX_WIDTH_FRACTION - style.padding * 2.0;
    let text_style = status_text_style(style.font_size);
    let sep_advance = engine
        .measure(text_style, SEGMENT_SEPARATOR, None)?
        .x_advance();

    let has_prefix = !prefix_text.is_empty();
    let separator_advance = if has_prefix && cluster_width > 0.0 {
        sep_advance
    } else {
        0.0
    };
    let (prefix_budget, prefix_width, prefix_height, prefix_bearing, prefix_advance, overflow) =
        if has_prefix {
            let min_prefix_budget = (max_width * MIN_PREFIX_BUDGET_FRACTION).min(max_width);
            let available = max_width - cluster_width - separator_advance;
            let prefix_budget = available.max(min_prefix_budget).max(1.0);
            // The floor binds when the cluster leaves less room than the
            // prefix is guaranteed; the caller sheds optional pieces then.
            let overflow = prefix_budget > available;
            let extents = engine.measure(text_style, prefix_text, Some(prefix_budget))?;
            let width = extents.width().min(prefix_budget);
            (
                prefix_budget,
                width,
                extents.height(),
                extents.y_bearing(),
                width + separator_advance,
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
pub(super) fn layout_mode_badges(
    engine: &UiTextEngine,
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
    // With the chip absent (zoom actions off, or master-hidden via
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
        && input_state.text_editing.edit_target().is_some()
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
        let Some((width, height)) =
            measure_badge_with_engine(engine, &spec.label, spec.font_size, spec.hint)
        else {
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
use super::content::{StatusHudPiece, status_text_style};
use super::*;
