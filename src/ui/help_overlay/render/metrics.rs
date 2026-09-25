use super::super::grid::GridStyle;
use crate::config::HelpOverlayStyle;

/// Spacing multiplier for the help overlay. Gaps, paddings, and line leading
/// are designed against the user-configurable `HelpOverlayStyle` base sizes
/// and uniformly tightened to 80% so the shortcut grid stays compact. Type
/// sizes are deliberately not tightened: the configured `font_size` is the
/// body size, so rows stay at a comfortable reading size.
const HELP_OVERLAY_SPACE_SCALE: f64 = 0.8;

/// M6: the help overlay reflows as a balanced grid of at most two columns
/// capped at ~60% of the screen width, so it reads as a focused reference
/// card instead of a wall-to-wall sheet.
pub(in crate::ui::help_overlay) const HELP_OVERLAY_MAX_WIDTH_RATIO: f64 = 0.60;

#[derive(Debug, Clone, Copy)]
pub(super) struct RenderMetrics {
    pub(super) body_font_size: f64,
    /// Label size inside the key chips drawn after each action label.
    pub(super) key_font_size: f64,
    pub(super) heading_font_size: f64,
    pub(super) title_font_size: f64,
    pub(super) subtitle_font_size: f64,
    pub(super) row_line_height: f64,
    pub(super) heading_line_height: f64,
    pub(super) heading_icon_size: f64,
    pub(super) heading_icon_gap: f64,
    pub(super) row_gap_after_heading: f64,
    pub(super) key_desc_gap: f64,
    pub(super) row_gap: f64,
    pub(super) column_gap: f64,
    pub(super) section_card_padding: f64,
    pub(super) section_card_radius: f64,
    pub(super) badge_font_size: f64,
    pub(super) badge_padding_x: f64,
    pub(super) badge_gap: f64,
    pub(super) badge_height: f64,
    pub(super) badge_corner_radius: f64,
    pub(super) badge_top_gap: f64,
    pub(super) accent_line_height: f64,
    pub(super) accent_line_bottom_spacing: f64,
    pub(super) title_bottom_spacing: f64,
    pub(super) subtitle_row_height: f64,
    pub(super) subtitle_bottom_spacing: f64,
    pub(super) nav_line_gap: f64,
    pub(super) nav_bottom_spacing: f64,
    pub(super) extra_line_gap: f64,
    pub(super) extra_line_bottom_spacing: f64,
    pub(super) columns_bottom_spacing: f64,
    pub(super) max_box_width: f64,
    pub(super) max_box_height: f64,
    pub(super) note_font_size: f64,
    pub(super) nav_font_size: f64,
    pub(super) note_to_close_gap: f64,
    /// Label size of the clickable footer pills (Replay Tour, About, and the
    /// unbound-actions toggle). They are buttons, so they use the body size.
    pub(super) footer_font_size: f64,
    /// Height of the clickable footer pills.
    pub(super) footer_action_height: f64,
    /// Gap below the footer pills before the note line.
    pub(super) footer_action_gap: f64,
    pub(super) padding: f64,
}

