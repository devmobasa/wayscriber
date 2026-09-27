use crate::draw::{ArrowStyle, Color};
use crate::util::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPropertyKind {
    Color,
    Thickness,
    Fill,
    FontSize,
    ArrowHead,
    ArrowStyle,
    ArrowLength,
    ArrowAngle,
    TextBackground,
    SpotlightMagnification,
}

impl SelectionPropertyKind {
    /// The kind's value with nothing known about it, for fixtures that only
    /// care about an entry's text.
    #[cfg(test)]
    pub fn unknown_value(self) -> SelectionPropertyValue {
        match self {
            Self::Color => SelectionPropertyValue::Color(None),
            Self::Fill | Self::TextBackground => SelectionPropertyValue::Toggle(None),
            Self::ArrowHead => SelectionPropertyValue::ArrowHead(None),
            Self::ArrowStyle => SelectionPropertyValue::ArrowStyle(None),
            Self::Thickness
            | Self::FontSize
            | Self::ArrowLength
            | Self::ArrowAngle
            | Self::SpotlightMagnification => SelectionPropertyValue::Number(None),
        }
    }
}

/// A property's current value, typed, beside the text an entry displays.
///
/// Surfaces that draw a control instead of a label (a swatch ring, a switch
/// position, the pressed arrow style) read this; the text stays for the ones
/// that only show a readout. `None` inside a variant means there is no single
/// value to show: the editable shapes disagree, or every shape is locked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectionPropertyValue {
    Color(Option<Color>),
    Number(Option<f64>),
    /// A pressure stroke stores a width per point, so it has no one number.
    PressureVaries,
    Toggle(Option<bool>),
    /// Whether the head sits at the end of the arrow (`true`) or its start.
    ArrowHead(Option<bool>),
    ArrowStyle(Option<ArrowStyle>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionPropertyEntry {
    pub label: String,
    pub value: String,
    pub kind: SelectionPropertyKind,
    pub state: SelectionPropertyValue,
    pub disabled: bool,
}

/// One quick-color swatch the properties panel offers.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertiesPanelSwatch {
    pub label: String,
    pub color: Color,
}

/// How much of the selection is locked, for the header's lock toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertiesPanelLock {
    Unlocked,
    /// Some selected shapes are locked and some are not.
    Partial,
    Locked,
}

/// A button in the panel's actions area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelAction {
    ToBack,
    Backward,
    Forward,
    ToFront,
    Duplicate,
    Delete,
}

impl PanelAction {
    /// The ordering buttons, bottom of the stack to top.
    pub const ORDER: [Self; 4] = [Self::ToBack, Self::Backward, Self::Forward, Self::ToFront];
    /// The buttons under them.
    pub const EDIT: [Self; 2] = [Self::Duplicate, Self::Delete];
}

/// Which actions can do anything for the current selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelActions {
    /// Some selected shape has an unselected one above it.
    pub can_raise: bool,
    /// Some selected shape has an unselected one below it.
    pub can_lower: bool,
    /// Some selected shape is unlocked, so duplicating or deleting has
    /// something to work on.
    pub can_edit: bool,
}

impl PanelActions {
    pub fn enabled(&self, action: PanelAction) -> bool {
        match action {
            PanelAction::ToBack | PanelAction::Backward => self.can_lower,
            PanelAction::Forward | PanelAction::ToFront => self.can_raise,
            PanelAction::Duplicate | PanelAction::Delete => self.can_edit,
        }
    }
}

/// The part of the properties panel under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertiesPanelHit {
    /// The title, which carries the shape details as a tooltip.
    Title,
    Lock,
    Action(PanelAction),
    /// A row away from its controls.
    Row(usize),
    Swatch {
        row: usize,
        index: usize,
    },
    /// The swatch row's trailing button that opens the full color picker.
    MoreColors(usize),
    StepDown(usize),
    StepUp(usize),
    Toggle(usize),
    ArrowHead {
        row: usize,
        at_end: bool,
    },
    ArrowStyle {
        row: usize,
        style: ArrowStyle,
    },
}

