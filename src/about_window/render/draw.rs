//! Paints the About dialog.
//!
//! Geometry comes from [`super::super::layout`], wording from
//! [`super::super::content`], and interaction state from the caller — this
//! module only decides color and paint order, which is why the other three can
//! be tested without a compositor.

use crate::ui::ellipsize_to_fit_with_engine;
use crate::ui::theme::{self, Rgba, Theme};
use crate::ui_text::{UiTextEngine, UiTextStyle};

use super::super::content::{AboutAction, AboutContent, UpdateState};
use super::super::interaction::Element;
use super::super::layout::{
    BUTTON_SIZE, CARD_TITLE_SIZE, DETAIL_SIZE, HINT_SIZE, META_SIZE, Plan, ROW_TITLE_SIZE, Rect,
    TAGLINE_SIZE, TITLE_SIZE,
};
use super::text::draw_text;
use super::widgets::{
    InteractionState, draw_chevron, draw_close_button, draw_icon, draw_status_dot, draw_surface,
    fill_rounded_rect, set_color, stroke_rounded_rect,
};

const WINDOW_RADIUS: f64 = 12.0;
const CARD_RADIUS: f64 = 10.0;
const ROW_RADIUS: f64 = 9.0;
/// Horizontal padding inside cards and rows.
const PADDING: f64 = 14.0;
/// Where card text starts, leaving room for the status dot.
const CARD_TEXT_LEFT: f64 = 32.0;
const DOT_RADIUS: f64 = 4.0;
const CHEVRON_SIZE: f64 = 9.0;
/// Tint strength for hovered rows: a translucent wash of the foreground color,
/// which lightens dark chrome and darkens light chrome without a second
/// palette.
const HOVER_TINT: f64 = 0.12;

/// "Up to date" dot: the toast palette's success green, the only hue in the
/// dialog that is not already a theme token.
const SUCCESS: Rgba = theme::rgba(theme::overlay::TOAST_SUCCESS, 1.0);

const KEY_HINT: &str = "Tab to move · Enter to open · Esc to close";
const CHECK_NOW: &str = "Check now";

/// Everything one frame needs.
pub(super) struct Frame<'a> {
    pub(super) plan: &'a Plan,
    pub(super) content: &'a AboutContent,
    pub(super) update: &'a UpdateState,
    pub(super) icon: Option<&'a cairo::ImageSurface>,
    pub(super) hover: Option<Element>,
    pub(super) focus: Option<Element>,
    /// Transient footer message ("Copied to clipboard").
    pub(super) notice: Option<&'a str>,
}

impl Frame<'_> {
    fn state_of(&self, element: Element) -> InteractionState {
        InteractionState {
            hovered: self.hover == Some(element),
            focused: self.focus == Some(element),
        }
    }
}

pub(super) fn draw_about(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    theme: &Theme,
    frame: &Frame<'_>,
) {
    backdrop(ctx, frame.plan, theme);
    header(engine, ctx, frame, theme);
    update_card(engine, ctx, frame, theme);
    link_rows(engine, ctx, frame, theme);
    meta_lines(engine, ctx, frame, theme);
    buttons(engine, ctx, frame, theme);
    footer(engine, ctx, frame, theme);
}

fn backdrop(ctx: &cairo::Context, plan: &Plan, theme: &Theme) {
    let rect = (0.0, 0.0, plan.width, plan.height);
    // The dialog owns its chrome (no server-side decorations), so the surface
    // is painted opaque with rounded corners of its own.
    fill_rounded_rect(ctx, rect, WINDOW_RADIUS, opaque(theme.surface_panel));
    stroke_rounded_rect(ctx, rect, WINDOW_RADIUS, theme.border_hairline, 1.0);
}

fn header(engine: &UiTextEngine, ctx: &cairo::Context, frame: &Frame<'_>, theme: &Theme) {
    let plan = frame.plan;

    match frame.icon {
        Some(icon) => draw_icon(ctx, icon, plan.icon),
        None => {
            // A themed monogram beats an empty gap when the PNG cannot be
            // decoded.
            fill_rounded_rect(ctx, plan.icon, CARD_RADIUS, theme.accent);
            let style = style(20.0, cairo::FontWeight::Bold);
            let x = plan.icon.0 + (plan.icon.2 - advance(engine, style, "W")) / 2.0;
            label(
                engine,
                ctx,
                style,
                (x, plan.icon.1 + plan.icon.3 * 0.72),
                (1.0, 1.0, 1.0, 1.0),
                "W",
            );
        }
    }

    label(
        engine,
        ctx,
        style(TITLE_SIZE, cairo::FontWeight::Bold),
        plan.title,
        theme.text_primary,
        frame.content.title,
    );
    label(
        engine,
        ctx,
        style(TAGLINE_SIZE, cairo::FontWeight::Normal),
        plan.tagline,
        theme.text_secondary,
        frame.content.tagline,
    );
    label(
        engine,
        ctx,
        style(META_SIZE, cairo::FontWeight::Normal),
        plan.version,
        theme.text_tertiary,
        &frame.content.version_line,
    );

    draw_close_button(
        ctx,
        plan.close,
        frame.state_of(Element::Close),
        theme.text_tertiary,
        theme.destructive,
    );
}

