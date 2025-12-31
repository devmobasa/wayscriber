use crate::config::HelpOverlayStyle;

#[derive(Debug, Clone, Copy)]
pub(super) struct RenderPalette {
    pub(super) bg_top: [f64; 4],
    pub(super) bg_bottom: [f64; 4],
    pub(super) accent: [f64; 4],
    pub(super) accent_muted: [f64; 4],
    pub(super) highlight: [f64; 4],
    pub(super) heading_icon: [f64; 4],
    pub(super) nav_key: [f64; 4],
    pub(super) search: [f64; 4],
    pub(super) subtitle: [f64; 4],
    pub(super) section_card_bg: [f64; 4],
    pub(super) section_card_border: [f64; 4],
    pub(super) body_text: [f64; 4],
    pub(super) description: [f64; 4],
    pub(super) note: [f64; 4],
}

impl RenderPalette {
    pub(super) fn from_style(style: &HelpOverlayStyle) -> Self {
        let [bg_r, bg_g, bg_b, bg_a] = style.bg_color;
        let bg_top = [
            (bg_r + 0.04).min(1.0),
            (bg_g + 0.04).min(1.0),
            (bg_b + 0.04).min(1.0),
            bg_a,
        ];
        let bg_bottom = [
            (bg_r - 0.03).max(0.0),
            (bg_g - 0.03).max(0.0),
            (bg_b - 0.03).max(0.0),
            bg_a,
        ];

        // Warmer, softer accent gold
        let accent = [0.91, 0.73, 0.42, 1.0];
        let accent_muted = [accent[0], accent[1], accent[2], 0.85];
        let highlight = [accent[0], accent[1], accent[2], 0.22];
        let heading_icon = [accent[0], accent[1], accent[2], 0.9];
        let nav_key = [0.58, 0.82, 0.88, 1.0];
        let search = [0.92, 0.58, 0.28, 1.0];
        let subtitle = [0.58, 0.62, 0.72, 1.0];
        let section_card_bg = [1.0, 1.0, 1.0, 0.04];
        let section_card_border = [1.0, 1.0, 1.0, 0.08];
        let body_text = style.text_color;
        let description = [
            lerp(body_text[0], subtitle[0], 0.35),
            lerp(body_text[1], subtitle[1], 0.35),
            lerp(body_text[2], subtitle[2], 0.35),
            body_text[3],
        ];
        let note = [subtitle[0], subtitle[1], subtitle[2], 0.9];

        Self {
            bg_top,
            bg_bottom,
            accent,
            accent_muted,
            highlight,
            heading_icon,
            nav_key,
            search,
            subtitle,
            section_card_bg,
            section_card_border,
            body_text,
            description,
            note,
        }
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a * (1.0 - t) + b * t
}