impl PropertiesPanelHit {
    /// The row this part belongs to; `None` for the header.
    pub fn row(self) -> Option<usize> {
        match self {
            Self::Title | Self::Lock | Self::Action(_) => None,
            Self::Row(row)
            | Self::Swatch { row, .. }
            | Self::MoreColors(row)
            | Self::StepDown(row)
            | Self::StepUp(row)
            | Self::Toggle(row)
            | Self::ArrowHead { row, .. }
            | Self::ArrowStyle { row, .. } => Some(row),
        }
    }
}

/// An axis-aligned rectangle in screen space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl PanelRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }
}

/// Where one property row's control sits.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertiesRowControl {
    Swatches {
        swatches: Vec<PanelRect>,
        more: PanelRect,
    },
    Stepper {
        down: PanelRect,
        value: PanelRect,
        up: PanelRect,
        /// A short stroke drawn at the current thickness.
        preview: Option<PanelRect>,
    },
    Toggle {
        switch: PanelRect,
    },
    ArrowHead {
        well: PanelRect,
        start: PanelRect,
        end: PanelRect,
    },
    ArrowStyles {
        buttons: Vec<(ArrowStyle, PanelRect)>,
    },
}

/// One property row laid out for this frame.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertiesRowGeometry {
    pub index: usize,
    pub rect: PanelRect,
    /// The left and right edges of the row's column content.
    pub content_x: f64,
    pub content_right: f64,
    pub label_baseline_y: f64,
    pub control: PropertiesRowControl,
}

/// The rows' scroll state when they overflow the panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelScroll {
    /// How far the rows are scrolled up.
    pub offset: f64,
    /// The largest offset, where the last row meets the viewport's bottom.
    pub max_offset: f64,
    /// The visible band of rows, from `rows_top` down to here.
    pub viewport_bottom: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct PropertiesPanelLayout {
    pub origin_x: f64,
    pub origin_y: f64,
    pub width: f64,
    pub height: f64,
    pub padding_x: f64,
    pub title_baseline_y: f64,
    pub title_width: f64,
    pub subtitle_baseline_y: Option<f64>,
    pub lock: PanelRect,
    pub divider_y: f64,
    pub rows_top: f64,
    /// Top of the actions area (ordering, Duplicate, Delete), under the rows.
    pub actions_top: f64,
    /// Top of the keyboard hint strip; `None` when the panel has no rows.
    pub footer_top: Option<f64>,
    /// Width of the readout between a stepper's − and + buttons.
    pub stepper_value_width: f64,
    /// Width of each half of the arrow-head Start/End control.
    pub head_segment_width: f64,
    /// Content width of one column of rows.
    pub column_width: f64,
    /// How tall a column may grow before the next row starts another one.
    /// Unbounded unless one column would not fit the screen.
    pub column_budget: f64,
    /// Set when even columns cannot fit the rows on screen: one column of
    /// rows scrolls inside the panel.
    pub scroll: Option<PanelScroll>,
    /// The hovered part's tooltip box, which may reach past the panel.
    pub tooltip: Option<PanelRect>,
}

#[derive(Debug, Clone)]
pub struct ShapePropertiesPanel {
    /// The shape type, or how many shapes are selected.
    pub title: String,
    /// Layer position and size, under the title.
    pub subtitle: Option<String>,
    /// Identity and creation time, shown as the title's tooltip.
    pub details: Option<String>,
    pub lock: PropertiesPanelLock,
    pub anchor: (f64, f64),
    pub anchor_rect: Option<Rect>,
    pub entries: Vec<SelectionPropertyEntry>,
    pub swatches: Vec<PropertiesPanelSwatch>,
    pub actions: PanelActions,
    /// The selection's shared color, for the thickness preview stroke.
    pub preview_color: Option<Color>,
    pub hover: Option<PropertiesPanelHit>,
    /// The part a pointer press landed on; a release activates it only there.
    pub pressed: Option<PropertiesPanelHit>,
    pub keyboard_focus: Option<usize>,
    /// Whether the focus ring shows: keyboard navigation moves focus visibly,
    /// while a click only remembers the row so arrow keys continue there.
    pub focus_visible: bool,
    /// How far the rows are scrolled, when they overflow a short screen.
    pub scroll: f64,
    pub multiple_selection: bool,
}

impl ShapePropertiesPanel {
    /// The row under the pointer, if any.
    pub fn hover_index(&self) -> Option<usize> {
        self.hover.and_then(PropertiesPanelHit::row)
    }
}
