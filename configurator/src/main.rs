use std::{path::PathBuf, sync::Arc};

use hyprmarker::config::{
    Config, StatusPosition, enums::ColorSpec, keybindings::KeybindingsConfig,
};
use hyprmarker::util::name_to_color;
use iced::alignment::Horizontal;
use iced::border::Radius;
use iced::executor;
use iced::theme::{self, Theme};
use iced::widget::container::Appearance;
use iced::widget::{
    Column, Row, Space, button, checkbox, column, container, horizontal_rule, pick_list, row,
    scrollable, text, text_input,
};
use iced::{Application, Background, Border, Command, Element, Length, Settings, Size};

fn main() -> iced::Result {
    let mut settings = Settings::default();
    settings.window.size = Size::new(960.0, 640.0);
    settings.window.resizable = true;
    settings.window.decorations = true;
    ConfiguratorApp::run(settings)
}

#[derive(Debug)]
struct ConfiguratorApp {
    draft: ConfigDraft,
    baseline: ConfigDraft,
    status: StatusMessage,
    active_tab: TabId,
    is_loading: bool,
    is_saving: bool,
    is_dirty: bool,
    config_path: Option<PathBuf>,
    last_backup_path: Option<PathBuf>,
}

impl Application for ConfiguratorApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let default_config = Config::default();
        let baseline = ConfigDraft::from_config(&default_config);
        let config_path = Config::get_config_path().ok();
        let app = Self {
            draft: baseline.clone(),
            baseline,
            status: StatusMessage::info("Loading configuration..."),
            active_tab: TabId::Drawing,
            is_loading: true,
            is_saving: false,
            is_dirty: false,
            config_path,
            last_backup_path: None,
        };

        let command = Command::batch(vec![Command::perform(
            load_config_from_disk(),
            Message::ConfigLoaded,
        )]);

        (app, command)
    }

    fn title(&self) -> String {
        "Hyprmarker Configurator (Iced)".to_string()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, message: Message) -> Command<Self::Message> {
        match message {
            Message::ConfigLoaded(result) => {
                self.is_loading = false;
                match result {
                    Ok(config) => {
                        let draft = ConfigDraft::from_config(config.as_ref());
                        self.draft = draft.clone();
                        self.baseline = draft;
                        self.is_dirty = false;
                        self.status = StatusMessage::success("Configuration loaded from disk.");
                    }
                    Err(err) => {
                        self.status =
                            StatusMessage::error(format!("Failed to load config from disk: {err}"));
                    }
                }
            }
            Message::ReloadRequested => {
                if !self.is_loading && !self.is_saving {
                    self.is_loading = true;
                    self.status = StatusMessage::info("Reloading configuration...");
                    return Command::perform(load_config_from_disk(), Message::ConfigLoaded);
                }
            }
            Message::ResetToDefaults => {
                if !self.is_loading {
                    let defaults = Config::default();
                    self.draft = ConfigDraft::from_config(&defaults);
                    self.status = StatusMessage::info("Loaded default configuration (not saved).");
                    self.refresh_dirty_flag();
                }
            }
            Message::SaveRequested => {
                if self.is_saving {
                    return Command::none();
                }

                match self.draft.to_config() {
                    Ok(mut config) => {
                        config.validate_and_clamp();
                        self.is_saving = true;
                        self.status = StatusMessage::info("Saving configuration...");
                        return Command::perform(save_config_to_disk(config), Message::ConfigSaved);
                    }
                    Err(errors) => {
                        let message = errors
                            .into_iter()
                            .map(|err| format!("{}: {}", err.field, err.message))
                            .collect::<Vec<_>>()
                            .join("\n");
                        self.status = StatusMessage::error(format!(
                            "Cannot save due to validation errors:\n{message}"
                        ));
                    }
                }
            }
            Message::ConfigSaved(result) => {
                self.is_saving = false;
                match result {
                    Ok((backup, saved_config)) => {
                        let draft = ConfigDraft::from_config(saved_config.as_ref());
                        self.last_backup_path = backup.clone();
                        self.draft = draft.clone();
                        self.baseline = draft;
                        self.is_dirty = false;
                        let mut msg = "Configuration saved successfully.".to_string();
                        if let Some(path) = backup {
                            msg.push_str(&format!("\nBackup created at {}", path.display()));
                        }
                        self.status = StatusMessage::success(msg);
                    }
                    Err(err) => {
                        self.status =
                            StatusMessage::error(format!("Failed to save configuration: {err}"));
                    }
                }
            }
            Message::TabSelected(tab) => {
                self.active_tab = tab;
            }
            Message::ToggleChanged(field, value) => {
                self.status = StatusMessage::idle();
                self.draft.set_toggle(field, value);
                self.refresh_dirty_flag();
            }
            Message::TextChanged(field, value) => {
                self.status = StatusMessage::idle();
                self.draft.set_text(field, value);
                self.refresh_dirty_flag();
            }
            Message::TripletChanged(field, index, value) => {
                self.status = StatusMessage::idle();
                self.draft.set_triplet(field, index, value);
                self.refresh_dirty_flag();
            }
            Message::QuadChanged(field, index, value) => {
                self.status = StatusMessage::idle();
                self.draft.set_quad(field, index, value);
                self.refresh_dirty_flag();
            }
            Message::ColorModeChanged(mode) => {
                self.status = StatusMessage::idle();
                self.draft.drawing_color.mode = mode;
                if matches!(mode, ColorMode::Named) {
                    if self.draft.drawing_color.name.trim().is_empty() {
                        self.draft.drawing_color.selected_named = NamedColorOption::Red;
                        self.draft.drawing_color.name = self
                            .draft
                            .drawing_color
                            .selected_named
                            .as_value()
                            .to_string();
                    } else {
                        self.draft.drawing_color.update_named_from_current();
                    }
                }
                self.refresh_dirty_flag();
            }
            Message::NamedColorSelected(option) => {
                self.status = StatusMessage::idle();
                self.draft.drawing_color.selected_named = option;
                if option != NamedColorOption::Custom {
                    self.draft.drawing_color.name = option.as_value().to_string();
                }
                self.refresh_dirty_flag();
            }
            Message::StatusPositionChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.ui_status_position = option;
                self.refresh_dirty_flag();
            }
            Message::BoardModeChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.board_default_mode = option;
                self.refresh_dirty_flag();
            }
            Message::BufferCountChanged(count) => {
                self.status = StatusMessage::idle();
                self.draft.performance_buffer_count = count;
                self.refresh_dirty_flag();
            }
            Message::FontStyleOptionSelected(option) => {
                self.status = StatusMessage::idle();
                self.draft.drawing_font_style_option = option;
                if option != FontStyleOption::Custom {
                    self.draft.drawing_font_style = option.canonical_value().to_string();
                }
                self.refresh_dirty_flag();
            }
            Message::FontWeightOptionSelected(option) => {
                self.status = StatusMessage::idle();
                self.draft.drawing_font_weight_option = option;
                if option != FontWeightOption::Custom {
                    self.draft.drawing_font_weight = option.canonical_value().to_string();
                }
                self.refresh_dirty_flag();
            }
            Message::KeybindingChanged(field, value) => {
                self.status = StatusMessage::idle();
                self.draft.keybindings.set(field, value);
                self.refresh_dirty_flag();
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let header = self.header_view();
        let content = self.tab_view();
        let footer = self.footer_view();

        column![header, content, footer]
            .spacing(12)
            .padding(16)
            .into()
    }
}

impl ConfiguratorApp {
    fn header_view(&self) -> Element<'_, Message> {
        let reload_button = if self.is_loading || self.is_saving {
            button("Reload").style(theme::Button::Secondary)
        } else {
            button("Reload")
                .on_press(Message::ReloadRequested)
                .style(theme::Button::Secondary)
        };

        let defaults_button = if self.is_loading {
            button("Defaults").style(theme::Button::Secondary)
        } else {
            button("Defaults")
                .on_press(Message::ResetToDefaults)
                .style(theme::Button::Secondary)
        };

