use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Alignment, Element, Length};

use crate::app::scroll::CONTENT_SCROLL_ID;
use crate::app::view::theme;
use crate::messages::Message;
use crate::models::color::rgb_to_hsv;
use crate::models::{
    ColorPickerId, RenderProfileExportOption, RenderProfileMappingSide,
    RenderProfileSelectionOption, RenderProfileTextField,
};
use wayscriber::render_profiles::parse_hex_rgb;

use super::super::search::{SearchArea, TabSearchSummary};
use super::super::state::ConfiguratorApp;
use super::widgets::{color_preview_badge, labeled_control, picker_panel};

impl ConfiguratorApp {
    pub(super) fn render_profiles_tab(
        &self,
        search: Option<&TabSearchSummary>,
    ) -> Element<'_, Message> {
        let profiles = &self.draft.render_profiles;
        let show_all = search.is_none_or(TabSearchSummary::show_all);
        let show_general =
            search.is_none_or(|search| search.area_matches(SearchArea::RenderProfilesGeneral));
        let profile_ids = profiles.profile_ids();
        let active_selection =
            RenderProfileSelectionOption::from_active(&profiles.active, &profile_ids);
        let export_selection = (!profiles.export_profile.is_empty()
            && profile_ids.contains(&profiles.export_profile))
        .then_some(profiles.export_profile.clone());

        let active_picker = pick_list(
            RenderProfileSelectionOption::list(&profile_ids),
            Some(active_selection),
            |selection| Message::RenderProfileActiveChanged(selection.profile_id()),
        )
        .width(Length::Fill);

        let export_picker = pick_list(
            RenderProfileExportOption::list(),
            Some(profiles.export),
            Message::RenderProfileExportChanged,
        )
        .width(Length::Fill);

        let mut content = column![text("Render Profiles").size(20)].spacing(12);

        if show_general || show_all {
            content = content
                .push(
                    row![
                        checkbox(profiles.apply_to_canvas)
                            .label("Preview canvas")
                            .on_toggle(Message::RenderProfileApplyCanvasChanged),
                        checkbox(profiles.apply_to_ui)
                            .label("Preview UI")
                            .on_toggle(Message::RenderProfileApplyUiChanged),
                    ]
                    .spacing(16)
                    .align_y(Alignment::Center),
                )
                .push(labeled_control(
                    "Startup profile",
                    active_picker.into(),
                    self.defaults.render_profiles.active.clone(),
                    profiles.active != self.defaults.render_profiles.active,
                ))
                .push(labeled_control(
                    "Canvas export profile",
                    export_picker.into(),
                    self.defaults.render_profiles.export.label().to_string(),
                    profiles.export != self.defaults.render_profiles.export,
                ));
        }

        if (show_general || show_all) && profiles.export == RenderProfileExportOption::Profile {
            let picker = pick_list(
                profile_ids,
                export_selection,
                Message::RenderProfileExportProfileChanged,
            )
            .width(Length::Fill);
            content = content.push(labeled_control(
                "Named export profile",
                picker.into(),
                self.defaults.render_profiles.export_profile.clone(),
                profiles.export_profile != self.defaults.render_profiles.export_profile,
            ));
        }

        if show_general || show_all {
            content = content.push(button("Add profile").on_press(Message::RenderProfileAdd));
        }

        let indices: Vec<usize> = if show_all {
            (0..profiles.profiles.len()).collect()
        } else {
            search
                .map(TabSearchSummary::render_profile_indices)
                .unwrap_or_default()
                .to_vec()
        };

        for index in indices {
            content = content.push(self.render_profile_section(index, search));
        }