fn update_card(engine: &UiTextEngine, ctx: &cairo::Context, frame: &Frame<'_>, theme: &Theme) {
    let rect = frame.plan.update_card;
    let state = frame.state_of(Element::UpdateCard);
    let available = frame.update.is_update_available();

    let (idle, hovered) = if available {
        (tint(theme.accent, 0.18), tint(theme.accent, 0.30))
    } else {
        (theme.surface_card, tint(theme.text_primary, HOVER_TINT))
    };
    fill_rounded_rect(
        ctx,
        rect,
        CARD_RADIUS,
        if state.hovered { hovered } else { idle },
    );
    // An available update gets a standing accent border, so the card reads as
    // "act on me" before the text is read.
    if available {
        stroke_rounded_rect(ctx, rect, CARD_RADIUS, tint(theme.accent, 0.6), 1.0);
    }
    if state.focused {
        stroke_rounded_rect(ctx, rect, CARD_RADIUS, theme.accent_bright, 1.5);
    }

    let dot = match frame.update {
        UpdateState::Available { .. } => theme.accent,
        UpdateState::UpToDate { .. } => SUCCESS,
        UpdateState::Failed(_) => theme.destructive,
        UpdateState::Unavailable | UpdateState::Unknown(_) | UpdateState::Checking => {
            theme.text_tertiary
        }
    };
    draw_status_dot(
        ctx,
        (rect.0 + PADDING + DOT_RADIUS, rect.1 + rect.3 / 2.0),
        DOT_RADIUS,
        dot,
    );

    let action_style = style(DETAIL_SIZE, cairo::FontWeight::Bold);
    let text_right = match frame.update.action() {
        Some(AboutAction::OpenUrl(_)) => {
            draw_chevron(
                ctx,
                rect.0 + rect.2 - PADDING - CHEVRON_SIZE / 2.0,
                rect.1 + rect.3 / 2.0,
                CHEVRON_SIZE,
                if state.is_highlighted() {
                    theme.accent_bright
                } else {
                    theme.accent
                },
            );
            rect.0 + rect.2 - PADDING - CHEVRON_SIZE - 8.0
        }
        Some(_) => {
            // The card doubles as the "check now" button whenever there is
            // nothing to open, so it says so.
            let width = advance(engine, action_style, CHECK_NOW);
            let x = rect.0 + rect.2 - PADDING - width;
            label(
                engine,
                ctx,
                action_style,
                (x, rect.1 + rect.3 / 2.0 + action_style.size * 0.36),
                if state.is_highlighted() {
                    theme.accent_bright
                } else {
                    theme.accent
                },
                CHECK_NOW,
            );
            x - 8.0
        }
        None => rect.0 + rect.2 - PADDING,
    };

    let text_left = rect.0 + CARD_TEXT_LEFT;
    let max_width = (text_right - text_left).max(0.0);
    let headline = style(CARD_TITLE_SIZE, cairo::FontWeight::Bold);
    let detail = style(DETAIL_SIZE, cairo::FontWeight::Normal);
    label(
        engine,
        ctx,
        headline,
        (text_left, rect.1 + 21.0),
        theme.text_primary,
        &fit(engine, ctx, &frame.update.headline(), headline, max_width),
    );
    label(
        engine,
        ctx,
        detail,
        (text_left, rect.1 + 36.0),
        theme.text_tertiary,
        &fit(engine, ctx, &frame.update.detail(), detail, max_width),
    );
}

fn link_rows(engine: &UiTextEngine, ctx: &cairo::Context, frame: &Frame<'_>, theme: &Theme) {
    let title_style = style(ROW_TITLE_SIZE, cairo::FontWeight::Normal);
    let detail_style = style(DETAIL_SIZE, cairo::FontWeight::Normal);

    for (index, (rect, link)) in frame
        .plan
        .link_rows
        .iter()
        .zip(frame.content.links.iter())
        .enumerate()
    {
        let state = frame.state_of(Element::Link(index));
        draw_surface(
            ctx,
            *rect,
            ROW_RADIUS,
            state,
            theme.surface_card,
            tint(theme.text_primary, HOVER_TINT),
            theme.accent_bright,
        );

        draw_chevron(
            ctx,
            chevron_x(*rect),
            rect.1 + rect.3 / 2.0,
            CHEVRON_SIZE,
            if state.is_highlighted() {
                theme.accent_bright
            } else {
                theme.text_tertiary
            },
        );

        let (text_left, max_width) = row_text_bounds(*rect);
        label(
            engine,
            ctx,
            title_style,
            (text_left, rect.1 + 16.0),
            if state.is_highlighted() {
                theme.accent_bright
            } else {
                theme.text_primary
            },
            &fit(engine, ctx, link.title, title_style, max_width),
        );
        label(
            engine,
            ctx,
            detail_style,
            (text_left, rect.1 + 30.0),
            theme.text_tertiary,
            &fit(engine, ctx, &link.detail, detail_style, max_width),
        );
    }
}

