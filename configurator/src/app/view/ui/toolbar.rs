use std::collections::BTreeSet;

use iced::widget::{button, checkbox, column, pick_list, row, scrollable, text};
use iced::{Element, Length};
use wayscriber::config::{
    ResolvedToolbarItems, ToolbarItemCategory, ToolbarItemDefinition, ToolbarItemOrderGroup,
    ToolbarItemSurface, ToolbarItemsConfig, toolbar_item_definitions, toolbar_item_order_group,
};

use crate::app::scroll::CONTENT_SCROLL_ID;
use crate::app::state::ConfiguratorApp;
use crate::messages::Message;
use crate::models::{
    TextField, ToggleField, ToolbarLayoutModeOption, ToolbarOverrideField,
    ToolbarRebindModifierOption,
};

use super::super::widgets::{labeled_control, labeled_input, override_row, toggle_row};

impl ConfiguratorApp {
    pub(super) fn ui_toolbar_tab(&self) -> Element<'_, Message> {
        let toolbar_layout = pick_list(
            ToolbarLayoutModeOption::list(),
            Some(self.draft.ui_toolbar_layout_mode),
            Message::ToolbarLayoutModeChanged,
        );
        let override_mode_pick = pick_list(
            ToolbarLayoutModeOption::list(),
            Some(self.override_mode),
            Message::ToolbarOverrideModeChanged,
        );
        let rebind_modifier = pick_list(
            ToolbarRebindModifierOption::ALL,
            Some(self.draft.ui_toolbar_rebind_modifier),
            Message::ToolbarRebindModifierChanged,
        );
        let overrides = self
            .draft
            .ui_toolbar_mode_overrides
            .for_mode(self.override_mode);

