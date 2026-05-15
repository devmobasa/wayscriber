use iced::{Settings, Size, application, window};

use super::state::ConfiguratorApp;

pub fn run() -> iced::Result {
    let settings = Settings {
        id: Some("wayscriber-configurator".to_string()),
        ..Settings::default()
    };
    let mut window = window::Settings {
        size: Size::new(960.0, 640.0),
        resizable: true,
        decorations: true,
        ..window::Settings::default()
    };
    #[cfg(target_os = "linux")]
    {
        window.platform_specific.application_id = "wayscriber-configurator".to_string();
    }
    application(
        ConfiguratorApp::new_app,
        ConfiguratorApp::update_message,
        ConfiguratorApp::view,
    )
    .title("Wayscriber Configurator (Iced)")
    .theme(iced::Theme::Dark)
    .settings(settings)
    .window(window)
    .run()
}
