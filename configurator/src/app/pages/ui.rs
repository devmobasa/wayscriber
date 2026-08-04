//! UI page: general chrome settings plus the seven UI sub-pages.
//!
//! This page is bigger than one preferences page, so it composes its own
//! root: a "General UI" preferences page above a stack holding one
//! preferences page per [`UiTabId`]. Which sub-page shows is model state —
//! the switcher only sends [`Message::UiTabSelected`], and a binding drives
//! the visible child from `active_ui_tab`, so a deep link and the search
//! realignment that moves that field both land on the right sub-page.

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;
use gtk::glib;

use wayscriber::config::{
    ResolvedToolbarItems, ToolbarItemCategory, ToolbarItemDefinition, ToolbarItemId,
    ToolbarItemOrderGroup, ToolbarItemSurface, ToolbarItemsConfig, toolbar_item_definitions,
    toolbar_item_ids, toolbar_item_order_group,
};

use crate::messages::Message;
use crate::models::color::parse_quad_values;
use crate::models::{
    ColorPickerId, InputHudModeOption, InputHudPositionOption, OverrideOption,
    PresenterToolBehaviorOption, PresenterToolbarModeOption, ReducedMotionOption,
    StatusPositionOption, TabId, TextField, ToggleField, ToolbarLayoutModeOption,
    ToolbarOverrideField, ToolbarRebindModifierOption, ToolbarSideLayoutOption, UiTabId,
    UiThemeOption, ZoomChipDisplayOption,
};

use super::super::search::SearchArea;
use super::super::state::ConfiguratorApp;
use super::color_rows::{ResolvedColor, color_row};
use super::{Binding, BuiltPage, PageBuilder, validate_u32_range};

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut bindings: Vec<Binding> = Vec::new();

    let general = build_general(sender);
    bindings.extend(general.bindings);
    let general_widget = general.widget;
    // Natural height inside the shared scroller: the general section and the
    // active sub-page scroll together the way the Iced column did.
    general_widget.set_vexpand(false);
    {
        let widget = general_widget.clone();
        bindings.push(Box::new(move |_app, summary| {
            let visible = !summary.is_active()
                || summary
                    .tab(TabId::Ui)
                    .is_some_and(|tab| tab.area_matches(SearchArea::UiGeneral));
            if widget.is_visible() != visible {
                widget.set_visible(visible);
            }
        }));
    }

    // Homogeneous sizing would make every sub-page as tall as the tallest
    // one (toolbar visibility lists every item), so measure the visible
    // child only.
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vhomogeneous(false)
        .vexpand(true)
        .build();

    let mut stack_pages: Vec<(UiTabId, gtk::StackPage)> = Vec::new();
    for tab in UiTabId::ALL {
        let built = build_ui_tab(sender, tab);
        bindings.extend(built.bindings);
        built.widget.set_vexpand(false);
        let stack_page = stack.add_titled(&built.widget, Some(tab.title()), tab.title());
        stack_pages.push((tab, stack_page));
    }
    {
        let sender = sender.clone();
        stack.connect_visible_child_name_notify(move |stack| {
            let Some(name) = stack.visible_child_name() else {
                return;
            };
            if let Some(tab) = UiTabId::ALL.into_iter().find(|tab| tab.title() == name) {
                sender.input(Message::UiTabSelected(tab));
            }
        });
    }

    let switcher = gtk::StackSwitcher::builder()
        .stack(&stack)
        .halign(gtk::Align::Center)
        .build();
    // Seven sub-tabs are wider than a narrow window: scrolling the row keeps
    // the switcher off the page's minimum width, so the sections below stay
    // centered instead of being pushed sideways.
    let switcher_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .margin_top(6)
        .margin_start(12)
        .margin_end(12)
        .child(&switcher)
        .build();

    {
        let stack = stack.clone();
        let switcher = switcher_scroll.clone();
        bindings.push(Box::new(move |app, summary| {
            let ui_summary = summary.tab(TabId::Ui);
            let searching = summary.is_active();
            let mut any_visible = false;
            for (tab, stack_page) in &stack_pages {
                // A hidden `GtkStackPage` also drops its switcher button,
                // which is how the Iced view narrowed the sub-tab row.
                let visible =
                    !searching || ui_summary.is_some_and(|summary| summary.ui_tab_visible(*tab));
                if stack_page.is_visible() != visible {
                    stack_page.set_visible(visible);
                }
                any_visible |= visible;
            }
            if switcher.is_visible() != any_visible {
                switcher.set_visible(any_visible);
            }
            if stack.is_visible() != any_visible {
                stack.set_visible(any_visible);
            }

            let name = app.active_ui_tab.title();
            if stack.visible_child_name().as_deref() != Some(name) {
                stack.set_visible_child_name(name);
            }
        }));
    }

    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.append(&general_widget);
    content.append(&switcher_scroll);
    content.append(&stack);

    // The preferences pages carry scrollers of their own, whose minimum
    // height is nearly zero, so a viewport on the default minimum policy
    // would squeeze them instead of scrolling. Scrolling the natural height
    // gives each section its full size and one scrollbar for the page.
    let viewport = gtk::Viewport::builder()
        .vscroll_policy(gtk::ScrollablePolicy::Natural)
        .child(&content)
        .build();
    let root = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&viewport)
        .build();

    BuiltPage {
        widget: root.upcast(),
        bindings,
    }
}

