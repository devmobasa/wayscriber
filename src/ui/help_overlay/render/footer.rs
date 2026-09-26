use super::header;
use crate::config::{Action, action_label};
use crate::ui::primitives::{
    draw_keycap_with_engine, draw_rounded_rect, keycap_size_with_engine,
    text_extents_for_with_engine,
};
use crate::ui::theme::toolbar;
use crate::ui_text::UiTextStyle;

/// Horizontal padding inside a footer pill, between its border and the
/// icon/label content.
const FOOTER_PILL_PAD_X: f64 = 14.0;
/// Gap between a pill's icon and its label.
const FOOTER_PILL_ICON_GAP: f64 = 8.0;
/// Gap between a pill's label and its key hint chip.
const FOOTER_PILL_KEY_GAP: f64 = 8.0;
/// Gap between the footer pills.
const FOOTER_PILL_GAP: f64 = 12.0;

/// Key that toggles unbound actions. Printable keys belong to type-to-search,
/// so the toggle uses Tab.
pub(crate) const TOGGLE_UNBOUND_KEY: &str = "Tab";

/// What a click on a footer pill does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FooterTarget {
    Action(Action),
    ToggleUnbound,
}

/// One footer pill: its click target, label, optional key hint, and glyph.
pub(super) struct FooterPill {
    pub(super) target: FooterTarget,
    pub(super) label: &'static str,
    pub(super) key_hint: Option<&'static str>,
    pub(super) icon: crate::toolbar_icons::ToolbarIconPainter,
}

impl FooterPill {
    fn action(action: Action, icon: crate::toolbar_icons::ToolbarIconPainter) -> Self {
        Self {
            target: FooterTarget::Action(action),
            label: action_label(action),
            key_hint: None,
            icon,
        }
    }
}

/// The footer pills in display order. Labels of action pills come from the
/// action registry, never a hardcoded string.
pub(super) fn footer_pills(show_unbound: bool) -> [FooterPill; 3] {
    [
        FooterPill::action(Action::ReplayTour, crate::toolbar_icons::draw_icon_refresh),
        FooterPill::action(Action::OpenAbout, crate::toolbar_icons::draw_icon_info),
        FooterPill {
            target: FooterTarget::ToggleUnbound,
            label: if show_unbound {
                "Hide Unbound"
            } else {
                "Show Unbound"
            },
            key_hint: Some(TOGGLE_UNBOUND_KEY),
            icon: crate::toolbar_icons::draw_icon_visibility,
        },
    ]
}

/// A drawn pill's clickable rectangle and what it does.
pub(super) struct FooterHit {
    pub(super) rect: (f64, f64, f64, f64),
    pub(super) target: FooterTarget,
}

/// Geometry and styling shared by every footer pill.
pub(super) struct FooterPillLayout<'a> {
    pub(super) inner_x: f64,
    pub(super) inner_width: f64,
    pub(super) top_y: f64,
    pub(super) pill_height: f64,
    pub(super) font_size: f64,
    pub(super) key_font_size: f64,
    pub(super) font_family: &'a str,
    pub(super) accent: [f64; 4],
    pub(super) accent_muted: [f64; 4],
}

struct PillWidths {
    label: f64,
    total: f64,
}

fn measure_pill(
    engine: &crate::ui_text::UiTextEngine,
    ctx: &cairo::Context,
    font_family: &str,
    font_size: f64,
    key_font_size: f64,
    pill: &FooterPill,
) -> PillWidths {
    let label = text_extents_for_with_engine(
        engine,
        ctx,
        font_family,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        font_size,
        pill.label,
    )
    .width();
    let key_hint = pill.key_hint.map_or(0.0, |key| {
        keycap_size_with_engine(engine, ctx, key, key_font_size).0
    });
    let key_span = if key_hint > 0.0 {
        FOOTER_PILL_KEY_GAP + key_hint
    } else {
        0.0
    };

    PillWidths {
        label,
        total: font_size + FOOTER_PILL_ICON_GAP + label + key_span + FOOTER_PILL_PAD_X * 2.0,
    }
}

