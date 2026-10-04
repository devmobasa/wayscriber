//! The unit-test binary doubles as the overlay process for real-child tests.
//!
//! Spawn candidate discovery tries `current_exe()` first, which under
//! `cargo test` is this test binary. A fixture marks the environment of the
//! broker it starts; when the daemon then launches this binary as an overlay
//! child, the constructor below runs the production child handshake, reports a
//! session when the test asks for one, and records how it was launched, before
//! libtest would parse the overlay arguments. Without both the fixture marker
//! and an overlay generation, the constructor returns and the binary runs its
//! tests as usual.

use std::convert::Infallible;
use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::daemon::protocol_v2::{
    ActiveGeneration, ReportedSession, active_generation_from_environment,
    publish_ready_from_environment, publish_session_from_environment,
    publish_signal_ready_from_environment,
};

/// Fixture marker inherited by overlay launches from the test's broker.
pub(super) const FAKE_OVERLAY_ENV: &str = "WAYSCRIBER_TEST_FAKE_OVERLAY";
/// Behave like an overlay that exits on SIGTERM.
pub(super) const STOPS_ON_TERM: &str = "stops-on-term";
/// Behave like an overlay that ignores SIGTERM and must be force-killed.
pub(super) const IGNORES_TERM: &str = "ignores-term";
/// Launched under this name, through a link to the test binary, the fake exits
/// before publishing readiness, like an overlay that dies during startup. The
/// marker above applies to every launch from the fixture's broker, so this
/// behavior is chosen per launch instead.
pub(super) const EXITS_BEFORE_READY: &str = "wayscriber-exits-before-ready";
/// Written to the fixture's runtime directory as that child starts, so a test
/// can tell a child that ran and exited from one that never started.
pub(super) const STARTED_THEN_EXITED: &str = "started-then-exited";
/// A test writes this JSON object to the fixture's runtime directory before a
/// show to direct the next fake overlay: `report` is the session it reports
/// once enabled, `"home"` or a path, and `exit` makes it exit after that.
pub(super) const SESSION_INSTRUCTION: &str = "fake-overlay-session.json";
const EXIT_BEFORE_READY_STATUS: i32 = 7;
/// The fake overlay could not serve; the test that launched it then fails.
const FAILURE_STATUS: i32 = 1;

// SAFETY: the loader calls each `.init_array` entry once, before `main`, with
// the C calling convention. glibc passes `argc`, `argv`, and `envp`; under the
// C ABI a function that declares no parameters ignores extra arguments, so an
// `extern "C" fn()` is sound to register here. Without both markers the
// function only reads the environment and returns. With them it never returns
// into `main`: it serves or exits, and a panic aborts instead of unwinding
// through the loader because the function is `extern "C"`.
#[used]
#[unsafe(link_section = ".init_array")]
static RUN_AS_FAKE_OVERLAY: extern "C" fn() = run_as_fake_overlay;

extern "C" fn run_as_fake_overlay() {
    let Some(behavior) = std::env::var_os(FAKE_OVERLAY_ENV) else {
        return;
    };
    if std::env::var_os(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV).is_none() {
        return;
    }

    if launched_as(EXITS_BEFORE_READY) {
        if let Err(error) = record(STARTED_THEN_EXITED, b"") {
            eprintln!("fake overlay failed: {error:#}");
        }
        std::process::exit(EXIT_BEFORE_READY_STATUS);
    }

    let Err(error) = serve(behavior == IGNORES_TERM);
    eprintln!("fake overlay failed: {error:#}");
    std::process::exit(FAILURE_STATUS);
}

fn serve(ignore_term: bool) -> Result<Infallible> {
    // SAFETY: installs ignore dispositions only; no handler code runs.
    unsafe {
        // Action delivery signals the overlay; the fake has no action handler.
        libc::signal(libc::SIGUSR2, libc::SIG_IGN);
        if ignore_term {
            libc::signal(libc::SIGTERM, libc::SIG_IGN);
        }
    }

    publish_ready_from_environment()?;
    publish_signal_ready_from_environment()?;
    while !matches!(
        active_generation_from_environment()?,
        ActiveGeneration::Enabled { .. }
    ) {
        std::thread::sleep(Duration::from_millis(2));
    }

    // Reported before the receipt, so a test that has the receipt can rely on
    // the report.
    let exit = report_session()?;
    write_receipt()?;
    if exit {
        std::process::exit(0);
    }

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

/// Follows the test's [`SESSION_INSTRUCTION`], returning whether to exit.
fn report_session() -> Result<bool> {
    let instruction = match std::fs::read(runtime_root()?.join(SESSION_INSTRUCTION)) {
        Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("failed to read the session instruction"),
    };
    if let Some(report) = instruction["report"].as_str() {
        let session = match report {
            "home" => ReportedSession::Home,
            path => ReportedSession::Named(path.into()),
        };
        anyhow::ensure!(
            publish_session_from_environment(&session)?,
            "the daemon did not ask for session reports"
        );
    }

    Ok(instruction["exit"].as_bool().unwrap_or(false))
}

fn write_receipt() -> Result<()> {
    // As launched: GTK may already have unset the startup-notification variables.
    let launched_with = |name: &str| {
        crate::launch_environment::var_os(name).map(|value| value.to_string_lossy().into_owned())
    };
    let receipt = serde_json::json!({
        "args": launch_arguments()?,
        "token": launched_with(crate::env_vars::XDG_ACTIVATION_TOKEN_ENV),
        "startup": launched_with(crate::env_vars::DESKTOP_STARTUP_ID_ENV),
        "resume": launched_with(crate::RESUME_SESSION_ENV),
        "detach": launched_with(crate::env_vars::NO_DETACH_ENV),
        "reports": launched_with(crate::env_vars::OVERLAY_SESSION_REPORTS_ENV),
        "home": launched_with(crate::env_vars::OVERLAY_HOME_SESSION_ENV),
        "preferred": launched_with(crate::env_vars::OVERLAY_PREFERRED_SESSION_ENV),
        "pid": std::process::id(),
        "generation": std::env::var(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV)?,
    });

    record("receipt", &serde_json::to_vec(&receipt)?)
}

/// Writes `<generation>.<kind>` into the fixture's runtime directory.
fn record(kind: &str, contents: &[u8]) -> Result<()> {
    let generation = std::env::var(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV)?;

    crate::durable_io::write_atomic(
        &runtime_root()?.join(format!("{generation}.{kind}")),
        contents,
        crate::durable_io::AtomicWriteOptions::private_runtime_file(),
    )?;

    Ok(())
}

fn runtime_root() -> Result<std::path::PathBuf> {
    std::env::var_os(crate::env_vars::XDG_RUNTIME_DIR_ENV)
        .map(Into::into)
        .context("fake overlay needs the fixture's runtime directory")
}

/// Whether this process was started as `name`: the broker passes the program
/// path it was given as `argv[0]`, which for a link is the link's own path.
fn launched_as(name: &str) -> bool {
    crate::test_fake_helper::launch_arguments()
        .ok()
        .and_then(|arguments| arguments.into_iter().next())
        .is_some_and(|program| Path::new(&program).file_name() == Some(OsStr::new(name)))
}

/// The overlay arguments this process was launched with, after `argv[0]`.
fn launch_arguments() -> Result<Vec<String>> {
    Ok(crate::test_fake_helper::launch_arguments()?
        .into_iter()
        .skip(1)
        .collect())
}