fn build_ui_tab(sender: &ComponentSender<ConfiguratorApp>, tab: UiTabId) -> BuiltPage {
    match tab {
        UiTabId::Toolbar => build_toolbar(sender),
        UiTabId::ToolbarVisibility => build_toolbar_visibility(sender),
        UiTabId::StatusBar => build_status_bar(sender),
        UiTabId::HelpOverlay => build_help_overlay(sender),
        UiTabId::ClickHighlight => build_click_highlight(sender),
        UiTabId::InputHud => build_input_hud(sender),
        UiTabId::PresenterMode => build_presenter_mode(sender),
    }
}

// ---- General ---------------------------------------------------------------

fn build_general(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (themes, theme_labels) = options(UiThemeOption::list(), |value| value.label());
    let (motions, motion_labels) = options(ReducedMotionOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);
    page.group_in_area("General UI", SearchArea::UiGeneral)
        .combo_row(
            "Theme",
            "\"Auto\" currently uses the dark theme; \"Light\" takes effect as overlay surfaces adopt the runtime theme.",
            themes,
            theme_labels,
            |app| app.draft.ui_theme,
            Message::UiThemeChanged,
        )
        .combo_row(
            "Reduced motion",
            "\"On\" disables UI animations. \"Auto\" follows the system preference in a future release and keeps full motion for now.",
            motions,
            motion_labels,
            |app| app.draft.ui_reduced_motion,
            Message::UiReducedMotionChanged,
        )
        .entry_row(
            "Preferred output (GNOME fallback)",
            |app| app.draft.ui_preferred_output.clone(),
            |value| Message::TextChanged(TextField::UiPreferredOutput, value),
        )
        .switch_row(
            "Use fullscreen xdg fallback",
            "Applies to the GNOME xdg-shell fallback overlay.",
            |app| app.draft.ui_xdg_fullscreen,
            |value| Message::ToggleChanged(ToggleField::UiXdgFullscreen, value),
        )
        .switch_row(
            "Keep open on xdg focus loss",
            "",
            |app| app.draft.ui_xdg_keep_on_focus_loss,
            |value| Message::ToggleChanged(ToggleField::UiXdgKeepOnFocusLoss, value),
        )
        .switch_row(
            "Enable context menu",
            "",
            |app| app.draft.ui_context_menu_enabled,
            |value| Message::ToggleChanged(ToggleField::UiContextMenuEnabled, value),
        )
        .switch_row(
            "Show capabilities warning toast",
            "",
            |app| app.draft.ui_show_capabilities_warning,
            |value| Message::ToggleChanged(ToggleField::UiShowCapabilitiesWarning, value),
        )
        .entry_row(
            "Command palette toast (ms)",
            |app| app.draft.ui_command_palette_toast_duration_ms.clone(),
            |value| Message::TextChanged(TextField::UiCommandPaletteToastDurationMs, value),
        );

    page.finish()
}

// ---- Toolbar ---------------------------------------------------------------

