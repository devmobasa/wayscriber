//! The arrow style chip and the menu it opens.
//!
//! The chip used to cycle Standard, Pointy, Curved, and Double on each click,
//! which hid three of the four styles and named them with words that do not
//! say what an arrow looks like. The chip now shows the current style drawn
//! and named, and opens a menu listing every style as a drawn preview, so
//! the user picks the arrow they can see in one click. The keyboard action
//! still cycles.

use super::*;
use crate::draw::ArrowStyle;

/// Chip geometry in spec units, shared by both frontends: its slot in the
/// pill and the drawn glyph before its label.
pub(crate) const ARROW_STYLE_CHIP_W: f64 = 120.0;
pub(crate) const ARROW_STYLE_CHIP_GLYPH_W: f64 = 26.0;
pub(crate) const ARROW_STYLE_CHIP_GLYPH_H: f64 = 12.0;
/// Menu geometry in spec units: the padding around the rows, each row, the
/// gap between rows, the preview slot at a row's left, and the inset of a
/// row's (and the chip's) content from its edge.
pub(crate) const ARROW_STYLE_MENU_PAD: f64 = 6.0;
pub(crate) const ARROW_STYLE_MENU_ROW_W: f64 = 150.0;
pub(crate) const ARROW_STYLE_MENU_ROW_H: f64 = 32.0;
pub(crate) const ARROW_STYLE_MENU_ROW_GAP: f64 = 2.0;
pub(crate) const ARROW_STYLE_MENU_PREVIEW_W: f64 = 54.0;
pub(crate) const ARROW_STYLE_MENU_PREVIEW_H: f64 = 20.0;
pub(crate) const ARROW_STYLE_MENU_INSET: f64 = 8.0;

/// One row of the arrow style menu.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ArrowStyleMenuEntry {
    pub(crate) style: ArrowStyle,
    pub(crate) label: &'static str,
    /// What sets the style apart, for the row's tooltip.
    pub(crate) hint: &'static str,
    /// Whether this is the style the next arrow will use.
    pub(crate) current: bool,
    pub(crate) event: ToolbarEvent,
}

impl ArrowStyleMenuEntry {
    pub(crate) fn id(&self) -> String {
        format!("top.arrow-style.{}", self.label.to_ascii_lowercase())
    }

    pub(crate) fn tooltip(&self) -> String {
        format!("{}: {}", self.label, self.hint)
    }
}

/// Every style, in [`ArrowStyle::ALL`] order, with `current` marked.
pub(crate) fn arrow_style_menu_entries(current: ArrowStyle) -> Vec<ArrowStyleMenuEntry> {
    ArrowStyle::ALL
        .into_iter()
        .map(|style| ArrowStyleMenuEntry {
            style,
            label: style.label(),
            hint: arrow_style_hint(style),
            current: style == current,
            event: ToolbarEvent::SetArrowStyle(style),
        })
        .collect()
}

/// The open menu's size, padding included.
pub(crate) fn arrow_style_menu_size() -> (f64, f64) {
    let rows = ArrowStyle::ALL.len() as f64;
    (
        ARROW_STYLE_MENU_ROW_W + ARROW_STYLE_MENU_PAD * 2.0,
        ARROW_STYLE_MENU_PAD * 2.0
            + rows * ARROW_STYLE_MENU_ROW_H
            + (rows - 1.0) * ARROW_STYLE_MENU_ROW_GAP,
    )
}

/// The chip's visible label: the current style, and a caret saying it opens
/// something.
pub(crate) fn arrow_style_chip_label(style: ArrowStyle) -> String {
    format!("{} \u{25BE}", style.label())
}

fn arrow_style_hint(style: ArrowStyle) -> &'static str {
    match style {
        ArrowStyle::Standard => "a tapered shaft into one head",
        ArrowStyle::Pointy => "a dart head with a notched back",
        ArrowStyle::Curved => "arcs around what sits in its way",
        ArrowStyle::Double => "a head at both ends",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_lists_every_style_once_and_marks_the_current_one() {
        let entries = arrow_style_menu_entries(ArrowStyle::Curved);

        assert_eq!(
            entries.iter().map(|entry| entry.style).collect::<Vec<_>>(),
            ArrowStyle::ALL
        );
        for entry in &entries {
            assert_eq!(entry.current, entry.style == ArrowStyle::Curved);
            assert_eq!(entry.event, ToolbarEvent::SetArrowStyle(entry.style));
        }
        assert_eq!(entries[1].id(), "top.arrow-style.pointy");
        assert_eq!(
            entries[1].tooltip(),
            "Pointy: a dart head with a notched back"
        );
    }

    #[test]
    fn the_menu_fits_its_rows() {
        let (w, h) = arrow_style_menu_size();

        assert_eq!(w, 162.0);
        assert_eq!(h, 12.0 + 4.0 * 32.0 + 3.0 * 2.0);
    }
}