        let column = column![
            text("Toolbar").size(18),
            labeled_control(
                "Layout mode",
                toolbar_layout.width(Length::Fill).into(),
                self.defaults.ui_toolbar_layout_mode.label().to_string(),
                self.draft.ui_toolbar_layout_mode != self.defaults.ui_toolbar_layout_mode,
            ),
            labeled_control(
                "Shortcut edit click",
                rebind_modifier.width(Length::Fill).into(),
                self.defaults.ui_toolbar_rebind_modifier.label().to_string(),
                self.draft.ui_toolbar_rebind_modifier != self.defaults.ui_toolbar_rebind_modifier,
            ),
            toggle_row(
                "Pin top toolbar",
                self.draft.ui_toolbar_top_pinned,
                self.defaults.ui_toolbar_top_pinned,
                ToggleField::UiToolbarTopPinned,
            ),
            toggle_row(
                "Pin side toolbar",
                self.draft.ui_toolbar_side_pinned,
                self.defaults.ui_toolbar_side_pinned,
                ToggleField::UiToolbarSidePinned,
            ),
            toggle_row(
                "Use icon-only buttons",
                self.draft.ui_toolbar_use_icons,
                self.defaults.ui_toolbar_use_icons,
                ToggleField::UiToolbarUseIcons,
            ),
            toggle_row(
                "Show extended colors",
                self.draft.ui_toolbar_show_more_colors,
                self.defaults.ui_toolbar_show_more_colors,
                ToggleField::UiToolbarShowMoreColors,
            ),
            toggle_row(
                "Show presets",
                self.draft.ui_toolbar_show_presets,
                self.defaults.ui_toolbar_show_presets,
                ToggleField::UiToolbarShowPresets,
            ),
            toggle_row(
                "Show actions",
                self.draft.ui_toolbar_show_actions_section,
                self.defaults.ui_toolbar_show_actions_section,
                ToggleField::UiToolbarShowActionsSection,
            ),
            toggle_row(
                "Show zoom actions",
                self.draft.ui_toolbar_show_zoom_actions,
                self.defaults.ui_toolbar_show_zoom_actions,
                ToggleField::UiToolbarShowZoomActions,
            ),
            toggle_row(
                "Show advanced actions",
                self.draft.ui_toolbar_show_actions_advanced,
                self.defaults.ui_toolbar_show_actions_advanced,
                ToggleField::UiToolbarShowActionsAdvanced,
            ),
            toggle_row(
                "Show pages section",
                self.draft.ui_toolbar_show_pages_section,
                self.defaults.ui_toolbar_show_pages_section,
                ToggleField::UiToolbarShowPagesSection,
            ),
            toggle_row(
                "Show boards section",
                self.draft.ui_toolbar_show_boards_section,
                self.defaults.ui_toolbar_show_boards_section,
                ToggleField::UiToolbarShowBoardsSection,
            ),
            toggle_row(
                "Show multi-step undo/redo",
                self.draft.ui_toolbar_show_step_section,
                self.defaults.ui_toolbar_show_step_section,
                ToggleField::UiToolbarShowStepSection,
            ),
            toggle_row(
                "Always show text controls",
                self.draft.ui_toolbar_show_text_controls,
                self.defaults.ui_toolbar_show_text_controls,
                ToggleField::UiToolbarShowTextControls,
            ),
            toggle_row(
                "Show delay sliders",
                self.draft.ui_toolbar_show_delay_sliders,
                self.defaults.ui_toolbar_show_delay_sliders,
                ToggleField::UiToolbarShowDelaySliders,
            ),
            toggle_row(
                "Show marker opacity controls",
                self.draft.ui_toolbar_show_marker_opacity_section,
                self.defaults.ui_toolbar_show_marker_opacity_section,
                ToggleField::UiToolbarShowMarkerOpacitySection,
            ),
            toggle_row(
                "Show tool preview bubble",
                self.draft.ui_toolbar_show_tool_preview,
                self.defaults.ui_toolbar_show_tool_preview,
                ToggleField::UiToolbarShowToolPreview,
            ),
            toggle_row(
                "Show preset action toasts",
                self.draft.ui_toolbar_show_preset_toasts,
                self.defaults.ui_toolbar_show_preset_toasts,
                ToggleField::UiToolbarPresetToasts,
            ),
            toggle_row(
                "Force inline toolbars",
                self.draft.ui_toolbar_force_inline,
                self.defaults.ui_toolbar_force_inline,
                ToggleField::UiToolbarForceInline,
            ),
            text("Mode overrides").size(16),
            row![text("Edit mode:"), override_mode_pick]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            text("Default keeps the mode preset.").size(12),
            override_row(ToolbarOverrideField::ShowPresets, overrides.show_presets),
            override_row(
                ToolbarOverrideField::ShowActionsSection,
                overrides.show_actions_section,
            ),
            override_row(
                ToolbarOverrideField::ShowZoomActions,
                overrides.show_zoom_actions
            ),
            override_row(
                ToolbarOverrideField::ShowActionsAdvanced,
                overrides.show_actions_advanced,
            ),
            override_row(
                ToolbarOverrideField::ShowPagesSection,
                overrides.show_pages_section,
            ),
            override_row(
                ToolbarOverrideField::ShowBoardsSection,
                overrides.show_boards_section,
            ),
            override_row(
                ToolbarOverrideField::ShowStepSection,
                overrides.show_step_section
            ),
            override_row(
                ToolbarOverrideField::ShowTextControls,
                overrides.show_text_controls
            ),
            text("Placement offsets").size(16),
            row![
                labeled_input(
                    "Top offset X (px)",
                    &self.draft.ui_toolbar_top_offset,
                    &self.defaults.ui_toolbar_top_offset,
                    TextField::ToolbarTopOffset,
                ),
                labeled_input(
                    "Top offset Y (px)",
                    &self.draft.ui_toolbar_top_offset_y,
                    &self.defaults.ui_toolbar_top_offset_y,
                    TextField::ToolbarTopOffsetY,
                )
            ]
            .spacing(12),
            row![
                labeled_input(
                    "Side offset Y (px)",
                    &self.draft.ui_toolbar_side_offset,
                    &self.defaults.ui_toolbar_side_offset,
                    TextField::ToolbarSideOffset,
                ),
                labeled_input(
                    "Side offset X (px)",
                    &self.draft.ui_toolbar_side_offset_x,
                    &self.defaults.ui_toolbar_side_offset_x,
                    TextField::ToolbarSideOffsetX,
                )
            ]
            .spacing(12),
        ]
        .spacing(12);

        scrollable(column).id(CONTENT_SCROLL_ID).into()
    }

    pub(super) fn ui_toolbar_visibility_tab(&self) -> Element<'_, Message> {
        let column = column![
            text("Toolbar Visibility").size(18),
            text("Checked items are shown. Uncheck an item to hide it from toolbar sizing, drawing, and hit testing. Existing section toggles and mode overrides can still hide checked items.").size(12),
            toolbar_item_visibility_section(
                &self.draft.ui_toolbar_items,
                &self.defaults.ui_toolbar_items,
            ),
        ]
        .spacing(12);

        scrollable(column).id(CONTENT_SCROLL_ID).into()
    }
}