fn build_toolbar(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (layout_modes, layout_labels) =
        options(ToolbarLayoutModeOption::list(), |value| value.label());
    let (side_layouts, side_layout_labels) =
        options(ToolbarSideLayoutOption::list(), |value| value.label());
    let (zoom_chips, zoom_chip_labels) =
        options(ZoomChipDisplayOption::list(), |value| value.label());
    let (rebinds, rebind_labels) = options(ToolbarRebindModifierOption::ALL.to_vec(), |value| {
        value.label()
    });
    let (override_modes, override_mode_labels) =
        options(ToolbarLayoutModeOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group("Toolbar").custom(&note(
        "These settings are configured defaults. Toolbar pins, position, display form, item visibility/order, pane state, and board pins changed in the overlay are saved separately as runtime preferences.",
    ));

    page.group("Layout")
        .combo_row(
            "Layout mode",
            "",
            layout_modes,
            layout_labels,
            |app| app.draft.ui_toolbar_layout_mode,
            Message::ToolbarLayoutModeChanged,
        )
        .combo_row(
            "Side layout",
            "Pill (the default) retires the side palette: drawing properties live in the top strip's style pill, canvas management in the status HUD and board picker, and Session/Settings in popovers on the top strip's overflow menu. Panel is the legacy escape hatch restoring the classic side palette; it is deprecated and planned for removal one release after the pill default.",
            side_layouts,
            side_layout_labels,
            |app| app.draft.ui_toolbar_side_layout,
            Message::ToolbarSideLayoutChanged,
        )
        .combo_row(
            "Zoom chip",
            "",
            zoom_chips,
            zoom_chip_labels,
            |app| app.draft.ui_toolbar_zoom_chip_display,
            Message::ToolbarZoomChipDisplayChanged,
        )
        .switch_row(
            "Show zoom chip",
            "",
            |app| app.draft.ui_toolbar_show_zoom_chip,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowZoomChip, value),
        )
        .combo_row(
            "Shortcut edit click",
            "",
            rebinds,
            rebind_labels,
            |app| app.draft.ui_toolbar_rebind_modifier,
            Message::ToolbarRebindModifierChanged,
        )
        .switch_row(
            "Configured default: pin top toolbar",
            "",
            |app| app.draft.ui_toolbar_top_pinned,
            |value| Message::ToggleChanged(ToggleField::UiToolbarTopPinned, value),
        )
        .switch_row(
            "Configured default: pin side toolbar",
            "",
            |app| app.draft.ui_toolbar_side_pinned,
            |value| Message::ToggleChanged(ToggleField::UiToolbarSidePinned, value),
        )
        .switch_row(
            "Use icon-only buttons",
            "",
            |app| app.draft.ui_toolbar_use_icons,
            |value| Message::ToggleChanged(ToggleField::UiToolbarUseIcons, value),
        );

    page.group("Sections")
        .switch_row(
            "Show extended colors",
            "",
            |app| app.draft.ui_toolbar_show_more_colors,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowMoreColors, value),
        )
        .switch_row(
            "Show presets",
            "",
            |app| app.draft.ui_toolbar_show_presets,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowPresets, value),
        )
        .switch_row(
            "Show actions",
            "",
            |app| app.draft.ui_toolbar_show_actions_section,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowActionsSection, value),
        )
        .switch_row(
            "Show zoom actions",
            "",
            |app| app.draft.ui_toolbar_show_zoom_actions,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowZoomActions, value),
        )
        .switch_row(
            "Show advanced actions",
            "",
            |app| app.draft.ui_toolbar_show_actions_advanced,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowActionsAdvanced, value),
        )
        .switch_row(
            "Show pages section",
            "",
            |app| app.draft.ui_toolbar_show_pages_section,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowPagesSection, value),
        )
        .switch_row(
            "Show boards section",
            "",
            |app| app.draft.ui_toolbar_show_boards_section,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowBoardsSection, value),
        )
        .switch_row(
            "Show multi-step undo/redo",
            "",
            |app| app.draft.ui_toolbar_show_step_section,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowStepSection, value),
        )
        .switch_row(
            "Always show text controls",
            "",
            |app| app.draft.ui_toolbar_show_text_controls,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowTextControls, value),
        )
        .switch_row(
            "Show delay sliders",
            "",
            |app| app.draft.ui_toolbar_show_delay_sliders,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowDelaySliders, value),
        )
        .switch_row(
            "Show marker opacity controls",
            "",
            |app| app.draft.ui_toolbar_show_marker_opacity_section,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowMarkerOpacitySection, value),
        )
        .switch_row(
            "Show tool preview bubble",
            "",
            |app| app.draft.ui_toolbar_show_tool_preview,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowToolPreview, value),
        )
        .switch_row(
            "Show preset action toasts",
            "",
            |app| app.draft.ui_toolbar_show_preset_toasts,
            |value| Message::ToggleChanged(ToggleField::UiToolbarPresetToasts, value),
        )
        .switch_row(
            "Force inline toolbars",
            "",
            |app| app.draft.ui_toolbar_force_inline,
            |value| Message::ToggleChanged(ToggleField::UiToolbarForceInline, value),
        );

    page.group("Mode overrides").combo_row(
        "Edit mode",
        "Overrides below apply to the mode selected here; \"Default\" keeps the mode preset.",
        override_modes,
        override_mode_labels,
        |app| app.override_mode,
        Message::ToolbarOverrideModeChanged,
    );
    for field in [
        ToolbarOverrideField::ShowPresets,
        ToolbarOverrideField::ShowActionsSection,
        ToolbarOverrideField::ShowZoomActions,
        ToolbarOverrideField::ShowActionsAdvanced,
        ToolbarOverrideField::ShowPagesSection,
        ToolbarOverrideField::ShowBoardsSection,
        ToolbarOverrideField::ShowStepSection,
        ToolbarOverrideField::ShowTextControls,
    ] {
        let (values, labels) = options(OverrideOption::list(), |value| value.label());
        page.combo_row(
            field.label(),
            "",
            values,
            labels,
            move |app| toolbar_override(app, field),
            move |value| Message::ToolbarOverrideChanged(field, value),
        );
    }

    page.group("Placement offsets")
        .entry_row(
            "Top offset X (px)",
            |app| app.draft.ui_toolbar_top_offset.clone(),
            |value| Message::TextChanged(TextField::ToolbarTopOffset, value),
        )
        .entry_row(
            "Top offset Y (px)",
            |app| app.draft.ui_toolbar_top_offset_y.clone(),
            |value| Message::TextChanged(TextField::ToolbarTopOffsetY, value),
        )
        .entry_row(
            "Side offset Y (px)",
            |app| app.draft.ui_toolbar_side_offset.clone(),
            |value| Message::TextChanged(TextField::ToolbarSideOffset, value),
        )
        .entry_row(
            "Side offset X (px)",
            |app| app.draft.ui_toolbar_side_offset_x.clone(),
            |value| Message::TextChanged(TextField::ToolbarSideOffsetX, value),
        )
        .custom(&note(
            "Configured defaults. Dragging a toolbar in the overlay saves that position as a runtime preference; editing a value here takes over from the saved drag.",
        ));

    page.finish()
}

