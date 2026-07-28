use iced::{Settings, Size, application, window};

use crate::models::StartupRequest;

use super::state::ConfiguratorApp;

pub(crate) fn run(startup: StartupRequest) -> iced::Result {
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
        // The launch request is state the app starts with, so it travels into
        // the model through the boot closure rather than through a static that
        // the model would have to reach out for.
        move || ConfiguratorApp::new_app_with_startup(startup.clone()),
        ConfiguratorApp::update_message,
        ConfiguratorApp::view,
    )
    .title("Wayscriber Configurator (Iced)")
    .subscription(ConfiguratorApp::subscription)
    .theme(iced::Theme::Dark)
    .settings(settings)
    .window(window)
    .run()
}
