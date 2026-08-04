use relm4::gtk;

use gtk::prelude::*;

use wayscriber::config::DragButtonConfig;

use crate::messages::Message;
use crate::models::{DragColorOption, DragMouseButton, DragToolField, DragToolOption, TabId};

use super::super::super::search::{AppSearchSummary, SearchArea};
use super::super::super::state::ConfiguratorApp;
use super::super::PageBuilder;
use super::{DRAG_BUTTONS, DRAG_FIELDS, boxed_list, section_combo_row, set_visible_if_changed};

pub(super) fn build(page: &mut PageBuilder) {
    page.group_in_area("Drag tool mapping", SearchArea::DrawingDragTools);

    let switcher = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::Start)
        .css_classes(["linked"])
        .build();
    for button in DRAG_BUTTONS {
        let toggle = gtk::Button::with_label(button.label());
        {
            let sender = page.sender();
            toggle.connect_clicked(move |_| {
                sender.input(Message::DrawingDragMappingSectionToggled(button));
            });
        }
        switcher.append(&toggle);
        page.bind(move |app, _summary| {
            let open = app.active_drawing_drag_button == Some(button);
            if toggle.has_css_class("suggested-action") != open {
                if open {
                    toggle.add_css_class("suggested-action");
                } else {
                    toggle.remove_css_class("suggested-action");
                }
            }
        });
    }
    page.custom(&switcher);

    for button in DRAG_BUTTONS {
        build_drag_button_section(page, button);
    }
}

fn build_drag_button_section(page: &mut PageBuilder, button: DragMouseButton) {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(12)
        .build();
    section.append(
        &gtk::Label::builder()
            .label(button.label())
            .xalign(0.0)
            .css_classes(["heading"])
            .build(),
    );
    let list = boxed_list();
    section.append(&list);
    page.custom(&section);
    page.bind(move |app, summary| {
        set_visible_if_changed(&section, drag_section_visible(app, summary, button));
    });

    let tools = DragToolOption::list_for_button(button);
    let tool_labels: Vec<String> = tools
        .iter()
        .map(|option| option.label().to_string())
        .collect();
    let colors = DragColorOption::list();
    let color_labels: Vec<String> = colors
        .iter()
        .map(|option| option.label().to_string())
        .collect();

    for field in DRAG_FIELDS {
        section_combo_row(
            page,
            &list,
            field.label(),
            tools.clone(),
            tool_labels.clone(),
            move |app| drag_tool_for_field(drag_button_config(app, button), field),
            move |option| Message::DrawingMouseDragToolChanged(button, field, option),
        );
        section_combo_row(
            page,
            &list,
            &format!("{} color", field.label()),
            colors.clone(),
            color_labels.clone(),
            move |app| drag_color_for_field(drag_button_config(app, button), field),
            move |option| Message::DrawingMouseDragColorChanged(button, field, option),
        );
    }
}

/// Mirrors the old view's `visible_drag_mapping_buttons`: a search that
/// matched the drag area on its own opens every button, otherwise only the
/// section the user picked is open.
fn drag_section_visible(
    app: &ConfiguratorApp,
    summary: &AppSearchSummary,
    button: DragMouseButton,
) -> bool {
    let matched_by_search = summary.tab(TabId::Drawing).is_some_and(|search| {
        !search.show_all() && search.area_matches(SearchArea::DrawingDragTools)
    });
    matched_by_search || app.active_drawing_drag_button == Some(button)
}

fn drag_button_config(app: &ConfiguratorApp, button: DragMouseButton) -> &DragButtonConfig {
    match button {
        DragMouseButton::Left => &app.draft.drawing_drag_tools.left,
        DragMouseButton::Right => &app.draft.drawing_drag_tools.right,
        DragMouseButton::Middle => &app.draft.drawing_drag_tools.middle,
    }
}

fn drag_tool_for_field(config: &DragButtonConfig, field: DragToolField) -> DragToolOption {
    match field {
        DragToolField::Drag => DragToolOption::from_drag_tool(config.drag_tool),
        DragToolField::ShiftDrag => DragToolOption::from_drag_tool(config.shift_drag_tool),
        DragToolField::CtrlDrag => DragToolOption::from_drag_tool(config.ctrl_drag_tool),
        DragToolField::CtrlShiftDrag => DragToolOption::from_drag_tool(config.ctrl_shift_drag_tool),
        DragToolField::TabDrag => DragToolOption::from_drag_tool(config.tab_drag_tool),
    }
}

fn drag_color_for_field(config: &DragButtonConfig, field: DragToolField) -> DragColorOption {
    match field {
        DragToolField::Drag => DragColorOption::from_color(config.drag_color.as_ref()),
        DragToolField::ShiftDrag => DragColorOption::from_color(config.shift_drag_color.as_ref()),
        DragToolField::CtrlDrag => DragColorOption::from_color(config.ctrl_drag_color.as_ref()),
        DragToolField::CtrlShiftDrag => {
            DragColorOption::from_color(config.ctrl_shift_drag_color.as_ref())
        }
        DragToolField::TabDrag => DragColorOption::from_color(config.tab_drag_color.as_ref()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SearchQuery;
    #[test]
    fn drag_sections_all_open_when_search_matches_the_drag_area() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new("shift");
        let summary = app.search_summary();

        assert_eq!(app.active_drawing_drag_button, None);
        for button in DRAG_BUTTONS {
            assert!(drag_section_visible(&app, &summary, button));
        }
    }

    #[test]
    fn drag_sections_follow_the_open_button_without_a_search() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        app.active_drawing_drag_button = Some(DragMouseButton::Right);
        let summary = app.search_summary();

        assert!(drag_section_visible(&app, &summary, DragMouseButton::Right));
        assert!(!drag_section_visible(&app, &summary, DragMouseButton::Left));
    }
}