fn toolbar_override(app: &ConfiguratorApp, field: ToolbarOverrideField) -> OverrideOption {
    let overrides = app
        .draft
        .ui_toolbar_mode_overrides
        .for_mode(app.override_mode);
    match field {
        ToolbarOverrideField::ShowPresets => overrides.show_presets,
        ToolbarOverrideField::ShowActionsSection => overrides.show_actions_section,
        ToolbarOverrideField::ShowActionsAdvanced => overrides.show_actions_advanced,
        ToolbarOverrideField::ShowZoomActions => overrides.show_zoom_actions,
        ToolbarOverrideField::ShowPagesSection => overrides.show_pages_section,
        ToolbarOverrideField::ShowBoardsSection => overrides.show_boards_section,
        ToolbarOverrideField::ShowStepSection => overrides.show_step_section,
        ToolbarOverrideField::ShowTextControls => overrides.show_text_controls,
    }
}

// ---- Toolbar visibility ----------------------------------------------------

/// One preferences group of item rows: a surface/category batch, or one of
/// the three order groups the configurator can reorder.
struct ItemSection {
    title: String,
    order_group: Option<ToolbarItemOrderGroup>,
    definitions: Vec<&'static ToolbarItemDefinition>,
}

struct ItemRow {
    id: ToolbarItemId,
    row: adw::SwitchRow,
    /// Kept so the refresh can write the switch with the handler blocked.
    /// `ToolbarItemVisibilityChanged` is not a plain setter: it pins an
    /// explicit visibility entry for section ids, so a refresh reporting the
    /// resolved value back would add entries to a config nobody edited — and
    /// the first refresh, which lifts every visible row off its built-in
    /// `false`, would do it to every section on startup.
    handler: glib::SignalHandlerId,
    move_buttons: Option<(gtk::Button, gtk::Button)>,
}

/// The widgets one [`ItemSection`] refreshes, kept so a single binding can
/// resolve the item config once for the whole page.
struct SectionWidgets {
    order_group: Option<ToolbarItemOrderGroup>,
    built_in_order: Vec<ToolbarItemId>,
    list: gtk::ListBox,
    reset: Option<gtk::Button>,
    rows: Vec<ItemRow>,
}

fn build_toolbar_visibility(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let built_in = ToolbarItemsConfig::default().resolved();
    let mut page = PageBuilder::new(sender, TabId::Ui);

    // Shown by the binding only while the config carries ids this build does
    // not know.
    let unknown_notice = note("");
    unknown_notice.set_visible(false);
    page.group("Toolbar Visibility")
        .custom(&note(
            "These are configured visibility defaults. Overlay customizations are stored separately as runtime preferences. Enabled items are shown; section toggles and mode overrides can still hide them.",
        ))
        .custom(&unknown_notice);

    let mut sections: Vec<SectionWidgets> = Vec::new();
    for section in item_sections(&built_in) {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();

        let mut rows: Vec<ItemRow> = Vec::new();
        for definition in &section.definitions {
            let id = definition.id;
            let row = adw::SwitchRow::builder()
                .title(definition.label)
                .subtitle(format!(
                    "{} - built-in default: {}",
                    id.as_str(),
                    visibility_label(!built_in.is_hidden(id))
                ))
                .build();
            let handler = {
                let sender = page.sender();
                row.connect_active_notify(move |row| {
                    sender.input(Message::ToolbarItemVisibilityChanged(id, row.is_active()));
                })
            };

            let mut move_buttons = None;
            if let Some(group) = section.order_group {
                let up = move_button(&page.sender(), group, id, -1);
                let down = move_button(&page.sender(), group, id, 1);
                row.add_suffix(&up);
                row.add_suffix(&down);
                move_buttons = Some((up, down));
            }

            list.append(&row);
            rows.push(ItemRow {
                id,
                row,
                handler,
                move_buttons,
            });
        }

        page.group(&section.title).custom(&list);

        let mut reset = None;
        if let Some(group) = section.order_group {
            let button = gtk::Button::builder()
                .label("Restore built-in order")
                .halign(gtk::Align::End)
                .margin_top(6)
                .build();
            {
                let sender = page.sender();
                button.connect_clicked(move |_| {
                    sender.input(Message::ToolbarItemOrderReset(group));
                });
            }
            page.custom(&button);
            reset = Some(button);
        }

        sections.push(SectionWidgets {
            order_group: section.order_group,
            built_in_order: section
                .order_group
                .map(|group| built_in.order.ordered_ids(group).to_vec())
                .unwrap_or_default(),
            list,
            reset,
            rows,
        });
    }

    // One binding for the whole page: resolving the item config allocates,
    // and doing it per row would repeat that work a hundred times a refresh.
    page.bind(move |app, _summary| {
        let resolved = app.draft.ui_toolbar_items.resolved();

        let unknown = resolved.unknown_hidden.len() + resolved.unknown_shown.len();
        let notice_text = if unknown > 0 {
            format!("Preserving {unknown} unknown toolbar item id(s) from config.")
        } else {
            String::new()
        };
        if unknown_notice.text() != notice_text {
            unknown_notice.set_text(&notice_text);
        }
        if unknown_notice.is_visible() != (unknown > 0) {
            unknown_notice.set_visible(unknown > 0);
        }

        for section in &sections {
            for item in &section.rows {
                let visible = !resolved.is_hidden(item.id);
                if item.row.is_active() != visible {
                    item.row.block_signal(&item.handler);
                    item.row.set_active(visible);
                    item.row.unblock_signal(&item.handler);
                }

                let (Some(group), Some((up, down))) = (section.order_group, &item.move_buttons)
                else {
                    continue;
                };
                let index = resolved.order.index_of(group, item.id);
                let length = resolved.order.ordered_ids(group).len();
                let can_move_up = index.is_some_and(|index| index > 0);
                let can_move_down = index.is_some_and(|index| index + 1 < length);
                if up.is_sensitive() != can_move_up {
                    up.set_sensitive(can_move_up);
                }
                if down.is_sensitive() != can_move_down {
                    down.set_sensitive(can_move_down);
                }
            }

            let Some(group) = section.order_group else {
                continue;
            };
            let ordered = resolved.order.ordered_ids(group);
            let desired: Vec<ToolbarItemId> = ordered
                .iter()
                .copied()
                .filter(|id| section.rows.iter().any(|item| item.id == *id))
                .collect();
            if current_row_order(section) != desired {
                for item in &section.rows {
                    section.list.remove(&item.row);
                }
                for id in &desired {
                    if let Some(item) = section.rows.iter().find(|item| item.id == *id) {
                        section.list.append(&item.row);
                    }
                }
            }

            if let Some(reset) = &section.reset {
                // The Iced view only offered the restore action once the
                // order left the built-in one; insensitive keeps the button
                // in place instead of making the section jump.
                let restorable = ordered != section.built_in_order;
                if reset.is_sensitive() != restorable {
                    reset.set_sensitive(restorable);
                }
            }
        }
    });

    page.finish()
}

