use super::content::status_text_style;
use super::*;

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
    // An empty paint must not initialize the legacy first-writer-wins theme.
    if super::status_hud_geometry(input_state, screen_width, screen_height).is_none() {
        return;
    }
    render_status_bar_with_theme(
        ctx,
        theme::current(),
        input_state,
        style,
        screen_width,
        screen_height,
    );
}

/// Paint with an explicit theme; the compatibility entry point uses the legacy process theme.
pub fn render_status_bar_with_theme(
    ctx: &cairo::Context,
    theme: &theme::Theme,
    input_state: &InputState,
    style: &crate::config::StatusBarStyle,
    screen_width: u32,
    screen_height: u32,
) {
    render_status_bar_with_resources(
        &UiTextEngine::default(),
        ctx,
        theme,
        input_state,
        style,
        screen_width,
        screen_height,
    );
}

pub(crate) fn render_status_bar_with_resources(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    theme: &theme::Theme,
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
        theme.border_hairline,
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

    // Hover backdrop: a faint rounded fill behind the hovered interactive
    // segment so clickable chips announce themselves on mouse-over
    // (`status_hud_hover` is only ever set while the HUD is interactive).
    if let Some(kind) = input_state.status_hud.hover()
        && let Some(segment) = layout.segments.iter().find(|segment| segment.kind == kind)
    {
        ctx.set_source_rgba(r, g, b, a * 0.12);
        draw_rounded_rect(
            ctx,
            segment.x,
            segment.y + 3.0,
            segment.width,
            (segment.height - 6.0).max(0.0),
            6.0,
        );
        let _ = ctx.fill();
    }

    if let Some(prefix) = &layout.prefix {
        // Center the (possibly wrapped) prefix block within the pill so a
        // second line never spills past the background.
        let pango = engine.layout(ctx, text_style, &prefix.text, Some(prefix.wrap_budget));
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
                engine.layout(ctx, text_style, text, None).show_at_baseline(
                    ctx,
                    *x,
                    layout.line_baseline,
                );
                if *accent && let Some(extents) = engine.measure(text_style, text, None) {
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
        draw_badge_with_engine(
            engine,
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
