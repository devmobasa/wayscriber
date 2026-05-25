mod color;
mod font;

use iced::widget::{button, column, pick_list, row, scrollable, text};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{
    DragColorOption, DragMouseButton, DragToolField, DragToolOption, EraserModeOption, TextField,
    ToggleField,
};
use wayscriber::config::DragButtonConfig;

use self::color::drawing_color_block;
use self::font::font_controls;
use super::super::state::ConfiguratorApp;
use super::theme;
use super::widgets::{
    labeled_control, labeled_input_with_feedback, toggle_row, validate_f64_range,
    validate_usize_min, validate_usize_range,
};

impl ConfiguratorApp {
    pub(super) fn drawing_tab(&self) -> Element<'_, Message> {
        let eraser_mode_pick = pick_list(
            EraserModeOption::list(),
            Some(self.draft.drawing_default_eraser_mode),
            Message::EraserModeChanged,
        );

        let column = column![
            text("Drawing Defaults").size(20),
            drawing_color_block(self),
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
                )
            ]
            .spacing(12),
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
            self.drag_mapping_block(),
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
            font_controls(self),
            toggle_row(
                "Enable text background",
                self.draft.drawing_text_background_enabled,
                self.defaults.drawing_text_background_enabled,
                ToggleField::DrawingTextBackground,
            ),
            toggle_row(
                "Start shapes filled",
                self.draft.drawing_default_fill_enabled,
                self.defaults.drawing_default_fill_enabled,
                ToggleField::DrawingFillEnabled,
            )
        ]
        .spacing(12)
        .width(Length::Fill);

        scrollable(column).into()
    }

    fn drag_mapping_block(&self) -> Element<'_, Message> {
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

        if let Some(mouse_button) = self.active_drawing_drag_button {
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
