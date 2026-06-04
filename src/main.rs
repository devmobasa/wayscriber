mod about_window;
mod app;
mod app_id;
mod backend;
mod base64;
mod build_info;
mod canvas_export;
mod capture;
mod cli;
mod config;
mod daemon;
mod draw;
mod file_uri;
mod image_decode;
mod input;
mod label_format;
mod logger;
mod notification;
mod onboarding;
mod paths;
mod render_profiles;
mod session;
mod session_override;
#[cfg(test)]
mod test_env;
#[cfg(test)]
mod test_temp;
mod time_utils;
mod toolbar_icons;
mod tray_action;
mod ui;
pub(crate) mod ui_text;
#[cfg(unix)]
mod unix_signals;
mod util;
#[cfg(feature = "portal")]
mod zbus_stream;

pub use session_override::{
    RESUME_SESSION_ENV, SESSION_OVERRIDE_FOLLOW_CONFIG, SESSION_OVERRIDE_FORCE_OFF,
    SESSION_OVERRIDE_FORCE_ON, SESSION_RESUME_OVERRIDE, decode_session_override,
    encode_session_override, runtime_session_override, set_runtime_session_override,
};

fn main() {
    let cli = cli::Cli::parse();
    logger::init(cli.daemon || cli.active);

    if let Err(err) = app::run(cli) {
        let already_running = err
            .chain()
            .any(|cause| cause.is::<daemon::AlreadyRunningError>());
        if already_running {
            eprintln!("wayscriber daemon is already running");
            std::process::exit(75);
        }
        eprintln!("{:#}", err);
        std::process::exit(1);
    }
}
