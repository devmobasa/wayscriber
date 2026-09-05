use super::super::types::HelpRowHit;
use super::header;
use crate::config::{Action, action_label};
use crate::ui::primitives::{draw_rounded_rect, text_extents_for_with_engine};
use crate::ui_text::UiTextStyle;

/// Horizontal padding inside the "Replay tour" footer pill, between its border
/// and the icon/label content.
const REPLAY_FOOTER_PAD_X: f64 = 12.0;
/// Gap between the refresh icon and the "Replay tour" label inside the pill.
const REPLAY_FOOTER_ICON_GAP: f64 = 7.0;
/// Gap between the footer pills.
const FOOTER_PILL_GAP: f64 = 10.0;

/// One footer pill: an action plus the glyph drawn beside its registry label.
pub(super) struct FooterPill {
    pub(super) action: Action,
    pub(super) icon: crate::toolbar_icons::ToolbarIconPainter,
}

/// Geometry and styling shared by every footer pill.
pub(super) struct FooterPillLayout<'a> {
    pub(super) inner_x: f64,
    pub(super) inner_width: f64,
    pub(super) top_y: f64,
    pub(super) pill_height: f64,
    pub(super) font_size: f64,
    pub(super) font_family: &'a str,
    pub(super) accent: [f64; 4],
    pub(super) accent_muted: [f64; 4],
}

/// Draw the footer pills as one centred row and return their clickable rects,
/// each tagged with the action a click should run.
pub(super) fn draw_footer_pills(
    engine: &crate::ui_text::UiTextEngine,
    ctx: &cairo::Context,
    layout: FooterPillLayout<'_>,
    pills: &[FooterPill],
) -> Vec<HelpRowHit> {
    let icon_size = layout.font_size;
    let label_style = UiTextStyle {
        family: layout.font_family,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: layout.font_size,
    };

    // Measure first so the row can be centred as a group rather than pill by
    // pill.
    let measured: Vec<(&FooterPill, &str, f64)> = pills
        .iter()
        .map(|pill| {
            let label = action_label(pill.action);
            let label_width = text_extents_for_with_engine(
                engine,
                ctx,
                layout.font_family,
                cairo::FontSlant::Normal,
                cairo::FontWeight::Bold,
                layout.font_size,
                label,
            )
            .width();
            let width =
                icon_size + REPLAY_FOOTER_ICON_GAP + label_width + REPLAY_FOOTER_PAD_X * 2.0;
            (pill, label, width)
        })
        .collect();

    let total_width: f64 = measured.iter().map(|(_, _, width)| width).sum::<f64>()
        + FOOTER_PILL_GAP * measured.len().saturating_sub(1) as f64;
    let mut pill_x = layout.inner_x + (layout.inner_width - total_width) / 2.0;

    let mut hits = Vec::with_capacity(measured.len());
    for (pill, label, pill_width) in measured {
        draw_rounded_rect(
            ctx,
            pill_x,
            layout.top_y,
            pill_width,
            layout.pill_height,
            header::PILL_RADIUS,
        );
        ctx.set_source_rgba(layout.accent[0], layout.accent[1], layout.accent[2], 0.14);
        let _ = ctx.fill();
        draw_rounded_rect(
            ctx,
            pill_x,
            layout.top_y,
            pill_width,
            layout.pill_height,
            header::PILL_RADIUS,
        );
        ctx.set_source_rgba(layout.accent[0], layout.accent[1], layout.accent[2], 0.38);
        ctx.set_line_width(1.0);
        let _ = ctx.stroke();

        let content_x = pill_x + REPLAY_FOOTER_PAD_X;
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

        let label_baseline = layout.top_y + layout.pill_height / 2.0 + layout.font_size * 0.35;
        ctx.set_source_rgba(
            layout.accent_muted[0],
            layout.accent_muted[1],
            layout.accent_muted[2],
            layout.accent_muted[3],
        );
        engine.draw_baseline(
            ctx,
            label_style,
            label,
            content_x + icon_size + REPLAY_FOOTER_ICON_GAP,
            label_baseline,
            None,
        );

        hits.push(HelpRowHit {
            x: pill_x,
            y: layout.top_y,
            w: pill_width,
            h: layout.pill_height,
            action: pill.action,
        });
        pill_x += pill_width + FOOTER_PILL_GAP;
    }

    hits
}
