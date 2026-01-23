use iced::theme;
use iced::widget::{button, checkbox, column, container, pick_list, row, text, text_input};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{BoardBackgroundOption, BoardItemTextField, BoardItemToggleField};

use super::super::super::state::ConfiguratorApp;
use super::super::widgets::color_preview_labeled;

impl ConfiguratorApp {
    pub(super) fn board_item_section(&self, index: usize) -> Element<'_, Message> {
        let Some(item) = self.draft.boards.items.get(index) else {
            return text("Missing board").size(12).into();
        };
        let is_collapsed = self.boards_collapsed.get(index).copied().unwrap_or(false);

        let title = if item.name.trim().is_empty() {
            format!("Board {}", index + 1)
        } else {
            item.name.trim().to_string()
        };
        let id_label = if item.id.trim().is_empty() {
            "id: <unset>".to_string()
        } else {
            format!("id: {}", item.id.trim())
        };

        let header = row![
            column![text(title).size(16), text(id_label).size(12)].spacing(2),
            row![
                button(if is_collapsed { "Expand" } else { "Collapse" })
                    .on_press(Message::BoardsCollapseToggled(index)),
                button("Up").on_press(Message::BoardsMoveItemUp(index)),
                button("Down").on_press(Message::BoardsMoveItemDown(index)),
                button("Duplicate").on_press(Message::BoardsDuplicateItem(index)),
                button("Remove")
                    .style(theme::Button::Secondary)
                    .on_press(Message::BoardsRemoveItem(index)),
            ]
            .spacing(6),
        ]
        .spacing(12)
        .align_items(iced::Alignment::Center);

        if is_collapsed {
            return container(column![header].spacing(8))
                .padding(12)
                .style(theme::Container::Box)
                .into();
        }

        let id_input = labeled_text_input("Board id", &item.id, index, BoardItemTextField::Id);
        let name_input =
            labeled_text_input("Display name", &item.name, index, BoardItemTextField::Name);

        let background_picker = pick_list(
            BoardBackgroundOption::list(),
            Some(item.background_kind),
            move |value| Message::BoardsBackgroundKindChanged(index, value),
        )
        .width(Length::Fill);

        let background_control = labeled_control_row("Background", background_picker.into());

        let background_color_row = if item.background_kind == BoardBackgroundOption::Color {
            color_triplet_row(
                "Background color (0-1)",
                &item.background_color,
                index,
                true,
            )
        } else {
            text("Background color disabled for transparent boards")
                .size(12)
                .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6)))
                .into()
        };

        let pen_override = row![
            checkbox("Override default pen color", item.default_pen_color.enabled,)
                .on_toggle(move |value| Message::BoardsDefaultPenEnabledChanged(index, value)),
        ]
        .spacing(8)
        .align_items(iced::Alignment::Center);

        let pen_color_row = if item.default_pen_color.enabled {
            color_triplet_row(
                "Pen color (0-1)",
                &item.default_pen_color.color,
                index,
                false,
            )
        } else {
            text("Pen color override disabled")
                .size(12)
                .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6)))
                .into()
        };

        let flags_row = row![
            checkbox("Auto-adjust pen", item.auto_adjust_pen).on_toggle(move |value| {
                Message::BoardsItemToggleChanged(index, BoardItemToggleField::AutoAdjustPen, value)
            }),
            checkbox("Persist", item.persist).on_toggle(move |value| {
                Message::BoardsItemToggleChanged(index, BoardItemToggleField::Persist, value)
            }),
            checkbox("Pinned", item.pinned).on_toggle(move |value| {
                Message::BoardsItemToggleChanged(index, BoardItemToggleField::Pinned, value)
            }),
        ]
        .spacing(12)
        .align_items(iced::Alignment::Center);

        let section = column![
            header,
            id_input,
            name_input,
            background_control,
            background_color_row,
            pen_override,
            pen_color_row,
            flags_row,
        ]
        .spacing(10);

        container(section)
            .padding(12)
            .style(theme::Container::Box)
            .into()
    }
}

fn labeled_text_input<'a>(
    label: &'static str,
    value: &'a str,
    index: usize,
    field: BoardItemTextField,
) -> Element<'a, Message> {
    let input = text_input(label, value)
        .on_input(move |val| Message::BoardsItemTextChanged(index, field, val));

    column![row![text(label).size(14)].spacing(8), input]
        .spacing(4)
        .into()
}

fn labeled_control_row<'a>(
    label: &'static str,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    column![row![text(label).size(14)], control]
        .spacing(4)
        .into()
}

fn color_triplet_row<'a>(
    label: &'static str,
    triplet: &'a crate::models::ColorTripletInput,
    index: usize,
    is_background: bool,
) -> Element<'a, Message> {
    let preview = color_preview(triplet);
    let row_inputs = row![
        text_input("R", &triplet.components[0]).on_input(move |val| {
            if is_background {
                Message::BoardsBackgroundColorChanged(index, 0, val)
            } else {
                Message::BoardsDefaultPenColorChanged(index, 0, val)
            }
        }),
        text_input("G", &triplet.components[1]).on_input(move |val| {
            if is_background {
                Message::BoardsBackgroundColorChanged(index, 1, val)
            } else {
                Message::BoardsDefaultPenColorChanged(index, 1, val)
            }
        }),
        text_input("B", &triplet.components[2]).on_input(move |val| {
            if is_background {
                Message::BoardsBackgroundColorChanged(index, 2, val)
            } else {
                Message::BoardsDefaultPenColorChanged(index, 2, val)
            }
        }),
        color_preview_labeled(preview),
    ]
    .spacing(8)
    .align_items(iced::Alignment::Center);

    column![text(label).size(14), row_inputs].spacing(4).into()
}

fn color_preview(triplet: &crate::models::ColorTripletInput) -> Option<iced::Color> {
    let parse = |value: &str| value.trim().parse::<f64>().ok();
    let r = parse(&triplet.components[0])?;
    let g = parse(&triplet.components[1])?;
    let b = parse(&triplet.components[2])?;
    if !(0.0..=1.0).contains(&r) || !(0.0..=1.0).contains(&g) || !(0.0..=1.0).contains(&b) {
        return None;
    }
    Some(iced::Color::from_rgb(r as f32, g as f32, b as f32))
}