/// Item rows grouped the way the Iced list read: by toolbar surface and
/// category, with each reorderable order group in a section of its own so it
/// can carry the move buttons and its restore action.
fn item_sections(built_in: &ResolvedToolbarItems) -> Vec<ItemSection> {
    let mut sections: Vec<ItemSection> = Vec::new();
    for definition in toolbar_item_definitions() {
        if definition.id == toolbar_item_ids::SIDE_GROUP_SETTINGS
            || definition.id == toolbar_item_ids::TOP_CHROME_OVERFLOW
        {
            continue;
        }

        let order_group = configurator_order_group(definition);
        let title = match order_group {
            Some((_, label)) => format!(
                "{}: {} (reorderable)",
                surface_label(definition.surface),
                label
            ),
            None => format!(
                "{}: {}",
                surface_label(definition.surface),
                category_label(definition.category)
            ),
        };

        match sections.iter_mut().find(|section| section.title == title) {
            Some(section) => section.definitions.push(definition),
            None => sections.push(ItemSection {
                title,
                order_group: order_group.map(|(group, _)| group),
                definitions: vec![definition],
            }),
        }
    }

    // Reorderable sections start in the built-in order; the binding puts them
    // in the configured order on the first refresh.
    for section in &mut sections {
        let Some(group) = section.order_group else {
            continue;
        };
        let order = built_in.order.ordered_ids(group);
        section.definitions.sort_by_key(|definition| {
            order
                .iter()
                .position(|id| *id == definition.id)
                .unwrap_or(usize::MAX)
        });
    }

    sections
}

/// The order groups this page can reorder, with their section label. The
/// remaining groups keep the order the config resolves them in.
fn configurator_order_group(
    definition: &ToolbarItemDefinition,
) -> Option<(ToolbarItemOrderGroup, &'static str)> {
    match toolbar_item_order_group(definition)? {
        ToolbarItemOrderGroup::TopTools => Some((ToolbarItemOrderGroup::TopTools, "Tools")),
        ToolbarItemOrderGroup::TopControls => {
            Some((ToolbarItemOrderGroup::TopControls, "Controls"))
        }
        ToolbarItemOrderGroup::SideSections => {
            Some((ToolbarItemOrderGroup::SideSections, "Sections"))
        }
        _ => None,
    }
}

/// The ids currently laid out in a section, read back from the list itself so
/// the reorder pass needs no state of its own.
fn current_row_order(section: &SectionWidgets) -> Vec<ToolbarItemId> {
    let mut order = Vec::with_capacity(section.rows.len());
    let mut child = section.list.first_child();
    while let Some(widget) = child {
        if let Some(item) = section
            .rows
            .iter()
            .find(|item| item.row.upcast_ref::<gtk::Widget>() == &widget)
        {
            order.push(item.id);
        }
        child = widget.next_sibling();
    }
    order
}