fn toolbar_item_visibility_section<'a>(
    items: &ToolbarItemsConfig,
    defaults: &ToolbarItemsConfig,
) -> Element<'a, Message> {
    let resolved = items.resolved();
    let default_resolved = defaults.resolved();
    let mut rows = column![text("Items").size(16)].spacing(8);
    let mut current_surface = None;
    let mut current_category = None;

    let unknown_count = resolved.unknown_hidden.len() + resolved.unknown_shown.len();
    if unknown_count > 0 {
        rows = rows.push(
            text(format!(
                "Preserving {} unknown toolbar item id(s) from config.",
                unknown_count
            ))
            .size(12),
        );
    }

    for definition in toolbar_item_definitions_for_display(&resolved) {
        if current_surface != Some(definition.surface) {
            current_surface = Some(definition.surface);
            current_category = None;
            rows = rows.push(text(toolbar_item_surface_label(definition.surface)).size(14));
        }
        if current_category != Some(definition.category) {
            current_category = Some(definition.category);
            rows = rows.push(text(toolbar_item_category_label(definition.category)).size(13));
        }

        rows = rows.push(toolbar_item_visibility_row(
            definition,
            &resolved,
            &default_resolved,
        ));
    }

    rows.into()
}

fn toolbar_item_visibility_row<'a>(
    definition: &ToolbarItemDefinition,
    resolved: &ResolvedToolbarItems,
    defaults: &ResolvedToolbarItems,
) -> Element<'a, Message> {
    let id = definition.id;
    let visible = !resolved.is_hidden(id);
    let default = format!(
        "default: {}",
        visibility_override_label(!defaults.is_hidden(id))
    );
    let order_group = configurator_order_group(definition);
    let order = order_group.and_then(|group| {
        let index = resolved.order.index_of(group, id)?;
        let len = resolved.order.ordered_ids(group).len();
        Some((group, index, index > 0, index + 1 < len))
    });

    let mut cells = row![
        checkbox(visible)
            .label(definition.label)
            .on_toggle(move |value| Message::ToolbarItemVisibilityChanged(id, value)),
        text(definition.id.as_str()).size(12).width(Length::Fill),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);
    if let Some((group, _, can_move_up, can_move_down)) = order {
        let up = if can_move_up {
            button(text("^")).on_press(Message::ToolbarItemMoveRequested(group, id, -1))
        } else {
            button(text("^"))
        };
        let down = if can_move_down {
            button(text("v")).on_press(Message::ToolbarItemMoveRequested(group, id, 1))
        } else {
            button(text("v"))
        };
        cells = cells
            .push(up)
            .push(down)
            .push(button(text("Reset")).on_press(Message::ToolbarItemOrderReset(group)));
    }
    cells.push(text(default).size(12)).into()
}

fn toolbar_item_definitions_for_display(
    resolved: &ResolvedToolbarItems,
) -> Vec<&'static ToolbarItemDefinition> {
    let mut result = Vec::new();
    let mut emitted = BTreeSet::new();
    let mut emitted_groups = BTreeSet::new();

    for definition in toolbar_item_definitions() {
        if definition.id == wayscriber::config::toolbar_item_ids::SIDE_GROUP_SETTINGS
            || definition.id == wayscriber::config::toolbar_item_ids::TOP_CHROME_OVERFLOW
        {
            continue;
        }
        if let Some(group) = configurator_order_group(definition) {
            if emitted_groups.insert(group) {
                for id in resolved.order.ordered_ids(group) {
                    if let Some(ordered_definition) = toolbar_item_definitions()
                        .iter()
                        .find(|candidate| candidate.id == *id)
                        && configurator_order_group(ordered_definition) == Some(group)
                    {
                        result.push(ordered_definition);
                        emitted.insert(ordered_definition.id);
                    }
                }
            }
            if emitted.contains(&definition.id) {
                continue;
            }
        }
        result.push(definition);
        emitted.insert(definition.id);
    }

    result
}

fn configurator_order_group(definition: &ToolbarItemDefinition) -> Option<ToolbarItemOrderGroup> {
    let group = toolbar_item_order_group(definition)?;
    matches!(
        group,
        ToolbarItemOrderGroup::TopTools
            | ToolbarItemOrderGroup::TopControls
            | ToolbarItemOrderGroup::SideSections
    )
    .then_some(group)
}

fn visibility_override_label(visible: bool) -> &'static str {
    if visible { "shown" } else { "hidden" }
}

fn toolbar_item_surface_label(surface: ToolbarItemSurface) -> &'static str {
    match surface {
        ToolbarItemSurface::Top => "Top toolbar",
        ToolbarItemSurface::Side => "Side toolbar",
    }
}

fn toolbar_item_category_label(category: ToolbarItemCategory) -> &'static str {
    match category {
        ToolbarItemCategory::Chrome => "Toolbar controls",
        ToolbarItemCategory::Tool => "Tools",
        ToolbarItemCategory::Utility => "Utilities",
        ToolbarItemCategory::Group => "Sections",
        ToolbarItemCategory::Action => "Actions",
        ToolbarItemCategory::Page => "Pages",
        ToolbarItemCategory::Board => "Boards",
        ToolbarItemCategory::Setting => "Settings",
        ToolbarItemCategory::Session => "Sessions",
        ToolbarItemCategory::ToolOption => "Tool options",
    }
}