        let save_button = if self.is_loading || self.is_saving {
            button("Save").style(theme::Button::Primary)
        } else {
            button("Save")
                .on_press(Message::SaveRequested)
                .style(theme::Button::Primary)
        };

        let mut toolbar = Row::new()
            .spacing(12)
            .align_items(iced::Alignment::Center)
            .push(reload_button)
            .push(defaults_button)
            .push(save_button);

        if self.is_saving {
            toolbar = toolbar.push(text("Saving...").size(16));
        } else if self.is_loading {
            toolbar = toolbar.push(text("Loading...").size(16));
        } else if self.is_dirty {
            toolbar = toolbar.push(
                text("Unsaved changes")
                    .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.72, 0.2))),
            );
        } else {
            toolbar = toolbar.push(
                text("All changes saved")
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.8, 0.6))),
            );
        }

        let banner: Element<'_, Message> = match &self.status {
            StatusMessage::Idle => Space::new(Length::Shrink, Length::Shrink).into(),
            StatusMessage::Info(message) => container(text(message))
                .padding(8)
                .style(theme::Container::Box)
                .into(),
            StatusMessage::Success(message) => container(
                text(message).style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.9, 0.6))),
            )
            .padding(8)
            .style(theme::Container::Box)
            .into(),
            StatusMessage::Error(message) => container(
                text(message).style(theme::Text::Color(iced::Color::from_rgb(1.0, 0.5, 0.5))),
            )
            .padding(8)
            .style(theme::Container::Box)
            .into(),
        };

        column![toolbar, banner].spacing(8).into()
    }

    fn tab_view(&self) -> Element<'_, Message> {
        let tab_bar = TabId::ALL.iter().fold(
            Row::new().spacing(8).align_items(iced::Alignment::Center),
            |row, tab| {
                let label = tab.title();
                let button = button(label)
                    .padding([6, 12])
                    .on_press(Message::TabSelected(*tab))
                    .style(if *tab == self.active_tab {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    });
                row.push(button)
            },
        );

        let content: Element<'_, Message> = match self.active_tab {
            TabId::Drawing => self.drawing_tab(),
            TabId::Arrow => self.arrow_tab(),
            TabId::Performance => self.performance_tab(),
            TabId::Ui => self.ui_tab(),
            TabId::Board => self.board_tab(),
            TabId::Capture => self.capture_tab(),
            TabId::Keybindings => self.keybindings_tab(),
        };

        column![tab_bar, horizontal_rule(2), content]
            .spacing(12)
            .into()
    }

    fn footer_view(&self) -> Element<'_, Message> {
        let mut info = Column::new().spacing(4);

        if let Some(path) = &self.config_path {
            info = info.push(text(format!("Config path: {}", path.display())).size(14));
        }
        if let Some(path) = &self.last_backup_path {
            info = info.push(text(format!("Last backup: {}", path.display())).size(14));
        }

        info.into()
    }

    fn drawing_tab(&self) -> Element<'_, Message> {
        let color_mode_picker = Row::new()
            .spacing(12)
            .push(
                button(if self.draft.drawing_color.mode == ColorMode::Named {
                    "Named Color"
                } else {
                    "Named Color"
                })
                .on_press(Message::ColorModeChanged(ColorMode::Named)),
            )
            .push(
                button(if self.draft.drawing_color.mode == ColorMode::Rgb {
                    "RGB Color"
                } else {
                    "RGB Color"
                })
                .on_press(Message::ColorModeChanged(ColorMode::Rgb)),
            );

        let color_section: Element<'_, Message> = match self.draft.drawing_color.mode {
            ColorMode::Named => {
                let picker = pick_list(
                    NamedColorOption::list(),
                    Some(self.draft.drawing_color.selected_named),
                    Message::NamedColorSelected,
                )
                .width(Length::Fixed(160.0));

                let picker_row = row![
                    picker,
                    color_preview_badge(self.draft.drawing_color.preview_color()),
                ]
                .spacing(8)
                .align_items(iced::Alignment::Center);

                let mut column = Column::new().spacing(8).push(picker_row);

                if self.draft.drawing_color.selected_named_is_custom() {
                    column = column.push(
                        text_input("Custom color name", &self.draft.drawing_color.name)
                            .on_input(|value| {
                                Message::TextChanged(TextField::DrawingColorName, value)
                            })
                            .width(Length::Fill),
                    );

                    if self.draft.drawing_color.preview_color().is_none()
                        && !self.draft.drawing_color.name.trim().is_empty()
                    {
                        column = column.push(
                            text("Unknown color name")
                                .size(12)
                                .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.6, 0.6))),
                        );
                    }
                }

                column.into()
            }
            ColorMode::Rgb => {
                let rgb_inputs = row![
                    text_input("R (0-255)", &self.draft.drawing_color.rgb[0]).on_input(|value| {
                        Message::TripletChanged(TripletField::DrawingColorRgb, 0, value)
                    }),
                    text_input("G (0-255)", &self.draft.drawing_color.rgb[1]).on_input(|value| {
                        Message::TripletChanged(TripletField::DrawingColorRgb, 1, value)
                    }),
                    text_input("B (0-255)", &self.draft.drawing_color.rgb[2]).on_input(|value| {
                        Message::TripletChanged(TripletField::DrawingColorRgb, 2, value)
                    }),
                    color_preview_badge(self.draft.drawing_color.preview_color()),
                ]
                .spacing(8)
                .align_items(iced::Alignment::Center);

                let mut column = Column::new().spacing(8).push(rgb_inputs);

                if self.draft.drawing_color.preview_color().is_none()
                    && self
                        .draft
                        .drawing_color
                        .rgb
                        .iter()
                        .any(|value| !value.trim().is_empty())
                {
                    column = column.push(
                        text("RGB values must be between 0 and 255")
                            .size(12)
                            .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.6, 0.6))),
                    );
                }

                column.into()
            }
        };

        let column = column![
            text("Drawing Defaults").size(20),
            color_mode_picker,
            color_section,
            row![
                labeled_input(
                    "Thickness (px)",
                    &self.draft.drawing_default_thickness,
                    TextField::DrawingThickness
                ),
                labeled_input(
                    "Font size (pt)",
                    &self.draft.drawing_default_font_size,
                    TextField::DrawingFontSize
                )
            ]
            .spacing(12),
            row![
                labeled_input(
                    "Font family",
                    &self.draft.drawing_font_family,
                    TextField::DrawingFontFamily
                ),
                column![
                    text("Font weight").size(14),
                    pick_list(
                        FontWeightOption::list(),
                        Some(self.draft.drawing_font_weight_option),
                        Message::FontWeightOptionSelected,
                    )
                    .width(Length::Fill),
                    text_input("Custom or numeric weight", &self.draft.drawing_font_weight)
                        .on_input(|value| Message::TextChanged(TextField::DrawingFontWeight, value))
                        .width(Length::Fill)
                ]
                .spacing(6),
                {
                    let mut column = column![
                        text("Font style").size(14),
                        pick_list(
                            FontStyleOption::list(),
                            Some(self.draft.drawing_font_style_option),
                            Message::FontStyleOptionSelected,
                        )
                        .width(Length::Fill),
                    ]
                    .spacing(6);

                    if self.draft.drawing_font_style_option == FontStyleOption::Custom {
                        column = column.push(
                            text_input("Custom style", &self.draft.drawing_font_style)
                                .on_input(|value| {
                                    Message::TextChanged(TextField::DrawingFontStyle, value)
                                })
                                .width(Length::Fill),
                        );
                    }

                    column
                }
            ]
            .spacing(12),
            checkbox(
                "Enable text background",
                self.draft.drawing_text_background_enabled
            )
            .on_toggle(|value| Message::ToggleChanged(ToggleField::DrawingTextBackground, value))
        ]
        .spacing(12)
        .width(Length::Fill);

        scrollable(column).into()
    }

    fn arrow_tab(&self) -> Element<'_, Message> {
        scrollable(
            column![
                text("Arrow Settings").size(20),
                row![
                    labeled_input(
                        "Arrow length (px)",
                        &self.draft.arrow_length,
                        TextField::ArrowLength
                    ),
                    labeled_input(
                        "Arrow angle (deg)",
                        &self.draft.arrow_angle,
                        TextField::ArrowAngle
                    )
                ]
                .spacing(12)
            ]
            .spacing(12),
        )
        .into()
    }

    fn performance_tab(&self) -> Element<'_, Message> {
        let buffer_pick = pick_list(
            vec![2u32, 3, 4],
            Some(self.draft.performance_buffer_count),
            Message::BufferCountChanged,
        );

        scrollable(
            column![
                text("Performance").size(20),
                row![
                    text("Buffer count:"),
                    buffer_pick.width(Length::Fixed(120.0)),
                    text(self.draft.performance_buffer_count.to_string())
                ]
                .spacing(12)
                .align_items(iced::Alignment::Center),
                checkbox("Enable VSync", self.draft.performance_enable_vsync).on_toggle(|value| {
                    Message::ToggleChanged(ToggleField::PerformanceVsync, value)
                })
            ]
            .spacing(12),
        )
        .into()
    }

    fn ui_tab(&self) -> Element<'_, Message> {
        let status_position = pick_list(
            StatusPositionOption::list(),
            Some(self.draft.ui_status_position),
            Message::StatusPositionChanged,
        );

        let column = column![
            text("UI Settings").size(20),
            checkbox("Show status bar", self.draft.ui_show_status_bar)
                .on_toggle(|value| Message::ToggleChanged(ToggleField::UiShowStatusBar, value)),
            row![text("Status bar position:"), status_position].spacing(12),
            text("Status Bar Style").size(18),
            color_quad_editor(
                "Background RGBA (0-1)",
                &self.draft.status_bar_bg_color,
                QuadField::StatusBarBg
            ),
            color_quad_editor(
                "Text RGBA (0-1)",
                &self.draft.status_bar_text_color,
                QuadField::StatusBarText
            ),
            row![
                labeled_input(
                    "Font size",
                    &self.draft.status_font_size,
                    TextField::StatusFontSize
                ),
                labeled_input(
                    "Padding",
                    &self.draft.status_padding,
                    TextField::StatusPadding
                ),
                labeled_input(
                    "Dot radius",
                    &self.draft.status_dot_radius,
                    TextField::StatusDotRadius
                )
            ]
            .spacing(12),
            text("Help Overlay Style").size(18),
            color_quad_editor(
                "Background RGBA (0-1)",
                &self.draft.help_bg_color,
                QuadField::HelpBg
            ),
            color_quad_editor(
                "Border RGBA (0-1)",
                &self.draft.help_border_color,
                QuadField::HelpBorder
            ),
            color_quad_editor(
                "Text RGBA (0-1)",
                &self.draft.help_text_color,
                QuadField::HelpText
            ),
            row![
                labeled_input(
                    "Font size",
                    &self.draft.help_font_size,
                    TextField::HelpFontSize
                ),
                labeled_input(
                    "Line height",
                    &self.draft.help_line_height,
                    TextField::HelpLineHeight
                ),
                labeled_input("Padding", &self.draft.help_padding, TextField::HelpPadding),
                labeled_input(
                    "Border width",
                    &self.draft.help_border_width,
                    TextField::HelpBorderWidth
                )
            ]
            .spacing(12)
        ]
        .spacing(12);

        scrollable(column).into()
    }

    fn board_tab(&self) -> Element<'_, Message> {
        let board_mode_pick = pick_list(
            BoardModeOption::list(),
            Some(self.draft.board_default_mode),
            Message::BoardModeChanged,
        );

        let column = column![
            text("Board Mode").size(20),
            checkbox("Enable board mode", self.draft.board_enabled)
                .on_toggle(|value| Message::ToggleChanged(ToggleField::BoardEnabled, value)),
            row![text("Default mode:"), board_mode_pick].spacing(12),
            color_triplet_editor(
                "Whiteboard color RGB (0-1)",
                &self.draft.board_whiteboard_color,
                TripletField::BoardWhiteboard
            ),
            color_triplet_editor(
                "Blackboard color RGB (0-1)",
                &self.draft.board_blackboard_color,
                TripletField::BoardBlackboard
            ),
            color_triplet_editor(
                "Whiteboard pen RGB (0-1)",
                &self.draft.board_whiteboard_pen,
                TripletField::BoardWhiteboardPen
            ),
            color_triplet_editor(
                "Blackboard pen RGB (0-1)",
                &self.draft.board_blackboard_pen,
                TripletField::BoardBlackboardPen
            ),
            checkbox("Auto-adjust pen color", self.draft.board_auto_adjust_pen)
                .on_toggle(|value| Message::ToggleChanged(ToggleField::BoardAutoAdjust, value))
        ]
        .spacing(12);

        scrollable(column).into()
    }

    fn capture_tab(&self) -> Element<'_, Message> {
        scrollable(
            column![
                text("Capture Settings").size(20),
                checkbox("Enable capture shortcuts", self.draft.capture_enabled)
                    .on_toggle(|value| Message::ToggleChanged(ToggleField::CaptureEnabled, value)),
                labeled_input(
                    "Save directory",
                    &self.draft.capture_save_directory,
                    TextField::CaptureSaveDirectory
                ),
                labeled_input(
                    "Filename template",
                    &self.draft.capture_filename_template,
                    TextField::CaptureFilename
                ),
                labeled_input(
                    "Format (png, jpg, ...)",
                    &self.draft.capture_format,
                    TextField::CaptureFormat
                ),
                checkbox("Copy to clipboard", self.draft.capture_copy_to_clipboard).on_toggle(
                    |value| Message::ToggleChanged(ToggleField::CaptureCopyToClipboard, value)
                )
            ]
            .spacing(12),
        )
        .into()
    }

    fn keybindings_tab(&self) -> Element<'_, Message> {
        let mut column = Column::new()
            .spacing(8)
            .push(text("Keybindings (comma-separated)").size(20));

        for entry in &self.draft.keybindings.entries {
            column = column.push(
                row![
                    container(text(entry.field.label()).size(16))
                        .width(Length::Fixed(220.0))
                        .align_x(Horizontal::Right),
                    text_input("Shortcut list", &entry.value)
                        .on_input({
                            let field = entry.field;
                            move |value| Message::KeybindingChanged(field, value)
                        })
                        .width(Length::Fill)
                ]
                .spacing(12)
                .align_items(iced::Alignment::Center),
            );
        }

        scrollable(column).into()
    }

    fn refresh_dirty_flag(&mut self) {
        self.is_dirty = self.draft != self.baseline;
    }
}

