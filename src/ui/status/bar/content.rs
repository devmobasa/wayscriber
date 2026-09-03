use super::helpers::*;
use super::measurement::*;
use super::*;

pub(super) fn status_text_style(font_size: f64) -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: font_size,
    }
}

/// One pill content piece before positioning: a text chip or the color dot.
pub(super) struct StatusHudPiece {
    /// `None` marks the color dot.
    pub(super) text: Option<String>,
    pub(super) kind: Option<StatusHudSegmentKind>,
    /// Optional display pieces are shed (last first) when the width budget
    /// binds.
    optional: bool,
    pub(super) extents: Option<UiTextExtents>,
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

    fn layout_advance(&self, dot_diameter: f64, separator_advance: f64) -> f64 {
        let natural = self.advance(dot_diameter);
        if self.kind.is_some() {
            natural.max((MIN_INTERACTIVE_WIDTH - separator_advance).max(0.0))
        } else {
            natural
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
    let prefix_text = build_prefix_text(input_state);
    if pieces.is_empty() && prefix_text.is_none() {
        return None;
    }
    for piece in &mut pieces {
        if let Some(text) = &piece.text {
            piece.extents = Some(measure_text(text_style, text, None)?);
        }
    }
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
    // Optional pieces can all be shed by the width-degradation ladder. Do
    // not leave behind a padding-only pill: it is not effective HUD chrome
    // and must not suppress the fallback badge/recovery paths.
    if pieces.is_empty() && prefix_text.is_none() {
        return None;
    }
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
        if !pieces.is_empty() {
            runs.push(StatusHudRun::Text {
                text: SEGMENT_SEPARATOR.to_string(),
                x: cursor,
                accent: false,
            });
            cursor += sep_advance;
        }
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
        let natural_advance = piece.advance(dot_diameter);
        let advance = piece.layout_advance(dot_diameter, sep_advance);
        let content_x = cursor + (advance - natural_advance) / 2.0;
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
                x: content_x,
                // The clickable-affordance underline must not advertise a
                // click that a display-only HUD
                // (`[ui] status_bar_interactive = false`) will reject; the
                // chip itself still shows the recovery binding there.
                accent: piece.kind == Some(StatusHudSegmentKind::Toolbar)
                    && input_state.ui_visibility.status_bar_interactive,
            }),
            None => runs.push(StatusHudRun::Dot { x: content_x }),
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
pub(super) fn build_cluster_pieces(input_state: &InputState) -> Vec<StatusHudPiece> {
    let mut pieces = Vec::new();

    if input_state.ui_visibility.show_status_board_badge && input_state.boards.show_badge() {
        pieces.push(StatusHudPiece::text(
            board_segment_label(input_state, Some(BOARD_NAME_MAX_CHARS)),
            Some(StatusHudSegmentKind::Board),
            false,
        ));
    }

    if input_state.ui_visibility.show_status_page_badge {
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

    if input_state.ui_visibility.show_status_color {
        pieces.push(StatusHudPiece::dot());
    }

    let tool = input_state.active_tool();
    if input_state.ui_visibility.show_status_tool {
        pieces.push(StatusHudPiece::text(
            tool_display_name(input_state, tool).to_string(),
            Some(StatusHudSegmentKind::Tool),
            false,
        ));
    }
    if input_state.ui_visibility.show_status_size {
        pieces.push(StatusHudPiece::text(
            format!("{}px", input_state.size_for_active_tool() as i32),
            Some(StatusHudSegmentKind::Size),
            false,
        ));
    }

    if input_state.ui_visibility.show_status_context_indicators {
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
    }

    // Hidden-toolbar hint: when every toolbar surface is gone (F9 toggle or
    // F2 cycle-hidden), point at the way back so an accidental hide is
    // recoverable from the status bar alone. Clicking the chip restores the
    // toolbar directly. Opt-out via `[ui] show_toolbar_hint = false` for
    // deliberate toolbar-less setups; suppressed while presenter mode owns
    // toolbar visibility (the toggle is a no-op there); shed first when the
    // width budget binds.
    if input_state.ui_visibility.show_toolbar_hint
        && !(input_state.toolbar_visible()
            || input_state.presenter_mode && input_state.presenter_mode_config.hide_toolbars)
    {
        pieces.push(StatusHudPiece::text(
            toolbar_hint_label(input_state),
            Some(StatusHudSegmentKind::Toolbar),
            true,
        ));
    }

    if input_state.ui_visibility.show_status_help {
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
    }

    // Version chip, last in the row: it states which build is running and
    // opens About when clicked. Marked optional so it is the first piece shed
    // when the width budget binds — a version badge must never cost the board
    // name or the help hint their space.
    if input_state.ui_visibility.show_status_about {
        pieces.push(StatusHudPiece::text(
            format!("About v{}", crate::build_info::version()),
            Some(StatusHudSegmentKind::About),
            true,
        ));
    }

    pieces
}

/// Wrappable non-interactive info before the segments (selection size,
/// output label), or `None` when nothing applies.
pub(super) fn build_prefix_text(input_state: &InputState) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if input_state.ui_visibility.show_active_output_badge
        && let Some(label) = input_state.active_output_label.as_ref()
    {
        let label = crate::util::truncate_with_ellipsis(label, 28);
        parts.push(format!("Output: {label}"));
    }
    if input_state.ui_visibility.show_status_selection_info
        && let Some(bounds) = input_state.selection_bounds()
    {
        let count = input_state.selected_shape_ids().len();
        parts.push(if count == 1 {
            format!("{}×{}px", bounds.width, bounds.height)
        } else {
            format!("{} items: {}×{}px", count, bounds.width, bounds.height)
        });
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
    let piece_widths: f64 = pieces
        .iter()
        .map(|piece| piece.layout_advance(dot_diameter, sep_advance))
        .sum();
    piece_widths + sep_advance * pieces.len().saturating_sub(1) as f64
}
