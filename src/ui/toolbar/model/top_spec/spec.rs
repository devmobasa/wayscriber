use super::*;

/// Width-degradation result shared by both top-toolbar frontends.
///
/// The built-in layout layer owns the geometry-dependent planner that fills
/// this value. The semantic specification only consumes the result.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TopStripPlan {
    pub(crate) swatch_count: usize,
    pub(crate) dropped_tools: Vec<Tool>,
    pub(crate) dropped_utilities: Vec<TopUtilityButton>,
    /// Whether the presets island has been dropped for width. Presets are a
    /// non-essential island and yield first under width pressure, before any
    /// tool or utility leaves the strip.
    pub(crate) drop_presets: bool,
    pub(crate) compact: bool,
}

impl TopStripPlan {
    pub(crate) const MAX_QUICK_COLORS: usize = 8;

    pub(crate) fn unconstrained() -> Self {
        Self {
            swatch_count: Self::MAX_QUICK_COLORS,
            dropped_tools: Vec::new(),
            dropped_utilities: Vec::new(),
            drop_presets: false,
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

        // Presets island: pill mode's on-strip home for saved presets. The
        // quick colors that used to sit here render only in the style pill
        // now (M7-C1). Gated on the "Presets" display toggle and dropped
        // first under compact/width pressure, like other non-essential
        // islands (M7-C2).
        if snapshot.show_presets && !plan.compact && !plan.drop_presets {
            let slot_count = snapshot.preset_slot_count.min(snapshot.presets.len());
            strip.extend(
                (0..slot_count)
                    .map(|index| TopToolbarNode::Control(TopToolbarControl::Preset(index))),
            );
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

    /// Chrome island content, in reading order: layout cycle, then About,
    /// then pin, then minimize. The layout cycle sits on the content-adjacent
    /// edge because it reshapes the strip's content, while the window-chrome
    /// trio (About leading among them because it is the only entry that
    /// leaves the overlay) stays against the window edge. All four are
    /// hideable through toolbar customization.
    fn chrome_controls(snapshot: &ToolbarSnapshot) -> [Option<TopToolbarControl>; 4] {
        [
            toolbar_item_visible(snapshot, ids::TOP_CHROME_LAYOUT)
                .then_some(TopToolbarControl::LayoutMode),
            toolbar_item_visible(snapshot, ids::TOP_CHROME_ABOUT)
                .then_some(TopToolbarControl::About),
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
    /// the Canvas/Session/Settings popover entries. Those three popover hosts
    /// are the only surfaces for those functions on the unified top toolbar,
    /// so they are unconditional rather than width-gated. Like the restore
    /// controls they are not hideable items and must always be reachable.
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
                TopToolbarControl::CanvasMenu,
                TopToolbarControl::SessionMenu,
                TopToolbarControl::SettingsMenu,
            ])
    }
}