impl RenderMetrics {
    pub(super) fn from_style(
        style: &HelpOverlayStyle,
        screen_width: u32,
        screen_height: u32,
    ) -> Self {
        let scale = HELP_OVERLAY_SPACE_SCALE;
        let min_font_size = MIN_SECONDARY_FONT_SIZE;
        let body_font_size = style.font_size;
        let key_font_size = (body_font_size - 2.0 * scale).max(MIN_KEY_CHIP_FONT_SIZE);
        let heading_font_size = body_font_size + 6.0 * scale;
        let title_font_size = heading_font_size + 6.0 * scale;
        let subtitle_font_size = body_font_size;
        // Rows keep at least 1.5x leading so the key chips never touch.
        let line_height = style.line_height * scale;
        let row_line_height = line_height.max(body_font_size * 1.5);
        let heading_line_height = heading_font_size + 10.0 * scale;
        let heading_icon_size = heading_font_size * 0.9;
        let heading_icon_gap = 10.0 * scale;
        let row_gap_after_heading = 10.0 * scale;
        let key_desc_gap = 18.0 * scale;
        let row_gap = 36.0 * scale;
        let column_gap = 56.0 * scale;
        let section_card_padding = 14.0 * scale;
        let section_card_radius = 10.0 * scale;
        let badge_font_size = (body_font_size - 2.0 * scale).max(min_font_size);
        let badge_padding_x = 12.0 * scale;
        let badge_padding_y = 6.0 * scale;
        let badge_gap = 12.0 * scale;
        let badge_height = badge_font_size + badge_padding_y * 2.0;
        let badge_corner_radius = 10.0 * scale;
        let badge_top_gap = 10.0 * scale;
        let accent_line_height = 2.0 * scale;
        let accent_line_bottom_spacing = 16.0 * scale;
        let title_bottom_spacing = 12.0 * scale;
        // The subtitle row now hosts keycap chips, which stand ~4px above and below
        // the text baseline. Reserve the full chip height so they never crowd the
        // title above or the navigation line below.
        let subtitle_row_height = subtitle_font_size + 8.0;
        let subtitle_bottom_spacing = 22.0 * scale;
        let nav_line_gap = 6.0 * scale;
        let nav_bottom_spacing = 18.0 * scale;
        let extra_line_gap = 30.0 * scale;
        let extra_line_bottom_spacing = 18.0 * scale;
        let columns_bottom_spacing = 28.0 * scale;
        let max_box_width = screen_width as f64 * HELP_OVERLAY_MAX_WIDTH_RATIO;
        let max_box_height = screen_height as f64 * 0.92;
        let note_font_size = (body_font_size - 2.0 * scale).max(min_font_size);
        let nav_font_size = (body_font_size - 1.0 * scale).max(min_font_size);
        let note_to_close_gap = 12.0 * scale;
        let footer_font_size = body_font_size.max(min_font_size);
        let footer_action_height = footer_font_size + 16.0 * scale;
        let footer_action_gap = 14.0 * scale;
        let padding = style.padding * scale;

        Self {
            body_font_size,
            key_font_size,
            heading_font_size,
            title_font_size,
            subtitle_font_size,
            row_line_height,
            heading_line_height,
            heading_icon_size,
            heading_icon_gap,
            row_gap_after_heading,
            key_desc_gap,
            row_gap,
            column_gap,
            section_card_padding,
            section_card_radius,
            badge_font_size,
            badge_padding_x,
            badge_gap,
            badge_height,
            badge_corner_radius,
            badge_top_gap,
            accent_line_height,
            accent_line_bottom_spacing,
            title_bottom_spacing,
            subtitle_row_height,
            subtitle_bottom_spacing,
            nav_line_gap,
            nav_bottom_spacing,
            extra_line_gap,
            extra_line_bottom_spacing,
            columns_bottom_spacing,
            max_box_width,
            max_box_height,
            note_font_size,
            nav_font_size,
            note_to_close_gap,
            footer_font_size,
            footer_action_height,
            footer_action_gap,
            padding,
        }
    }

    /// Section-grid metrics shared by layout measurement and drawing, so the
    /// two can never disagree about a card's size.
    pub(super) fn grid_style<'a>(&self, help_font_family: &'a str) -> GridStyle<'a> {
        GridStyle {
            help_font_family,
            body_font_size: self.body_font_size,
            key_font_size: self.key_font_size,
            heading_font_size: self.heading_font_size,
            heading_line_height: self.heading_line_height,
            heading_icon_size: self.heading_icon_size,
            heading_icon_gap: self.heading_icon_gap,
            row_line_height: self.row_line_height,
            row_gap_after_heading: self.row_gap_after_heading,
            key_desc_gap: self.key_desc_gap,
            badge_font_size: self.badge_font_size,
            badge_padding_x: self.badge_padding_x,
            badge_gap: self.badge_gap,
            badge_height: self.badge_height,
            badge_corner_radius: self.badge_corner_radius,
            badge_top_gap: self.badge_top_gap,
            section_card_padding: self.section_card_padding,
            section_card_radius: self.section_card_radius,
            row_gap: self.row_gap,
            column_gap: self.column_gap,
        }
    }
}

/// Readable floor (logical px) for secondary help text: the nav line, the
/// footer note, badges, and footer pills. Logical pixels already follow the
/// output scale, so the floor also holds on HiDPI outputs.
const MIN_SECONDARY_FONT_SIZE: f64 = 12.0;

/// Readable floor (logical px) for the labels inside key chips.
const MIN_KEY_CHIP_FONT_SIZE: f64 = 11.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_style_reads_at_the_configured_body_size() {
        let style = HelpOverlayStyle::default();
        let metrics = RenderMetrics::from_style(&style, 1920, 1080);

        assert_eq!(metrics.body_font_size, style.font_size);
        assert!(metrics.footer_font_size >= metrics.body_font_size);
        assert!(metrics.footer_action_height > metrics.footer_font_size);
    }

    #[test]
    fn secondary_text_never_drops_below_the_readable_floor() {
        let style = HelpOverlayStyle {
            font_size: 9.0,
            ..HelpOverlayStyle::default()
        };
        let metrics = RenderMetrics::from_style(&style, 1920, 1080);

        assert!(metrics.key_font_size >= MIN_KEY_CHIP_FONT_SIZE);
        for size in [
            metrics.badge_font_size,
            metrics.note_font_size,
            metrics.nav_font_size,
            metrics.footer_font_size,
        ] {
            assert!(size >= MIN_SECONDARY_FONT_SIZE, "{size}");
        }
    }
}