/// Width the whole footer pill row occupies, so the overlay box can grow to
/// fit it.
pub(super) fn measure_footer_pills(
    engine: &crate::ui_text::UiTextEngine,
    ctx: &cairo::Context,
    font_family: &str,
    font_size: f64,
    key_font_size: f64,
    pills: &[FooterPill],
) -> f64 {
    pills
        .iter()
        .map(|pill| measure_pill(engine, ctx, font_family, font_size, key_font_size, pill).total)
        .sum::<f64>()
        + FOOTER_PILL_GAP * pills.len().saturating_sub(1) as f64
}

/// Draw the footer pills as one centred row and return their clickable rects,
/// each tagged with what a click should do.
pub(super) fn draw_footer_pills(
    engine: &crate::ui_text::UiTextEngine,
    ctx: &cairo::Context,
    layout: FooterPillLayout<'_>,
    pills: &[FooterPill],
) -> Vec<FooterHit> {
    let icon_size = layout.font_size;
    let label_style = UiTextStyle {
        family: layout.font_family,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: layout.font_size,
    };

    // Measure first so the row can be centred as a group rather than pill by
    // pill.
    let measured: Vec<(&FooterPill, PillWidths)> = pills
        .iter()
        .map(|pill| {
            let widths = measure_pill(
                engine,
                ctx,
                layout.font_family,
                layout.font_size,
                layout.key_font_size,
                pill,
            );
            (pill, widths)
        })
        .collect();
    let total_width: f64 = measured.iter().map(|(_, widths)| widths.total).sum::<f64>()
        + FOOTER_PILL_GAP * measured.len().saturating_sub(1) as f64;
    let mut pill_x = layout.inner_x + (layout.inner_width - total_width) / 2.0;

    let mut hits = Vec::with_capacity(measured.len());
    for (pill, widths) in measured {
        draw_rounded_rect(
            ctx,
            pill_x,
            layout.top_y,
            widths.total,
            layout.pill_height,
            header::PILL_RADIUS,
        );
        ctx.set_source_rgba(layout.accent[0], layout.accent[1], layout.accent[2], 0.14);
        let _ = ctx.fill();
        draw_rounded_rect(
            ctx,
            pill_x,
            layout.top_y,
            widths.total,
            layout.pill_height,
            header::PILL_RADIUS,
        );
        ctx.set_source_rgba(layout.accent[0], layout.accent[1], layout.accent[2], 0.38);
        ctx.set_line_width(1.0);
        let _ = ctx.stroke();

        let content_x = pill_x + FOOTER_PILL_PAD_X;
        let icon_y = layout.top_y + (layout.pill_height - icon_size) / 2.0;
        let _ = ctx.save();
        ctx.set_source_rgba(
            layout.accent_muted[0],
            layout.accent_muted[1],
            layout.accent_muted[2],
            layout.accent_muted[3],
        );
        (pill.icon)(ctx, content_x, icon_y, icon_size);
        let _ = ctx.restore();

        let label_x = content_x + icon_size + FOOTER_PILL_ICON_GAP;
        let label_baseline = layout.top_y + layout.pill_height / 2.0 + layout.font_size * 0.35;
        ctx.set_source_rgba(
            layout.accent_muted[0],
            layout.accent_muted[1],
            layout.accent_muted[2],
            layout.accent_muted[3],
        );
        engine.draw_baseline(ctx, label_style, pill.label, label_x, label_baseline, None);

        if let Some(key) = pill.key_hint {
            let (_, cap_height) = keycap_size_with_engine(engine, ctx, key, layout.key_font_size);
            draw_keycap_with_engine(
                engine,
                ctx,
                label_x + widths.label + FOOTER_PILL_KEY_GAP,
                layout.top_y + (layout.pill_height - cap_height) / 2.0,
                key,
                layout.key_font_size,
                toolbar::COLOR_BADGE_BACKGROUND,
                (
                    layout.accent_muted[0],
                    layout.accent_muted[1],
                    layout.accent_muted[2],
                    layout.accent_muted[3],
                ),
            );
        }

        hits.push(FooterHit {
            rect: (pill_x, layout.top_y, widths.total, layout.pill_height),
            target: pill.target,
        });
        pill_x += widths.total + FOOTER_PILL_GAP;
    }

    hits
}
