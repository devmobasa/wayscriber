use std::borrow::Cow;

use crate::config::{
    Action, ToolbarItemId, ToolbarLayoutMode, action_label, action_short_label,
    toolbar_item_ids as ids,
};
use crate::input::Tool;
use crate::label_format::format_binding_label;
use crate::ui::toolbar::bindings::{tool_label, tool_tooltip_label};
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

use super::{
    SemanticToolIcon, TopToolGroup, TopUtilityButton, current_shape_tool, default_drag_hint,
    semantic_icon_for_tool, toolbar_item_id_for_tool, toolbar_item_visible,
    top_highlight_ring_visible, top_highlight_visible, top_shape_picker_visible, top_tool_group,
    visible_top_tool_buttons, visible_top_utility_buttons,
};

/// Width-degradation result shared by both top-toolbar frontends.
///
/// The built-in layout layer owns the geometry-dependent planner that fills
/// this value. The semantic specification only consumes the result.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TopStripPlan {
    pub(crate) swatch_count: usize,
    pub(crate) dropped_tools: Vec<Tool>,
    pub(crate) dropped_utilities: Vec<TopUtilityButton>,
    pub(crate) compact: bool,
}

impl TopStripPlan {
    pub(crate) const MAX_QUICK_COLORS: usize = 8;

    pub(crate) fn unconstrained() -> Self {
        Self {
            swatch_count: Self::MAX_QUICK_COLORS,
            dropped_tools: Vec::new(),
            dropped_utilities: Vec::new(),
            compact: false,
        }
    }
}

/// Renderer-neutral top-toolbar structure for one snapshot and layout plan.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TopToolbarSpec {
    strip: Vec<TopToolbarNode>,
    chrome: Vec<TopToolbarControl>,
    overflow: Vec<TopToolbarControl>,
    contextual: Vec<TopToolbarControl>,
}

