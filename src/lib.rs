//! Wayscriber library and application entry point.
//!
//! Exposes reusable configuration and domain modules for companion tools while
//! keeping the desktop runtime private behind [`run_from_env`].

mod about_window;
mod app;
mod app_id;
mod backend;
pub(crate) mod base64;
pub mod build_info;
pub mod canvas_export;
pub mod capture;
mod cli;
pub mod config;
mod daemon;
pub mod domain;
pub mod draw;
pub mod durable_io;
pub mod env_vars;
pub(crate) mod file_uri;
pub(crate) mod image_decode;
pub mod input;
mod label_format;
mod logger;
mod notification;
mod onboarding;
pub mod palette_recents;
pub mod paths;
mod process_broker;
pub mod render_profiles;
pub mod runtime_capabilities;
// Phases 2-3 establish the controller and storage contracts before later
// phases route live UI producers through them.
#[allow(dead_code)]
pub(crate) mod runtime_ui_state;
pub mod session;
mod session_override;
pub mod shortcut_hint;
pub mod systemd_user_service;
#[cfg(test)]
pub(crate) mod test_env;
#[cfg(test)]
pub(crate) mod test_temp;
pub mod time_utils;
mod toolbar_gtk;
pub mod toolbar_icons;
mod tray_action;
pub mod ui;
pub(crate) mod ui_text;
#[cfg(unix)]
mod unix_signals;
pub mod util;
#[cfg(feature = "portal")]
pub(crate) mod zbus_stream;

pub use config::Config;
pub(crate) use session_override::{
    RESUME_SESSION_ENV, decode_session_override, encode_session_override, runtime_session_override,
    set_runtime_session_override,
};

use std::process::ExitCode;
use std::sync::Mutex;

use cli::CliOutcome;

static RUN_ENTRY_LEASE: Mutex<()> = Mutex::new(());

/// Run wayscriber using the current process arguments and return its process exit status.
pub fn run_from_env() -> ExitCode {
    let _run_entry = RUN_ENTRY_LEASE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(exit_code) = process_broker::run_internal_broker_if_requested() {
        return exit_code;
    }
    match cli::Cli::parse() {
        Ok(CliOutcome::Run(cli)) => {
            logger::init(cli.daemon || cli.active);
            exit_code_for_app_result(app::run(cli))
        }
        Ok(CliOutcome::Help) => {
            cli::print_help();
            ExitCode::SUCCESS
        }
        Ok(CliOutcome::Version) => {
            cli::print_version();
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            eprintln!("Try 'wayscriber --help' for usage.");
            ExitCode::from(2)
        }
    }
}

fn exit_code_for_app_result(result: anyhow::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let already_running = err
                .chain()
                .any(|cause| cause.is::<daemon::AlreadyRunningError>());
            if already_running {
                eprintln!("wayscriber daemon is already running");
                ExitCode::from(75)
            } else {
                eprintln!("{err:#}");
                ExitCode::FAILURE
            }
        }
    }
}