        scrollable(content).id(CONTENT_SCROLL_ID).into()
    }

    fn render_profile_section(
        &self,
        profile_index: usize,
        search: Option<&TabSearchSummary>,
    ) -> Element<'_, Message> {
        let profile = &self.draft.render_profiles.profiles[profile_index];
        let show_all = search.is_none_or(TabSearchSummary::show_all);
        let show_profile_controls =
            search.is_none_or(|search| search.render_profile_controls_visible(profile_index));
        let header = row![
            text(if profile.name.trim().is_empty() {
                "Profile"
            } else {
                profile.name.trim()
            })
            .size(16),
            button("Duplicate").on_press(Message::RenderProfileDuplicate(profile_index)),
            button("Delete")
                .style(theme::Button::Warning)
                .on_press(Message::RenderProfileRemove(profile_index)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let mut mappings = column![].spacing(8);
        let mapping_indices: Vec<usize> = if show_all || show_profile_controls {
            (0..profile.mappings.len()).collect()
        } else {
            search
                .map(TabSearchSummary::render_profile_mapping_indices)
                .unwrap_or_default()
                .iter()
                .filter_map(|(matched_profile, mapping)| {
                    (*matched_profile == profile_index).then_some(*mapping)
                })
                .collect()
        };
        for mapping_index in mapping_indices {
            mappings = mappings.push(self.render_profile_mapping_row(profile_index, mapping_index));
        }

        let mut section = column![header].spacing(10);
        if show_profile_controls {
            section = section.push(
                row![
                    text_input("id", &profile.id)
                        .on_input(move |value| Message::RenderProfileTextChanged(
                            profile_index,
                            RenderProfileTextField::Id,
                            value
                        ))
                        .width(Length::FillPortion(1)),
                    text_input("name", &profile.name)
                        .on_input(move |value| Message::RenderProfileTextChanged(
                            profile_index,
                            RenderProfileTextField::Name,
                            value
                        ))
                        .width(Length::FillPortion(2)),
                ]
                .spacing(8),
            );
        }
        section = section.push(mappings);
        if show_profile_controls {
            section = section.push(
                button("Add mapping").on_press(Message::RenderProfileMappingAdd(profile_index)),
            );
        }

        container(section)
            .padding(12)
            .style(theme::Container::Box)
            .into()
    }

    fn render_profile_mapping_row(
        &self,
        profile_index: usize,
        mapping_index: usize,
    ) -> Element<'_, Message> {
        let mapping = &self.draft.render_profiles.profiles[profile_index].mappings[mapping_index];
        let from = self.render_profile_color_control(
            "From",
            ColorPickerId::RenderProfileMappingFrom(profile_index, mapping_index),
            &mapping.from,
            profile_index,
            mapping_index,
            RenderProfileMappingSide::From,
        );
        let to = self.render_profile_color_control(
            "To",
            ColorPickerId::RenderProfileMappingTo(profile_index, mapping_index),
            &mapping.to,
            profile_index,
            mapping_index,
            RenderProfileMappingSide::To,
        );

        column![
            row![
                from,
                text("->").size(16),
                to,
                button("Remove").on_press(Message::RenderProfileMappingRemove(
                    profile_index,
                    mapping_index
                )),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(6)
        .into()
    }

    fn render_profile_color_control<'a>(
        &'a self,
        label: &'static str,
        id: ColorPickerId,
        value: &'a str,
        profile_index: usize,
        mapping_index: usize,
        side: RenderProfileMappingSide,
    ) -> Element<'a, Message> {
        let hex_value = self
            .color_picker_hex
            .get(&id)
            .map(|value| value.as_str())
            .unwrap_or(value);
        let rgb = render_profile_rgb(hex_value).unwrap_or([0.0, 0.0, 0.0]);
        let preview = render_profile_rgb(hex_value)
            .map(|rgb| iced::Color::from_rgb(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32));
        let (hue, saturation, value_slider) = rgb_to_hsv(rgb);
        let picker = if self.color_picker_open == Some(id) {
            picker_panel(id, hue, saturation, value_slider, rgb, None)
        } else {
            column![].into()
        };

        column![
            row![
                text(label).size(12),
                color_preview_badge(preview),
                text_input("#RRGGBB", hex_value)
                    .on_input(move |value| Message::RenderProfileMappingColorChanged(
                        profile_index,
                        mapping_index,
                        side,
                        value
                    ))
                    .width(Length::Fixed(120.0)),
                button(if self.color_picker_open == Some(id) {
                    "Hide"
                } else {
                    "Pick"
                })
                .on_press(Message::ColorPickerToggled(id)),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            picker,
        ]
        .spacing(6)
        .width(Length::FillPortion(1))
        .into()
    }
}

fn render_profile_rgb(value: &str) -> Option<[f64; 3]> {
    let color = parse_hex_rgb(value)?;
    Some([
        f64::from(color.r) / 255.0,
        f64::from(color.g) / 255.0,
        f64::from(color.b) / 255.0,
    ])
}
