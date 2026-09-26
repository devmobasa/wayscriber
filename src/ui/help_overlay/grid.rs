use super::super::primitives::{draw_rounded_rect, text_extents_for_with_engine};
use super::keycaps::{KeyComboStyle, draw_key_combo, draw_key_combo_highlight, measure_key_combo};
use super::layout::GridLayout;
use super::search::{HighlightStyle, draw_highlight_with_engine, find_match_range};
use super::types::{HelpRowHit, MeasuredSection, Row};
use crate::ui::theme::{self, Rgba};
use crate::ui_text::UiTextStyle;

/// Section badge label text: near-white, slightly softer than the pure-white
/// token so it sits comfortably on the tinted badge fill (no matching theme
/// token).
const BADGE_LABEL_TEXT: Rgba = (1.0, 1.0, 1.0, 0.92);

/// Clearance between a label's end (or the key column) and its dot leader.
const LEADER_PAD: f64 = 6.0;
/// Leaders shorter than this are skipped; the label already sits by its keys.
const LEADER_MIN_LENGTH: f64 = 10.0;
/// Dot pitch of the leader (one dot, then a gap).
const LEADER_DOT_PITCH: f64 = 4.0;
/// Optical middle of a label's capitals above its baseline, as a fraction of
/// the font size.
const LABEL_CAP_CENTER: f64 = 0.36;

pub(crate) struct GridStyle<'a> {
    pub(crate) help_font_family: &'a str,
    pub(crate) body_font_size: f64,
    pub(crate) key_font_size: f64,
    pub(crate) heading_font_size: f64,
    pub(crate) heading_line_height: f64,
    pub(crate) heading_icon_size: f64,
    pub(crate) heading_icon_gap: f64,
    pub(crate) row_line_height: f64,
    pub(crate) row_gap_after_heading: f64,
    pub(crate) key_desc_gap: f64,
    pub(crate) badge_font_size: f64,
    pub(crate) badge_padding_x: f64,
    pub(crate) badge_gap: f64,
    pub(crate) badge_height: f64,
    pub(crate) badge_corner_radius: f64,
    pub(crate) badge_top_gap: f64,
    pub(crate) section_card_padding: f64,
    pub(crate) section_card_radius: f64,
    pub(crate) row_gap: f64,
    pub(crate) column_gap: f64,
}

pub(crate) struct GridColors {
    pub(crate) accent: [f64; 4],
    pub(crate) heading_icon: [f64; 4],
    pub(crate) description: [f64; 4],
    /// Dot leader between a label and its keys, and the "Not bound" marker.
    pub(crate) muted: [f64; 4],
    pub(crate) highlight: [f64; 4],
    pub(crate) section_card_bg: [f64; 4],
    pub(crate) section_card_border: [f64; 4],
    /// Chrome behind the color badges, for their contrast outline.
    pub(crate) badge_backdrop: [f64; 3],
}

fn set_rgba(ctx: &cairo::Context, color: [f64; 4]) {
    ctx.set_source_rgba(color[0], color[1], color[2], color[3]);
}

/// Dotted leader on the text baseline, guiding the eye from a label to its
/// keys.
fn draw_leader(ctx: &cairo::Context, from_x: f64, to_x: f64, baseline: f64, color: [f64; 4]) {
    let start = from_x + LEADER_PAD;
    let end = to_x - LEADER_PAD;
    if end - start < LEADER_MIN_LENGTH {
        return;
    }

    let _ = ctx.save();
    set_rgba(ctx, color);
    ctx.set_line_width(1.2);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_dash(&[0.0, LEADER_DOT_PITCH], 0.0);
    ctx.move_to(start, baseline - 1.0);
    ctx.line_to(end, baseline - 1.0);
    let _ = ctx.stroke();
    let _ = ctx.restore();
}

fn text_style(family: &str, weight: cairo::FontWeight, size: f64) -> UiTextStyle<'_> {
    UiTextStyle {
        family,
        slant: cairo::FontSlant::Normal,
        weight,
        size,
    }
}

/// Where one section card sits on screen.
struct CardPlacement {
    x: f64,
    y: f64,
    width: f64,
    label_column_width: f64,
}

/// Borrowed paint inputs shared by every card, row, and badge of one grid.
struct GridPainter<'a> {
    engine: &'a crate::ui_text::UiTextEngine,
    ctx: &'a cairo::Context,
    style: &'a GridStyle<'a>,
    colors: &'a GridColors,
    key_combo_style: &'a KeyComboStyle<'a>,
    /// Lowercase search query while search is active.
    search: Option<&'a str>,
    /// Vertical band of the visible grid; hit rects are clipped to it so a
    /// partially scrolled row never reports a click outside the view.
    clip: (f64, f64),
}