impl TopToolbarSpec {
    pub(crate) fn build(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> Self {
        if snapshot.top_minimized {
            return Self {
                strip: vec![TopToolbarNode::Control(TopToolbarControl::Restore)],
                chrome: Vec::new(),
                overflow: Vec::new(),
                contextual: Vec::new(),
            };
        }
        if snapshot.top_micro_active() {
            return Self {
                strip: vec![TopToolbarNode::Control(TopToolbarControl::MicroChip)],
                chrome: Vec::new(),
                overflow: Vec::new(),
                contextual: Vec::new(),
            };
        }

        let simple = snapshot.layout_mode == ToolbarLayoutMode::Simple;
        let mut strip = Vec::new();

        if toolbar_item_visible(snapshot, ids::TOP_CHROME_DRAG) {
            strip.push(TopToolbarNode::Control(TopToolbarControl::DragHandle));
        }

        let mut previous_tool_group = None;
        let mut tool_control_present = false;
        for tool in visible_top_tool_buttons(simple, snapshot) {
            if plan.dropped_tools.contains(&tool) {
                continue;
            }
            let group = top_tool_group(tool);
            if previous_tool_group.is_some_and(|previous| previous != group) {
                strip.push(TopToolbarNode::Divider(TopToolbarDivider::Tools));
            }
            previous_tool_group = Some(group);
            strip.push(TopToolbarNode::Control(TopToolbarControl::Tool(tool)));
            tool_control_present = true;
        }

        if Self::shape_picker_visible(snapshot) {
            if previous_tool_group == Some(TopToolGroup::Pens) {
                strip.push(TopToolbarNode::Divider(TopToolbarDivider::Tools));
            }
            strip.push(TopToolbarNode::Control(TopToolbarControl::ShapePicker));
            tool_control_present = true;
        }

        let visible_utilities = visible_top_utility_buttons(snapshot, simple, snapshot.use_icons);
        let utilities: Vec<_> = visible_utilities
            .iter()
            .copied()
            .filter(|utility| *utility != TopUtilityButton::ClearCanvas)
            .filter(|utility| !plan.dropped_utilities.contains(utility))
            .collect();
        if tool_control_present && !utilities.is_empty() {
            strip.push(TopToolbarNode::Divider(TopToolbarDivider::Annotations));
        }
        strip.extend(utilities.into_iter().filter_map(|utility| {
            TopToolbarUtility::from_model(utility)
                .map(TopToolbarControl::Utility)
                .map(TopToolbarNode::Control)
        }));

        if toolbar_item_visible(snapshot, ids::TOP_GROUP_QUICK_COLORS) {
            strip.push(TopToolbarNode::Divider(TopToolbarDivider::Colors));
            strip.extend(
                snapshot
                    .quick_colors
                    .rendered_entries()
                    .iter()
                    .take(plan.swatch_count)
                    .enumerate()
                    .map(|(index, _)| {
                        TopToolbarNode::Control(TopToolbarControl::QuickColor(index))
                    }),
            );
            strip.push(TopToolbarNode::Control(TopToolbarControl::CurrentColor));
        }

        // The history island: Undo/Redo plus the always-anchored overflow
        // toggle. Clear lives inside the overflow menu (first entry), so the
        // toggle shows whenever the menu has content — not only under width
        // pressure.
        let undo_visible = toolbar_item_visible(snapshot, ids::TOP_UTILITY_UNDO);
        let redo_visible = toolbar_item_visible(snapshot, ids::TOP_UTILITY_REDO);
        if undo_visible {
            strip.push(TopToolbarNode::Control(TopToolbarControl::Undo));
        }
        if redo_visible {
            strip.push(TopToolbarNode::Control(TopToolbarControl::Redo));
        }
        let overflow: Vec<_> =
            Self::overflow_controls(Self::clear_canvas_in_overflow(snapshot), plan).collect();
        if !overflow.is_empty() {
            strip.push(TopToolbarNode::Control(TopToolbarControl::Overflow));
        }

        let chrome = Self::chrome_controls(snapshot)
            .into_iter()
            .flatten()
            .collect();
        let contextual = if Self::contextual_highlight_ring_visible(snapshot, plan) {
            vec![TopToolbarControl::HighlightRing]
        } else {
            Vec::new()
        };

        Self {
            strip,
            chrome,
            overflow,
            contextual,
        }
    }

    pub(crate) fn strip(&self) -> &[TopToolbarNode] {
        &self.strip
    }

    pub(crate) fn chrome(&self) -> &[TopToolbarControl] {
        &self.chrome
    }

    pub(crate) fn overflow(&self) -> &[TopToolbarControl] {
        &self.overflow
    }

    pub(crate) fn contextual(&self) -> &[TopToolbarControl] {
        &self.contextual
    }

    pub(crate) fn shape_picker_visible(snapshot: &ToolbarSnapshot) -> bool {
        !snapshot.top_minimized
            && !snapshot.top_micro_active()
            && top_shape_picker_visible(snapshot)
    }

    pub(crate) fn contextual_highlight_ring_visible(
        snapshot: &ToolbarSnapshot,
        plan: &TopStripPlan,
    ) -> bool {
        !snapshot.top_minimized
            && !snapshot.top_micro_active()
            && snapshot.layout_mode != ToolbarLayoutMode::Simple
            && snapshot.use_icons
            && snapshot.highlight_tool_active
            && top_highlight_visible(snapshot)
            && top_highlight_ring_visible(snapshot)
            && !plan
                .dropped_utilities
                .contains(&TopUtilityButton::Highlight)
    }

    pub(crate) fn chrome_control_count(snapshot: &ToolbarSnapshot, _plan: &TopStripPlan) -> usize {
        if snapshot.top_minimized || snapshot.top_micro_active() {
            return 0;
        }
        Self::chrome_controls(snapshot)
            .into_iter()
            .flatten()
            .count()
    }

    pub(crate) fn overflow_control_count(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> usize {
        if snapshot.top_minimized || snapshot.top_micro_active() {
            return 0;
        }
        Self::overflow_controls(Self::clear_canvas_in_overflow(snapshot), plan).count()
    }

    fn chrome_controls(snapshot: &ToolbarSnapshot) -> [Option<TopToolbarControl>; 2] {
        [
            toolbar_item_visible(snapshot, ids::TOP_CHROME_PIN).then_some(TopToolbarControl::Pin),
            toolbar_item_visible(snapshot, ids::TOP_CHROME_CLOSE)
                .then_some(TopToolbarControl::Minimize),
        ]
    }

    /// Clear moved off the strip into the overflow menu; it stays subject to
    /// the same visibility rules it had as a strip utility.
    fn clear_canvas_in_overflow(snapshot: &ToolbarSnapshot) -> bool {
        let simple = snapshot.layout_mode == ToolbarLayoutMode::Simple;
        visible_top_utility_buttons(snapshot, simple, snapshot.use_icons)
            .contains(&TopUtilityButton::ClearCanvas)
    }

    /// Overflow menu content, in menu order: the destructive Clear first,
    /// then the width-dropped tools and utilities in configured order, then
    /// the Session/Settings popover entries. The two menu entries appear in
    /// every layout (`side_layout = "panel"` included): under `pill` they
    /// are the only surface hosting those functions, and under `panel` the
    /// popovers are transient quick surfaces that cannot be confused with
    /// the pinned side panes — so they are unconditional rather than
    /// pill-gated. Like the restore controls they are not hideable items:
    /// under `pill` they must always be reachable.
    fn overflow_controls(
        clear_visible: bool,
        plan: &TopStripPlan,
    ) -> impl Iterator<Item = TopToolbarControl> + '_ {
        clear_visible
            .then_some(TopToolbarControl::ClearCanvas)
            .into_iter()
            .chain(
                plan.dropped_tools
                    .iter()
                    .copied()
                    .map(TopToolbarControl::Tool),
            )
            .chain(
                plan.dropped_utilities
                    .iter()
                    .copied()
                    .filter_map(TopToolbarUtility::from_model)
                    .map(TopToolbarControl::Utility),
            )
            .chain([
                TopToolbarControl::SessionMenu,
                TopToolbarControl::SettingsMenu,
            ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarNode {
    Divider(TopToolbarDivider),
    Control(TopToolbarControl),
}

impl TopToolbarNode {
    /// Island membership of a strip node. Thin dividers only exist inside
    /// the tools island; the island boundaries themselves are gaps, not
    /// divider nodes.
    pub(crate) fn island(&self) -> TopToolbarIsland {
        match self {
            Self::Divider(_) => TopToolbarIsland::Tools,
            Self::Control(control) => control.island(),
        }
    }
}

/// The three detached pill islands of the top strip, in reading order:
/// tools (drag grip through colors), history (undo/redo/overflow), chrome
/// (pin/minimize). Both frontends and the contract tests derive island
/// membership from this one accessor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TopToolbarIsland {
    Tools,
    History,
    Chrome,
}

impl TopToolbarIsland {
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Tools => "tools",
            Self::History => "history",
            Self::Chrome => "chrome",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarDivider {
    Tools,
    Annotations,
    Colors,
}

impl TopToolbarDivider {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Tools => "top.divider.tools",
            Self::Annotations => "top.divider.annotations",
            Self::Colors => "top.divider.colors",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarControl {
    Restore,
    /// The micro-mode chip: the whole strip collapsed to one round chip
    /// showing the active tool inside a ring in the current color. Like
    /// `Restore`, it is the way back and therefore never hideable.
    MicroChip,
    DragHandle,
    Tool(Tool),
    ShapePicker,
    Utility(TopToolbarUtility),
    QuickColor(usize),
    CurrentColor,
    Undo,
    Redo,
    ClearCanvas,
    Pin,
    Overflow,
    Minimize,
    HighlightRing,
    /// Overflow menu entry opening the Session popover (open/save/recent).
    SessionMenu,
    /// Overflow menu entry opening the Settings popover (toolbar options
    /// and customization).
    SettingsMenu,
}

impl TopToolbarControl {
    pub(crate) fn id(self) -> TopToolbarControlId {
        let id = match self {
            Self::Restore => return TopToolbarControlId::Restore,
            Self::MicroChip => return TopToolbarControlId::MicroChip,
            Self::SessionMenu => return TopToolbarControlId::SessionMenu,
            Self::SettingsMenu => return TopToolbarControlId::SettingsMenu,
            Self::DragHandle => ids::TOP_CHROME_DRAG,
            Self::Tool(tool) => toolbar_item_id_for_tool(tool),
            Self::ShapePicker => ids::TOP_UTILITY_SHAPE_PICKER,
            Self::Utility(utility) => match utility {
                TopToolbarUtility::Text => ids::TOP_UTILITY_TEXT,
                TopToolbarUtility::StickyNote => ids::TOP_UTILITY_STICKY_NOTE,
                TopToolbarUtility::Screenshot => ids::TOP_UTILITY_SCREENSHOT,
                TopToolbarUtility::Highlight => ids::TOP_UTILITY_HIGHLIGHT,
            },
            Self::QuickColor(index) => return TopToolbarControlId::QuickColor(index),
            Self::CurrentColor => ids::TOP_GROUP_QUICK_COLORS,
            Self::Undo => ids::TOP_UTILITY_UNDO,
            Self::Redo => ids::TOP_UTILITY_REDO,
            Self::ClearCanvas => ids::TOP_UTILITY_CLEAR_CANVAS,
            Self::Pin => ids::TOP_CHROME_PIN,
            Self::Overflow => ids::TOP_CHROME_OVERFLOW,
            Self::Minimize => ids::TOP_CHROME_CLOSE,
            Self::HighlightRing => ids::TOP_UTILITY_HIGHLIGHT_RING,
        };
        TopToolbarControlId::Item(id)
    }

    pub(crate) fn event(self, snapshot: &ToolbarSnapshot) -> ToolbarEvent {
        match self {
            Self::Restore => ToolbarEvent::SetTopMinimized(false),
            Self::MicroChip => ToolbarEvent::SetTopDisplayMode(crate::config::TopDisplayMode::Full),
            Self::DragHandle => ToolbarEvent::MoveTopToolbar { x: 0.0, y: 0.0 },
            Self::Tool(tool) => ToolbarEvent::SelectTool(tool),
            Self::ShapePicker => ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
            Self::Utility(utility) => utility_event(utility, snapshot),
            Self::QuickColor(index) => {
                let entry = &snapshot.quick_colors.rendered_entries()[index];
                ToolbarEvent::SetQuickColor {
                    color: entry.color,
                    action: crate::config::QuickColorPalette::action_for_index(index),
                }
            }
            Self::CurrentColor => ToolbarEvent::OpenColorPickerPopup,
            Self::Undo => ToolbarEvent::Undo,
            Self::Redo => ToolbarEvent::Redo,
            // The mouse path clears with an undo toast; the frontends
            // upgrade to the instant variant when Shift is held.
            Self::ClearCanvas => ToolbarEvent::ClearCanvas { instant: false },
            Self::Pin => ToolbarEvent::PinTopToolbar(!snapshot.top_pinned),
            Self::Overflow => ToolbarEvent::ToggleTopOverflow(!snapshot.top_overflow_open),
            Self::SessionMenu => ToolbarEvent::ToggleSessionPopover(!snapshot.session_popover_open),
            Self::SettingsMenu => {
                ToolbarEvent::ToggleSettingsPopover(!snapshot.settings_popover_open)
            }
            Self::Minimize => ToolbarEvent::SetTopMinimized(true),
            Self::HighlightRing => {
                ToolbarEvent::ToggleHighlightToolRing(!snapshot.highlight_tool_ring_enabled)
            }
        }
    }

    pub(crate) fn action(self, snapshot: &ToolbarSnapshot) -> Option<Action> {
        self.event(snapshot).action()
    }

    pub(crate) fn enabled(self, snapshot: &ToolbarSnapshot) -> bool {
        match self {
            Self::Undo => snapshot.undo_available,
            Self::Redo => snapshot.redo_available,
            _ => true,
        }
    }

    pub(crate) fn active(self, snapshot: &ToolbarSnapshot) -> bool {
        match self {
            Self::Tool(tool) => {
                snapshot.active_tool == tool || snapshot.tool_override == Some(tool)
            }
            Self::ShapePicker => {
                snapshot.shape_picker_open
                    || current_shape_tool(snapshot.active_tool, snapshot.tool_override).is_some()
            }
            Self::Utility(TopToolbarUtility::Text) => snapshot.text_active,
            Self::Utility(TopToolbarUtility::StickyNote) => snapshot.note_active,
            Self::Utility(TopToolbarUtility::Highlight) => snapshot.any_highlight_active,
            Self::Utility(TopToolbarUtility::Screenshot) => false,
            Self::QuickColor(index) => {
                snapshot.quick_colors.rendered_entries()[index].color == snapshot.color
            }
            Self::CurrentColor => true,
            Self::Pin => snapshot.top_pinned,
            Self::Overflow => snapshot.top_overflow_open,
            Self::SessionMenu => snapshot.session_popover_open,
            Self::SettingsMenu => snapshot.settings_popover_open,
            Self::HighlightRing => snapshot.highlight_tool_ring_enabled,
            _ => false,
        }
    }

    pub(crate) fn role(self) -> TopToolbarControlRole {
        match self {
            // The micro chip shares the restore role: a single non-hideable
            // control whose click brings the full strip back.
            Self::Restore | Self::MicroChip => TopToolbarControlRole::Restore,
            Self::DragHandle => TopToolbarControlRole::DragHandle,
            Self::QuickColor(_) | Self::CurrentColor => TopToolbarControlRole::Swatch,
            Self::ClearCanvas => TopToolbarControlRole::Destructive,
            Self::ShapePicker
            | Self::Utility(TopToolbarUtility::Highlight)
            | Self::Overflow
            | Self::SessionMenu
            | Self::SettingsMenu
            | Self::HighlightRing => TopToolbarControlRole::Toggle,
            // The chrome island (pin, minimize) renders quieter than the
            // content islands; both frontends key that styling off this role.
            Self::Pin | Self::Minimize => TopToolbarControlRole::Chrome,
            _ => TopToolbarControlRole::Button,
        }
    }

    /// Which pill island the control belongs to. Total over all controls so
    /// non-strip lanes (chrome, overflow, contextual) answer consistently.
    pub(crate) fn island(self) -> TopToolbarIsland {
        match self {
            Self::Undo
            | Self::Redo
            | Self::Overflow
            | Self::ClearCanvas
            | Self::SessionMenu
            | Self::SettingsMenu => TopToolbarIsland::History,
            Self::Pin | Self::Minimize | Self::Restore | Self::MicroChip => {
                TopToolbarIsland::Chrome
            }
            _ => TopToolbarIsland::Tools,
        }
    }

    pub(crate) fn icon(self, snapshot: &ToolbarSnapshot) -> Option<TopToolbarIcon> {
        Some(match self {
            Self::Restore => TopToolbarIcon::Restore,
            // The chip shows the active tool's glyph; the ring around it is
            // frontend paint, not an icon.
            Self::MicroChip => TopToolbarIcon::Tool(semantic_icon_for_tool(snapshot.active_tool)),
            Self::DragHandle => TopToolbarIcon::Drag,
            Self::Tool(tool) => TopToolbarIcon::Tool(semantic_icon_for_tool(tool)),
            Self::ShapePicker => TopToolbarIcon::ShapePicker,
            Self::Utility(TopToolbarUtility::Text) => TopToolbarIcon::Text,
            Self::Utility(TopToolbarUtility::StickyNote) => TopToolbarIcon::StickyNote,
            Self::Utility(TopToolbarUtility::Screenshot) => TopToolbarIcon::Screenshot,
            Self::ClearCanvas => TopToolbarIcon::ClearCanvas,
            Self::Utility(TopToolbarUtility::Highlight) => TopToolbarIcon::Highlight,
            Self::Undo => TopToolbarIcon::Undo,
            Self::Redo => TopToolbarIcon::Redo,
            Self::Pin if snapshot.top_pinned => TopToolbarIcon::Pin,
            Self::Pin => TopToolbarIcon::Unpin,
            Self::Overflow => TopToolbarIcon::Overflow,
            Self::SessionMenu => TopToolbarIcon::Session,
            Self::SettingsMenu => TopToolbarIcon::Settings,
            Self::Minimize => TopToolbarIcon::Minimize,
            Self::QuickColor(_) | Self::CurrentColor | Self::HighlightRing => return None,
        })
    }

    pub(crate) fn label(self, snapshot: &ToolbarSnapshot) -> Cow<'static, str> {
        match self {
            Self::Restore => Cow::Borrowed("Show toolbar"),
            Self::MicroChip => Cow::Borrowed("Show full toolbar"),
            Self::DragHandle => Cow::Borrowed("Drag toolbar"),
            Self::Tool(tool) => Cow::Borrowed(tool_label(tool)),
            Self::ShapePicker => Cow::Borrowed("Shapes"),
            Self::Utility(utility) => Cow::Borrowed(utility_short_label(utility)),
            Self::QuickColor(index) => Cow::Owned(
                snapshot.quick_colors.rendered_entries()[index]
                    .label
                    .clone(),
            ),
            Self::CurrentColor => Cow::Borrowed("Color picker"),
            Self::Undo => Cow::Borrowed(action_short_label(Action::Undo)),
            Self::Redo => Cow::Borrowed(action_short_label(Action::Redo)),
            Self::ClearCanvas => Cow::Borrowed(action_short_label(Action::ClearCanvas)),
            Self::Pin if snapshot.top_pinned => Cow::Borrowed("Unpin top toolbar"),
            Self::Pin => Cow::Borrowed("Pin top toolbar"),
            Self::Overflow => Cow::Borrowed("More tools"),
            Self::SessionMenu => Cow::Borrowed("Session..."),
            Self::SettingsMenu => Cow::Borrowed("Settings..."),
            Self::Minimize => Cow::Borrowed("Minimize top toolbar"),
            Self::HighlightRing => Cow::Borrowed("Ring"),
        }
    }

    pub(crate) fn accessible_label(self, snapshot: &ToolbarSnapshot) -> Cow<'static, str> {
        match self {
            Self::Tool(tool) => Cow::Borrowed(tool_tooltip_label(tool)),
            Self::Utility(utility) => Cow::Borrowed(utility_accessible_label(utility)),
            Self::Undo => Cow::Borrowed(action_label(Action::Undo)),
            Self::Redo => Cow::Borrowed(action_label(Action::Redo)),
            Self::ClearCanvas => Cow::Borrowed(action_label(Action::ClearCanvas)),
            Self::QuickColor(index) => Cow::Owned(
                snapshot.quick_colors.rendered_entries()[index]
                    .label
                    .clone(),
            ),
            Self::SessionMenu => Cow::Borrowed("Session menu"),
            Self::SettingsMenu => Cow::Borrowed("Settings menu"),
            _ => self.label(snapshot),
        }
    }

    pub(crate) fn tooltip(self, snapshot: &ToolbarSnapshot) -> String {
        match self {
            Self::Tool(tool) => tool_tooltip(snapshot, tool),
            Self::Utility(utility) => action_tooltip(snapshot, utility_action(utility)),
            Self::QuickColor(index) => {
                let entry = &snapshot.quick_colors.rendered_entries()[index];
                let binding = crate::config::QuickColorPalette::action_for_index(index)
                    .and_then(|action| snapshot.binding_hints.binding_for_action(action));
                format_binding_label(&entry.label, binding)
            }
            Self::Undo => action_tooltip(snapshot, Action::Undo),
            Self::Redo => action_tooltip(snapshot, Action::Redo),
            Self::ClearCanvas => action_tooltip(snapshot, Action::ClearCanvas),
            Self::Pin if snapshot.top_pinned => {
                "Pinned: opens at startup (click to disable)".to_string()
            }
            Self::Pin => "Pin: click to open at startup".to_string(),
            Self::Minimize => "Minimize (leaves a restore tab)".to_string(),
            Self::MicroChip => "Micro toolbar (click to show the full toolbar)".to_string(),
            Self::SessionMenu => "Session: open, save, recent files".to_string(),
            Self::SettingsMenu => "Settings: toolbar options and customization".to_string(),
            Self::HighlightRing => "Highlight ring".to_string(),
            _ => self.accessible_label(snapshot).into_owned(),
        }
    }

    pub(crate) fn overflow_tooltip(self, snapshot: &ToolbarSnapshot) -> String {
        match self {
            Self::Utility(_) => self.accessible_label(snapshot).into_owned(),
            _ => self.tooltip(snapshot),
        }
    }

    pub(crate) fn shortcut_badge(self, snapshot: &ToolbarSnapshot) -> Option<String> {
        match self {
            Self::Tool(tool) => snapshot
                .binding_hints
                .badge_for_tool(tool)
                .map(str::to_owned),
            Self::QuickColor(index) => snapshot
                .binding_hints
                .quick_color_badge(index)
                .map(str::to_owned),
            _ => self
                .action(snapshot)
                .and_then(|action| snapshot.binding_hints.badge_for_action(action))
                .map(str::to_owned),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarControlId {
    Item(ToolbarItemId),
    QuickColor(usize),
    Restore,
    MicroChip,
    SessionMenu,
    SettingsMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarUtility {
    Text,
    StickyNote,
    Screenshot,
    Highlight,
}

impl TopToolbarUtility {
    fn from_model(utility: TopUtilityButton) -> Option<Self> {
        match utility {
            TopUtilityButton::Text => Some(Self::Text),
            TopUtilityButton::StickyNote => Some(Self::StickyNote),
            TopUtilityButton::Screenshot => Some(Self::Screenshot),
            TopUtilityButton::Highlight => Some(Self::Highlight),
            TopUtilityButton::ClearCanvas | TopUtilityButton::IconMode => None,
        }
    }
}

impl TopToolbarControlId {
    pub(crate) fn render_id(self) -> Cow<'static, str> {
        match self {
            Self::Item(id) => Cow::Borrowed(id.as_str()),
            Self::QuickColor(index) => Cow::Owned(format!("top.quick-color.{index}")),
            Self::Restore => Cow::Borrowed("top.chrome.restore"),
            Self::MicroChip => Cow::Borrowed("top.chrome.micro"),
            Self::SessionMenu => Cow::Borrowed("top.menu.session"),
            Self::SettingsMenu => Cow::Borrowed("top.menu.settings"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarControlRole {
    Button,
    Toggle,
    Swatch,
    Destructive,
    Chrome,
    DragHandle,
    Restore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarIcon {
    Restore,
    Drag,
    Tool(SemanticToolIcon),
    ShapePicker,
    Text,
    StickyNote,
    Screenshot,
    Highlight,
    ClearCanvas,
    Undo,
    Redo,
    Pin,
    Unpin,
    Overflow,
    Minimize,
    /// Session popover entry (file glyph).
    Session,
    /// Settings popover entry (gear glyph).
    Settings,
}

fn utility_event(utility: TopToolbarUtility, snapshot: &ToolbarSnapshot) -> ToolbarEvent {
    match utility {
        TopToolbarUtility::Text => ToolbarEvent::EnterTextMode,
        TopToolbarUtility::StickyNote => ToolbarEvent::EnterStickyNoteMode,
        TopToolbarUtility::Screenshot => ToolbarEvent::CaptureScreenshot,
        TopToolbarUtility::Highlight => {
            ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active)
        }
    }
}

fn utility_action(utility: TopToolbarUtility) -> Action {
    match utility {
        TopToolbarUtility::Text => Action::EnterTextMode,
        TopToolbarUtility::StickyNote => Action::EnterStickyNoteMode,
        TopToolbarUtility::Screenshot => Action::CaptureSelection,
        TopToolbarUtility::Highlight => Action::ToggleHighlightTool,
    }
}

fn utility_short_label(utility: TopToolbarUtility) -> &'static str {
    match utility {
        TopToolbarUtility::Screenshot => "Shot",
        TopToolbarUtility::Highlight => "Highlight",
        _ => action_short_label(utility_action(utility)),
    }
}

fn utility_accessible_label(utility: TopToolbarUtility) -> &'static str {
    action_label(utility_action(utility))
}

/// Ring stroke width of the micro chip for a given stroke thickness.
///
/// Perceptual mapping shared by both frontends: thickness 1px → 1.5px ring,
/// growing linearly to a 5px ring at 20px thickness and clamped there — the
/// upper stroke range (to 50px) would otherwise swallow the chip.
pub(crate) fn micro_ring_width(thickness: f64) -> f64 {
    const MIN_RING: f64 = 1.5;
    const MAX_RING: f64 = 5.0;
    const THICKNESS_AT_MAX: f64 = 20.0;
    let normalized = ((thickness - 1.0) / (THICKNESS_AT_MAX - 1.0)).clamp(0.0, 1.0);
    MIN_RING + normalized * (MAX_RING - MIN_RING)
}

pub(crate) fn action_tooltip(snapshot: &ToolbarSnapshot, action: Action) -> String {
    format_binding_label(
        action_label(action),
        snapshot.binding_hints.binding_for_action(action),
    )
}

fn tool_tooltip(snapshot: &ToolbarSnapshot, tool: Tool) -> String {
    let label = tool_tooltip_label(tool);
    let default_hint = default_drag_hint(tool);
    let binding = match (snapshot.binding_hints.for_tool(tool), default_hint) {
        (Some(binding), Some(fallback)) => Some(format!("{binding}, {fallback}")),
        (Some(binding), None) => Some(binding.to_string()),
        (None, Some(fallback)) => Some(fallback.to_string()),
        (None, None) => None,
    };
    format_binding_label(label, binding.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ToolbarItemOrderGroup, ToolbarItemsConfig};
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarBindingHints;

    fn snapshot() -> ToolbarSnapshot {
        let state = make_test_input_state();
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default())
    }

    fn strip_control_ids(spec: &TopToolbarSpec) -> Vec<String> {
        spec.strip()
            .iter()
            .filter_map(|node| match node {
                TopToolbarNode::Control(control) => Some(control.id().render_id().into_owned()),
                TopToolbarNode::Divider(_) => None,
            })
            .collect()
    }

    fn chrome_ids(spec: &TopToolbarSpec) -> Vec<String> {
        spec.chrome()
            .iter()
            .map(|control| control.id().render_id().into_owned())
            .collect()
    }

    #[test]
    fn regular_spec_owns_control_order_ids_and_events() {
        let snapshot = snapshot();
        let spec = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
        let ids = strip_control_ids(&spec);

        let expected_order = [
            "top.chrome.drag",
            "top.tool.select",
            "top.tool.pen",
            "top.tool.marker",
            "top.tool.step-marker",
            "top.tool.eraser",
            "top.tool.line",
            "top.tool.arrow",
            "top.utility.shape-picker",
            "top.utility.text",
            "top.utility.sticky-note",
            "top.utility.highlight",
            "top.quick-color.0",
            "top.group.quick-colors",
            "top.utility.undo",
            "top.utility.redo",
            "top.chrome.overflow",
        ];
        let mut position = 0;
        for expected in expected_order {
            let next = ids[position..]
                .iter()
                .position(|id| id == expected)
                .map(|offset| position + offset)
                .unwrap_or_else(|| panic!("{expected} missing from {ids:?}"));
            position = next + 1;
        }

        assert!(
            !ids.contains(&ids::TOP_UTILITY_CLEAR_CANVAS.as_str().to_string()),
            "Clear lives in the overflow menu, not the strip: {ids:?}"
        );
        assert_eq!(
            spec.overflow(),
            [
                TopToolbarControl::ClearCanvas,
                TopToolbarControl::SessionMenu,
                TopToolbarControl::SettingsMenu,
            ],
            "an unconstrained plan still anchors Clear plus the Session/Settings entries"
        );

        assert_eq!(chrome_ids(&spec), ["top.chrome.pin", "top.chrome.close"]);
        let pen = spec
            .strip()
            .iter()
            .find_map(|node| match node {
                TopToolbarNode::Control(control @ TopToolbarControl::Tool(Tool::Pen)) => {
                    Some(*control)
                }
                _ => None,
            })
            .expect("pen control");
        assert_eq!(pen.event(&snapshot), ToolbarEvent::SelectTool(Tool::Pen));
        assert_eq!(pen.id(), TopToolbarControlId::Item(ids::TOP_TOOL_PEN));

        let divider_ids: Vec<_> = spec
            .strip()
            .iter()
            .filter_map(|node| match node {
                TopToolbarNode::Divider(divider) => Some(divider.id()),
                TopToolbarNode::Control(_) => None,
            })
            .collect();
        assert_eq!(
            divider_ids,
            [
                "top.divider.tools",
                "top.divider.annotations",
                "top.divider.colors",
            ],
            "thin dividers exist only inside the tools island"
        );
    }

    #[test]
    fn simple_and_compact_specs_preserve_semantics_without_geometry() {
        let mut simple = snapshot();
        simple.layout_mode = ToolbarLayoutMode::Simple;
        let simple_spec = TopToolbarSpec::build(&simple, &TopStripPlan::unconstrained());
        let simple_ids = strip_control_ids(&simple_spec);
        assert!(!simple_ids.contains(&ids::TOP_TOOL_LINE.as_str().to_string()));
        assert!(!simple_ids.contains(&ids::TOP_TOOL_ARROW.as_str().to_string()));
        assert!(!simple_ids.contains(&ids::TOP_UTILITY_HIGHLIGHT.as_str().to_string()));
        assert!(!simple_ids.contains(&ids::TOP_UTILITY_CLEAR_CANVAS.as_str().to_string()));
        assert!(simple_ids.contains(&ids::TOP_UTILITY_SHAPE_PICKER.as_str().to_string()));

        let regular = snapshot();
        let mut compact_plan = TopStripPlan::unconstrained();
        compact_plan.compact = true;
        let compact_spec = TopToolbarSpec::build(&regular, &compact_plan);
        assert_eq!(
            strip_control_ids(&compact_spec),
            strip_control_ids(&TopToolbarSpec::build(
                &regular,
                &TopStripPlan::unconstrained()
            ))
        );
    }

    #[test]
    fn minimized_spec_contains_only_the_non_hideable_restore_control() {
        let mut snapshot = snapshot();
        snapshot.top_minimized = true;
        snapshot
            .resolved_toolbar_items
            .hidden
            .insert(ids::TOP_CHROME_CLOSE);

        let spec = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
        assert_eq!(strip_control_ids(&spec), ["top.chrome.restore"]);
        assert!(spec.chrome().is_empty());
        assert!(spec.overflow().is_empty());
        assert_eq!(
            match spec.strip() {
                [TopToolbarNode::Control(control)] => control.event(&snapshot),
                other => panic!("unexpected minimized spec: {other:?}"),
            },
            ToolbarEvent::SetTopMinimized(false)
        );
    }

    #[test]
    fn micro_spec_contains_only_the_non_hideable_micro_chip_control() {
        let mut snapshot = snapshot();
        snapshot.top_display_mode = crate::config::TopDisplayMode::Micro;
        snapshot
            .resolved_toolbar_items
            .hidden
            .insert(ids::TOP_CHROME_CLOSE);

        let spec = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
        assert_eq!(strip_control_ids(&spec), ["top.chrome.micro"]);
        assert!(spec.chrome().is_empty());
        assert!(spec.overflow().is_empty());
        assert!(spec.contextual().is_empty());
        let chip = match spec.strip() {
            [TopToolbarNode::Control(control)] => *control,
            other => panic!("unexpected micro spec: {other:?}"),
        };
        assert_eq!(
            chip.event(&snapshot),
            ToolbarEvent::SetTopDisplayMode(crate::config::TopDisplayMode::Full)
        );
        assert_eq!(chip.role(), TopToolbarControlRole::Restore);
        assert_eq!(
            chip.icon(&snapshot),
            Some(TopToolbarIcon::Tool(semantic_icon_for_tool(
                snapshot.active_tool
            )))
        );

        // Minimized wins when both states are somehow set.
        snapshot.top_minimized = true;
        let minimized = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
        assert_eq!(strip_control_ids(&minimized), ["top.chrome.restore"]);
    }

    #[test]
    fn micro_ring_width_maps_thickness_into_the_clamped_ring_range() {
        assert_eq!(micro_ring_width(0.0), 1.5);
        assert_eq!(micro_ring_width(1.0), 1.5);
        assert_eq!(micro_ring_width(20.0), 5.0);
        assert_eq!(micro_ring_width(50.0), 5.0);
        let mid = micro_ring_width(10.5);
        assert!(mid > 1.5 && mid < 5.0);
    }

    #[test]
    fn narrow_spec_moves_dropped_controls_to_one_ordered_overflow() {
        let snapshot = snapshot();
        let mut plan = TopStripPlan::unconstrained();
        plan.swatch_count = 0;
        plan.dropped_tools = vec![Tool::Line, Tool::Arrow];
        plan.dropped_utilities = vec![
            TopUtilityButton::Text,
            TopUtilityButton::StickyNote,
            TopUtilityButton::Highlight,
        ];
        plan.compact = true;

        let spec = TopToolbarSpec::build(&snapshot, &plan);
        let strip_ids = strip_control_ids(&spec);
        for dropped in [
            ids::TOP_TOOL_LINE,
            ids::TOP_TOOL_ARROW,
            ids::TOP_UTILITY_TEXT,
            ids::TOP_UTILITY_STICKY_NOTE,
            ids::TOP_UTILITY_HIGHLIGHT,
        ] {
            assert!(!strip_ids.contains(&dropped.as_str().to_string()));
        }
        let overflow_ids: Vec<_> = spec
            .overflow()
            .iter()
            .map(|control| control.id().render_id().into_owned())
            .collect();
        assert_eq!(
            overflow_ids,
            [
                "top.utility.clear-canvas",
                "top.tool.line",
                "top.tool.arrow",
                "top.utility.text",
                "top.utility.sticky-note",
                "top.utility.highlight",
                "top.menu.session",
                "top.menu.settings",
            ],
            "Clear leads the overflow menu; dropped items follow in order; \
             the Session/Settings entries close it"
        );
        assert!(
            strip_ids.contains(&ids::TOP_CHROME_OVERFLOW.as_str().to_string()),
            "the overflow toggle anchors in the history island: {strip_ids:?}"
        );
        assert_eq!(chrome_ids(&spec), ["top.chrome.pin", "top.chrome.close"]);
        assert_eq!(
            spec.overflow()[0].event(&snapshot),
            ToolbarEvent::ClearCanvas { instant: false }
        );
        assert_eq!(
            spec.overflow()[0].role(),
            TopToolbarControlRole::Destructive
        );
        assert_eq!(
            spec.overflow()[1].event(&snapshot),
            ToolbarEvent::SelectTool(Tool::Line)
        );
    }

    #[test]
    fn overflow_hosts_session_and_settings_entries_in_every_layout() {
        // Decision (M4-B3): the two popover entries are unconditional —
        // they show under both `side_layout` values and are not hideable
        // items, because under `pill` they are the only surface hosting
        // Session/Settings.
        for layout_mode in [ToolbarLayoutMode::Regular, ToolbarLayoutMode::Simple] {
            let mut snapshot = snapshot();
            snapshot.layout_mode = layout_mode;
            let spec = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
            let tail: Vec<_> = spec
                .overflow()
                .iter()
                .rev()
                .take(2)
                .rev()
                .copied()
                .collect();
            assert_eq!(
                tail,
                [
                    TopToolbarControl::SessionMenu,
                    TopToolbarControl::SettingsMenu,
                ],
                "{layout_mode:?}: the menu entries close the overflow"
            );
            assert!(
                !spec.overflow().is_empty(),
                "the overflow toggle always has content"
            );
            assert!(
                spec.strip()
                    .contains(&TopToolbarNode::Control(TopToolbarControl::Overflow))
            );
        }

        let mut snapshot = snapshot();
        let session = TopToolbarControl::SessionMenu;
        let settings = TopToolbarControl::SettingsMenu;

        assert_eq!(session.id().render_id(), "top.menu.session");
        assert_eq!(settings.id().render_id(), "top.menu.settings");
        assert_eq!(session.label(&snapshot), "Session...");
        assert_eq!(settings.label(&snapshot), "Settings...");
        assert_eq!(session.icon(&snapshot), Some(TopToolbarIcon::Session));
        assert_eq!(settings.icon(&snapshot), Some(TopToolbarIcon::Settings));
        assert_eq!(session.role(), TopToolbarControlRole::Toggle);
        assert_eq!(settings.role(), TopToolbarControlRole::Toggle);
        assert_eq!(session.island(), TopToolbarIsland::History);
        assert_eq!(settings.island(), TopToolbarIsland::History);
        assert!(session.enabled(&snapshot) && settings.enabled(&snapshot));

        // Entries toggle their popover open state and report it as active.
        assert_eq!(
            session.event(&snapshot),
            ToolbarEvent::ToggleSessionPopover(true)
        );
        assert_eq!(
            settings.event(&snapshot),
            ToolbarEvent::ToggleSettingsPopover(true)
        );
        assert!(!session.active(&snapshot) && !settings.active(&snapshot));
        snapshot.session_popover_open = true;
        assert!(session.active(&snapshot));
        assert_eq!(
            session.event(&snapshot),
            ToolbarEvent::ToggleSessionPopover(false)
        );
        snapshot.session_popover_open = false;
        snapshot.settings_popover_open = true;
        assert!(settings.active(&snapshot));
        assert_eq!(
            settings.event(&snapshot),
            ToolbarEvent::ToggleSettingsPopover(false)
        );

        // Minimized and micro strips carry no overflow (and so no entries).
        let mut minimized = self::snapshot();
        minimized.top_minimized = true;
        assert!(
            TopToolbarSpec::build(&minimized, &TopStripPlan::unconstrained())
                .overflow()
                .is_empty()
        );
        let mut micro = self::snapshot();
        micro.top_display_mode = crate::config::TopDisplayMode::Micro;
        assert!(
            TopToolbarSpec::build(&micro, &TopStripPlan::unconstrained())
                .overflow()
                .is_empty()
        );
    }

    #[test]
    fn island_assignment_is_total_and_ordered() {
        let regular = snapshot();
        let mut narrow_plan = TopStripPlan::unconstrained();
        narrow_plan.swatch_count = 0;
        narrow_plan.dropped_tools = vec![Tool::Line];
        narrow_plan.dropped_utilities = vec![TopUtilityButton::Text];

        for plan in [TopStripPlan::unconstrained(), narrow_plan] {
            let spec = TopToolbarSpec::build(&regular, &plan);
            let islands: Vec<_> = spec.strip().iter().map(TopToolbarNode::island).collect();

            // Total: every strip node maps to exactly one non-chrome island.
            assert!(!islands.is_empty());
            assert!(
                islands
                    .iter()
                    .all(|island| *island != TopToolbarIsland::Chrome),
                "chrome controls never appear in the strip: {islands:?}"
            );
            // Ordered: the tools island fully precedes the history island.
            assert!(
                islands.windows(2).all(|pair| pair[0] <= pair[1]),
                "islands must be contiguous and ordered: {islands:?}"
            );
            assert!(islands.contains(&TopToolbarIsland::Tools));
            assert!(islands.contains(&TopToolbarIsland::History));

            // Chrome lane is exactly the chrome island.
            for control in spec.chrome() {
                assert_eq!(control.island(), TopToolbarIsland::Chrome);
            }
        }
    }

    #[test]
    fn contextual_ring_is_owned_by_the_highlight_control_spec() {
        let mut snapshot = snapshot();
        snapshot.highlight_tool_active = true;
        snapshot.highlight_tool_ring_enabled = false;
        let plan = TopStripPlan::unconstrained();

        let spec = TopToolbarSpec::build(&snapshot, &plan);
        assert_eq!(spec.contextual(), [TopToolbarControl::HighlightRing]);
        assert_eq!(
            spec.contextual()[0].event(&snapshot),
            ToolbarEvent::ToggleHighlightToolRing(true)
        );

        let mut dropped = plan.clone();
        dropped.dropped_utilities = vec![TopUtilityButton::Highlight];
        assert!(
            TopToolbarSpec::build(&snapshot, &dropped)
                .contextual()
                .is_empty()
        );

        snapshot
            .resolved_toolbar_items
            .hidden
            .insert(ids::TOP_UTILITY_HIGHLIGHT_RING);
        assert!(
            TopToolbarSpec::build(&snapshot, &plan)
                .contextual()
                .is_empty()
        );
    }

    #[test]
    fn allocation_free_queries_match_the_materialized_spec() {
        let regular = snapshot();
        let mut highlighted = regular.clone();
        highlighted.highlight_tool_active = true;
        let mut minimized = highlighted.clone();
        minimized.top_minimized = true;
        let mut narrow_plan = TopStripPlan::unconstrained();
        narrow_plan.dropped_tools = vec![Tool::Line, Tool::Arrow];
        narrow_plan.dropped_utilities = vec![TopUtilityButton::Text];

        for (snapshot, plan) in [
            (&regular, TopStripPlan::unconstrained()),
            (&highlighted, TopStripPlan::unconstrained()),
            (&highlighted, narrow_plan),
            (&minimized, TopStripPlan::unconstrained()),
        ] {
            let spec = TopToolbarSpec::build(snapshot, &plan);
            assert_eq!(
                TopToolbarSpec::shape_picker_visible(snapshot),
                spec.strip()
                    .contains(&TopToolbarNode::Control(TopToolbarControl::ShapePicker))
            );
            assert_eq!(
                TopToolbarSpec::contextual_highlight_ring_visible(snapshot, &plan),
                !spec.contextual().is_empty()
            );
            assert_eq!(
                TopToolbarSpec::chrome_control_count(snapshot, &plan),
                spec.chrome().len()
            );
            assert_eq!(
                TopToolbarSpec::overflow_control_count(snapshot, &plan),
                spec.overflow().len()
            );
        }
    }

    #[test]
    fn customized_visibility_and_order_flow_through_the_spec() {
        let mut snapshot = snapshot();
        let mut items = ToolbarItemsConfig::default();
        items.set_hidden(ids::TOP_UTILITY_STICKY_NOTE, true);
        assert!(
            items.move_item_to_index(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_ERASER, 0,)
        );
        snapshot.resolved_toolbar_items = items.resolved();

        let spec = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
        let ids = strip_control_ids(&spec);
        let first_tool = ids
            .iter()
            .find(|id| id.starts_with("top.tool."))
            .map(String::as_str);
        assert_eq!(first_tool, Some("top.tool.eraser"));
        assert!(!ids.contains(&"top.utility.sticky-note".to_string()));
    }
}
