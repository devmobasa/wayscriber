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
use wayscriber::config::{Config, PRESET_SLOTS_MAX, PRESET_SLOTS_MIN};

use crate::messages::Message;
use crate::models::{
    BoardModeOption, ColorMode, ColorQuadInput, ColorTripletInput, ConfigDraft, EraserModeOption,
    FontStyleOption, FontWeightOption, KeybindingsTabId, NamedColorOption, OverrideOption,
    PresetEraserKindOption, PresetEraserModeOption, PresetTextField, PresetToggleField, QuadField,
    SessionCompressionOption, SessionStorageModeOption, StatusPositionOption, TabId, TextField,
    ToggleField, ToolOption, ToolbarLayoutModeOption, ToolbarOverrideField, TripletField, UiTabId,
};

pub fn run() -> iced::Result {
    let mut settings = Settings::default();
    settings.window.size = Size::new(960.0, 640.0);
    settings.window.resizable = true;
    settings.window.decorations = true;
    if std::env::var_os("ICED_BACKEND").is_none() && should_force_tiny_skia() {
        // GNOME Wayland + wgpu can crash on dma-buf/present mode selection; tiny-skia avoids this.
        // SAFETY: setting a process-local env var before initializing iced is safe here.
        unsafe {
            std::env::set_var("ICED_BACKEND", "tiny-skia");
        }
        eprintln!(
            "wayscriber-configurator: GNOME Wayland detected; using tiny-skia renderer (set ICED_BACKEND=wgpu to override)."
        );
    }
    ConfiguratorApp::run(settings)
}

fn should_force_tiny_skia() -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return false;
    }
    let current = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session = std::env::var("XDG_SESSION_DESKTOP").unwrap_or_default();
    let combined = format!("{current};{session}");
    let combined = combined.to_ascii_lowercase();
    combined.contains("gnome") || combined.contains("ubuntu")
}

#[derive(Debug)]
pub struct ConfiguratorApp {
    draft: ConfigDraft,
    baseline: ConfigDraft,
    defaults: ConfigDraft,
    base_config: Arc<Config>,
    status: StatusMessage,
    active_tab: TabId,
    active_ui_tab: UiTabId,
    active_keybindings_tab: KeybindingsTabId,
    override_mode: ToolbarLayoutModeOption,
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
        let override_mode = defaults.ui_toolbar_layout_mode;
        let config_path = Config::get_config_path().ok();
        let base_config = Arc::new(default_config.clone());

