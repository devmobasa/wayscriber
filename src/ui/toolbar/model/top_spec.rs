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

mod control;
mod control_meta;
mod spec;

pub(crate) use control::{TopToolbarControl, TopToolbarDivider, TopToolbarIsland, TopToolbarNode};
pub(crate) use control_meta::{
    TopToolbarControlId, TopToolbarControlRole, TopToolbarIcon, TopToolbarUtility, action_tooltip,
    micro_ring_width, preset_slot,
};
pub(crate) use spec::{TopStripPlan, TopToolbarSpec};

#[cfg(test)]
mod tests;