impl GridPainter<'_> {
    fn draw_card(
        &self,
        card: &CardPlacement,
        measured: &MeasuredSection,
        hits: &mut Vec<HelpRowHit>,
    ) {
        let style = self.style;
        let ctx = self.ctx;
        let section = &measured.section;

        draw_rounded_rect(
            ctx,
            card.x,
            card.y,
            card.width,
            measured.height,
            style.section_card_radius,
        );
        set_rgba(ctx, self.colors.section_card_bg);
        let _ = ctx.fill_preserve();
        set_rgba(ctx, self.colors.section_card_border);
        ctx.set_line_width(1.0);
        let _ = ctx.stroke();

        // Content starts inside card padding
        let content_x = card.x + style.section_card_padding;
        let mut section_y = card.y + style.section_card_padding;

        let mut heading_text_x = content_x;
        if let Some(icon) = section.icon {
            let icon_y = section_y + (style.heading_line_height - style.heading_icon_size) * 0.5;
            let _ = ctx.save();
            set_rgba(ctx, self.colors.heading_icon);
            icon(ctx, content_x, icon_y, style.heading_icon_size);
            let _ = ctx.restore();
            heading_text_x += style.heading_icon_size + style.heading_icon_gap;
        }
        set_rgba(ctx, self.colors.accent);
        self.engine.draw_baseline(
            ctx,
            text_style(
                style.help_font_family,
                cairo::FontWeight::Bold,
                style.heading_font_size,
            ),
            section.title,
            heading_text_x,
            section_y + style.heading_font_size,
            None,
        );
        section_y += style.heading_line_height;

        if !section.rows.is_empty() {
            section_y += style.row_gap_after_heading;
            for row in &section.rows {
                self.draw_row(card, row, section_y, hits);
                section_y += style.row_line_height;
            }
        }

        if !section.badges.is_empty() {
            self.draw_badges(measured, content_x, section_y + style.badge_top_gap);
        }
    }

    /// Draw one "label … keys" row and register its clickable band.
    fn draw_row(&self, card: &CardPlacement, row: &Row, top: f64, hits: &mut Vec<HelpRowHit>) {
        let style = self.style;
        let ctx = self.ctx;
        let content_x = card.x + style.section_card_padding;
        let key_x = content_x + card.label_column_width + style.key_desc_gap;
        let baseline = top + style.body_font_size;
        // Keycaps centre on `key_baseline - key_font_size / 2`; place that
        // centre on the label's cap-height middle so chips sit level with the
        // text they follow.
        let key_baseline =
            baseline - style.body_font_size * LABEL_CAP_CENTER + style.key_font_size / 2.0;

        if let Some(action) = row.action_id {
            let hit_top = top.max(self.clip.0);
            let hit_bottom = (top + style.row_line_height).min(self.clip.1);
            if hit_bottom > hit_top {
                hits.push(HelpRowHit {
                    x: card.x,
                    y: hit_top,
                    w: card.width,
                    h: hit_bottom - hit_top,
                    action,
                });
            }
        }

        if let Some(search) = self.search {
            self.draw_search_highlights(row, search, content_x, key_x, baseline, key_baseline);
        }

        // Action label first, so each row reads left to right.
        let unbound = row.is_unbound();
        set_rgba(
            ctx,
            if unbound {
                self.colors.muted
            } else {
                self.colors.description
            },
        );
        let label_extents = self.engine.draw_baseline(
            ctx,
            text_style(
                style.help_font_family,
                cairo::FontWeight::Normal,
                style.body_font_size,
            ),
            row.action,
            content_x,
            baseline,
            None,
        );
        if row.key.is_empty() {
            return;
        }

        draw_leader(
            ctx,
            content_x + label_extents.width(),
            key_x,
            baseline,
            self.colors.muted,
        );
        if unbound {
            // No chip for a missing binding: a muted marker reads as "nothing
            // to press" instead of a key named "Not bound".
            let marker_style = UiTextStyle {
                slant: cairo::FontSlant::Italic,
                ..text_style(
                    style.help_font_family,
                    cairo::FontWeight::Normal,
                    style.key_font_size,
                )
            };
            set_rgba(ctx, self.colors.muted);
            self.engine
                .draw_baseline(ctx, marker_style, row.key.as_str(), key_x, baseline, None);
        } else {
            let _ = draw_key_combo(
                self.engine,
                ctx,
                key_x,
                key_baseline,
                row.key.as_str(),
                self.key_combo_style,
            );
        }
    }

    fn draw_search_highlights(
        &self,
        row: &Row,
        search: &str,
        content_x: f64,
        key_x: f64,
        baseline: f64,
        key_baseline: f64,
    ) {
        let style = self.style;
        if !row.key.is_empty() && find_match_range(&row.key, search).is_some() {
            let key_width = measure_key_combo(
                self.engine,
                self.ctx,
                row.key.as_str(),
                style.help_font_family,
                style.key_font_size,
            );
            draw_key_combo_highlight(
                self.ctx,
                key_x,
                key_baseline,
                style.key_font_size,
                key_width,
                self.colors.highlight,
            );
        }
        if let Some(range) = find_match_range(row.action, search) {
            let highlight_style = HighlightStyle {
                font_family: style.help_font_family,
                font_size: style.body_font_size,
                font_weight: cairo::FontWeight::Normal,
                color: self.colors.highlight,
            };
            draw_highlight_with_engine(
                self.engine,
                self.ctx,
                content_x,
                baseline,
                row.action,
                range,
                &highlight_style,
            );
        }
    }

    fn draw_badges(&self, measured: &MeasuredSection, content_x: f64, top: f64) {
        let style = self.style;
        let ctx = self.ctx;
        let badge_style = text_style(
            style.help_font_family,
            cairo::FontWeight::Bold,
            style.badge_font_size,
        );
        let mut badge_x = content_x;

        for (badge_index, badge) in measured.section.badges.iter().enumerate() {
            if badge_index > 0 {
                badge_x += style.badge_gap;
            }

            ctx.new_path();
            let badge_metrics = measured
                .badge_text_metrics
                .get(badge_index)
                .map(|metrics| (metrics.width, metrics.height, metrics.y_bearing))
                .unwrap_or_else(|| {
                    let extents = text_extents_for_with_engine(
                        self.engine,
                        ctx,
                        style.help_font_family,
                        cairo::FontSlant::Normal,
                        cairo::FontWeight::Bold,
                        style.badge_font_size,
                        badge.label.as_str(),
                    );
                    (extents.width(), extents.height(), extents.y_bearing())
                });
            let badge_width = badge_metrics.0 + style.badge_padding_x * 2.0;

            draw_rounded_rect(
                ctx,
                badge_x,
                top,
                badge_width,
                style.badge_height,
                style.badge_corner_radius,
            );
            ctx.set_source_rgba(badge.color[0], badge.color[1], badge.color[2], 0.25);
            let _ = ctx.fill_preserve();

            // A badge whose color melts into the panel (the palette's
            // black) takes a contrast ring for its border.
            let border = (badge.color[0], badge.color[1], badge.color[2], 0.85);
            let [bg_r, bg_g, bg_b] = self.colors.badge_backdrop;
            let (edge, edge_width) =
                theme::swatch::swatch_edge_stroke(border, (bg_r, bg_g, bg_b), border, 1.0);
            theme::set_color(ctx, edge);
            ctx.set_line_width(edge_width);
            let _ = ctx.stroke();

            theme::set_color(ctx, BADGE_LABEL_TEXT);
            let text_x = badge_x + style.badge_padding_x;
            let text_y = top + (style.badge_height - badge_metrics.1) / 2.0 - badge_metrics.2;
            self.engine
                .draw_baseline(ctx, badge_style, badge.label.as_str(), text_x, text_y, None);

            badge_x += badge_width;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_sections_grid(
    engine: &crate::ui_text::UiTextEngine,
    ctx: &cairo::Context,
    grid: &GridLayout,
    grid_start_y: f64,
    inner_x: f64,
    inner_width: f64,
    grid_view_height: f64,
    scroll_offset: f64,
    search_active: bool,
    search_lower: &str,
    style: &GridStyle<'_>,
    colors: &GridColors,
    key_combo_style: &KeyComboStyle<'_>,
    hits: &mut Vec<HelpRowHit>,
) {
    if grid_view_height <= 0.0 {
        return;
    }

    let painter = GridPainter {
        engine,
        ctx,
        style,
        colors,
        key_combo_style,
        search: search_active.then_some(search_lower),
        clip: (grid_start_y, grid_start_y + grid_view_height),
    };

    let _ = ctx.save();
    ctx.rectangle(inner_x, grid_start_y, inner_width, grid_view_height);
    ctx.clip();

    let mut row_y = grid_start_y - scroll_offset;
    for (row_index, row) in grid.rows.iter().enumerate() {
        let row_height = *grid.row_heights.get(row_index).unwrap_or(&0.0);
        let row_width = *grid.row_widths.get(row_index).unwrap_or(&inner_width);
        let mut section_x = inner_x + (inner_width - row_width) / 2.0;
        for (section_index, measured) in row.iter().enumerate() {
            if section_index > 0 {
                section_x += style.column_gap;
            }
            let card = CardPlacement {
                x: section_x,
                y: row_y,
                width: measured.width,
                label_column_width: measured.label_column_width,
            };
            painter.draw_card(&card, measured, hits);
            section_x += measured.width;
        }

        row_y += row_height;
        if row_index + 1 < grid.rows.len() {
            row_y += style.row_gap;
        }
    }

    let _ = ctx.restore();
}
