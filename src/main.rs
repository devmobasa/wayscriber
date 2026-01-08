mod about_window;
mod app;
mod backend;
mod capture;
mod cli;
mod config;
mod daemon;
mod draw;
mod input;
mod label_format;
mod notification;
mod onboarding;
mod paths;
mod session;
mod session_override;
mod time_utils;
mod toolbar_icons;
mod tray_action;
mod ui;
mod util;

pub use session_override::{
    RESUME_SESSION_ENV, SESSION_OVERRIDE_FOLLOW_CONFIG, SESSION_OVERRIDE_FORCE_OFF,
    SESSION_OVERRIDE_FORCE_ON, SESSION_RESUME_OVERRIDE, decode_session_override,
    encode_session_override, runtime_session_override, set_runtime_session_override,
};

fn main() {
    env_logger::init();

    if let Err(err) = app::run() {
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
