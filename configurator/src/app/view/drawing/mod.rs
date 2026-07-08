mod color;
mod font;

use iced::widget::{button, column, pick_list, row, scrollable, text};
use iced::{Element, Length};

use crate::app::scroll::CONTENT_SCROLL_ID;
use crate::messages::Message;
use crate::models::{
    DragColorOption, DragMouseButton, DragToolField, DragToolOption, EraserModeOption, TextField,
    ToggleField,
};
use wayscriber::config::DragButtonConfig;

use self::color::{drawing_color_block, quick_colors_block};
use self::font::font_controls;
use super::super::search::{SearchArea, TabSearchSummary};
use super::super::state::ConfiguratorApp;
use super::theme;
use super::widgets::{
    labeled_control, labeled_input_with_feedback, toggle_row, validate_f64_range,
    validate_usize_min, validate_usize_range,
};

impl ConfiguratorApp {
    pub(super) fn drawing_tab(&self, search: Option<&TabSearchSummary>) -> Element<'_, Message> {
        let show_color = search.is_none_or(|search| search.area_matches(SearchArea::DrawingColor));
        let show_defaults =
            search.is_none_or(|search| search.area_matches(SearchArea::DrawingDefaults));
        let show_drag =
            search.is_none_or(|search| search.area_matches(SearchArea::DrawingDragTools));
        let show_font = search.is_none_or(|search| search.area_matches(SearchArea::DrawingFont));
        let eraser_mode_pick = pick_list(
            EraserModeOption::list(),
            Some(self.draft.drawing_default_eraser_mode),
            Message::EraserModeChanged,
        );

        let mut column = column![text("Drawing Defaults").size(20)].spacing(12);

        if show_color {
            column = column
                .push(drawing_color_block(self))
                .push(quick_colors_block(self));
        }

        if show_defaults {
            column = column
                .push(
                    row![
                        labeled_input_with_feedback(
                            "Thickness (px)",
                            &self.draft.drawing_default_thickness,
                            &self.defaults.drawing_default_thickness,
                            TextField::DrawingThickness,
                            Some("Range: 1-50 px"),
                            validate_f64_range(&self.draft.drawing_default_thickness, 1.0, 50.0),
                        ),
                        labeled_input_with_feedback(
                            "Font size (pt)",
                            &self.draft.drawing_default_font_size,
                            &self.defaults.drawing_default_font_size,
                            TextField::DrawingFontSize,
                            Some("Range: 8-72 pt"),
                            validate_f64_range(&self.draft.drawing_default_font_size, 8.0, 72.0),
                        ),
                        labeled_input_with_feedback(
                            "Polygon sides",
                            &self.draft.drawing_polygon_sides,
                            &self.defaults.drawing_polygon_sides,
                            TextField::DrawingPolygonSides,
                            Some("Range: 3-12"),
                            validate_usize_range(&self.draft.drawing_polygon_sides, 3, 12),
                        )
                    ]
                    .spacing(12),
                )
                .push(
                    row![
                        labeled_input_with_feedback(
                            "Eraser size (px)",
                            &self.draft.drawing_default_eraser_size,
                            &self.defaults.drawing_default_eraser_size,
                            TextField::DrawingEraserSize,
                            Some("Range: 1-50 px"),
                            validate_f64_range(&self.draft.drawing_default_eraser_size, 1.0, 50.0),
                        ),
                        labeled_control(
                            "Eraser mode",
                            eraser_mode_pick.width(Length::Fill).into(),
                            self.defaults
                                .drawing_default_eraser_mode
                                .label()
                                .to_string(),
                            self.draft.drawing_default_eraser_mode
                                != self.defaults.drawing_default_eraser_mode,
                        )
                    ]
                    .spacing(12),
                )
                .push(
                    row![
                        labeled_input_with_feedback(
                            "Marker opacity (0.05-0.9)",
                            &self.draft.drawing_marker_opacity,
                            &self.defaults.drawing_marker_opacity,
                            TextField::DrawingMarkerOpacity,
                            None,
                            validate_f64_range(&self.draft.drawing_marker_opacity, 0.05, 0.9),
                        ),
                        labeled_input_with_feedback(
                            "Undo stack limit",
                            &self.draft.drawing_undo_stack_limit,
                            &self.defaults.drawing_undo_stack_limit,
                            TextField::DrawingUndoStackLimit,
                            Some("Range: 10-1000"),
                            validate_usize_range(&self.draft.drawing_undo_stack_limit, 10, 1000),
                        )
                    ]
                    .spacing(12),
                )
                .push(
                    row![
                        labeled_input_with_feedback(
                            "Hit-test tolerance (px)",
                            &self.draft.drawing_hit_test_tolerance,
                            &self.defaults.drawing_hit_test_tolerance,
                            TextField::DrawingHitTestTolerance,
                            Some("Range: 1-20 px"),
                            validate_f64_range(&self.draft.drawing_hit_test_tolerance, 1.0, 20.0),
                        ),
                        labeled_input_with_feedback(
                            "Hit-test threshold",
                            &self.draft.drawing_hit_test_linear_threshold,
                            &self.defaults.drawing_hit_test_linear_threshold,
                            TextField::DrawingHitTestThreshold,
                            Some("Minimum: 1"),
                            validate_usize_min(&self.draft.drawing_hit_test_linear_threshold, 1),
                        )
                    ]
                    .spacing(12),
                )
                .push(toggle_row(
                    "Enable text background",
                    self.draft.drawing_text_background_enabled,
                    self.defaults.drawing_text_background_enabled,
                    ToggleField::DrawingTextBackground,
                ))
                .push(toggle_row(
                    "Start shapes filled",
                    self.draft.drawing_default_fill_enabled,
                    self.defaults.drawing_default_fill_enabled,
                    ToggleField::DrawingFillEnabled,
                ));
        }