fn move_button(
    sender: &ComponentSender<ConfiguratorApp>,
    group: ToolbarItemOrderGroup,
    id: ToolbarItemId,
    delta: isize,
) -> gtk::Button {
    let (icon, tooltip) = if delta < 0 {
        ("go-up-symbolic", "Move up")
    } else {
        ("go-down-symbolic", "Move down")
    };
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    let sender = sender.clone();
    button.connect_clicked(move |_| {
        sender.input(Message::ToolbarItemMoveRequested(group, id, delta));
    });
    button
}

fn visibility_label(visible: bool) -> &'static str {
    if visible { "shown" } else { "hidden" }
}

fn surface_label(surface: ToolbarItemSurface) -> &'static str {
    match surface {
        ToolbarItemSurface::Top => "Top toolbar",
        ToolbarItemSurface::Side => "Side toolbar",
    }
}

fn category_label(category: ToolbarItemCategory) -> &'static str {
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

// ---- Status bar ------------------------------------------------------------

fn build_status_bar(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (positions, position_labels) = options(StatusPositionOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group("Status Bar")
        .switch_row(
            "Show status bar",
            "",
            |app| app.draft.ui_show_status_bar,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusBar, value),
        )
        .switch_row(
            "Clickable status bar segments",
            "",
            |app| app.draft.ui_status_bar_interactive,
            |value| Message::ToggleChanged(ToggleField::UiStatusBarInteractive, value),
        );

    page.group("Contents")
        .switch_row(
            "Show active output",
            "",
            |app| app.draft.ui_active_output_badge,
            |value| Message::ToggleChanged(ToggleField::UiActiveOutputBadge, value),
        )
        .switch_row(
            "Show selection dimensions",
            "",
            |app| app.draft.ui_show_status_selection_info,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusSelectionInfo, value),
        )
        .switch_row(
            "Show board label",
            "",
            |app| app.draft.ui_show_status_board_badge,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusBoardBadge, value),
        )
        .switch_row(
            "Show page counter",
            "",
            |app| app.draft.ui_show_status_page_badge,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusPageBadge, value),
        )
        .switch_row(
            "Show current color",
            "",
            |app| app.draft.ui_show_status_color,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusColor, value),
        )
        .switch_row(
            "Show active tool",
            "",
            |app| app.draft.ui_show_status_tool,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusTool, value),
        )
        .switch_row(
            "Show tool size",
            "",
            |app| app.draft.ui_show_status_size,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusSize, value),
        )
        .switch_row(
            "Show context indicators",
            "",
            |app| app.draft.ui_show_status_context_indicators,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusContextIndicators, value),
        )
        .switch_row(
            "Show toolbar hint while toolbars are hidden",
            "",
            |app| app.draft.ui_show_toolbar_hint,
            |value| Message::ToggleChanged(ToggleField::UiShowToolbarHint, value),
        )
        .switch_row(
            "Show Help shortcut",
            "",
            |app| app.draft.ui_show_status_help,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusHelp, value),
        )
        .switch_row(
            "Show About and version",
            "",
            |app| app.draft.ui_show_status_about,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusAbout, value),
        );

    page.group("Additional Badges")
        .switch_row(
            "Show board/page badge",
            "",
            |app| app.draft.ui_show_floating_badge,
            |value| Message::ToggleChanged(ToggleField::UiShowFloatingBadge, value),
        )
        .switch_row(
            "Also show badge with status bar",
            "",
            |app| app.draft.ui_show_page_badge_with_status_bar,
            |value| Message::ToggleChanged(ToggleField::UiShowPageBadgeWithStatusBar, value),
        )
        .switch_row(
            "Show frozen badge",
            "",
            |app| app.draft.ui_show_frozen_badge,
            |value| Message::ToggleChanged(ToggleField::UiShowFrozenBadge, value),
        )
        .combo_row(
            "Status bar position",
            "",
            positions,
            position_labels,
            |app| app.draft.ui_status_position,
            Message::StatusPositionChanged,
        );

    page.group("Status Bar Style");
    color_row(
        &mut page,
        "Background (hex)",
        ColorPickerId::StatusBarBg,
        |app| quad_color(&app.draft.status_bar_bg_color.components),
    );
    color_row(
        &mut page,
        "Text (hex)",
        ColorPickerId::StatusBarText,
        |app| quad_color(&app.draft.status_bar_text_color.components),
    );
    page.entry_row(
        "Font size",
        |app| app.draft.status_font_size.clone(),
        |value| Message::TextChanged(TextField::StatusFontSize, value),
    )
    .entry_row(
        "Padding",
        |app| app.draft.status_padding.clone(),
        |value| Message::TextChanged(TextField::StatusPadding, value),
    )
    .entry_row(
        "Dot radius",
        |app| app.draft.status_dot_radius.clone(),
        |value| Message::TextChanged(TextField::StatusDotRadius, value),
    );

    page.finish()
}

// ---- Help overlay ----------------------------------------------------------