        let app = Self {
            draft: baseline.clone(),
            baseline,
            defaults,
            base_config,
            status: StatusMessage::info("Loading configuration..."),
            active_tab: TabId::Drawing,
            active_ui_tab: UiTabId::Toolbar,
            active_keybindings_tab: KeybindingsTabId::General,
            override_mode,
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
                        self.base_config = config.clone();
                        self.override_mode = self.draft.ui_toolbar_layout_mode;
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
                    self.override_mode = self.draft.ui_toolbar_layout_mode;
                    self.status = StatusMessage::info("Loaded default configuration (not saved).");
                    self.refresh_dirty_flag();
                }
            }
            Message::SaveRequested => {
                if self.is_saving {
                    return Command::none();
                }

                match self.draft.to_config(self.base_config.as_ref()) {
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
                        self.base_config = saved_config.clone();
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
            Message::UiTabSelected(tab) => {
                self.active_ui_tab = tab;
            }
            Message::KeybindingsTabSelected(tab) => {
                self.active_keybindings_tab = tab;
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
            Message::EraserModeChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.drawing_default_eraser_mode = option;
                self.refresh_dirty_flag();
            }
            Message::StatusPositionChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.ui_status_position = option;
                self.refresh_dirty_flag();
            }
            Message::ToolbarLayoutModeChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.apply_toolbar_layout_mode(option);
                self.refresh_dirty_flag();
            }
            Message::ToolbarOverrideModeChanged(option) => {
                self.override_mode = option;
            }
            Message::ToolbarOverrideChanged(field, option) => {
                self.status = StatusMessage::idle();
                self.draft
                    .set_toolbar_override(self.override_mode, field, option);
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
            Message::PresetSlotCountChanged(count) => {
                self.status = StatusMessage::idle();
                self.draft.presets.slot_count = count;
                self.refresh_dirty_flag();
            }
            Message::PresetSlotEnabledChanged(slot_index, enabled) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    slot.enabled = enabled;
                }
                self.refresh_dirty_flag();
            }
            Message::PresetToolChanged(slot_index, tool) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    slot.tool = tool;
                }
                self.refresh_dirty_flag();
            }
            Message::PresetColorModeChanged(slot_index, mode) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    slot.color.mode = mode;
                    if matches!(mode, ColorMode::Named) {
                        if slot.color.name.trim().is_empty() {
                            slot.color.selected_named = NamedColorOption::Red;
                            slot.color.name = slot.color.selected_named.as_value().to_string();
                        } else {
                            slot.color.update_named_from_current();
                        }
                    }
                }
                self.refresh_dirty_flag();
            }
            Message::PresetNamedColorSelected(slot_index, option) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    slot.color.selected_named = option;
                    if option != NamedColorOption::Custom {
                        slot.color.name = option.as_value().to_string();
                    }
                }
                self.refresh_dirty_flag();
            }
            Message::PresetColorComponentChanged(slot_index, component, value) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index)
                    && let Some(entry) = slot.color.rgb.get_mut(component)
                {
                    *entry = value;
                }
                self.refresh_dirty_flag();
            }
            Message::PresetTextChanged(slot_index, field, value) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    match field {
                        PresetTextField::Name => {
                            slot.name = value;
                        }
                        PresetTextField::ColorName => {
                            slot.color.name = value;
                            slot.color.update_named_from_current();
                        }
                        PresetTextField::Size => slot.size = value,
                        PresetTextField::MarkerOpacity => slot.marker_opacity = value,
                        PresetTextField::FontSize => slot.font_size = value,
                        PresetTextField::ArrowLength => slot.arrow_length = value,
                        PresetTextField::ArrowAngle => slot.arrow_angle = value,
                    }
                }
                self.refresh_dirty_flag();
            }
            Message::PresetToggleOptionChanged(slot_index, field, value) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    match field {
                        PresetToggleField::FillEnabled => slot.fill_enabled = value,
                        PresetToggleField::TextBackgroundEnabled => {
                            slot.text_background_enabled = value;
                        }
                        PresetToggleField::ArrowHeadAtEnd => slot.arrow_head_at_end = value,
                        PresetToggleField::ShowStatusBar => slot.show_status_bar = value,
                    }
                }
                self.refresh_dirty_flag();
            }
            Message::PresetEraserKindChanged(slot_index, value) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    slot.eraser_kind = value;
                }
                self.refresh_dirty_flag();
            }
            Message::PresetEraserModeChanged(slot_index, value) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    slot.eraser_mode = value;
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
            TabId::Presets => self.presets_tab(),
            TabId::Arrow => self.arrow_tab(),
            TabId::History => self.history_tab(),
            TabId::Performance => self.performance_tab(),
            TabId::Ui => self.ui_tab(),
            TabId::Board => self.board_tab(),
            TabId::Capture => self.capture_tab(),
            TabId::Session => self.session_tab(),
            TabId::Keybindings => self.keybindings_tab(),
            TabId::Tablet => self.tablet_tab(),
        };

        let legend = self.defaults_legend();

        column![tab_bar, horizontal_rule(2), legend, content]
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

    fn defaults_legend(&self) -> Element<'_, Message> {
        row![
            text("Default labels:").size(12),
            text("blue = matches")
                .size(12)
                .style(theme::Text::Color(default_label_color(false))),
            text("yellow = changed")
                .size(12)
                .style(theme::Text::Color(default_label_color(true))),
        ]
        .spacing(12)
        .align_items(iced::Alignment::Center)
        .into()
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
                default_value_text(
                    self.defaults.drawing_color.summary(),
                    self.draft.drawing_color != self.defaults.drawing_color,
                ),
            ]
            .spacing(DEFAULT_LABEL_GAP)
            .align_items(iced::Alignment::Center),
            color_mode_picker,
            color_section
        ]
        .spacing(8);

        let eraser_mode_pick = pick_list(
            EraserModeOption::list(),
            Some(self.draft.drawing_default_eraser_mode),
            Message::EraserModeChanged,
        );

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
                    "Eraser size (px)",
                    &self.draft.drawing_default_eraser_size,
                    &self.defaults.drawing_default_eraser_size,
                    TextField::DrawingEraserSize,
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
            row![
                labeled_input(
                    "Marker opacity (0.05-0.9)",
                    &self.draft.drawing_marker_opacity,
                    &self.defaults.drawing_marker_opacity,
                    TextField::DrawingMarkerOpacity,
                ),
                labeled_input(
                    "Undo stack limit",
                    &self.draft.drawing_undo_stack_limit,
                    &self.defaults.drawing_undo_stack_limit,
                    TextField::DrawingUndoStackLimit,
                )
            ]
            .spacing(12),
            row![
                labeled_input(
                    "Hit-test tolerance (px)",
                    &self.draft.drawing_hit_test_tolerance,
                    &self.defaults.drawing_hit_test_tolerance,
                    TextField::DrawingHitTestTolerance,
                ),
                labeled_input(
                    "Hit-test threshold",
                    &self.draft.drawing_hit_test_linear_threshold,
                    &self.defaults.drawing_hit_test_linear_threshold,
                    TextField::DrawingHitTestThreshold,
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
                        default_value_text(
                            self.defaults.drawing_font_weight.clone(),
                            self.draft.drawing_font_weight != self.defaults.drawing_font_weight,
                        )
                    ]
                    .spacing(DEFAULT_LABEL_GAP)
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
                            default_value_text(
                                self.defaults.drawing_font_style.clone(),
                                self.draft.drawing_font_style != self.defaults.drawing_font_style,
                            )
                        ]
                        .spacing(DEFAULT_LABEL_GAP)
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

    fn presets_tab(&self) -> Element<'_, Message> {
        let slot_counts: Vec<usize> = (PRESET_SLOTS_MIN..=PRESET_SLOTS_MAX).collect();
        let slot_picker = pick_list(
            slot_counts,
            Some(self.draft.presets.slot_count),
            Message::PresetSlotCountChanged,
        )
        .width(Length::Fixed(140.0));

        let slot_count_control = labeled_control(
            "Visible slots",
            slot_picker.into(),
            self.defaults.presets.slot_count.to_string(),
            self.draft.presets.slot_count != self.defaults.presets.slot_count,
        );

        let mut column = Column::new()
            .spacing(12)
            .push(text("Preset Slots").size(20))
            .push(slot_count_control);

        for slot_index in 1..=PRESET_SLOTS_MAX {
            column = column.push(self.preset_slot_section(slot_index));
        }

        scrollable(column).into()
    }

    fn preset_slot_section(&self, slot_index: usize) -> Element<'_, Message> {
        let Some(slot) = self.draft.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        let Some(default_slot) = self.defaults.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };

        let enabled_row = row![
            checkbox("Enabled", slot.enabled)
                .on_toggle(move |val| Message::PresetSlotEnabledChanged(slot_index, val)),
            default_value_text(
                bool_label(default_slot.enabled),
                slot.enabled != default_slot.enabled
            ),
        ]
        .spacing(DEFAULT_LABEL_GAP)
        .align_items(iced::Alignment::Center);

        let mut section = Column::new()
            .spacing(8)
            .push(text(format!("Slot {slot_index}")).size(18))
            .push(enabled_row);

        if slot_index > self.draft.presets.slot_count {
            section = section.push(
                text(format!(
                    "Hidden (slot count is {})",
                    self.draft.presets.slot_count
                ))
                .size(12)
                .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            );
        }

        if !slot.enabled {
            section = section.push(
                text("Slot disabled. Enable to configure.")
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            );
            return container(section)
                .padding(12)
                .style(theme::Container::Box)
                .into();
        }

        let tool_picker = pick_list(ToolOption::list(), Some(slot.tool), move |opt| {
            Message::PresetToolChanged(slot_index, opt)
        })
        .width(Length::Fill);

        let header_row = row![
            preset_input(
                "Label",
                &slot.name,
                &default_slot.name,
                slot_index,
                PresetTextField::Name,
                true,
            ),
            labeled_control(
                "Tool",
                tool_picker.into(),
                default_slot.tool.label(),
                slot.tool != default_slot.tool,
            )
        ]
        .spacing(12);

        let color_mode_picker = Row::new()
            .spacing(12)
            .push(
                button("Named Color")
                    .style(if slot.color.mode == ColorMode::Named {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::PresetColorModeChanged(
                        slot_index,
                        ColorMode::Named,
                    )),
            )
            .push(
                button("RGB Color")
                    .style(if slot.color.mode == ColorMode::Rgb {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::PresetColorModeChanged(slot_index, ColorMode::Rgb)),
            );

        let color_section: Element<'_, Message> = match slot.color.mode {
            ColorMode::Named => {
                let picker = pick_list(
                    NamedColorOption::list(),
                    Some(slot.color.selected_named),
                    move |opt| Message::PresetNamedColorSelected(slot_index, opt),
                )
                .width(Length::Fixed(160.0));

                let picker_row = row![picker, color_preview_badge(slot.color.preview_color()),]
                    .spacing(8)
                    .align_items(iced::Alignment::Center);

                let mut column = Column::new().spacing(8).push(picker_row);

                if slot.color.selected_named_is_custom() {
                    column = column.push(
                        text_input("Custom color name", &slot.color.name)
                            .on_input(move |value| {
                                Message::PresetTextChanged(
                                    slot_index,
                                    PresetTextField::ColorName,
                                    value,
                                )
                            })
                            .width(Length::Fill),
                    );

                    if slot.color.preview_color().is_none() && !slot.color.name.trim().is_empty() {
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
                    text_input("R (0-255)", &slot.color.rgb[0]).on_input(move |value| {
                        Message::PresetColorComponentChanged(slot_index, 0, value)
                    }),
                    text_input("G (0-255)", &slot.color.rgb[1]).on_input(move |value| {
                        Message::PresetColorComponentChanged(slot_index, 1, value)
                    }),
                    text_input("B (0-255)", &slot.color.rgb[2]).on_input(move |value| {
                        Message::PresetColorComponentChanged(slot_index, 2, value)
                    }),
                    color_preview_badge(slot.color.preview_color()),
                ]
                .spacing(8)
                .align_items(iced::Alignment::Center);

                let mut column = Column::new().spacing(8).push(rgb_inputs);

                if slot.color.preview_color().is_none()
                    && slot.color.rgb.iter().any(|value| !value.trim().is_empty())
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
                text("Color").size(14),
                default_value_text(
                    default_slot.color.summary(),
                    slot.color != default_slot.color,
                ),
            ]
            .spacing(DEFAULT_LABEL_GAP)
            .align_items(iced::Alignment::Center),
            color_mode_picker,
            color_section
        ]
        .spacing(8);

        let size_row = row![
            preset_input(
                "Size (px)",
                &slot.size,
                &default_slot.size,
                slot_index,
                PresetTextField::Size,
                false,
            ),
            preset_input(
                "Marker opacity (0.05-0.9)",
                &slot.marker_opacity,
                &default_slot.marker_opacity,
                slot_index,
                PresetTextField::MarkerOpacity,
                true,
            )
        ]
        .spacing(12);

        let eraser_row = row![
            labeled_control(
                "Eraser kind",
                pick_list(
                    PresetEraserKindOption::list(),
                    Some(slot.eraser_kind),
                    move |opt| Message::PresetEraserKindChanged(slot_index, opt),
                )
                .width(Length::Fill)
                .into(),
                default_slot.eraser_kind.label(),
                slot.eraser_kind != default_slot.eraser_kind,
            ),
            labeled_control(
                "Eraser mode",
                pick_list(
                    PresetEraserModeOption::list(),
                    Some(slot.eraser_mode),
                    move |opt| Message::PresetEraserModeChanged(slot_index, opt),
                )
                .width(Length::Fill)
                .into(),
                default_slot.eraser_mode.label(),
                slot.eraser_mode != default_slot.eraser_mode,
            )
        ]
        .spacing(12);

        let fill_row = row![
            preset_override_control(
                "Fill enabled",
                slot.fill_enabled,
                default_slot.fill_enabled,
                slot_index,
                PresetToggleField::FillEnabled,
            ),
            preset_override_control(
                "Text background",
                slot.text_background_enabled,
                default_slot.text_background_enabled,
                slot_index,
                PresetToggleField::TextBackgroundEnabled,
            )
        ]
        .spacing(12);

        let font_row = row![
            preset_input(
                "Font size (pt)",
                &slot.font_size,
                &default_slot.font_size,
                slot_index,
                PresetTextField::FontSize,
                true,
            ),
            preset_input(
                "Arrow length (px)",
                &slot.arrow_length,
                &default_slot.arrow_length,
                slot_index,
                PresetTextField::ArrowLength,
                true,
            )
        ]
        .spacing(12);

        let arrow_row = row![
            preset_input(
                "Arrow angle (deg)",
                &slot.arrow_angle,
                &default_slot.arrow_angle,
                slot_index,
                PresetTextField::ArrowAngle,
                true,
            ),
            preset_override_control(
                "Arrow head at end",
                slot.arrow_head_at_end,
                default_slot.arrow_head_at_end,
                slot_index,
                PresetToggleField::ArrowHeadAtEnd,
            )
        ]
        .spacing(12);

        let status_row = row![preset_override_control(
            "Show status bar",
            slot.show_status_bar,
            default_slot.show_status_bar,
            slot_index,
            PresetToggleField::ShowStatusBar,
        )];

        section = section
            .push(header_row)
            .push(color_block)
            .push(size_row)
            .push(eraser_row)
            .push(fill_row)
            .push(font_row)
            .push(arrow_row)
            .push(status_row);

        container(section)
            .padding(12)
            .style(theme::Container::Box)
            .into()
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
                .spacing(12),
                toggle_row(
                    "Place arrowhead at end of line",
                    self.draft.arrow_head_at_end,
                    self.defaults.arrow_head_at_end,
                    ToggleField::ArrowHeadAtEnd,
                )
            ]
            .spacing(12),
        )
        .into()
    }

    fn history_tab(&self) -> Element<'_, Message> {
        scrollable(
            column![
                text("History").size(20),
                row![
                    labeled_input(
                        "Undo all delay (ms)",
                        &self.draft.history_undo_all_delay_ms,
                        &self.defaults.history_undo_all_delay_ms,
                        TextField::HistoryUndoAllDelayMs,
                    ),
                    labeled_input(
                        "Redo all delay (ms)",
                        &self.draft.history_redo_all_delay_ms,
                        &self.defaults.history_redo_all_delay_ms,
                        TextField::HistoryRedoAllDelayMs,
                    )
                ]
                .spacing(12),
                toggle_row(
                    "Enable custom undo/redo section",
                    self.draft.history_custom_section_enabled,
                    self.defaults.history_custom_section_enabled,
                    ToggleField::HistoryCustomSectionEnabled,
                ),
                row![
                    labeled_input(
                        "Custom undo delay (ms)",
                        &self.draft.history_custom_undo_delay_ms,
                        &self.defaults.history_custom_undo_delay_ms,
                        TextField::HistoryCustomUndoDelayMs,
                    ),
                    labeled_input(
                        "Custom redo delay (ms)",
                        &self.draft.history_custom_redo_delay_ms,
                        &self.defaults.history_custom_redo_delay_ms,
                        TextField::HistoryCustomRedoDelayMs,
                    )
                ]
                .spacing(12),
                row![
                    labeled_input(
                        "Custom undo steps",
                        &self.draft.history_custom_undo_steps,
                        &self.defaults.history_custom_undo_steps,
                        TextField::HistoryCustomUndoSteps,
                    ),
                    labeled_input(
                        "Custom redo steps",
                        &self.draft.history_custom_redo_steps,
                        &self.defaults.history_custom_redo_steps,
                        TextField::HistoryCustomRedoSteps,
                    )
                ]
                .spacing(12),
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
        let buffer_control = row![
            buffer_pick.width(Length::Fixed(120.0)),
            text(self.draft.performance_buffer_count.to_string())
        ]
        .spacing(12)
        .align_items(iced::Alignment::Center)
        .into();

        scrollable(
            column![
                text("Performance").size(20),
                labeled_control(
                    "Buffer count",
                    buffer_control,
                    self.defaults.performance_buffer_count.to_string(),
                    self.draft.performance_buffer_count != self.defaults.performance_buffer_count,
                ),
                labeled_input(
                    "UI animation FPS (0 = unlimited)",
                    &self.draft.performance_ui_animation_fps,
                    &self.defaults.performance_ui_animation_fps,
                    TextField::PerformanceUiAnimationFps,
                ),
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
        let tab_bar = UiTabId::ALL.iter().fold(
            Row::new().spacing(8).align_items(iced::Alignment::Center),
            |row, tab| {
                let label = tab.title();
                let button = button(label)
                    .padding([4, 10])
                    .style(if *tab == self.active_ui_tab {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::UiTabSelected(*tab));
                row.push(button)
            },
        );

        let content = match self.active_ui_tab {
            UiTabId::Toolbar => self.ui_toolbar_tab(),
            UiTabId::StatusBar => self.ui_status_bar_tab(),
            UiTabId::HelpOverlay => self.ui_help_overlay_tab(),
            UiTabId::ClickHighlight => self.ui_click_highlight_tab(),
        };

        let general = column![
            text("General UI").size(18),
            labeled_input(
                "Preferred output (xdg fallback)",
                &self.draft.ui_preferred_output,
                &self.defaults.ui_preferred_output,
                TextField::UiPreferredOutput,
            ),
            toggle_row(
                "Use fullscreen xdg fallback",
                self.draft.ui_xdg_fullscreen,
                self.defaults.ui_xdg_fullscreen,
                ToggleField::UiXdgFullscreen,
            ),
            toggle_row(
                "Enable context menu",
                self.draft.ui_context_menu_enabled,
                self.defaults.ui_context_menu_enabled,
                ToggleField::UiContextMenuEnabled,
            )
        ]
        .spacing(12);

        column![text("UI Settings").size(20), general, tab_bar, content]
            .spacing(12)
            .into()
    }

    fn ui_toolbar_tab(&self) -> Element<'_, Message> {
        let toolbar_layout = pick_list(
            ToolbarLayoutModeOption::list(),
            Some(self.draft.ui_toolbar_layout_mode),
            Message::ToolbarLayoutModeChanged,
        );
        let override_mode_pick = pick_list(
            ToolbarLayoutModeOption::list(),
            Some(self.override_mode),
            Message::ToolbarOverrideModeChanged,
        );
        let overrides = self
            .draft
            .ui_toolbar_mode_overrides
            .for_mode(self.override_mode);

        let column = column![
            text("Toolbar").size(18),
            labeled_control(
                "Layout mode",
                toolbar_layout.width(Length::Fill).into(),
                self.defaults.ui_toolbar_layout_mode.label().to_string(),
                self.draft.ui_toolbar_layout_mode != self.defaults.ui_toolbar_layout_mode,
            ),
            toggle_row(
                "Pin top toolbar",
                self.draft.ui_toolbar_top_pinned,
                self.defaults.ui_toolbar_top_pinned,
                ToggleField::UiToolbarTopPinned,
            ),
            toggle_row(
                "Pin side toolbar",
                self.draft.ui_toolbar_side_pinned,
                self.defaults.ui_toolbar_side_pinned,
                ToggleField::UiToolbarSidePinned,
            ),
            toggle_row(
                "Use icon-only buttons",
                self.draft.ui_toolbar_use_icons,
                self.defaults.ui_toolbar_use_icons,
                ToggleField::UiToolbarUseIcons,
            ),
            toggle_row(
                "Show extended colors",
                self.draft.ui_toolbar_show_more_colors,
                self.defaults.ui_toolbar_show_more_colors,
                ToggleField::UiToolbarShowMoreColors,
            ),
            toggle_row(
                "Show presets",
                self.draft.ui_toolbar_show_presets,
                self.defaults.ui_toolbar_show_presets,
                ToggleField::UiToolbarShowPresets,
            ),
            toggle_row(
                "Show actions (basic)",
                self.draft.ui_toolbar_show_actions_section,
                self.defaults.ui_toolbar_show_actions_section,
                ToggleField::UiToolbarShowActionsSection,
            ),
            toggle_row(
                "Show advanced actions",
                self.draft.ui_toolbar_show_actions_advanced,
                self.defaults.ui_toolbar_show_actions_advanced,
                ToggleField::UiToolbarShowActionsAdvanced,
            ),
            toggle_row(
                "Show Step Undo/Redo",
                self.draft.ui_toolbar_show_step_section,
                self.defaults.ui_toolbar_show_step_section,
                ToggleField::UiToolbarShowStepSection,
            ),
            toggle_row(
                "Always show text controls",
                self.draft.ui_toolbar_show_text_controls,
                self.defaults.ui_toolbar_show_text_controls,
                ToggleField::UiToolbarShowTextControls,
            ),
            toggle_row(
                "Show settings section",
                self.draft.ui_toolbar_show_settings_section,
                self.defaults.ui_toolbar_show_settings_section,
                ToggleField::UiToolbarShowSettingsSection,
            ),
            toggle_row(
                "Show delay sliders",
                self.draft.ui_toolbar_show_delay_sliders,
                self.defaults.ui_toolbar_show_delay_sliders,
                ToggleField::UiToolbarShowDelaySliders,
            ),
            toggle_row(
                "Show marker opacity controls",
                self.draft.ui_toolbar_show_marker_opacity_section,
                self.defaults.ui_toolbar_show_marker_opacity_section,
                ToggleField::UiToolbarShowMarkerOpacitySection,
            ),
            toggle_row(
                "Show tool preview bubble",
                self.draft.ui_toolbar_show_tool_preview,
                self.defaults.ui_toolbar_show_tool_preview,
                ToggleField::UiToolbarShowToolPreview,
            ),
            toggle_row(
                "Show preset action toasts",
                self.draft.ui_toolbar_show_preset_toasts,
                self.defaults.ui_toolbar_show_preset_toasts,
                ToggleField::UiToolbarPresetToasts,
            ),
            toggle_row(
                "Force inline toolbars",
                self.draft.ui_toolbar_force_inline,
                self.defaults.ui_toolbar_force_inline,
                ToggleField::UiToolbarForceInline,
            ),
            text("Mode overrides").size(16),
            row![text("Edit mode:"), override_mode_pick]
                .spacing(12)
                .align_items(iced::Alignment::Center),
            text("Default keeps the mode preset.").size(12),
            override_row(ToolbarOverrideField::ShowPresets, overrides.show_presets),
            override_row(
                ToolbarOverrideField::ShowActionsSection,
                overrides.show_actions_section,
            ),
            override_row(
                ToolbarOverrideField::ShowActionsAdvanced,
                overrides.show_actions_advanced,
            ),
            override_row(
                ToolbarOverrideField::ShowStepSection,
                overrides.show_step_section
            ),
            override_row(
                ToolbarOverrideField::ShowTextControls,
                overrides.show_text_controls
            ),
            override_row(
                ToolbarOverrideField::ShowSettingsSection,
                overrides.show_settings_section,
            ),
            text("Placement offsets").size(16),
            row![
                labeled_input(
                    "Top offset X",
                    &self.draft.ui_toolbar_top_offset,
                    &self.defaults.ui_toolbar_top_offset,
                    TextField::ToolbarTopOffset,
                ),
                labeled_input(
                    "Top offset Y",
                    &self.draft.ui_toolbar_top_offset_y,
                    &self.defaults.ui_toolbar_top_offset_y,
                    TextField::ToolbarTopOffsetY,
                )
            ]
            .spacing(12),
            row![
                labeled_input(
                    "Side offset Y",
                    &self.draft.ui_toolbar_side_offset,
                    &self.defaults.ui_toolbar_side_offset,
                    TextField::ToolbarSideOffset,
                ),
                labeled_input(
                    "Side offset X",
                    &self.draft.ui_toolbar_side_offset_x,
                    &self.defaults.ui_toolbar_side_offset_x,
                    TextField::ToolbarSideOffsetX,
                )
            ]
            .spacing(12),
        ]
        .spacing(12);

        scrollable(column).into()
    }

    fn ui_status_bar_tab(&self) -> Element<'_, Message> {
        let status_position = pick_list(
            StatusPositionOption::list(),
            Some(self.draft.ui_status_position),
            Message::StatusPositionChanged,
        );

        let column = column![
            text("Status Bar").size(18),
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
            labeled_control(
                "Status bar position",
                status_position.width(Length::Fill).into(),
                self.defaults.ui_status_position.label().to_string(),
                self.draft.ui_status_position != self.defaults.ui_status_position,
            ),
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
        ]
        .spacing(12);

        scrollable(column).into()
    }

    fn ui_help_overlay_tab(&self) -> Element<'_, Message> {
        let column = column![
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
        ]
        .spacing(12);

        scrollable(column).into()
    }

    fn ui_click_highlight_tab(&self) -> Element<'_, Message> {
        let column = column![
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
            labeled_control(
                "Default mode",
                board_mode_pick.width(Length::Fill).into(),
                self.defaults.board_default_mode.label().to_string(),
                self.draft.board_default_mode != self.defaults.board_default_mode,
            ),
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
                ),
                toggle_row(
                    "Always exit overlay after capture",
                    self.draft.capture_exit_after,
                    self.defaults.capture_exit_after,
                    ToggleField::CaptureExitAfter,
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
                "Persist undo/redo history",
                self.draft.session_persist_history,
                self.defaults.session_persist_history,
                ToggleField::SessionPersistHistory,
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
            labeled_control(
                "Storage mode",
                storage_pick.width(Length::Fill).into(),
                self.defaults.session_storage_mode.label().to_string(),
                self.draft.session_storage_mode != self.defaults.session_storage_mode,
            ),
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
            .push(labeled_control(
                "Compression",
                compression_pick.width(Length::Fill).into(),
                self.defaults.session_compression.label().to_string(),
                self.draft.session_compression != self.defaults.session_compression,
            ))
            .push(labeled_input(
                "Max shapes per frame",
                &self.draft.session_max_shapes_per_frame,
                &self.defaults.session_max_shapes_per_frame,
                TextField::SessionMaxShapesPerFrame,
            ))
            .push(labeled_input(
                "Max persisted undo depth (blank = runtime limit)",
                &self.draft.session_max_persisted_undo_depth,
                &self.defaults.session_max_persisted_undo_depth,
                TextField::SessionMaxPersistedUndoDepth,
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

    fn tablet_tab(&self) -> Element<'_, Message> {
        scrollable(
            column![
                text("Tablet / Stylus").size(20),
                toggle_row(
                    "Enable tablet input",
                    self.draft.tablet_enabled,
                    self.defaults.tablet_enabled,
                    ToggleField::TabletEnabled,
                ),
                toggle_row(
                    "Enable pressure-to-thickness",
                    self.draft.tablet_pressure_enabled,
                    self.defaults.tablet_pressure_enabled,
                    ToggleField::TabletPressureEnabled,
                ),
                row![
                    labeled_input(
                        "Min thickness",
                        &self.draft.tablet_min_thickness,
                        &self.defaults.tablet_min_thickness,
                        TextField::TabletMinThickness,
                    ),
                    labeled_input(
                        "Max thickness",
                        &self.draft.tablet_max_thickness,
                        &self.defaults.tablet_max_thickness,
                        TextField::TabletMaxThickness,
                    )
                ]
                .spacing(12),
            ]
            .spacing(12),
        )
        .into()
    }

    fn keybindings_tab(&self) -> Element<'_, Message> {
        let tab_bar = KeybindingsTabId::ALL.iter().fold(
            Row::new().spacing(8).align_items(iced::Alignment::Center),
            |row, tab| {
                let label = tab.title();
                let button = button(label)
                    .padding([6, 12])
                    .style(if *tab == self.active_keybindings_tab {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::KeybindingsTabSelected(*tab));
                row.push(button)
            },
        );

        let mut column = Column::new()
            .spacing(8)
            .push(text("Keybindings (comma-separated)").size(20))
            .push(tab_bar);

        for entry in self
            .draft
            .keybindings
            .entries
            .iter()
            .filter(|entry| entry.field.tab() == self.active_keybindings_tab)
        {
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
                        row![default_value_text(default_value.to_string(), changed)]
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
        row![text(label).size(14), default_value_text(default, changed)]
            .spacing(DEFAULT_LABEL_GAP)
            .align_items(iced::Alignment::Center),
        text_input(label, value).on_input(move |val| Message::TextChanged(field, val))
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

fn labeled_control<'a>(
    label: &'static str,
    control: Element<'a, Message>,
    default: impl Into<String>,
    changed: bool,
) -> Element<'a, Message> {
    column![
        row![text(label).size(14), default_value_text(default, changed)]
            .spacing(DEFAULT_LABEL_GAP)
            .align_items(iced::Alignment::Center),
        control
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

fn preset_input<'a>(
    label: &'static str,
    value: &'a str,
    default: &'a str,
    slot_index: usize,
    field: PresetTextField,
    show_unset: bool,
) -> Element<'a, Message> {
    let changed = value.trim() != default.trim();
    let default_label = if show_unset && default.trim().is_empty() {
        "unset".to_string()
    } else {
        default.trim().to_string()
    };

    column![
        row![
            text(label).size(14),
            default_value_text(default_label, changed)
        ]
        .spacing(DEFAULT_LABEL_GAP)
        .align_items(iced::Alignment::Center),
        text_input(label, value)
            .on_input(move |val| Message::PresetTextChanged(slot_index, field, val))
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

fn preset_override_control<'a>(
    label: &'static str,
    value: OverrideOption,
    default: OverrideOption,
    slot_index: usize,
    field: PresetToggleField,
) -> Element<'a, Message> {
    let picker = pick_list(OverrideOption::list(), Some(value), move |opt| {
        Message::PresetToggleOptionChanged(slot_index, field, opt)
    })
    .width(Length::Fill);

    labeled_control(label, picker.into(), default.label(), value != default)
}

fn override_row<'a>(field: ToolbarOverrideField, value: OverrideOption) -> Element<'a, Message> {
    row![
        text(field.label()).size(14),
        pick_list(OverrideOption::list(), Some(value), move |opt| {
            Message::ToolbarOverrideChanged(field, opt)
        },)
        .width(Length::Fixed(140.0)),
    ]
    .spacing(12)
    .align_items(iced::Alignment::Center)
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
            default_value_text(default.summary(), changed),
        ]
        .spacing(DEFAULT_LABEL_GAP)
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
            default_value_text(default.summary(), changed),
        ]
        .spacing(DEFAULT_LABEL_GAP)
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

const DEFAULT_LABEL_GAP: f32 = 12.0;

fn default_label_color(changed: bool) -> iced::Color {
    if changed {
        iced::Color::from_rgb(0.95, 0.6, 0.2)
    } else {
        iced::Color::from_rgb(0.65, 0.74, 0.82)
    }
}

fn default_value_text<'a>(value: impl Into<String>, changed: bool) -> iced::widget::Text<'a> {
    let label = format!("Default: {}", value.into());
    text(label)
        .size(12)
        .style(theme::Text::Color(default_label_color(changed)))
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
        default_value_text(bool_label(default), changed),
    ]
    .spacing(DEFAULT_LABEL_GAP)
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
