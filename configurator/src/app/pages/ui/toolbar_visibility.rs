use super::*;

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

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
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
