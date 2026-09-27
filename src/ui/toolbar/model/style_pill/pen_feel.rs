//! The Pen feel chip and panel: the default home of pen smoothing and Shape
//! Pen detection in the style pill (`stroke_controls = "panel"`).
//!
//! Two inline meters crowd the pill and say little at pill size. One chip keeps
//! the pill short, and the panel it opens has room for what the inline
//! controls could not show: the name of each level, a one-line description of
//! it, and a live preview of what the smoothing level does to a stroke. The
//! meters inside reuse [`StrokeSetting::meter`], so they click and scroll
//! exactly like the inline ones.

use super::*;

/// The chip's visible label. The caret says it opens something.
pub(crate) const PEN_FEEL_CHIP_LABEL: &str = "Pen feel \u{25BE}";
/// The panel's title, and the chip's accessible name.
pub(crate) const PEN_FEEL_TITLE: &str = "Pen feel";

/// Panel geometry in spec units, shared by both frontends so the panel has
/// the same footprint in each: the panel padding, the content column (the
/// bar rows and the preview span it), and the row heights and gaps.
pub(crate) const PEN_FEEL_PAD: f64 = 10.0;
pub(crate) const PEN_FEEL_CONTENT_W: f64 = 220.0;
pub(crate) const PEN_FEEL_TITLE_H: f64 = 18.0;
pub(crate) const PEN_FEEL_SECTION_GAP: f64 = 10.0;
pub(crate) const PEN_FEEL_HEADER_H: f64 = 16.0;
pub(crate) const PEN_FEEL_BARS_H: f64 = 20.0;
pub(crate) const PEN_FEEL_PREVIEW_H: f64 = 48.0;
pub(crate) const PEN_FEEL_HINT_H: f64 = 14.0;
pub(crate) const PEN_FEEL_ROW_GAP: f64 = 4.0;

/// What each smoothing level does, weakest first.
const SMOOTHING_LEVEL_HINTS: [&str; 7] = [
    "Strokes stay exactly as drawn",
    "Irons out the finest jitter",
    "Evens out small wobbles",
    "Steadies a shaky hand",
    "Smooths out visible shake",
    "Rounds off small corners too",
    "Smoothest; tight corners soften",
];

/// What each Shape Pen detection level turns into shapes, strictest first.
const DETECTION_LEVEL_HINTS: [&str; 5] = [
    "Only near-perfect strokes become shapes",
    "Clean strokes become shapes",
    "Most deliberate strokes become shapes",
    "Rough strokes become shapes",
    "Even quick scribbles become shapes",
];

impl StrokeSetting {
    /// One line saying what `level` does, shown under the panel's meter.
    pub(crate) fn level_hint(self, level: u8) -> &'static str {
        let hints: &[&str] = match self {
            Self::Smoothing => &SMOOTHING_LEVEL_HINTS,
            Self::ShapeDetection => &DETECTION_LEVEL_HINTS,
        };
        hints[usize::from(level.min(self.max()))]
    }

    /// Id fragment of the setting's panel section: `top.feel.<key>`.
    pub(crate) const fn panel_key(self) -> &'static str {
        match self {
            Self::Smoothing => "smoothing",
            Self::ShapeDetection => "detection",
        }
    }
}

/// One section of the open panel: a setting and everything shown for its
/// current level.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PenFeelSection {
    pub(crate) setting: StrokeSetting,
    pub(crate) level: u8,
    pub(crate) level_name: &'static str,
    pub(crate) hint: &'static str,
    /// The section's meter; bars are `top.feel.<key>.level-<n>`.
    pub(crate) meter: StylePillMeter,
}

impl PenFeelSection {
    fn of(setting: StrokeSetting, snapshot: &ToolbarSnapshot) -> Self {
        let level = setting.level(snapshot);
        Self {
            setting,
            level,
            level_name: setting.level_name(level),
            hint: setting.level_hint(level),
            meter: setting.meter(snapshot, &format!("top.feel.{}", setting.panel_key())),
        }
    }