fn labeled_input<'a>(
    label: &'static str,
    value: &'a str,
    field: TextField,
) -> Element<'a, Message> {
    column![
        text(label).size(14),
        text_input(label, value).on_input(move |val| Message::TextChanged(field, val))
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

fn color_triplet_editor<'a>(
    label: &'static str,
    colors: &'a ColorTripletInput,
    field: TripletField,
) -> Element<'a, Message> {
    column![
        text(label).size(14),
        row![
            text_input("R", &colors.components[0])
                .on_input(move |val| Message::TripletChanged(field, 0, val)),
            text_input("G", &colors.components[1])
                .on_input(move |val| Message::TripletChanged(field, 1, val)),
            text_input("B", &colors.components[2])
                .on_input(move |val| Message::TripletChanged(field, 2, val)),
        ]
        .spacing(8)
    ]
    .spacing(4)
    .into()
}

fn color_quad_editor<'a>(
    label: &'static str,
    colors: &'a ColorQuadInput,
    field: QuadField,
) -> Element<'a, Message> {
    column![
        text(label).size(14),
        row![
            text_input("R", &colors.components[0])
                .on_input(move |val| Message::QuadChanged(field, 0, val)),
            text_input("G", &colors.components[1])
                .on_input(move |val| Message::QuadChanged(field, 1, val)),
            text_input("B", &colors.components[2])
                .on_input(move |val| Message::QuadChanged(field, 2, val)),
            text_input("A", &colors.components[3])
                .on_input(move |val| Message::QuadChanged(field, 3, val)),
        ]
        .spacing(8)
    ]
    .spacing(4)
    .into()
}