fn chevron_x(rect: Rect) -> f64 {
    rect.0 + rect.2 - PADDING - CHEVRON_SIZE / 2.0
}

/// Where a link row's text starts and how wide it may be before it runs into
/// the chevron. Shared with the test that keeps row wording inside the dialog's
/// fixed width.
fn row_text_bounds(rect: Rect) -> (f64, f64) {
    let text_left = rect.0 + PADDING;
    (text_left, (chevron_x(rect) - 8.0 - text_left).max(0.0))
}

fn meta_lines(engine: &UiTextEngine, ctx: &cairo::Context, frame: &Frame<'_>, theme: &Theme) {
    let meta_style = style(META_SIZE, cairo::FontWeight::Normal);
    let max_width = frame.plan.width - frame.plan.icon.0 * 2.0;

    for (baseline, line) in frame
        .plan
        .meta_baselines
        .iter()
        .zip(frame.content.meta_lines.iter())
    {
        label(
            engine,
            ctx,
            meta_style,
            *baseline,
            theme.text_tertiary,
            &fit(engine, ctx, line, meta_style, max_width),
        );
    }
}

fn buttons(engine: &UiTextEngine, ctx: &cairo::Context, frame: &Frame<'_>, theme: &Theme) {
    let button_style = style(BUTTON_SIZE, cairo::FontWeight::Normal);
    let specs = frame.content.buttons();

    for (index, (rect, spec)) in frame.plan.buttons.iter().zip(specs.iter()).enumerate() {
        let state = frame.state_of(Element::Button(index));
        draw_surface(
            ctx,
            *rect,
            rect.3 / 2.0,
            state,
            theme.surface_card,
            tint(theme.text_primary, HOVER_TINT + 0.02),
            theme.accent_bright,
        );

        let label_text = fit(engine, ctx, spec.label, button_style, rect.2 - 16.0);
        let x = rect.0 + (rect.2 - advance(engine, button_style, &label_text)) / 2.0;
        label(
            engine,
            ctx,
            button_style,
            (x, baseline_in(*rect, button_style.size)),
            if state.is_highlighted() {
                theme.text_primary
            } else {
                theme.text_secondary
            },
            &label_text,
        );
    }
}

fn footer(engine: &UiTextEngine, ctx: &cairo::Context, frame: &Frame<'_>, theme: &Theme) {
    let hint_style = style(HINT_SIZE, cairo::FontWeight::Normal);
    let (text, color) = match frame.notice {
        Some(notice) => (notice, theme.accent_bright),
        None => (KEY_HINT, theme.text_tertiary),
    };
    let max_width = frame.plan.width - frame.plan.icon.0 * 2.0;
    label(
        engine,
        ctx,
        hint_style,
        frame.plan.hint,
        color,
        &fit(engine, ctx, text, hint_style, max_width),
    );
}

fn style(size: f64, weight: cairo::FontWeight) -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight,
        size,
    }
}

fn label(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    at: (f64, f64),
    color: Rgba,
    text: &str,
) {
    set_color(ctx, color);
    draw_text(engine, ctx, style, at.0, at.1, text);
}

/// Trim to the available width so a long version string or install-source name
/// cannot run under a chevron or off the surface.
fn fit(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    text: &str,
    style: UiTextStyle<'_>,
    max_width: f64,
) -> String {
    ellipsize_to_fit_with_engine(
        engine,
        ctx,
        text,
        style.family,
        style.size,
        style.weight,
        max_width,
    )
}

fn advance(engine: &UiTextEngine, style: UiTextStyle<'_>, text: &str) -> f64 {
    engine
        .measure(style, text, None)
        .map_or(0.0, |extents| extents.x_advance())
}

/// Vertically centered baseline inside `rect` for text of `size`.
fn baseline_in(rect: Rect, size: f64) -> f64 {
    rect.1 + rect.3 / 2.0 + size * 0.36
}

fn tint(color: Rgba, alpha: f64) -> Rgba {
    (color.0, color.1, color.2, alpha)
}

fn opaque(color: Rgba) -> Rgba {
    (color.0, color.1, color.2, 1.0)
}

#[cfg(test)]
mod tests;