fn build_help_overlay(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group("Help Overlay").switch_row(
        "Filter sections by enabled features",
        "",
        |app| app.draft.help_context_filter,
        |value| Message::ToggleChanged(ToggleField::UiHelpOverlayContextFilter, value),
    );

    page.group("Help Overlay Style");
    color_row(
        &mut page,
        "Background (hex)",
        ColorPickerId::HelpBg,
        |app| quad_color(&app.draft.help_bg_color.components),
    );
    color_row(
        &mut page,
        "Border (hex)",
        ColorPickerId::HelpBorder,
        |app| quad_color(&app.draft.help_border_color.components),
    );
    color_row(&mut page, "Text (hex)", ColorPickerId::HelpText, |app| {
        quad_color(&app.draft.help_text_color.components)
    });
    page.entry_row(
        "Font family",
        |app| app.draft.help_font_family.clone(),
        |value| Message::TextChanged(TextField::HelpFontFamily, value),
    )
    .entry_row(
        "Font size",
        |app| app.draft.help_font_size.clone(),
        |value| Message::TextChanged(TextField::HelpFontSize, value),
    )
    .entry_row(
        "Line height",
        |app| app.draft.help_line_height.clone(),
        |value| Message::TextChanged(TextField::HelpLineHeight, value),
    )
    .entry_row(
        "Padding",
        |app| app.draft.help_padding.clone(),
        |value| Message::TextChanged(TextField::HelpPadding, value),
    )
    .entry_row(
        "Border width",
        |app| app.draft.help_border_width.clone(),
        |value| Message::TextChanged(TextField::HelpBorderWidth, value),
    );

    page.finish()
}

// ---- Click highlight -------------------------------------------------------

fn build_click_highlight(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group("Click Highlight")
        .switch_row(
            "Enable click highlight",
            "",
            |app| app.draft.click_highlight_enabled,
            |value| Message::ToggleChanged(ToggleField::UiClickHighlightEnabled, value),
        )
        .switch_row(
            "Show ring while highlight tool is active",
            "",
            |app| app.draft.click_highlight_show_on_highlight_tool,
            |value| Message::ToggleChanged(ToggleField::UiClickHighlightShowOnHighlightTool, value),
        )
        .switch_row(
            "Link highlight color to current pen",
            "",
            |app| app.draft.click_highlight_use_pen_color,
            |value| Message::ToggleChanged(ToggleField::UiClickHighlightUsePenColor, value),
        )
        .switch_row(
            "Force on when entering light mode",
            "",
            |app| app.draft.click_highlight_force_in_light_mode,
            |value| Message::ToggleChanged(ToggleField::UiClickHighlightForceInLightMode, value),
        );

    page.group("Ring")
        .entry_row_validated(
            "Radius",
            |app| app.draft.click_highlight_radius.clone(),
            |value| Message::TextChanged(TextField::HighlightRadius, value),
            |app| validate_f64_range(&app.draft.click_highlight_radius, 16.0, 160.0),
        )
        .entry_row_validated(
            "Outline thickness",
            |app| app.draft.click_highlight_outline_thickness.clone(),
            |value| Message::TextChanged(TextField::HighlightOutlineThickness, value),
            |app| validate_f64_range(&app.draft.click_highlight_outline_thickness, 1.0, 12.0),
        )
        .entry_row_validated(
            "Duration (ms)",
            |app| app.draft.click_highlight_duration_ms.clone(),
            |value| Message::TextChanged(TextField::HighlightDurationMs, value),
            |app| validate_u32_range(&app.draft.click_highlight_duration_ms, 150, 1500),
        );

    page.group("Colors");
    color_row(
        &mut page,
        "Fill (hex)",
        ColorPickerId::HighlightFill,
        |app| quad_color(&app.draft.click_highlight_fill_color.components),
    );
    color_row(
        &mut page,
        "Outline (hex)",
        ColorPickerId::HighlightOutline,
        |app| quad_color(&app.draft.click_highlight_outline_color.components),
    );

    page.finish()
}

// ---- Input HUD -------------------------------------------------------------