fn color_preview_badge<'a>(color: Option<iced::Color>) -> Element<'a, Message> {
    let (preview_color, is_valid) = match color {
        Some(color) => (color, true),
        None => (iced::Color::from_rgb(0.2, 0.2, 0.2), false),
    };

    container(Space::with_width(Length::Fixed(20.0)).height(Length::Fixed(20.0)))
        .width(Length::Fixed(24.0))
        .height(Length::Fixed(24.0))
        .style(theme::Container::Custom(Box::new(ColorPreviewStyle {
            color: preview_color,
            is_invalid: !is_valid,
        })))
        .into()
}

#[derive(Clone, Copy)]
struct ColorPreviewStyle {
    color: iced::Color,
    is_invalid: bool,
}

impl container::StyleSheet for ColorPreviewStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            background: Some(Background::Color(self.color)),
            text_color: None,
            border: Border {
                color: if self.is_invalid {
                    iced::Color::from_rgb(0.9, 0.4, 0.4)
                } else {
                    iced::Color::from_rgb(0.4, 0.4, 0.4)
                },
                width: 1.0,
                radius: Radius::from(6.0),
            },
            shadow: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabId {
    Drawing,
    Arrow,
    Performance,
    Ui,
    Board,
    Capture,
    Keybindings,
}

impl TabId {
    const ALL: [TabId; 7] = [
        TabId::Drawing,
        TabId::Arrow,
        TabId::Performance,
        TabId::Ui,
        TabId::Board,
        TabId::Capture,
        TabId::Keybindings,
    ];

    fn title(&self) -> &'static str {
        match self {
            TabId::Drawing => "Drawing",
            TabId::Arrow => "Arrow",
            TabId::Performance => "Performance",
            TabId::Ui => "UI",
            TabId::Board => "Board",
            TabId::Capture => "Capture",
            TabId::Keybindings => "Keybindings",
        }
    }
}

#[derive(Debug, Clone)]
enum StatusMessage {
    Idle,
    Info(String),
    Success(String),
    Error(String),
}

impl StatusMessage {
    fn idle() -> Self {
        StatusMessage::Idle
    }

    fn info(message: impl Into<String>) -> Self {
        StatusMessage::Info(message.into())
    }

    fn success(message: impl Into<String>) -> Self {
        StatusMessage::Success(message.into())
    }

    fn error(message: impl Into<String>) -> Self {
        StatusMessage::Error(message.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorMode {
    Named,
    Rgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamedColorOption {
    Red,
    Green,
    Blue,
    Yellow,
    Orange,
    Pink,
    White,
    Black,
    Custom,
}

impl NamedColorOption {
    fn list() -> Vec<Self> {
        vec![
            Self::Red,
            Self::Green,
            Self::Blue,
            Self::Yellow,
            Self::Orange,
            Self::Pink,
            Self::White,
            Self::Black,
            Self::Custom,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Yellow => "Yellow",
            Self::Orange => "Orange",
            Self::Pink => "Pink",
            Self::White => "White",
            Self::Black => "Black",
            Self::Custom => "Custom",
        }
    }

    fn as_value(&self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Yellow => "yellow",
            Self::Orange => "orange",
            Self::Pink => "pink",
            Self::White => "white",
            Self::Black => "black",
            Self::Custom => "",
        }
    }

    fn from_str(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "red" => Self::Red,
            "green" => Self::Green,
            "blue" => Self::Blue,
            "yellow" => Self::Yellow,
            "orange" => Self::Orange,
            "pink" => Self::Pink,
            "white" => Self::White,
            "black" => Self::Black,
            _ => Self::Custom,
        }
    }
}

impl std::fmt::Display for NamedColorOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FontStyleOption {
    Normal,
    Italic,
    Oblique,
    Custom,
}

impl FontStyleOption {
    fn list() -> Vec<Self> {
        vec![Self::Normal, Self::Italic, Self::Oblique, Self::Custom]
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Italic => "Italic",
            Self::Oblique => "Oblique",
            Self::Custom => "Custom",
        }
    }

    fn canonical_value(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Italic => "italic",
            Self::Oblique => "oblique",
            Self::Custom => "",
        }
    }

    fn from_value(value: &str) -> (Self, String) {
        let lower = value.trim().to_lowercase();
        match lower.as_str() {
            "normal" => (Self::Normal, "normal".to_string()),
            "italic" => (Self::Italic, "italic".to_string()),
            "oblique" => (Self::Oblique, "oblique".to_string()),
            _ => (Self::Custom, value.to_string()),
        }
    }
}

impl std::fmt::Display for FontStyleOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FontWeightOption {
    Normal,
    Bold,
    Light,
    Ultralight,
    Heavy,
    Ultrabold,
    Custom,
}

impl FontWeightOption {
    fn list() -> Vec<Self> {
        vec![
            Self::Normal,
            Self::Bold,
            Self::Light,
            Self::Ultralight,
            Self::Heavy,
            Self::Ultrabold,
            Self::Custom,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Bold => "Bold",
            Self::Light => "Light",
            Self::Ultralight => "Ultralight",
            Self::Heavy => "Heavy",
            Self::Ultrabold => "Ultrabold",
            Self::Custom => "Custom",
        }
    }

    fn canonical_value(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Bold => "bold",
            Self::Light => "light",
            Self::Ultralight => "ultralight",
            Self::Heavy => "heavy",
            Self::Ultrabold => "ultrabold",
            Self::Custom => "",
        }
    }

    fn from_value(value: &str) -> (Self, String) {
        let lower = value.trim().to_lowercase();
        match lower.as_str() {
            "normal" => (Self::Normal, "normal".to_string()),
            "bold" => (Self::Bold, "bold".to_string()),
            "light" => (Self::Light, "light".to_string()),
            "ultralight" => (Self::Ultralight, "ultralight".to_string()),
            "heavy" => (Self::Heavy, "heavy".to_string()),
            "ultrabold" => (Self::Ultrabold, "ultrabold".to_string()),
            _ => (Self::Custom, value.to_string()),
        }
    }
}

impl std::fmt::Display for FontWeightOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ColorInput {
    mode: ColorMode,
    name: String,
    rgb: [String; 3],
    selected_named: NamedColorOption,
}

impl ColorInput {
    fn from_color(spec: &ColorSpec) -> Self {
        match spec {
            ColorSpec::Name(name) => Self {
                mode: ColorMode::Named,
                name: name.clone(),
                selected_named: NamedColorOption::from_str(name),
                rgb: ["255".into(), "0".into(), "0".into()],
            },
            ColorSpec::Rgb([r, g, b]) => Self {
                mode: ColorMode::Rgb,
                name: "".into(),
                rgb: [r.to_string(), g.to_string(), b.to_string()],
                selected_named: NamedColorOption::Custom,
            },
        }
    }

    fn to_color_spec(&self) -> Result<ColorSpec, FormError> {
        match self.mode {
            ColorMode::Named => {
                let value = if self.selected_named_is_custom() {
                    self.name.trim().to_string()
                } else {
                    self.selected_named.as_value().to_string()
                };

                if value.trim().is_empty() {
                    Err(FormError::new(
                        "drawing.default_color",
                        "Please enter a color name.",
                    ))
                } else {
                    Ok(ColorSpec::Name(value))
                }
            }
            ColorMode::Rgb => {
                let mut rgb = [0u8; 3];
                for (index, component) in self.rgb.iter().enumerate() {
                    let field = format!("drawing.default_color[{}]", index);
                    let parsed = component.trim().parse::<i64>().map_err(|_| {
                        FormError::new(&field, "Expected integer between 0 and 255")
                    })?;
                    if !(0..=255).contains(&parsed) {
                        return Err(FormError::new(&field, "Value must be between 0 and 255"));
                    }
                    rgb[index] = parsed as u8;
                }
                Ok(ColorSpec::Rgb(rgb))
            }
        }
    }

    fn update_named_from_current(&mut self) {
        self.selected_named = NamedColorOption::from_str(&self.name);
    }

    fn selected_named_is_custom(&self) -> bool {
        self.selected_named == NamedColorOption::Custom
    }

