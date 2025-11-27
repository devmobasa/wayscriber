use std::{path::PathBuf, sync::Arc};

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
use wayscriber::config::Config;

use crate::messages::Message;
use crate::models::{
    BoardModeOption, ColorMode, ColorQuadInput, ColorTripletInput, ConfigDraft, FontStyleOption,
    FontWeightOption, NamedColorOption, QuadField, SessionCompressionOption,
    SessionStorageModeOption, StatusPositionOption, TabId, TextField, ToggleField, TripletField,
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
    defaults: ConfigDraft,
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
        let defaults = ConfigDraft::from_config(&default_config);
        let baseline = defaults.clone();
        let config_path = Config::get_config_path().ok();

        let app = Self {
            draft: baseline.clone(),
            baseline,
            defaults,
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
                    self.draft = self.defaults.clone();
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
            Message::SessionStorageModeChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.session_storage_mode = option;
                self.refresh_dirty_flag();
            }
            Message::SessionCompressionChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.session_compression = option;
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
        let reload_button = button("Reload")
            .style(theme::Button::Secondary)
            .on_press(Message::ReloadRequested);

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
            TabId::Session => self.session_tab(),
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

        let color_block = column![
            row![
                text("Pen color").size(14),
                Space::with_width(Length::Fill),
                default_value_text(
                    self.defaults.drawing_color.summary(),
                    self.draft.drawing_color != self.defaults.drawing_color,
                ),
            ]
            .align_items(iced::Alignment::Center),
            color_mode_picker,
            color_section
        ]
        .spacing(8);

        let column = column![
            text("Drawing Defaults").size(20),
            color_block,
            row![
                labeled_input(
                    "Thickness (px)",
                    &self.draft.drawing_default_thickness,
                    &self.defaults.drawing_default_thickness,
                    TextField::DrawingThickness,
                ),
                labeled_input(
                    "Font size (pt)",
                    &self.draft.drawing_default_font_size,
                    &self.defaults.drawing_default_font_size,
                    TextField::DrawingFontSize,
                )
            ]
            .spacing(12),
            row![
                labeled_input(
                    "Font family",
                    &self.draft.drawing_font_family,
                    &self.defaults.drawing_font_family,
                    TextField::DrawingFontFamily,
                ),
                column![
                    row![
                        text("Font weight").size(14),
                        Space::with_width(Length::Fill),
                        default_value_text(
                            self.defaults.drawing_font_weight.clone(),
                            self.draft.drawing_font_weight != self.defaults.drawing_font_weight,
                        )
                    ]
                    .align_items(iced::Alignment::Center),
                    pick_list(
                        FontWeightOption::list(),
                        Some(self.draft.drawing_font_weight_option),
                        Message::FontWeightOptionSelected,
                    )
                    .width(Length::Fill),
                    labeled_input(
                        "Custom or numeric weight",
                        &self.draft.drawing_font_weight,
                        &self.defaults.drawing_font_weight,
                        TextField::DrawingFontWeight,
                    )
                ]
                .spacing(6),
                {
                    let mut column = column![
                        row![
                            text("Font style").size(14),
                            Space::with_width(Length::Fill),
                            default_value_text(
                                self.defaults.drawing_font_style.clone(),
                                self.draft.drawing_font_style != self.defaults.drawing_font_style,
                            )
                        ]
                        .align_items(iced::Alignment::Center),
                        pick_list(
                            FontStyleOption::list(),
                            Some(self.draft.drawing_font_style_option),
                            Message::FontStyleOptionSelected,
                        )
                        .width(Length::Fill),
                    ]
                    .spacing(6);

                    if self.draft.drawing_font_style_option == FontStyleOption::Custom {
                        column = column.push(labeled_input(
                            "Custom style",
                            &self.draft.drawing_font_style,
                            &self.defaults.drawing_font_style,
                            TextField::DrawingFontStyle,
                        ));
                    }

                    column
                }
            ]
            .spacing(12),
            toggle_row(
                "Enable text background",
                self.draft.drawing_text_background_enabled,
                self.defaults.drawing_text_background_enabled,
                ToggleField::DrawingTextBackground,
            )
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
                        &self.defaults.arrow_length,
                        TextField::ArrowLength,
                    ),
                    labeled_input(
                        "Arrow angle (deg)",
                        &self.draft.arrow_angle,
                        &self.defaults.arrow_angle,
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
                    Space::with_width(Length::Fill),
                    default_value_text(
                        self.defaults.performance_buffer_count.to_string(),
                        self.draft.performance_buffer_count
                            != self.defaults.performance_buffer_count,
                    )
                ]
                .spacing(12)
                .align_items(iced::Alignment::Center),
                toggle_row(
                    "Enable VSync",
                    self.draft.performance_enable_vsync,
                    self.defaults.performance_enable_vsync,
                    ToggleField::PerformanceVsync,
                )
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
            toggle_row(
                "Show status bar",
                self.draft.ui_show_status_bar,
                self.defaults.ui_show_status_bar,
                ToggleField::UiShowStatusBar,
            ),
            toggle_row(
                "Show frozen badge",
                self.draft.ui_show_frozen_badge,
                self.defaults.ui_show_frozen_badge,
                ToggleField::UiShowFrozenBadge,
            ),
            row![
                text("Status bar position:"),
                status_position,
                Space::with_width(Length::Fill),
                default_value_text(
                    self.defaults.ui_status_position.label().to_string(),
                    self.draft.ui_status_position != self.defaults.ui_status_position,
                )
            ]
            .spacing(12)
            .align_items(iced::Alignment::Center),
            text("Status Bar Style").size(18),
            color_quad_editor(
                "Background RGBA (0-1)",
                &self.draft.status_bar_bg_color,
                &self.defaults.status_bar_bg_color,
                QuadField::StatusBarBg,
            ),
            color_quad_editor(
                "Text RGBA (0-1)",
                &self.draft.status_bar_text_color,
                &self.defaults.status_bar_text_color,
                QuadField::StatusBarText,
            ),
            row![
                labeled_input(
                    "Font size",
                    &self.draft.status_font_size,
                    &self.defaults.status_font_size,
                    TextField::StatusFontSize,
                ),
                labeled_input(
                    "Padding",
                    &self.draft.status_padding,
                    &self.defaults.status_padding,
                    TextField::StatusPadding,
                ),
                labeled_input(
                    "Dot radius",
                    &self.draft.status_dot_radius,
                    &self.defaults.status_dot_radius,
                    TextField::StatusDotRadius,
                )
            ]
            .spacing(12),
            text("Help Overlay Style").size(18),
            color_quad_editor(
                "Background RGBA (0-1)",
                &self.draft.help_bg_color,
                &self.defaults.help_bg_color,
                QuadField::HelpBg,
            ),
            color_quad_editor(
                "Border RGBA (0-1)",
                &self.draft.help_border_color,
                &self.defaults.help_border_color,
                QuadField::HelpBorder,
            ),
            color_quad_editor(
                "Text RGBA (0-1)",
                &self.draft.help_text_color,
                &self.defaults.help_text_color,
                QuadField::HelpText,
            ),
            row![
                labeled_input(
                    "Font size",
                    &self.draft.help_font_size,
                    &self.defaults.help_font_size,
                    TextField::HelpFontSize,
                ),
                labeled_input(
                    "Line height",
                    &self.draft.help_line_height,
                    &self.defaults.help_line_height,
                    TextField::HelpLineHeight,
                ),
                labeled_input(
                    "Padding",
                    &self.draft.help_padding,
                    &self.defaults.help_padding,
                    TextField::HelpPadding,
                ),
                labeled_input(
                    "Border width",
                    &self.draft.help_border_width,
                    &self.defaults.help_border_width,
                    TextField::HelpBorderWidth,
                )
            ]
            .spacing(12),
            text("Click Highlight").size(18),
            toggle_row(
                "Enable click highlight",
                self.draft.click_highlight_enabled,
                self.defaults.click_highlight_enabled,
                ToggleField::UiClickHighlightEnabled,
            ),
            toggle_row(
                "Link highlight color to current pen",
                self.draft.click_highlight_use_pen_color,
                self.defaults.click_highlight_use_pen_color,
                ToggleField::UiClickHighlightUsePenColor,
            ),
            row![
                labeled_input(
                    "Radius",
                    &self.draft.click_highlight_radius,
                    &self.defaults.click_highlight_radius,
                    TextField::HighlightRadius,
                ),
                labeled_input(
                    "Outline thickness",
                    &self.draft.click_highlight_outline_thickness,
                    &self.defaults.click_highlight_outline_thickness,
                    TextField::HighlightOutlineThickness,
                ),
                labeled_input(
                    "Duration (ms)",
                    &self.draft.click_highlight_duration_ms,
                    &self.defaults.click_highlight_duration_ms,
                    TextField::HighlightDurationMs,
                )
            ]
            .spacing(12),
            color_quad_editor(
                "Fill RGBA (0-1)",
                &self.draft.click_highlight_fill_color,
                &self.defaults.click_highlight_fill_color,
                QuadField::HighlightFill,
            ),
            color_quad_editor(
                "Outline RGBA (0-1)",
                &self.draft.click_highlight_outline_color,
                &self.defaults.click_highlight_outline_color,
                QuadField::HighlightOutline,
            )
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
            toggle_row(
                "Enable board mode",
                self.draft.board_enabled,
                self.defaults.board_enabled,
                ToggleField::BoardEnabled,
            ),
            row![
                text("Default mode:"),
                board_mode_pick,
                Space::with_width(Length::Fill),
                default_value_text(
                    self.defaults.board_default_mode.label().to_string(),
                    self.draft.board_default_mode != self.defaults.board_default_mode,
                )
            ]
            .spacing(12)
            .align_items(iced::Alignment::Center),
            color_triplet_editor(
                "Whiteboard color RGB (0-1)",
                &self.draft.board_whiteboard_color,
                &self.defaults.board_whiteboard_color,
                TripletField::BoardWhiteboard,
            ),
            color_triplet_editor(
                "Blackboard color RGB (0-1)",
                &self.draft.board_blackboard_color,
                &self.defaults.board_blackboard_color,
                TripletField::BoardBlackboard,
            ),
            color_triplet_editor(
                "Whiteboard pen RGB (0-1)",
                &self.draft.board_whiteboard_pen,
                &self.defaults.board_whiteboard_pen,
                TripletField::BoardWhiteboardPen,
            ),
            color_triplet_editor(
                "Blackboard pen RGB (0-1)",
                &self.draft.board_blackboard_pen,
                &self.defaults.board_blackboard_pen,
                TripletField::BoardBlackboardPen,
            ),
            toggle_row(
                "Auto-adjust pen color",
                self.draft.board_auto_adjust_pen,
                self.defaults.board_auto_adjust_pen,
                ToggleField::BoardAutoAdjust,
            )
        ]
        .spacing(12);

        scrollable(column).into()
    }

    fn capture_tab(&self) -> Element<'_, Message> {
        scrollable(
            column![
                text("Capture Settings").size(20),
                toggle_row(
                    "Enable capture shortcuts",
                    self.draft.capture_enabled,
                    self.defaults.capture_enabled,
                    ToggleField::CaptureEnabled,
                ),
                labeled_input(
                    "Save directory",
                    &self.draft.capture_save_directory,
                    &self.defaults.capture_save_directory,
                    TextField::CaptureSaveDirectory,
                ),
                labeled_input(
                    "Filename template",
                    &self.draft.capture_filename_template,
                    &self.defaults.capture_filename_template,
                    TextField::CaptureFilename,
                ),
                labeled_input(
                    "Format (png, jpg, ...)",
                    &self.draft.capture_format,
                    &self.defaults.capture_format,
                    TextField::CaptureFormat,
                ),
                toggle_row(
                    "Copy to clipboard",
                    self.draft.capture_copy_to_clipboard,
                    self.defaults.capture_copy_to_clipboard,
                    ToggleField::CaptureCopyToClipboard,
                )
            ]
            .spacing(12),
        )
        .into()
    }

    fn session_tab(&self) -> Element<'_, Message> {
        let storage_pick = pick_list(
            SessionStorageModeOption::list(),
            Some(self.draft.session_storage_mode),
            Message::SessionStorageModeChanged,
        );
        let compression_pick = pick_list(
            SessionCompressionOption::list(),
            Some(self.draft.session_compression),
            Message::SessionCompressionChanged,
        );

        let mut column = column![
            text("Session Persistence").size(20),
            toggle_row(
                "Persist transparent mode drawings",
                self.draft.session_persist_transparent,
                self.defaults.session_persist_transparent,
                ToggleField::SessionPersistTransparent,
            ),
            toggle_row(
                "Persist whiteboard mode drawings",
                self.draft.session_persist_whiteboard,
                self.defaults.session_persist_whiteboard,
                ToggleField::SessionPersistWhiteboard,
            ),
            toggle_row(
                "Persist blackboard mode drawings",
                self.draft.session_persist_blackboard,
                self.defaults.session_persist_blackboard,
                ToggleField::SessionPersistBlackboard,
            ),
            toggle_row(
                "Restore tool state on startup",
                self.draft.session_restore_tool_state,
                self.defaults.session_restore_tool_state,
                ToggleField::SessionRestoreToolState,
            ),
            toggle_row(
                "Per-output persistence",
                self.draft.session_per_output,
                self.defaults.session_per_output,
                ToggleField::SessionPerOutput,
            ),
            row![
                text("Storage mode:"),
                storage_pick,
                Space::with_width(Length::Fill),
                default_value_text(
                    self.defaults.session_storage_mode.label().to_string(),
                    self.draft.session_storage_mode != self.defaults.session_storage_mode,
                )
            ]
            .spacing(12)
            .align_items(iced::Alignment::Center),
        ]
        .spacing(12);

        if self.draft.session_storage_mode == SessionStorageModeOption::Custom {
            column = column.push(labeled_input(
                "Custom directory",
                &self.draft.session_custom_directory,
                &self.defaults.session_custom_directory,
                TextField::SessionCustomDirectory,
            ));
        }

        column = column
            .push(
                row![
                    text("Compression:"),
                    compression_pick,
                    Space::with_width(Length::Fill),
                    default_value_text(
                        self.defaults.session_compression.label().to_string(),
                        self.draft.session_compression != self.defaults.session_compression,
                    )
                ]
                .spacing(12)
                .align_items(iced::Alignment::Center),
            )
            .push(labeled_input(
                "Max shapes per frame",
                &self.draft.session_max_shapes_per_frame,
                &self.defaults.session_max_shapes_per_frame,
                TextField::SessionMaxShapesPerFrame,
            ))
            .push(labeled_input(
                "Max file size (MB)",
                &self.draft.session_max_file_size_mb,
                &self.defaults.session_max_file_size_mb,
                TextField::SessionMaxFileSizeMb,
            ))
            .push(labeled_input(
                "Auto-compress threshold (KB)",
                &self.draft.session_auto_compress_threshold_kb,
                &self.defaults.session_auto_compress_threshold_kb,
                TextField::SessionAutoCompressThresholdKb,
            ))
            .push(labeled_input(
                "Backup retention count",
                &self.draft.session_backup_retention,
                &self.defaults.session_backup_retention,
                TextField::SessionBackupRetention,
            ));

        scrollable(column).into()
    }

    fn keybindings_tab(&self) -> Element<'_, Message> {
        let mut column = Column::new()
            .spacing(8)
            .push(text("Keybindings (comma-separated)").size(20));

        for entry in &self.draft.keybindings.entries {
            let default_value = self
                .defaults
                .keybindings
                .value_for(entry.field)
                .unwrap_or("");
            let changed = entry.value.trim() != default_value.trim();
            column = column.push(
                row![
                    container(text(entry.field.label()).size(16))
                        .width(Length::Fixed(220.0))
                        .align_x(Horizontal::Right),
                    column![
                        text_input("Shortcut list", &entry.value)
                            .on_input({
                                let field = entry.field;
                                move |value| Message::KeybindingChanged(field, value)
                            })
                            .width(Length::Fill),
                        row![
                            Space::with_width(Length::Fill),
                            default_value_text(default_value.to_string(), changed)
                        ]
                        .align_items(iced::Alignment::Center)
                    ]
                    .spacing(4)
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
    default: &'a str,
    field: TextField,
) -> Element<'a, Message> {
    let changed = value.trim() != default.trim();
    column![
        row![
            text(label).size(14),
            Space::with_width(Length::Fill),
            default_value_text(default, changed),
        ]
        .align_items(iced::Alignment::Center),
        text_input(label, value).on_input(move |val| Message::TextChanged(field, val))
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

fn color_triplet_editor<'a>(
    label: &'static str,
    colors: &'a ColorTripletInput,
    default: &'a ColorTripletInput,
    field: TripletField,
) -> Element<'a, Message> {
    let changed = colors != default;
    column![
        row![
            text(label).size(14),
            Space::with_width(Length::Fill),
            default_value_text(default.summary(), changed),
        ]
        .align_items(iced::Alignment::Center),
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
    default: &'a ColorQuadInput,
    field: QuadField,
) -> Element<'a, Message> {
    let changed = colors != default;
    column![
        row![
            text(label).size(14),
            Space::with_width(Length::Fill),
            default_value_text(default.summary(), changed),
        ]
        .align_items(iced::Alignment::Center),
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

fn default_value_text<'a>(value: impl Into<String>, changed: bool) -> iced::widget::Text<'a> {
    let label = format!("Default: {}", value.into());
    let color = if changed {
        iced::Color::from_rgb(0.95, 0.6, 0.2)
    } else {
        iced::Color::from_rgb(0.65, 0.74, 0.82)
    };
    text(label).size(12).style(theme::Text::Color(color))
}

fn bool_label(value: bool) -> &'static str {
    if value { "On" } else { "Off" }
}

fn toggle_row<'a>(
    label: &'static str,
    value: bool,
    default: bool,
    field: ToggleField,
) -> Element<'a, Message> {
    let changed = value != default;
    row![
        checkbox(label, value).on_toggle(move |val| Message::ToggleChanged(field, val)),
        Space::with_width(Length::Fill),
        default_value_text(bool_label(default), changed),
    ]
    .align_items(iced::Alignment::Center)
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
