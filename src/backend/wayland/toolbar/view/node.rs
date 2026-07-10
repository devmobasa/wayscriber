//! Typed widget nodes: what exists, where it is, and what it does.

use std::borrow::Cow;
use std::fmt;

use crate::backend::wayland::toolbar::events::HitKind;
use crate::ui::toolbar::ToolbarEvent;

/// Stable identity of a widget across rebuilds of the tree.
///
/// Ids are dotted paths (e.g. `top.tool.pen`, `side.draw.colors.swatch.3`)
/// so keyboard focus survives a rebuild even when node indices shift, and
/// golden dumps stay readable.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WidgetId(Cow<'static, str>);

impl WidgetId {
    #[allow(dead_code)] // Reserved for the side-palette tree port; top ids use From directly.
    pub fn new(id: impl Into<Cow<'static, str>>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&'static str> for WidgetId {
    fn from(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

impl From<String> for WidgetId {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value))
    }
}

/// A glyph-drawing function from `toolbar_icons`. Compared by function
/// address; rendered in debug output as an opaque tag so golden dumps stay
/// stable (semantic identity lives in [`WidgetId`]).
#[derive(Clone, Copy)]
pub struct IconFn(pub fn(&cairo::Context, f64, f64, f64));

impl PartialEq for IconFn {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::fn_addr_eq(self.0, other.0)
    }
}

impl fmt::Debug for IconFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IconFn")
    }
}

/// Text content plus the minimal style the painter needs.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelSpec {
    pub text: String,
    pub size: f64,
    pub bold: bool,
}

impl LabelSpec {
    pub fn new(text: impl Into<String>, size: f64, bold: bool) -> Self {
        Self {
            text: text.into(),
            size,
            bold,
        }
    }
}

/// Structural button state. Hover and keyboard focus are paint-time inputs,
/// not node data, so pointer motion never rebuilds a tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ButtonStyle {
    /// Selected/active — reserved for the current tool and selected values.
    pub active: bool,
    /// Red destructive treatment (clear, delete).
    pub destructive: bool,
    /// Dimmed and non-interactive.
    pub disabled: bool,
}

impl ButtonStyle {
    pub fn plain() -> Self {
        Self::default()
    }

    pub fn active(value: bool) -> Self {
        Self {
            active: value,
            ..Self::default()
        }
    }

    pub fn destructive() -> Self {
        Self {
            destructive: true,
            ..Self::default()
        }
    }

    pub fn disabled() -> Self {
        Self {
            disabled: true,
            ..Self::default()
        }
    }
}

/// What a node looks like. A closed set the painter exhaustively matches on;
/// variants are added as surfaces move onto the engine.
#[derive(Debug, Clone, PartialEq)]
pub enum WidgetKind {
    /// Full-surface panel background.
    Panel,
    /// Rounded group-card background.
    #[allow(dead_code)] // Used by the staged side-palette tree port.
    Card,
    /// Thin separator line.
    Divider { vertical: bool },
    /// Drag grip for moving a toolbar.
    DragHandle,
    /// Button body with a centered icon glyph of the given size.
    IconButton {
        glyph: IconFn,
        icon_size: f64,
        style: ButtonStyle,
    },
    /// Button body with a centered text label.
    TextButton {
        label: LabelSpec,
        style: ButtonStyle,
    },
    /// Standalone icon glyph (no button body).
    #[allow(dead_code)] // Used by the staged side-palette tree port.
    Icon { glyph: IconFn },
    /// Standalone text.
    Label(LabelSpec),
    /// Small labelled checkbox.
    MiniCheckbox { checked: bool, label: LabelSpec },
    /// Full-size labelled checkbox.
    Checkbox { checked: bool, label: LabelSpec },
    /// Two-segment control; the halves' interactions are separate
    /// [`WidgetKind::HitArea`] nodes layered on top.
    #[allow(dead_code)] // Used by the staged side-palette tree port.
    SegmentedControl {
        left: LabelSpec,
        right: LabelSpec,
        active_right: bool,
    },
    /// Invisible interactive region (segment halves, full-row toggles).
    #[allow(dead_code)] // Used by the staged side-palette tree port.
    HitArea,
    /// Color swatch tile.
    Swatch {
        color: (f64, f64, f64, f64),
        selected: bool,
    },
    /// Pin (open-at-startup) toggle.
    PinButton { pinned: bool },
    /// Minimize chrome button (collapses the bar to its restore tab).
    MinimizeButton,
    /// Anchored popover panel (shadow, background, caret at `caret_x`).
    Popover { caret_x: f64, caret_up: bool },
}

/// What a node does when hit. Nodes without an interaction are decoration.
#[derive(Debug, Clone, PartialEq)]
pub struct Interaction {
    pub event: ToolbarEvent,
    pub kind: HitKind,
    pub tooltip: Option<String>,
}

impl Interaction {
    pub fn click(event: ToolbarEvent, tooltip: Option<String>) -> Self {
        Self {
            event,
            kind: HitKind::Click,
            tooltip,
        }
    }
}

/// One positioned widget in a [`super::WidgetTree`]. Nodes are stored in
/// paint order (background first); the hit-tester walks them topmost-first.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetNode {
    pub id: WidgetId,
    /// Logical-space rect: (x, y, w, h).
    pub rect: (f64, f64, f64, f64),
    pub kind: WidgetKind,
    pub interact: Option<Interaction>,
}

impl WidgetNode {
    pub fn new(
        id: impl Into<WidgetId>,
        rect: (f64, f64, f64, f64),
        kind: WidgetKind,
        interact: Option<Interaction>,
    ) -> Self {
        Self {
            id: id.into(),
            rect,
            kind,
            interact,
        }
    }

    /// Decoration-only node.
    pub fn decor(id: impl Into<WidgetId>, rect: (f64, f64, f64, f64), kind: WidgetKind) -> Self {
        Self::new(id, rect, kind, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_id_debug_is_the_bare_path() {
        let id = WidgetId::from("top.tool.pen");
        assert_eq!(format!("{id:?}"), "top.tool.pen");
        assert_eq!(id.as_str(), "top.tool.pen");
    }

    #[test]
    fn icon_fn_compares_by_address_and_debugs_stably() {
        fn a(_: &cairo::Context, _: f64, _: f64, _: f64) {}
        fn b(_: &cairo::Context, _: f64, _: f64, _: f64) {}

        assert_eq!(IconFn(a), IconFn(a));
        assert_ne!(IconFn(a), IconFn(b));
        assert_eq!(format!("{:?}", IconFn(a)), "IconFn");
    }
}