    fn preview_color(&self) -> Option<iced::Color> {
        match self.mode {
            ColorMode::Named => {
                let name = if self.selected_named_is_custom() {
                    self.name.trim().to_string()
                } else {
                    self.selected_named.as_value().to_string()
                };

                if name.is_empty() {
                    return None;
                }

                if let Some(color) = name_to_color(&name) {
                    Some(iced::Color::from_rgba(
                        color.r as f32,
                        color.g as f32,
                        color.b as f32,
                        color.a as f32,
                    ))
                } else {
                    None
                }
            }
            ColorMode::Rgb => {
                let mut components = [0.0f32; 3];
                for (index, value) in self.rgb.iter().enumerate() {
                    let trimmed = value.trim();
                    if trimmed.is_empty() {
                        return None;
                    }
                    let parsed = trimmed.parse::<f32>().ok()?;
                    if !(0.0..=255.0).contains(&parsed) {
                        return None;
                    }
                    components[index] = parsed / 255.0;
                }
                Some(iced::Color::from_rgba(
                    components[0],
                    components[1],
                    components[2],
                    1.0,
                ))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ColorTripletInput {
    components: [String; 3],
}

impl ColorTripletInput {
    fn from(values: [f64; 3]) -> Self {
        Self {
            components: values.map(format_float),
        }
    }

    fn set_component(&mut self, index: usize, value: String) {
        if let Some(slot) = self.components.get_mut(index) {
            *slot = value;
        }
    }

    fn to_array(&self, field: &'static str) -> Result<[f64; 3], FormError> {
        let mut out = [0.0f64; 3];
        for (index, value) in self.components.iter().enumerate() {
            let parsed = parse_f64(value.trim())
                .map_err(|err| FormError::new(format!("{field}[{index}]"), err))?;
            out[index] = parsed;
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ColorQuadInput {
    components: [String; 4],
}

impl ColorQuadInput {
    fn from(values: [f64; 4]) -> Self {
        Self {
            components: values.map(format_float),
        }
    }

    fn set_component(&mut self, index: usize, value: String) {
        if let Some(slot) = self.components.get_mut(index) {
            *slot = value;
        }
    }

    fn to_array(&self, field: &'static str) -> Result<[f64; 4], FormError> {
        let mut out = [0.0f64; 4];
        for (index, value) in self.components.iter().enumerate() {
            let parsed = parse_f64(value.trim())
                .map_err(|err| FormError::new(format!("{field}[{index}]"), err))?;
            out[index] = parsed;
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusPositionOption {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl StatusPositionOption {
    fn list() -> Vec<Self> {
        vec![
            StatusPositionOption::TopLeft,
            StatusPositionOption::TopRight,
            StatusPositionOption::BottomLeft,
            StatusPositionOption::BottomRight,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            StatusPositionOption::TopLeft => "Top Left",
            StatusPositionOption::TopRight => "Top Right",
            StatusPositionOption::BottomLeft => "Bottom Left",
            StatusPositionOption::BottomRight => "Bottom Right",
        }
    }

    fn to_status_position(&self) -> StatusPosition {
        match self {
            StatusPositionOption::TopLeft => StatusPosition::TopLeft,
            StatusPositionOption::TopRight => StatusPosition::TopRight,
            StatusPositionOption::BottomLeft => StatusPosition::BottomLeft,
            StatusPositionOption::BottomRight => StatusPosition::BottomRight,
        }
    }

    fn from_status_position(position: StatusPosition) -> Self {
        match position {
            StatusPosition::TopLeft => StatusPositionOption::TopLeft,
            StatusPosition::TopRight => StatusPositionOption::TopRight,
            StatusPosition::BottomLeft => StatusPositionOption::BottomLeft,
            StatusPosition::BottomRight => StatusPositionOption::BottomRight,
        }
    }
}

impl std::fmt::Display for StatusPositionOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoardModeOption {
    Transparent,
    Whiteboard,
    Blackboard,
}

impl BoardModeOption {
    fn list() -> Vec<Self> {
        vec![
            BoardModeOption::Transparent,
            BoardModeOption::Whiteboard,
            BoardModeOption::Blackboard,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            BoardModeOption::Transparent => "Transparent",
            BoardModeOption::Whiteboard => "Whiteboard",
            BoardModeOption::Blackboard => "Blackboard",
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            BoardModeOption::Transparent => "transparent",
            BoardModeOption::Whiteboard => "whiteboard",
            BoardModeOption::Blackboard => "blackboard",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "transparent" => Some(BoardModeOption::Transparent),
            "whiteboard" => Some(BoardModeOption::Whiteboard),
            "blackboard" => Some(BoardModeOption::Blackboard),
            _ => None,
        }
    }
}

impl std::fmt::Display for BoardModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ConfigDraft {
    drawing_color: ColorInput,
    drawing_default_thickness: String,
    drawing_default_font_size: String,
    drawing_font_family: String,
    drawing_font_weight: String,
    drawing_font_style: String,
    drawing_text_background_enabled: bool,
    drawing_font_style_option: FontStyleOption,
    drawing_font_weight_option: FontWeightOption,

    arrow_length: String,
    arrow_angle: String,

    performance_buffer_count: u32,
    performance_enable_vsync: bool,

    ui_show_status_bar: bool,
    ui_status_position: StatusPositionOption,
    status_font_size: String,
    status_padding: String,
    status_bar_bg_color: ColorQuadInput,
    status_bar_text_color: ColorQuadInput,
    status_dot_radius: String,

    help_font_size: String,
    help_line_height: String,
    help_padding: String,
    help_bg_color: ColorQuadInput,
    help_border_color: ColorQuadInput,
    help_border_width: String,
    help_text_color: ColorQuadInput,

    board_enabled: bool,
    board_default_mode: BoardModeOption,
    board_whiteboard_color: ColorTripletInput,
    board_blackboard_color: ColorTripletInput,
    board_whiteboard_pen: ColorTripletInput,
    board_blackboard_pen: ColorTripletInput,
    board_auto_adjust_pen: bool,

    capture_enabled: bool,
    capture_save_directory: String,
    capture_filename_template: String,
    capture_format: String,
    capture_copy_to_clipboard: bool,

    keybindings: KeybindingsDraft,
}

impl ConfigDraft {
    fn from_config(config: &Config) -> Self {
        let (style_option, style_value) = FontStyleOption::from_value(&config.drawing.font_style);
        let (weight_option, weight_value) =
            FontWeightOption::from_value(&config.drawing.font_weight);
        Self {
            drawing_color: ColorInput::from_color(&config.drawing.default_color),
            drawing_default_thickness: format_float(config.drawing.default_thickness),
            drawing_default_font_size: format_float(config.drawing.default_font_size),
            drawing_font_family: config.drawing.font_family.clone(),
            drawing_font_weight: weight_value,
            drawing_font_style: style_value,
            drawing_text_background_enabled: config.drawing.text_background_enabled,
            drawing_font_style_option: style_option,
            drawing_font_weight_option: weight_option,

            arrow_length: format_float(config.arrow.length),
            arrow_angle: format_float(config.arrow.angle_degrees),

            performance_buffer_count: config.performance.buffer_count,
            performance_enable_vsync: config.performance.enable_vsync,

            ui_show_status_bar: config.ui.show_status_bar,
            ui_status_position: StatusPositionOption::from_status_position(
                config.ui.status_bar_position,
            ),
            status_font_size: format_float(config.ui.status_bar_style.font_size),
            status_padding: format_float(config.ui.status_bar_style.padding),
            status_bar_bg_color: ColorQuadInput::from(config.ui.status_bar_style.bg_color),
            status_bar_text_color: ColorQuadInput::from(config.ui.status_bar_style.text_color),
            status_dot_radius: format_float(config.ui.status_bar_style.dot_radius),

            help_font_size: format_float(config.ui.help_overlay_style.font_size),
            help_line_height: format_float(config.ui.help_overlay_style.line_height),
            help_padding: format_float(config.ui.help_overlay_style.padding),
            help_bg_color: ColorQuadInput::from(config.ui.help_overlay_style.bg_color),
            help_border_color: ColorQuadInput::from(config.ui.help_overlay_style.border_color),
            help_border_width: format_float(config.ui.help_overlay_style.border_width),
            help_text_color: ColorQuadInput::from(config.ui.help_overlay_style.text_color),

            board_enabled: config.board.enabled,
            board_default_mode: BoardModeOption::from_str(&config.board.default_mode)
                .unwrap_or(BoardModeOption::Transparent),
            board_whiteboard_color: ColorTripletInput::from(config.board.whiteboard_color),
            board_blackboard_color: ColorTripletInput::from(config.board.blackboard_color),
            board_whiteboard_pen: ColorTripletInput::from(config.board.whiteboard_pen_color),
            board_blackboard_pen: ColorTripletInput::from(config.board.blackboard_pen_color),
            board_auto_adjust_pen: config.board.auto_adjust_pen,

            capture_enabled: config.capture.enabled,
            capture_save_directory: config.capture.save_directory.clone(),
            capture_filename_template: config.capture.filename_template.clone(),
            capture_format: config.capture.format.clone(),
            capture_copy_to_clipboard: config.capture.copy_to_clipboard,

            keybindings: KeybindingsDraft::from_config(&config.keybindings),
        }
    }

    fn to_config(&self) -> Result<Config, Vec<FormError>> {
        let mut errors = Vec::new();
        let mut config = Config::default();

        match self.drawing_color.to_color_spec() {
            Ok(color) => config.drawing.default_color = color,
            Err(err) => errors.push(err),
        }
        parse_field(
            &self.drawing_default_thickness,
            "drawing.default_thickness",
            &mut errors,
            |value| config.drawing.default_thickness = value,
        );
        parse_field(
            &self.drawing_default_font_size,
            "drawing.default_font_size",
            &mut errors,
            |value| config.drawing.default_font_size = value,
        );
        config.drawing.font_family = self.drawing_font_family.clone();
        config.drawing.font_weight = self.drawing_font_weight.clone();
        config.drawing.font_style = self.drawing_font_style.clone();
        config.drawing.text_background_enabled = self.drawing_text_background_enabled;

        parse_field(&self.arrow_length, "arrow.length", &mut errors, |value| {
            config.arrow.length = value
        });
        parse_field(
            &self.arrow_angle,
            "arrow.angle_degrees",
            &mut errors,
            |value| config.arrow.angle_degrees = value,
        );

        config.performance.buffer_count = self.performance_buffer_count;
        config.performance.enable_vsync = self.performance_enable_vsync;

        config.ui.show_status_bar = self.ui_show_status_bar;
        config.ui.status_bar_position = self.ui_status_position.to_status_position();
        parse_field(
            &self.status_font_size,
            "ui.status_bar_style.font_size",
            &mut errors,
            |value| config.ui.status_bar_style.font_size = value,
        );
        parse_field(
            &self.status_padding,
            "ui.status_bar_style.padding",
            &mut errors,
            |value| config.ui.status_bar_style.padding = value,
        );
        match self
            .status_bar_bg_color
            .to_array("ui.status_bar_style.bg_color")
        {
            Ok(values) => config.ui.status_bar_style.bg_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .status_bar_text_color
            .to_array("ui.status_bar_style.text_color")
        {
            Ok(values) => config.ui.status_bar_style.text_color = values,
            Err(err) => errors.push(err),
        }
        parse_field(
            &self.status_dot_radius,
            "ui.status_bar_style.dot_radius",
            &mut errors,
            |value| config.ui.status_bar_style.dot_radius = value,
        );

        parse_field(
            &self.help_font_size,
            "ui.help_overlay_style.font_size",
            &mut errors,
            |value| config.ui.help_overlay_style.font_size = value,
        );
        parse_field(
            &self.help_line_height,
            "ui.help_overlay_style.line_height",
            &mut errors,
            |value| config.ui.help_overlay_style.line_height = value,
        );
        parse_field(
            &self.help_padding,
            "ui.help_overlay_style.padding",
            &mut errors,
            |value| config.ui.help_overlay_style.padding = value,
        );
        match self
            .help_bg_color
            .to_array("ui.help_overlay_style.bg_color")
        {
            Ok(values) => config.ui.help_overlay_style.bg_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .help_border_color
            .to_array("ui.help_overlay_style.border_color")
        {
            Ok(values) => config.ui.help_overlay_style.border_color = values,
            Err(err) => errors.push(err),
        }
        parse_field(
            &self.help_border_width,
            "ui.help_overlay_style.border_width",
            &mut errors,
            |value| config.ui.help_overlay_style.border_width = value,
        );
        match self
            .help_text_color
            .to_array("ui.help_overlay_style.text_color")
        {
            Ok(values) => config.ui.help_overlay_style.text_color = values,
            Err(err) => errors.push(err),
        }

        config.board.enabled = self.board_enabled;
        config.board.default_mode = self.board_default_mode.as_str().to_string();
        match self
            .board_whiteboard_color
            .to_array("board.whiteboard_color")
        {
            Ok(values) => config.board.whiteboard_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .board_blackboard_color
            .to_array("board.blackboard_color")
        {
            Ok(values) => config.board.blackboard_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .board_whiteboard_pen
            .to_array("board.whiteboard_pen_color")
        {
            Ok(values) => config.board.whiteboard_pen_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .board_blackboard_pen
            .to_array("board.blackboard_pen_color")
        {
            Ok(values) => config.board.blackboard_pen_color = values,
            Err(err) => errors.push(err),
        }
        config.board.auto_adjust_pen = self.board_auto_adjust_pen;

        config.capture.enabled = self.capture_enabled;
        config.capture.save_directory = self.capture_save_directory.clone();
        config.capture.filename_template = self.capture_filename_template.clone();
        config.capture.format = self.capture_format.clone();
        config.capture.copy_to_clipboard = self.capture_copy_to_clipboard;

        match self.keybindings.to_config() {
            Ok(cfg) => config.keybindings = cfg,
            Err(errs) => errors.extend(errs),
        }

        if errors.is_empty() {
            Ok(config)
        } else {
            Err(errors)
        }
    }

    fn set_toggle(&mut self, field: ToggleField, value: bool) {
        match field {
            ToggleField::DrawingTextBackground => {
                self.drawing_text_background_enabled = value;
            }
            ToggleField::PerformanceVsync => self.performance_enable_vsync = value,
            ToggleField::UiShowStatusBar => self.ui_show_status_bar = value,
            ToggleField::BoardEnabled => self.board_enabled = value,
            ToggleField::BoardAutoAdjust => self.board_auto_adjust_pen = value,
            ToggleField::CaptureEnabled => self.capture_enabled = value,
            ToggleField::CaptureCopyToClipboard => self.capture_copy_to_clipboard = value,
        }
    }

    fn set_text(&mut self, field: TextField, value: String) {
        match field {
            TextField::DrawingColorName => {
                self.drawing_color.name = value;
                self.drawing_color.update_named_from_current();
            }
            TextField::DrawingThickness => self.drawing_default_thickness = value,
            TextField::DrawingFontSize => self.drawing_default_font_size = value,
            TextField::DrawingFontFamily => self.drawing_font_family = value,
            TextField::DrawingFontWeight => {
                self.drawing_font_weight = value;
                self.drawing_font_weight_option = FontWeightOption::Custom;
            }
            TextField::DrawingFontStyle => {
                self.drawing_font_style = value;
                self.drawing_font_style_option = FontStyleOption::Custom;
            }
            TextField::ArrowLength => self.arrow_length = value,
            TextField::ArrowAngle => self.arrow_angle = value,
            TextField::StatusFontSize => self.status_font_size = value,
            TextField::StatusPadding => self.status_padding = value,
            TextField::StatusDotRadius => self.status_dot_radius = value,
            TextField::HelpFontSize => self.help_font_size = value,
            TextField::HelpLineHeight => self.help_line_height = value,
            TextField::HelpPadding => self.help_padding = value,
            TextField::HelpBorderWidth => self.help_border_width = value,
            TextField::CaptureSaveDirectory => self.capture_save_directory = value,
            TextField::CaptureFilename => self.capture_filename_template = value,
            TextField::CaptureFormat => self.capture_format = value,
        }
    }

    fn set_triplet(&mut self, field: TripletField, index: usize, value: String) {
        match field {
            TripletField::DrawingColorRgb => {
                if let Some(slot) = self.drawing_color.rgb.get_mut(index) {
                    *slot = value;
                }
            }
            TripletField::BoardWhiteboard => {
                self.board_whiteboard_color.set_component(index, value)
            }
            TripletField::BoardBlackboard => {
                self.board_blackboard_color.set_component(index, value)
            }
            TripletField::BoardWhiteboardPen => {
                self.board_whiteboard_pen.set_component(index, value)
            }
            TripletField::BoardBlackboardPen => {
                self.board_blackboard_pen.set_component(index, value)
            }
        }
    }

    fn set_quad(&mut self, field: QuadField, index: usize, value: String) {
        match field {
            QuadField::StatusBarBg => self.status_bar_bg_color.set_component(index, value),
            QuadField::StatusBarText => self.status_bar_text_color.set_component(index, value),
            QuadField::HelpBg => self.help_bg_color.set_component(index, value),
            QuadField::HelpBorder => self.help_border_color.set_component(index, value),
            QuadField::HelpText => self.help_text_color.set_component(index, value),
        }
    }
}

#[derive(Debug, Clone)]
struct FormError {
    field: String,
    message: String,
}

impl FormError {
    fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToggleField {
    DrawingTextBackground,
    PerformanceVsync,
    UiShowStatusBar,
    BoardEnabled,
    BoardAutoAdjust,
    CaptureEnabled,
    CaptureCopyToClipboard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextField {
    DrawingColorName,
    DrawingThickness,
    DrawingFontSize,
    DrawingFontFamily,
    DrawingFontWeight,
    DrawingFontStyle,
    ArrowLength,
    ArrowAngle,
    StatusFontSize,
    StatusPadding,
    StatusDotRadius,
    HelpFontSize,
    HelpLineHeight,
    HelpPadding,
    HelpBorderWidth,
    CaptureSaveDirectory,
    CaptureFilename,
    CaptureFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TripletField {
    DrawingColorRgb,
    BoardWhiteboard,
    BoardBlackboard,
    BoardWhiteboardPen,
    BoardBlackboardPen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuadField {
    StatusBarBg,
    StatusBarText,
    HelpBg,
    HelpBorder,
    HelpText,
}

fn parse_field<F>(value: &str, field: &'static str, errors: &mut Vec<FormError>, apply: F)
where
    F: FnOnce(f64),
{
    match parse_f64(value.trim()) {
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err)),
    }
}

fn parse_f64(input: &str) -> Result<f64, String> {
    input
        .parse::<f64>()
        .map_err(|_| "Expected a numeric value".to_string())
}

fn format_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{:.0}", value)
    } else {
        format!("{:.3}", value)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct KeybindingsDraft {
    entries: Vec<KeybindingEntry>,
}

#[derive(Debug, Clone, PartialEq)]
struct KeybindingEntry {
    field: KeybindingField,
    value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeybindingField {
    Exit,
    EnterTextMode,
    ClearCanvas,
    Undo,
    IncreaseThickness,
    DecreaseThickness,
    IncreaseFontSize,
    DecreaseFontSize,
    ToggleWhiteboard,
    ToggleBlackboard,
    ReturnToTransparent,
    ToggleHelp,
    OpenConfigurator,
    SetColorRed,
    SetColorGreen,
    SetColorBlue,
    SetColorYellow,
    SetColorOrange,
    SetColorPink,
    SetColorWhite,
    SetColorBlack,
    CaptureFullScreen,
    CaptureActiveWindow,
    CaptureSelection,
    CaptureClipboardFull,
    CaptureFileFull,
    CaptureClipboardSelection,
    CaptureFileSelection,
    CaptureClipboardRegion,
    CaptureFileRegion,
}

impl KeybindingField {
    fn all() -> Vec<Self> {
        vec![
            KeybindingField::Exit,
            KeybindingField::EnterTextMode,
            KeybindingField::ClearCanvas,
            KeybindingField::Undo,
            KeybindingField::IncreaseThickness,
            KeybindingField::DecreaseThickness,
            KeybindingField::IncreaseFontSize,
            KeybindingField::DecreaseFontSize,
            KeybindingField::ToggleWhiteboard,
            KeybindingField::ToggleBlackboard,
            KeybindingField::ReturnToTransparent,
            KeybindingField::ToggleHelp,
            KeybindingField::OpenConfigurator,
            KeybindingField::SetColorRed,
            KeybindingField::SetColorGreen,
            KeybindingField::SetColorBlue,
            KeybindingField::SetColorYellow,
            KeybindingField::SetColorOrange,
            KeybindingField::SetColorPink,
            KeybindingField::SetColorWhite,
            KeybindingField::SetColorBlack,
            KeybindingField::CaptureFullScreen,
            KeybindingField::CaptureActiveWindow,
            KeybindingField::CaptureSelection,
            KeybindingField::CaptureClipboardFull,
            KeybindingField::CaptureFileFull,
            KeybindingField::CaptureClipboardSelection,
            KeybindingField::CaptureFileSelection,
            KeybindingField::CaptureClipboardRegion,
            KeybindingField::CaptureFileRegion,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            KeybindingField::Exit => "Exit",
            KeybindingField::EnterTextMode => "Enter text mode",
            KeybindingField::ClearCanvas => "Clear canvas",
            KeybindingField::Undo => "Undo",
            KeybindingField::IncreaseThickness => "Increase thickness",
            KeybindingField::DecreaseThickness => "Decrease thickness",
            KeybindingField::IncreaseFontSize => "Increase font size",
            KeybindingField::DecreaseFontSize => "Decrease font size",
            KeybindingField::ToggleWhiteboard => "Toggle whiteboard",
            KeybindingField::ToggleBlackboard => "Toggle blackboard",
            KeybindingField::ReturnToTransparent => "Return to transparent",
            KeybindingField::ToggleHelp => "Toggle help",
            KeybindingField::OpenConfigurator => "Open configurator",
            KeybindingField::SetColorRed => "Color: red",
            KeybindingField::SetColorGreen => "Color: green",
            KeybindingField::SetColorBlue => "Color: blue",
            KeybindingField::SetColorYellow => "Color: yellow",
            KeybindingField::SetColorOrange => "Color: orange",
            KeybindingField::SetColorPink => "Color: pink",
            KeybindingField::SetColorWhite => "Color: white",
            KeybindingField::SetColorBlack => "Color: black",
            KeybindingField::CaptureFullScreen => "Capture full screen",
            KeybindingField::CaptureActiveWindow => "Capture active window",
            KeybindingField::CaptureSelection => "Capture selection",
            KeybindingField::CaptureClipboardFull => "Clipboard full screen",
            KeybindingField::CaptureFileFull => "File full screen",
            KeybindingField::CaptureClipboardSelection => "Clipboard selection",
            KeybindingField::CaptureFileSelection => "File selection",
            KeybindingField::CaptureClipboardRegion => "Clipboard region",
            KeybindingField::CaptureFileRegion => "File region",
        }
    }

    fn field_key(&self) -> &'static str {
        match self {
            KeybindingField::Exit => "exit",
            KeybindingField::EnterTextMode => "enter_text_mode",
            KeybindingField::ClearCanvas => "clear_canvas",
            KeybindingField::Undo => "undo",
            KeybindingField::IncreaseThickness => "increase_thickness",
            KeybindingField::DecreaseThickness => "decrease_thickness",
            KeybindingField::IncreaseFontSize => "increase_font_size",
            KeybindingField::DecreaseFontSize => "decrease_font_size",
            KeybindingField::ToggleWhiteboard => "toggle_whiteboard",
            KeybindingField::ToggleBlackboard => "toggle_blackboard",
            KeybindingField::ReturnToTransparent => "return_to_transparent",
            KeybindingField::ToggleHelp => "toggle_help",
            KeybindingField::OpenConfigurator => "open_configurator",
            KeybindingField::SetColorRed => "set_color_red",
            KeybindingField::SetColorGreen => "set_color_green",
            KeybindingField::SetColorBlue => "set_color_blue",
            KeybindingField::SetColorYellow => "set_color_yellow",
            KeybindingField::SetColorOrange => "set_color_orange",
            KeybindingField::SetColorPink => "set_color_pink",
            KeybindingField::SetColorWhite => "set_color_white",
            KeybindingField::SetColorBlack => "set_color_black",
            KeybindingField::CaptureFullScreen => "capture_full_screen",
            KeybindingField::CaptureActiveWindow => "capture_active_window",
            KeybindingField::CaptureSelection => "capture_selection",
            KeybindingField::CaptureClipboardFull => "capture_clipboard_full",
            KeybindingField::CaptureFileFull => "capture_file_full",
            KeybindingField::CaptureClipboardSelection => "capture_clipboard_selection",
            KeybindingField::CaptureFileSelection => "capture_file_selection",
            KeybindingField::CaptureClipboardRegion => "capture_clipboard_region",
            KeybindingField::CaptureFileRegion => "capture_file_region",
        }
    }

    fn get<'a>(&self, config: &'a KeybindingsConfig) -> &'a Vec<String> {
        match self {
            KeybindingField::Exit => &config.exit,
            KeybindingField::EnterTextMode => &config.enter_text_mode,
            KeybindingField::ClearCanvas => &config.clear_canvas,
            KeybindingField::Undo => &config.undo,
            KeybindingField::IncreaseThickness => &config.increase_thickness,
            KeybindingField::DecreaseThickness => &config.decrease_thickness,
            KeybindingField::IncreaseFontSize => &config.increase_font_size,
            KeybindingField::DecreaseFontSize => &config.decrease_font_size,
            KeybindingField::ToggleWhiteboard => &config.toggle_whiteboard,
            KeybindingField::ToggleBlackboard => &config.toggle_blackboard,
            KeybindingField::ReturnToTransparent => &config.return_to_transparent,
            KeybindingField::ToggleHelp => &config.toggle_help,
            KeybindingField::OpenConfigurator => &config.open_configurator,
            KeybindingField::SetColorRed => &config.set_color_red,
            KeybindingField::SetColorGreen => &config.set_color_green,
            KeybindingField::SetColorBlue => &config.set_color_blue,
            KeybindingField::SetColorYellow => &config.set_color_yellow,
            KeybindingField::SetColorOrange => &config.set_color_orange,
            KeybindingField::SetColorPink => &config.set_color_pink,
            KeybindingField::SetColorWhite => &config.set_color_white,
            KeybindingField::SetColorBlack => &config.set_color_black,
            KeybindingField::CaptureFullScreen => &config.capture_full_screen,
            KeybindingField::CaptureActiveWindow => &config.capture_active_window,
            KeybindingField::CaptureSelection => &config.capture_selection,
            KeybindingField::CaptureClipboardFull => &config.capture_clipboard_full,
            KeybindingField::CaptureFileFull => &config.capture_file_full,
            KeybindingField::CaptureClipboardSelection => &config.capture_clipboard_selection,
            KeybindingField::CaptureFileSelection => &config.capture_file_selection,
            KeybindingField::CaptureClipboardRegion => &config.capture_clipboard_region,
            KeybindingField::CaptureFileRegion => &config.capture_file_region,
        }
    }

    fn set(&self, config: &mut KeybindingsConfig, value: Vec<String>) {
        match self {
            KeybindingField::Exit => config.exit = value,
            KeybindingField::EnterTextMode => config.enter_text_mode = value,
            KeybindingField::ClearCanvas => config.clear_canvas = value,
            KeybindingField::Undo => config.undo = value,
            KeybindingField::IncreaseThickness => config.increase_thickness = value,
            KeybindingField::DecreaseThickness => config.decrease_thickness = value,
            KeybindingField::IncreaseFontSize => config.increase_font_size = value,
            KeybindingField::DecreaseFontSize => config.decrease_font_size = value,
            KeybindingField::ToggleWhiteboard => config.toggle_whiteboard = value,
            KeybindingField::ToggleBlackboard => config.toggle_blackboard = value,
            KeybindingField::ReturnToTransparent => config.return_to_transparent = value,
            KeybindingField::ToggleHelp => config.toggle_help = value,
            KeybindingField::OpenConfigurator => config.open_configurator = value,
            KeybindingField::SetColorRed => config.set_color_red = value,
            KeybindingField::SetColorGreen => config.set_color_green = value,
            KeybindingField::SetColorBlue => config.set_color_blue = value,
            KeybindingField::SetColorYellow => config.set_color_yellow = value,
            KeybindingField::SetColorOrange => config.set_color_orange = value,
            KeybindingField::SetColorPink => config.set_color_pink = value,
            KeybindingField::SetColorWhite => config.set_color_white = value,
            KeybindingField::SetColorBlack => config.set_color_black = value,
            KeybindingField::CaptureFullScreen => config.capture_full_screen = value,
            KeybindingField::CaptureActiveWindow => config.capture_active_window = value,
            KeybindingField::CaptureSelection => config.capture_selection = value,
            KeybindingField::CaptureClipboardFull => config.capture_clipboard_full = value,
            KeybindingField::CaptureFileFull => config.capture_file_full = value,
            KeybindingField::CaptureClipboardSelection => {
                config.capture_clipboard_selection = value
            }
            KeybindingField::CaptureFileSelection => config.capture_file_selection = value,
            KeybindingField::CaptureClipboardRegion => config.capture_clipboard_region = value,
            KeybindingField::CaptureFileRegion => config.capture_file_region = value,
        }
    }
}

impl KeybindingsDraft {
    fn from_config(config: &KeybindingsConfig) -> Self {
        let entries = KeybindingField::all()
            .into_iter()
            .map(|field| KeybindingEntry {
                value: field.get(config).join(", "),
                field,
            })
            .collect();
        Self { entries }
    }

    fn set(&mut self, field: KeybindingField, value: String) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.field == field) {
            entry.value = value;
        }
    }

    fn to_config(&self) -> Result<KeybindingsConfig, Vec<FormError>> {
        let mut config = KeybindingsConfig::default();
        let mut errors = Vec::new();

        for entry in &self.entries {
            match parse_keybinding_list(&entry.value) {
                Ok(list) => entry.field.set(&mut config, list),
                Err(err) => errors.push(FormError::new(
                    format!("keybindings.{}", entry.field.field_key()),
                    err,
                )),
            }
        }

        if errors.is_empty() {
            Ok(config)
        } else {
            Err(errors)
        }
    }
}

fn parse_keybinding_list(value: &str) -> Result<Vec<String>, String> {
    let mut entries = Vec::new();

    for part in value.split(',') {
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            entries.push(trimmed.to_string());
        }
    }

    Ok(entries)
}

async fn load_config_from_disk() -> Result<Arc<Config>, String> {
    Config::load().map(Arc::new).map_err(|err| err.to_string())
}

async fn save_config_to_disk(config: Config) -> Result<(Option<PathBuf>, Arc<Config>), String> {
    let backup = config.save_with_backup().map_err(|err| err.to_string())?;
    Ok((backup, Arc::new(config)))
}

#[derive(Debug, Clone)]
enum Message {
    ConfigLoaded(Result<Arc<Config>, String>),
    ReloadRequested,
    ResetToDefaults,
    SaveRequested,
    ConfigSaved(Result<(Option<PathBuf>, Arc<Config>), String>),
    TabSelected(TabId),
    ToggleChanged(ToggleField, bool),
    TextChanged(TextField, String),
    TripletChanged(TripletField, usize, String),
    QuadChanged(QuadField, usize, String),
    ColorModeChanged(ColorMode),
    NamedColorSelected(NamedColorOption),
    StatusPositionChanged(StatusPositionOption),
    BoardModeChanged(BoardModeOption),
    BufferCountChanged(u32),
    KeybindingChanged(KeybindingField, String),
    FontStyleOptionSelected(FontStyleOption),
    FontWeightOptionSelected(FontWeightOption),
}
