use std::time::Duration;

use crate::config::ClickHighlightConfig;
use crate::draw::Color;

/// Runtime settings for click highlight rendering.
#[derive(Clone)]
pub struct ClickHighlightSettings {
    pub enabled: bool,
    pub radius: f64,
    pub outline_thickness: f64,
    pub duration: Duration,
    pub fill_color: Color,
    pub outline_color: Color,
    pub base_fill_color: Color,
    pub base_outline_color: Color,
    pub use_pen_color: bool,
}

impl ClickHighlightSettings {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn disabled() -> Self {
        let base_fill = Color {
            r: 1.0,
            g: 0.8,
            b: 0.0,
            a: 0.35,
        };
        let base_outline = Color {
            r: 1.0,
            g: 0.6,
            b: 0.0,
            a: 0.9,
        };
        Self {
            enabled: false,
            radius: 24.0,
            outline_thickness: 4.0,
            duration: Duration::from_millis(750),
            fill_color: base_fill,
            outline_color: base_outline,
            base_fill_color: base_fill,
            base_outline_color: base_outline,
            use_pen_color: true,
        }
    }
}

impl From<&ClickHighlightConfig> for ClickHighlightSettings {
    fn from(cfg: &ClickHighlightConfig) -> Self {
        let fill = Color {
            r: cfg.fill_color[0],
            g: cfg.fill_color[1],
            b: cfg.fill_color[2],
            a: cfg.fill_color[3],
        };
        let outline = Color {
            r: cfg.outline_color[0],
            g: cfg.outline_color[1],
            b: cfg.outline_color[2],
            a: cfg.outline_color[3],
        };
        ClickHighlightSettings {
            enabled: cfg.enabled,
            radius: cfg.radius,
            outline_thickness: cfg.outline_thickness,
            duration: Duration::from_millis(cfg.duration_ms),
            fill_color: fill,
            outline_color: outline,
            base_fill_color: fill,
            base_outline_color: outline,
            use_pen_color: cfg.use_pen_color,
        }
    }
}