fn build_input_hud(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (modes, mode_labels) = options(InputHudModeOption::list(), |value| value.label());
    let (positions, position_labels) =
        options(InputHudPositionOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group("Input HUD")
        .custom(&note(
            "Show a live row of keystroke and click chips for demos and screencasts.",
        ))
        .switch_row(
            "Enable input HUD",
            "",
            |app| app.draft.input_hud_enabled,
            |value| Message::ToggleChanged(ToggleField::UiInputHudEnabled, value),
        )
        .combo_row(
            "Input source",
            "\"Overlay only\" shows what Wayscriber itself receives. \"System-wide\" also shows input that goes to other apps; it needs a build with the input-monitor feature and read access to /dev/input (usually `input` group membership), and it sees every keystroke on the seat - including passwords typed elsewhere.",
            modes,
            mode_labels,
            |app| app.draft.input_hud_mode,
            Message::InputHudModeChanged,
        )
        .combo_row(
            "Screen position",
            "",
            positions,
            position_labels,
            |app| app.draft.input_hud_position,
            Message::InputHudPositionChanged,
        )
        .switch_row(
            "Show mouse buttons and scroll",
            "",
            |app| app.draft.input_hud_show_mouse,
            |value| Message::ToggleChanged(ToggleField::UiInputHudShowMouse, value),
        )
        .switch_row(
            "Show bare modifier taps",
            "",
            |app| app.draft.input_hud_show_bare_modifiers,
            |value| Message::ToggleChanged(ToggleField::UiInputHudShowBareModifiers, value),
        )
        .switch_row(
            "Combine repeats into a counter",
            "",
            |app| app.draft.input_hud_combine_repeats,
            |value| Message::ToggleChanged(ToggleField::UiInputHudCombineRepeats, value),
        );

    page.group("Chips")
        .entry_row_validated(
            "Hold (ms)",
            |app| app.draft.input_hud_display_ms.clone(),
            |value| Message::TextChanged(TextField::InputHudDisplayMs, value),
            |app| validate_u32_range(&app.draft.input_hud_display_ms, 200, 30_000),
        )
        .entry_row_validated(
            "Fade (ms)",
            |app| app.draft.input_hud_fade_ms.clone(),
            |value| Message::TextChanged(TextField::InputHudFadeMs, value),
            |app| validate_u32_range(&app.draft.input_hud_fade_ms, 0, 5_000),
        )
        .entry_row_validated(
            "Max chips",
            |app| app.draft.input_hud_max_entries.clone(),
            |value| Message::TextChanged(TextField::InputHudMaxEntries, value),
            |app| validate_u32_range(&app.draft.input_hud_max_entries, 1, 16),
        )
        .entry_row_validated(
            "Font size",
            |app| app.draft.input_hud_font_size.clone(),
            |value| Message::TextChanged(TextField::InputHudFontSize, value),
            |app| validate_f64_range(&app.draft.input_hud_font_size, 6.0, 72.0),
        );

    page.finish()
}

// ---- Presenter mode --------------------------------------------------------

fn build_presenter_mode(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (toolbar_modes, toolbar_mode_labels) =
        options(PresenterToolbarModeOption::list(), |value| value.label());
    let (behaviors, behavior_labels) =
        options(PresenterToolBehaviorOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group("Presenter Mode")
        .custom(&note("Customize what presenter mode changes when toggled."))
        .switch_row(
            "Hide status bar",
            "",
            |app| app.draft.presenter_hide_status_bar,
            |value| Message::ToggleChanged(ToggleField::PresenterHideStatusBar, value),
        )
        .switch_row(
            "Hide toolbars",
            "",
            |app| app.draft.presenter_hide_toolbars,
            |value| Message::ToggleChanged(ToggleField::PresenterHideToolbars, value),
        )
        .combo_row(
            "Top toolbar while presenting",
            "",
            toolbar_modes,
            toolbar_mode_labels,
            |app| app.draft.presenter_toolbar_mode,
            Message::PresenterToolbarModeChanged,
        )
        .switch_row(
            "Hide tool preview",
            "",
            |app| app.draft.presenter_hide_tool_preview,
            |value| Message::ToggleChanged(ToggleField::PresenterHideToolPreview, value),
        )
        .switch_row(
            "Close help overlay on entry",
            "",
            |app| app.draft.presenter_close_help_overlay,
            |value| Message::ToggleChanged(ToggleField::PresenterCloseHelpOverlay, value),
        )
        .switch_row(
            "Force click highlights on",
            "",
            |app| app.draft.presenter_enable_click_highlight,
            |value| Message::ToggleChanged(ToggleField::PresenterEnableClickHighlight, value),
        )
        .switch_row(
            "Force input HUD on",
            "",
            |app| app.draft.presenter_enable_input_hud,
            |value| Message::ToggleChanged(ToggleField::PresenterEnableInputHud, value),
        )
        .combo_row(
            "Tool behavior",
            "",
            behaviors,
            behavior_labels,
            |app| app.draft.presenter_tool_behavior,
            Message::PresenterToolBehaviorChanged,
        )
        .switch_row(
            "Show enter/exit toast",
            "",
            |app| app.draft.presenter_show_toast,
            |value| Message::ToggleChanged(ToggleField::PresenterShowToast, value),
        );

    page.finish()
}

// ---- Shared helpers --------------------------------------------------------

/// A combo row's values with their labels, in one call.
fn options<O: Copy>(values: Vec<O>, label: impl Fn(&O) -> &'static str) -> (Vec<O>, Vec<String>) {
    let labels = values
        .iter()
        .map(|value| label(value).to_string())
        .collect();
    (values, labels)
}

/// Explanatory text a row cannot carry: `AdwEntryRow` has no subtitle, and a
/// preferences group renders a plain widget under its rows.
fn note(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .margin_top(6)
        .css_classes(["caption", "dim-label"])
        .build()
}

fn quad_color(components: &[String; 4]) -> ResolvedColor {
    let values = parse_quad_values(components);
    Some((values[0], values[1], values[2], values[3]))
}

/// Error text for a decimal field constrained to `min..=max`, `None` while
/// the input is acceptable.
fn validate_f64_range(value: &str, min: f64, max: f64) -> Option<String> {
    match value.trim().parse::<f64>() {
        Ok(parsed) if parsed.is_finite() && (min..=max).contains(&parsed) => None,
        _ => Some(format!("Enter a number between {min} and {max}.")),
    }
}
