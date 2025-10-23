use std::{path::PathBuf, sync::Arc};

use wayscriber::config::Config;
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

use crate::messages::Message;
use crate::models::{
    BoardModeOption, ColorMode, ColorQuadInput, ColorTripletInput, ConfigDraft, FontStyleOption,
    FontWeightOption, NamedColorOption, QuadField, StatusPositionOption, TabId, TextField,
    ToggleField, TripletField,
};

pub fn run() -> iced::Result {
    let mut settings = Settings::default();
    settings.window.size = Size::new(960.0, 640.0);
    settings.window.resizable = true;
    settings.window.decorations = true;
    ConfiguratorApp::run(settings)
}

#[derive(Debug)]
pub struct ConfiguratorApp {
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
        "Wayscriber Configurator (Iced)".to_string()
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
            Message::KeybindingChanged(field, value) => {
                self.status = StatusMessage::idle();
                self.draft.keybindings.set(field, value);
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
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
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
        let reload_button = button("Reload").style(theme::Button::Secondary).on_press(
            if self.is_loading || self.is_saving {
                Message::ReloadRequested
            } else {
                Message::ReloadRequested
            },
        );

        let defaults_button = button("Defaults")
            .style(theme::Button::Secondary)
            .on_press(Message::ResetToDefaults);

        let save_button = button("Save")
            .style(theme::Button::Primary)
            .on_press(Message::SaveRequested);

        let mut toolbar = Row::new()
            .spacing(12)
            .align_items(iced::Alignment::Center)
            .push(reload_button)
            .push(defaults_button)
            .push(save_button);

        toolbar = if self.is_saving {
            toolbar.push(text("Saving...").size(16))
        } else if self.is_loading {
            toolbar.push(text("Loading...").size(16))
        } else if self.is_dirty {
            toolbar.push(
                text("Unsaved changes")
                    .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.72, 0.2))),
            )
        } else {
            toolbar.push(
                text("All changes saved")
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.8, 0.6))),
            )
        };

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
                    .style(if *tab == self.active_tab {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::TabSelected(*tab));
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
                button("Named Color")
                    .style(if self.draft.drawing_color.mode == ColorMode::Named {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::ColorModeChanged(ColorMode::Named)),
            )
            .push(
                button("RGB Color")
                    .style(if self.draft.drawing_color.mode == ColorMode::Rgb {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
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
                    TextField::DrawingThickness,
                ),
                labeled_input(
                    "Font size (pt)",
                    &self.draft.drawing_default_font_size,
                    TextField::DrawingFontSize,
                )
            ]
            .spacing(12),
            row![
                labeled_input(
                    "Font family",
                    &self.draft.drawing_font_family,
                    TextField::DrawingFontFamily,
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
                self.draft.drawing_text_background_enabled,
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
                        TextField::ArrowLength,
                    ),
                    labeled_input(
                        "Arrow angle (deg)",
                        &self.draft.arrow_angle,
                        TextField::ArrowAngle,
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
                    text(self.draft.performance_buffer_count.to_string()),
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
                QuadField::StatusBarBg,
            ),
            color_quad_editor(
                "Text RGBA (0-1)",
                &self.draft.status_bar_text_color,
                QuadField::StatusBarText,
            ),
            row![
                labeled_input(
                    "Font size",
                    &self.draft.status_font_size,
                    TextField::StatusFontSize,
                ),
                labeled_input(
                    "Padding",
                    &self.draft.status_padding,
                    TextField::StatusPadding,
                ),
                labeled_input(
                    "Dot radius",
                    &self.draft.status_dot_radius,
                    TextField::StatusDotRadius,
                )
            ]
            .spacing(12),
            text("Help Overlay Style").size(18),
            color_quad_editor(
                "Background RGBA (0-1)",
                &self.draft.help_bg_color,
                QuadField::HelpBg,
            ),
            color_quad_editor(
                "Border RGBA (0-1)",
                &self.draft.help_border_color,
                QuadField::HelpBorder,
            ),
            color_quad_editor(
                "Text RGBA (0-1)",
                &self.draft.help_text_color,
                QuadField::HelpText,
            ),
            row![
                labeled_input(
                    "Font size",
                    &self.draft.help_font_size,
                    TextField::HelpFontSize,
                ),
                labeled_input(
                    "Line height",
                    &self.draft.help_line_height,
                    TextField::HelpLineHeight,
                ),
                labeled_input("Padding", &self.draft.help_padding, TextField::HelpPadding,),
                labeled_input(
                    "Border width",
                    &self.draft.help_border_width,
                    TextField::HelpBorderWidth,
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
                TripletField::BoardWhiteboard,
            ),
            color_triplet_editor(
                "Blackboard color RGB (0-1)",
                &self.draft.board_blackboard_color,
                TripletField::BoardBlackboard,
            ),
            color_triplet_editor(
                "Whiteboard pen RGB (0-1)",
                &self.draft.board_whiteboard_pen,
                TripletField::BoardWhiteboardPen,
            ),
            color_triplet_editor(
                "Blackboard pen RGB (0-1)",
                &self.draft.board_blackboard_pen,
                TripletField::BoardBlackboardPen,
            ),
            checkbox("Auto-adjust pen color", self.draft.board_auto_adjust_pen)
                .on_toggle(|value| Message::ToggleChanged(ToggleField::BoardAutoAdjust, value),)
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
                    TextField::CaptureSaveDirectory,
                ),
                labeled_input(
                    "Filename template",
                    &self.draft.capture_filename_template,
                    TextField::CaptureFilename,
                ),
                labeled_input(
                    "Format (png, jpg, ...)",
                    &self.draft.capture_format,
                    TextField::CaptureFormat,
                ),
                checkbox("Copy to clipboard", self.draft.capture_copy_to_clipboard).on_toggle(
                    |value| Message::ToggleChanged(ToggleField::CaptureCopyToClipboard, value),
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

async fn load_config_from_disk() -> Result<Arc<Config>, String> {
    Config::load()
        .map(|loaded| Arc::new(loaded.config))
        .map_err(|err| err.to_string())
}

async fn save_config_to_disk(config: Config) -> Result<(Option<PathBuf>, Arc<Config>), String> {
    let backup = config.save_with_backup().map_err(|err| err.to_string())?;
    Ok((backup, Arc::new(config)))
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