        if show_drag {
            column = column.push(self.drag_mapping_block(search));
        }

        if show_font {
            column = column.push(font_controls(self));
        }

        let column = column.width(Length::Fill);

        scrollable(column).id(CONTENT_SCROLL_ID).into()
    }

    fn drag_mapping_block(&self, search: Option<&TabSearchSummary>) -> Element<'_, Message> {
        let section_button = |mouse_button: DragMouseButton| {
            button(mouse_button.label())
                .style(if self.active_drawing_drag_button == Some(mouse_button) {
                    theme::Button::Primary
                } else {
                    theme::Button::Secondary
                })
                .on_press(Message::DrawingDragMappingSectionToggled(mouse_button))
        };

        let mut column = column![
            text("Drag Tool Mapping").size(16),
            row![
                section_button(DragMouseButton::Left),
                section_button(DragMouseButton::Right),
                section_button(DragMouseButton::Middle),
            ]
            .spacing(8)
        ]
        .spacing(8);

        for mouse_button in self.visible_drag_mapping_buttons(search) {
            let (current, defaults) = match mouse_button {
                DragMouseButton::Left => (
                    &self.draft.drawing_drag_tools.left,
                    &self.defaults.drawing_drag_tools.left,
                ),
                DragMouseButton::Right => (
                    &self.draft.drawing_drag_tools.right,
                    &self.defaults.drawing_drag_tools.right,
                ),
                DragMouseButton::Middle => (
                    &self.draft.drawing_drag_tools.middle,
                    &self.defaults.drawing_drag_tools.middle,
                ),
            };

            column = column.push(drag_button_controls(mouse_button, current, defaults));
        }

        column.into()
    }

    fn visible_drag_mapping_buttons(
        &self,
        search: Option<&TabSearchSummary>,
    ) -> Vec<DragMouseButton> {
        if search.is_some_and(|search| {
            !search.show_all() && search.area_matches(SearchArea::DrawingDragTools)
        }) {
            return vec![
                DragMouseButton::Left,
                DragMouseButton::Right,
                DragMouseButton::Middle,
            ];
        }

        self.active_drawing_drag_button.into_iter().collect()
    }
}

fn drag_button_controls<'a>(
    button: DragMouseButton,
    current: &DragButtonConfig,
    defaults: &DragButtonConfig,
) -> Element<'a, Message> {
    column![
        row![
            drag_binding_control(button, DragToolField::Drag, current, defaults,),
            drag_binding_control(button, DragToolField::ShiftDrag, current, defaults,)
        ]
        .spacing(12),
        row![
            drag_binding_control(button, DragToolField::CtrlDrag, current, defaults,),
            drag_binding_control(button, DragToolField::CtrlShiftDrag, current, defaults,)
        ]
        .spacing(12),
        row![drag_binding_control(
            button,
            DragToolField::TabDrag,
            current,
            defaults,
        )]
        .spacing(12)
    ]
    .spacing(8)
    .into()
}

fn drag_binding_control<'a>(
    button: DragMouseButton,
    field: DragToolField,
    current: &DragButtonConfig,
    defaults: &DragButtonConfig,
) -> Element<'a, Message> {
    let selected = drag_tool_for_field(current, field);
    let default = drag_tool_for_field(defaults, field);
    let color = drag_color_for_field(current, field);
    let default_color = drag_color_for_field(defaults, field);
    column![
        labeled_control(
            field.label(),
            pick_list(
                DragToolOption::list_for_button(button),
                Some(selected),
                move |option| { Message::DrawingMouseDragToolChanged(button, field, option) }
            )
            .width(Length::Fill)
            .into(),
            default.label().to_string(),
            selected != default,
        ),
        labeled_control(
            "Color",
            pick_list(DragColorOption::list(), Some(color), move |option| {
                Message::DrawingMouseDragColorChanged(button, field, option)
            })
            .width(Length::Fill)
            .into(),
            default_color.label().to_string(),
            color != default_color,
        )
    ]
    .spacing(6)
    .into()
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
    use crate::models::{SearchQuery, TabId};

    #[test]
    fn drag_tool_search_expands_all_drag_button_mappings() {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new("shift");

        let search = app.search_summary();
        let drawing = search.tab(TabId::Drawing).expect("drawing match");

        assert_eq!(app.active_drawing_drag_button, None);
        assert_eq!(
            app.visible_drag_mapping_buttons(Some(drawing)),
            vec![
                DragMouseButton::Left,
                DragMouseButton::Right,
                DragMouseButton::Middle,
            ],
        );
        assert_eq!(app.active_drawing_drag_button, None);
    }
}
