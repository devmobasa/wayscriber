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
    pub(crate) show_overflow: bool,
    pub(crate) compact: bool,
}

impl TopStripPlan {
    pub(crate) const MAX_QUICK_COLORS: usize = 8;

    pub(crate) fn unconstrained() -> Self {
        Self {
            swatch_count: Self::MAX_QUICK_COLORS,
            dropped_tools: Vec::new(),
            dropped_utilities: Vec::new(),
            show_overflow: false,
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
        let clear_visible = visible_utilities.contains(&TopUtilityButton::ClearCanvas);
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

        let undo_visible = toolbar_item_visible(snapshot, ids::TOP_UTILITY_UNDO);
        let redo_visible = toolbar_item_visible(snapshot, ids::TOP_UTILITY_REDO);
        if undo_visible || redo_visible {
            strip.push(TopToolbarNode::Divider(TopToolbarDivider::History));
        }
        if undo_visible {
            strip.push(TopToolbarNode::Control(TopToolbarControl::Undo));
        }
        if redo_visible {
            strip.push(TopToolbarNode::Control(TopToolbarControl::Redo));
        }
        if clear_visible {
            strip.push(TopToolbarNode::Control(TopToolbarControl::ClearCanvas));
        }

        let chrome = Self::chrome_controls(snapshot, plan)
            .into_iter()
            .flatten()
            .collect();
        let overflow = Self::overflow_controls(plan).collect();
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
        !snapshot.top_minimized && top_shape_picker_visible(snapshot)
    }

    pub(crate) fn contextual_highlight_ring_visible(
        snapshot: &ToolbarSnapshot,
        plan: &TopStripPlan,
    ) -> bool {
        !snapshot.top_minimized
            && snapshot.layout_mode != ToolbarLayoutMode::Simple
            && snapshot.use_icons
            && snapshot.highlight_tool_active
            && top_highlight_visible(snapshot)
            && top_highlight_ring_visible(snapshot)
            && !plan
                .dropped_utilities
                .contains(&TopUtilityButton::Highlight)
    }

    pub(crate) fn chrome_control_count(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> usize {
        if snapshot.top_minimized {
            return 0;
        }
        Self::chrome_controls(snapshot, plan)
            .into_iter()
            .flatten()
            .count()
    }

    pub(crate) fn overflow_control_count(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> usize {
        if snapshot.top_minimized {
            return 0;
        }
        Self::overflow_controls(plan).count()
    }

    fn chrome_controls(
        snapshot: &ToolbarSnapshot,
        plan: &TopStripPlan,
    ) -> [Option<TopToolbarControl>; 3] {
        [
            toolbar_item_visible(snapshot, ids::TOP_CHROME_PIN).then_some(TopToolbarControl::Pin),
            plan.show_overflow.then_some(TopToolbarControl::Overflow),
            toolbar_item_visible(snapshot, ids::TOP_CHROME_CLOSE)
                .then_some(TopToolbarControl::Minimize),
        ]
    }

    fn overflow_controls(plan: &TopStripPlan) -> impl Iterator<Item = TopToolbarControl> + '_ {
        plan.dropped_tools
            .iter()
            .copied()
            .map(TopToolbarControl::Tool)
            .chain(
                plan.dropped_utilities
                    .iter()
                    .copied()
                    .filter_map(TopToolbarUtility::from_model)
                    .map(TopToolbarControl::Utility),
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarNode {
    Divider(TopToolbarDivider),
    Control(TopToolbarControl),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarDivider {
    Tools,
    Annotations,
    Colors,
    History,
}

impl TopToolbarDivider {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Tools => "top.divider.tools",
            Self::Annotations => "top.divider.annotations",
            Self::Colors => "top.divider.colors",
            Self::History => "top.divider.history",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarControl {
    Restore,
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
}

impl TopToolbarControl {
    pub(crate) fn id(self) -> TopToolbarControlId {
        let id = match self {
            Self::Restore => return TopToolbarControlId::Restore,
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
            Self::ClearCanvas => ToolbarEvent::ClearCanvas,
            Self::Pin => ToolbarEvent::PinTopToolbar(!snapshot.top_pinned),
            Self::Overflow => ToolbarEvent::ToggleTopOverflow(!snapshot.top_overflow_open),
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
            Self::HighlightRing => snapshot.highlight_tool_ring_enabled,
            _ => false,
        }
    }

    pub(crate) fn role(self) -> TopToolbarControlRole {
        match self {
            Self::Restore => TopToolbarControlRole::Restore,
            Self::DragHandle => TopToolbarControlRole::DragHandle,
            Self::QuickColor(_) | Self::CurrentColor => TopToolbarControlRole::Swatch,
            Self::ClearCanvas => TopToolbarControlRole::Destructive,
            Self::ShapePicker
            | Self::Utility(TopToolbarUtility::Highlight)
            | Self::Pin
            | Self::Overflow
            | Self::HighlightRing => TopToolbarControlRole::Toggle,
            Self::Minimize => TopToolbarControlRole::Chrome,
            _ => TopToolbarControlRole::Button,
        }
    }

    pub(crate) fn icon(self, snapshot: &ToolbarSnapshot) -> Option<TopToolbarIcon> {
        Some(match self {
            Self::Restore => TopToolbarIcon::Restore,
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
            Self::Minimize => TopToolbarIcon::Minimize,
            Self::QuickColor(_) | Self::CurrentColor | Self::HighlightRing => return None,
        })
    }

    pub(crate) fn label(self, snapshot: &ToolbarSnapshot) -> Cow<'static, str> {
        match self {
            Self::Restore => Cow::Borrowed("Show toolbar"),
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
            "top.utility.clear-canvas",
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
                "top.divider.history",
            ]
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
        plan.show_overflow = true;
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
                "top.tool.line",
                "top.tool.arrow",
                "top.utility.text",
                "top.utility.sticky-note",
                "top.utility.highlight",
            ]
        );
        assert_eq!(
            chrome_ids(&spec),
            ["top.chrome.pin", "top.chrome.overflow", "top.chrome.close"]
        );
        assert_eq!(
            spec.overflow()[0].event(&snapshot),
            ToolbarEvent::SelectTool(Tool::Line)
        );
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
        narrow_plan.show_overflow = true;
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