    /// Whether the section draws the live smoothing preview under its meter.
    pub(crate) fn has_preview(&self) -> bool {
        self.setting == StrokeSetting::Smoothing
    }

    /// Id of one part of this section, `top.feel.<key>.<part>`.
    pub(crate) fn id(&self, part: &str) -> String {
        pen_feel_id(self.setting, part)
    }
}

/// Id of one part of a setting's panel section, `top.feel.<key>.<part>`.
pub(crate) fn pen_feel_id(setting: StrokeSetting, part: &str) -> String {
    format!("top.feel.{}.{part}", setting.panel_key())
}

/// The settings the Pen feel panel shows for the active tool, in panel
/// order: smoothing for tools that draw a smoothed path (Pen, Marker, Shape
/// Pen), detection for Shape Pen. Follows the same tool context as the
/// inline controls, so the chip, the meters, and the steppers agree.
pub(crate) fn pen_feel_settings(snapshot: &ToolbarSnapshot) -> Vec<StrokeSetting> {
    let context = ToolContext::from_snapshot(snapshot);
    [
        context
            .show_pen_smoothing
            .then_some(StrokeSetting::Smoothing),
        context
            .show_shape_sensitivity
            .then_some(StrokeSetting::ShapeDetection),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The open panel's sections for the active tool.
pub(crate) fn pen_feel_sections(snapshot: &ToolbarSnapshot) -> Vec<PenFeelSection> {
    pen_feel_settings(snapshot)
        .into_iter()
        .map(|setting| PenFeelSection::of(setting, snapshot))
        .collect()
}

/// The current levels in one line, "Smoothing: Medium · Shape detection:
/// Forgiving", naming only the settings the active tool uses.
fn pen_feel_summary(snapshot: &ToolbarSnapshot) -> String {
    pen_feel_settings(snapshot)
        .into_iter()
        .map(|setting| {
            format!(
                "{}: {}",
                setting.name(),
                setting.current_level_name(snapshot)
            )
        })
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// Height of one section: header, meter, the preview where there is one,
/// and the hint, with the gap that separates it from what comes before.
fn pen_feel_section_height(setting: StrokeSetting) -> f64 {
    let preview = if setting == StrokeSetting::Smoothing {
        PEN_FEEL_PREVIEW_H + PEN_FEEL_ROW_GAP
    } else {
        0.0
    };
    PEN_FEEL_SECTION_GAP
        + PEN_FEEL_HEADER_H
        + PEN_FEEL_ROW_GAP
        + PEN_FEEL_BARS_H
        + PEN_FEEL_ROW_GAP
        + preview
        + PEN_FEEL_HINT_H
}

/// The open panel's size for the active tool: padding, title, and one
/// section per setting.
pub(crate) fn pen_feel_panel_size(snapshot: &ToolbarSnapshot) -> (f64, f64) {
    let sections: f64 = pen_feel_settings(snapshot)
        .into_iter()
        .map(pen_feel_section_height)
        .sum();
    (
        PEN_FEEL_CONTENT_W + PEN_FEEL_PAD * 2.0,
        PEN_FEEL_PAD * 2.0 + PEN_FEEL_TITLE_H + sections,
    )
}

impl StylePillControl {
    /// The chip's tooltip: the current levels and what a click does.
    pub(crate) fn pen_feel_tooltip(snapshot: &ToolbarSnapshot) -> String {
        format!("{} \u{2014} click to adjust", pen_feel_summary(snapshot))
    }

    /// The chip's accessible name: the title with the current levels, so a
    /// screen reader hears the values without opening the panel.
    pub(crate) fn pen_feel_accessible_label(snapshot: &ToolbarSnapshot) -> String {
        format!("{PEN_FEEL_TITLE}. {}", pen_feel_summary(snapshot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Tool;

    fn snapshot_for(tool: Tool, smoothing: u8, sensitivity: u8) -> ToolbarSnapshot {
        let mut state = crate::input::state::test_support::make_test_input_state();
        state.set_pen_smoothing(smoothing);
        state.set_shape_recognition_sensitivity(sensitivity);
        let mut snapshot = ToolbarSnapshot::from_input(&state);
        snapshot.active_tool = tool;
        snapshot.tool_override = None;
        snapshot
    }

    #[test]
    fn the_panel_shows_smoothing_for_pens_and_detection_only_for_shape_pen() {
        for tool in [Tool::Pen, Tool::Marker] {
            assert_eq!(
                pen_feel_settings(&snapshot_for(tool, 3, 3)),
                [StrokeSetting::Smoothing],
                "{tool:?}"
            );
        }
        assert_eq!(
            pen_feel_settings(&snapshot_for(Tool::LiveShape, 3, 3)),
            [StrokeSetting::Smoothing, StrokeSetting::ShapeDetection]
        );
        for tool in [Tool::Line, Tool::Rect, Tool::Eraser, Tool::Arrow] {
            assert!(
                pen_feel_settings(&snapshot_for(tool, 3, 3)).is_empty(),
                "{tool:?}"
            );
        }
    }

    #[test]
    fn the_summary_names_only_the_settings_the_tool_uses() {
        assert_eq!(
            pen_feel_summary(&snapshot_for(Tool::LiveShape, 3, 3)),
            "Smoothing: Medium \u{b7} Shape detection: Forgiving"
        );
        assert_eq!(
            StylePillControl::pen_feel_tooltip(&snapshot_for(Tool::Pen, 0, 3)),
            "Smoothing: Off \u{2014} click to adjust"
        );
        assert_eq!(
            StylePillControl::pen_feel_accessible_label(&snapshot_for(Tool::Marker, 6, 3)),
            "Pen feel. Smoothing: Maximum"
        );
    }

    #[test]
    fn each_section_names_its_level_and_describes_it() {
        let sections = pen_feel_sections(&snapshot_for(Tool::LiveShape, 2, 0));

        let smoothing = &sections[0];
        assert_eq!(smoothing.setting, StrokeSetting::Smoothing);
        assert_eq!(
            (smoothing.level, smoothing.level_name, smoothing.hint),
            (2, "Light", "Evens out small wobbles")
        );
        assert!(smoothing.has_preview());
        assert_eq!(smoothing.meter.segments.len(), 6);
        assert_eq!(smoothing.meter.segments[0].id, "top.feel.smoothing.level-1");

        let detection = &sections[1];
        assert_eq!(
            (detection.level, detection.level_name, detection.hint),
            (0, "Precise", "Only near-perfect strokes become shapes")
        );
        assert!(!detection.has_preview());
        assert_eq!(detection.id("hint"), "top.feel.detection.hint");
    }

    #[test]
    fn every_level_has_a_short_hint() {
        for (setting, levels) in [
            (StrokeSetting::Smoothing, 0..=6),
            (StrokeSetting::ShapeDetection, 0..=4),
        ] {
            for level in levels {
                let hint = setting.level_hint(level);
                assert!(!hint.is_empty(), "{setting:?} {level}");
                // One line at the panel's content width in both frontends.
                assert!(hint.chars().count() <= 40, "{setting:?} {level}: {hint}");
            }
        }
        assert_eq!(
            StrokeSetting::ShapeDetection.level_hint(4),
            "Even quick scribbles become shapes"
        );
    }

    #[test]
    fn the_panel_grows_by_one_section_for_shape_pen() {
        let (pen_w, pen_h) = pen_feel_panel_size(&snapshot_for(Tool::Pen, 3, 3));
        let (shape_w, shape_h) = pen_feel_panel_size(&snapshot_for(Tool::LiveShape, 3, 3));

        assert_eq!(pen_w, shape_w);
        assert_eq!(
            shape_h - pen_h,
            pen_feel_section_height(StrokeSetting::ShapeDetection)
        );
        assert!(
            pen_feel_section_height(StrokeSetting::Smoothing)
                > pen_feel_section_height(StrokeSetting::ShapeDetection),
            "only smoothing carries the preview"
        );
    }
}
