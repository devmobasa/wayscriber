mod env;
mod session;
mod usage;

use crate::backend::ExitAfterCaptureMode;
use crate::cli::Cli;
use crate::daemon::DaemonToggleRequest;
use crate::env_vars::{DETACHED_ENV, NO_DETACH_ENV, NO_TRAY_ENV, WAYLAND_DISPLAY_ENV};
use crate::paths::{PathResolver, PreparedRuntimePaths};
use crate::session::try_lock_exclusive;
use anyhow::Context;
use env::env_flag_enabled;
use session::run_session_cli_commands;
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use usage::{log_overlay_controls, print_usage};

fn acquire_overlay_lock(runtime_paths: &PreparedRuntimePaths) -> anyhow::Result<Option<File>> {
    let lock_path = runtime_paths.overlay_lock_file();
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)?;

    match try_lock_exclusive(&lock_file) {
        Ok(()) => Ok(Some(lock_file)),
        Err(err) if err.kind() == ErrorKind::WouldBlock => {
            log::warn!("Overlay already running; skipping duplicate --active launch");
            Ok(None)
        }
        Err(err) => Err(err.into()),
    }
}

fn maybe_detach_active(
    cli: &Cli,
    process_broker: &crate::process_broker::ProcessBrokerHandle,
) -> anyhow::Result<bool> {
    if !(cli.active || cli.freeze) {
        return Ok(false);
    }
    if env_flag_enabled(NO_DETACH_ENV) || std::env::var_os(DETACHED_ENV).is_some() {
        return Ok(false);
    }
    let exe = std::env::current_exe()?;
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    process_broker.spawn(
        crate::process_broker::HelperKind::InitialDetach,
        crate::process_broker::HelperLifetime::DetachedAfterExec,
        exe.as_os_str(),
        args,
        vec![(DETACHED_ENV.into(), Some("1".into()))],
    )?;
    Ok(true)
}

fn normalized_named_session_file(
    cli: &Cli,
    paths: &crate::paths::PathResolver,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(raw_path) = cli.session_file.as_ref() else {
        return Ok(None);
    };
    let raw = raw_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("--session-file path must be valid UTF-8"))?;
    let current_dir = std::env::current_dir()
        .context("failed to resolve current directory for --session-file")?;
    Ok(Some(crate::session::normalize_named_session_file_arg(
        raw,
        paths,
        &current_dir,
    )?))
}

fn daemon_request_session_file(path: Option<PathBuf>) -> anyhow::Result<Option<PathBuf>> {
    let Some(path) = path else {
        return Ok(None);
    };
    if !path.is_absolute() {
        return Err(anyhow::anyhow!(
            "internal daemon request session path was not anchored"
        ));
    }
    Ok(Some(path))
}

fn preflight_named_overlay_session(cli: &Cli, path: Option<&Path>) -> anyhow::Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    if cli.active || cli.freeze || cli.daemon || cli.daemon_toggle {
        crate::session::validate_named_session_file_for_foreground(path)?;
    }
    if cli.active || cli.freeze {
        crate::backend::preflight_wayland_connection()?;
    }
    Ok(())
}

/// `--check-update`: an explicit, user-initiated check. It bypasses the
/// `[updates] check` setting and the opt-out variable — asking for a check is
/// consent to make the request — and never changes anything on disk beyond the
/// cached result.
enum ExplicitUpdateCheck<'a> {
    CompiledOut(&'static str),
    Brokered(&'a crate::process_broker::ProcessBrokerHandle),
}

const fn update_modes_need_process_broker(
    about: bool,
    check_update: bool,
    update_check_compiled_out: bool,
) -> bool {
    about || (check_update && !update_check_compiled_out)
}

fn runtime_signal_profile(cli: &Cli) -> Option<crate::unix_signals::SignalProfile> {
    if cli.daemon {
        Some(crate::unix_signals::SignalProfile::Daemon)
    } else if !cli.daemon_toggle && (cli.active || cli.freeze) {
        Some(crate::unix_signals::SignalProfile::Overlay)
    } else {
        None
    }
}

fn finish_signal_owner(
    run_result: anyhow::Result<()>,
    signal_owner: &mut Option<crate::unix_signals::SignalOwner>,
) -> anyhow::Result<()> {
    let finish_result = match signal_owner.as_mut() {
        Some(owner) => owner
            .finish()
            .context("failed to restore the root signal mask"),
        None => Ok(()),
    };
    match (run_result, finish_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(run_error), Ok(())) => Err(run_error),
        (Ok(()), Err(finish_error)) => Err(finish_error),
        (Err(run_error), Err(finish_error)) => Err(anyhow::anyhow!(
            "{run_error:#}; signal teardown also failed: {finish_error:#}"
        )),
    }
}

fn finish_daemon_watchdog(
    run_result: anyhow::Result<()>,
    watchdog: &mut Option<crate::daemon::protocol_v2::DaemonWatchdogOwner>,
) -> anyhow::Result<()> {
    let finish_result = match watchdog.as_mut() {
        Some(owner) => owner.finish().context("failed to stop daemon watchdog"),
        None => Ok(()),
    };
    match (run_result, finish_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(run_error), Ok(())) => Err(run_error),
        (Ok(()), Err(finish_error)) => Err(finish_error),
        (Err(run_error), Err(finish_error)) => Err(anyhow::anyhow!(
            "{run_error:#}; daemon watchdog teardown also failed: {finish_error:#}"
        )),
    }
}

fn finish_logger_owner(
    run_result: anyhow::Result<()>,
    logger_owner: &mut crate::logger::LoggerOwner,
) -> anyhow::Result<()> {
    let finish_result = logger_owner
        .finish()
        .context("failed to drain and flush the ordinary logger owner");
    match (run_result, finish_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(run_error), Ok(())) => Err(run_error),
        (Ok(()), Err(finish_error)) => Err(finish_error),
        (Err(run_error), Err(finish_error)) => Err(anyhow::anyhow!(
            "{run_error:#}; logger teardown also failed: {finish_error:#}"
        )),
    }
}

fn prepare_explicit_update_check(
    update_check_compiled_out: bool,
    process_broker: Option<&crate::process_broker::ProcessBrokerHandle>,
) -> anyhow::Result<ExplicitUpdateCheck<'_>> {
    if update_check_compiled_out {
        return Ok(ExplicitUpdateCheck::CompiledOut(
            crate::update_check::COMPILED_OUT_MESSAGE,
        ));
    }
    process_broker
        .map(ExplicitUpdateCheck::Brokered)
        .ok_or_else(|| anyhow::anyhow!("update-check process broker was not started"))
}

fn run_update_check(
    process_broker: Option<&crate::process_broker::ProcessBrokerHandle>,
    cache_store: &crate::update_check::UpdateCacheStore,
) -> anyhow::Result<()> {
    use crate::update_check::{CheckOutcome, check_now, compiled_out, current_version};

    println!("Installed version: {}", current_version());
    let process_broker = match prepare_explicit_update_check(compiled_out(), process_broker)? {
        ExplicitUpdateCheck::CompiledOut(message) => {
            println!("{message}; ask your package manager for updates.");
            return Ok(());
        }
        ExplicitUpdateCheck::Brokered(process_broker) => process_broker,
    };
    match check_now(process_broker, cache_store) {
        Ok(CheckOutcome::UpToDate { latest }) => {
            println!("Latest release:    {latest}");
            println!("Wayscriber is up to date.");
            Ok(())
        }
        Ok(CheckOutcome::Update(update)) => {
            println!("Latest release:    {}", update.version);
            if let Some(released) = update.released.as_deref() {
                println!("Released:          {released}");
            }
            println!();
            println!("An update is available. Wayscriber does not install updates itself.");
            println!("Update instructions: {}", update.update_url);
            println!("Release notes:       {}", update.notes_url);
            Ok(())
        }
        Err(err) => Err(anyhow::anyhow!("Update check failed: {err}")),
    }
}

#[cfg(unix)]
fn detach_from_tty() {
    // Start a new session to drop the controlling terminal (prevents stuck shells).
    unsafe {
        let _ = libc::setsid();
    }
    // Best-effort close of stdio if they still point to a TTY.
    for fd in [libc::STDIN_FILENO, libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        let is_tty = unsafe { libc::isatty(fd) } == 1;
        if is_tty {
            let _ = unsafe { libc::close(fd) };
        }
    }
}

pub fn run(cli: Cli, path_resolver: PathResolver) -> anyhow::Result<()> {
    if cli.runtime_capabilities {
        print!(
            "{}",
            crate::runtime_capabilities::render_runtime_capabilities(
                crate::runtime_capabilities::current_runtime_capabilities()
            )
        );
        return Ok(());
    }

    let signal_profile = runtime_signal_profile(&cli);
    if signal_profile.is_none()
        && std::env::var_os(crate::env_vars::DAEMON_WATCHDOG_FD_ENV).is_some()
    {
        return Err(anyhow::anyhow!(
            "daemon watchdog inheritance requires an overlay or daemon runtime signal owner"
        ));
    }
    // Protect and consume the internal watchdog capability before creating a
    // subprocess. Preparation owns no thread; activation waits until after the
    // root signal mask is installed.
    let prepared_daemon_watchdog =
        crate::daemon::protocol_v2::prepare_daemon_watchdog_from_environment()?;

    // Runtime and update-checking modes create their process broker before
    // acquiring locks or starting threads. The guard spans the complete run.
    let update_mode_needs_broker = update_modes_need_process_broker(
        cli.about,
        cli.check_update,
        crate::update_check::compiled_out(),
    );
    let prepared_process_broker =
        (cli.daemon || cli.active || cli.freeze || update_mode_needs_broker)
            .then(crate::process_broker::prepare_for_runtime)
            .transpose()?;
    // The broker subprocess is execed above with the caller's exact pre-entry
    // signal mask. Install the root signal descriptor before activating the
    // broker actor or starting watchdog/Tokio/daemon threads, so every ordinary
    // application thread inherits the blocked runtime signals while brokered
    // helpers preserve that caller-supplied baseline.
    let mut signal_owner = signal_profile
        .map(crate::unix_signals::SignalOwner::install)
        .transpose()?;
    // The explicit logger worker is an ordinary application thread. Start it
    // only after the root mask is installed, and join it before restoring that
    // mask on every exit.
    let (mut logger_owner, logger) =
        match crate::logger::LoggerOwner::start(cli.daemon || cli.active, &path_resolver) {
            Ok(logger) => logger,
            Err(error) => {
                drop(prepared_process_broker);
                return finish_signal_owner(Err(error.into()), &mut signal_owner);
            }
        };
    // Start the prepared watchdog before any other ordinary application
    // runtime thread. It inherits the root mask and remains owned through
    // teardown.
    let mut daemon_watchdog = match prepared_daemon_watchdog
        .map(crate::daemon::protocol_v2::PreparedDaemonWatchdog::start)
        .transpose()
    {
        Ok(owner) => owner,
        Err(error) => {
            drop(prepared_process_broker);
            logger.error(
                "wayscriber::entry",
                format!("ordinary run failed: {error:#}"),
            );
            drop(logger);
            let result = finish_logger_owner(Err(error), &mut logger_owner);
            return finish_signal_owner(result, &mut signal_owner);
        }
    };
    let process_broker_owner = match prepared_process_broker
        .map(crate::process_broker::PreparedProcessBroker::activate)
        .transpose()
    {
        Ok(owner) => owner,
        Err(error) => {
            let result = finish_daemon_watchdog(Err(error), &mut daemon_watchdog);
            if let Err(error) = &result {
                logger.error(
                    "wayscriber::entry",
                    format!("ordinary run failed: {error:#}"),
                );
            }
            drop(logger);
            let result = finish_logger_owner(result, &mut logger_owner);
            return finish_signal_owner(result, &mut signal_owner);
        }
    };
    let process_broker = process_broker_owner
        .as_ref()
        .map(crate::process_broker::ProcessBrokerOwner::handle);
    let run_result = (|| -> anyhow::Result<()> {
        logger.info("wayscriber::app", "ordinary application root started");
        #[cfg(unix)]
        if std::env::var_os(DETACHED_ENV).is_some() {
            detach_from_tty();
        }

        let config_store = crate::config::ConfigStore::from_resolver(&path_resolver)?;
        let runtime_paths = (cli.daemon
            || cli.daemon_toggle
            || cli.active
            || cli.freeze
            || cli.daemon_overlay_action().ok().flatten().is_some())
        .then(|| PreparedRuntimePaths::prepare(&path_resolver))
        .transpose()?;

        let named_session_file = normalized_named_session_file(&cli, &path_resolver)?;
        preflight_named_overlay_session(&cli, named_session_file.as_deref())?;

        let named_overlay_session =
            named_session_file.is_some() && (cli.active || cli.freeze || cli.daemon);
        let session_override = if named_overlay_session || cli.resume_session {
            Some(true)
        } else if cli.no_resume_session {
            Some(false)
        } else {
            None
        };

        if cli.about {
            let process_broker = process_broker
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("About window process broker was not started"))?;
            let update_cache =
                crate::update_check::UpdateCacheStore::from_resolver(&path_resolver)?;
            crate::about_window::run_about_window(process_broker, &config_store, update_cache)?;
            return Ok(());
        }

        if cli.check_update {
            let update_cache =
                crate::update_check::UpdateCacheStore::from_resolver(&path_resolver)?;
            return run_update_check(process_broker.as_ref(), &update_cache);
        }

        if let Some(action) = cli
            .daemon_overlay_action()
            .map_err(|err| anyhow::anyhow!(err))?
        {
            crate::daemon::send_daemon_overlay_action(
                action,
                runtime_paths
                    .as_ref()
                    .context("daemon runtime identity was not prepared")?,
            )?;
            return Ok(());
        }

        if cli.daemon_toggle {
            let session_file = daemon_request_session_file(named_session_file)?;
            let request = DaemonToggleRequest {
                mode: cli.mode,
                freeze: cli.freeze,
                exit_after_capture: cli.exit_after_capture,
                no_exit_after_capture: cli.no_exit_after_capture,
                resume_session: cli.resume_session,
                no_resume_session: cli.no_resume_session,
                session_file,
                overlay_action: None,
            };
            crate::daemon::send_daemon_toggle_request(
                &request,
                runtime_paths
                    .as_ref()
                    .context("daemon runtime identity was not prepared")?,
            )?;
            return Ok(());
        }

        if cli.clear_session || cli.clear_tool_state || cli.session_info {
            run_session_cli_commands(&cli, &config_store, &path_resolver)?;
            return Ok(());
        }

        // Check for Wayland environment
        if std::env::var(WAYLAND_DISPLAY_ENV).is_err() && (cli.daemon || cli.active || cli.freeze) {
            return Err(anyhow::anyhow!(
                "{WAYLAND_DISPLAY_ENV} not set - this application requires Wayland."
            ));
        }

        if cli.daemon {
            // Daemon mode: background service with toggle activation
            log::info!("Starting in daemon mode");
            let tray_disabled = cli.no_tray || env_flag_enabled(NO_TRAY_ENV);
            if tray_disabled {
                log::info!("Tray disabled via --no-tray / {NO_TRAY_ENV}");
            }
            let process_broker = process_broker
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("daemon process broker was not started"))?
                .clone();
            let mut daemon = crate::daemon::Daemon::new_with_process_broker(
                crate::daemon::DaemonLaunchOptions {
                    initial_mode: cli.mode,
                    tray_enabled: !tray_disabled,
                    session_resume_override: session_override,
                    initial_named_session_file: named_session_file,
                },
                crate::daemon::DaemonRuntimeOwners {
                    process_broker,
                    path_resolver: path_resolver.clone(),
                    runtime_paths: runtime_paths
                        .clone()
                        .context("daemon runtime identity was not prepared")?,
                    config_store: config_store.clone(),
                    logger: logger.clone(),
                },
            );
            daemon.set_freeze_on_show(cli.freeze_on_show);
            daemon.run(
                signal_owner
                    .as_mut()
                    .context("daemon signal owner was not installed")?,
            )?;
        } else if cli.active || cli.freeze {
            let process_broker = process_broker
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("overlay process broker was not started"))?;
            if maybe_detach_active(&cli, process_broker)? {
                return Ok(());
            }
            let _overlay_lock = match acquire_overlay_lock(
                runtime_paths
                    .as_ref()
                    .context("overlay runtime identity was not prepared")?,
            )? {
                Some(lock) => lock,
                None => return Ok(()),
            };
            crate::daemon::protocol_v2::publish_ready_from_environment(
                &runtime_paths
                    .as_ref()
                    .context("overlay runtime identity was not prepared")?
                    .protocol_v2_root(),
            )
            .context("failed to publish daemon overlay readiness")?;
            // One-shot mode: show overlay immediately and exit when done
            log_overlay_controls(cli.freeze);

            let exit_after_capture_mode = if cli.exit_after_capture {
                ExitAfterCaptureMode::Always
            } else if cli.no_exit_after_capture {
                ExitAfterCaptureMode::Never
            } else {
                ExitAfterCaptureMode::Auto
            };

            // Run Wayland backend
            crate::backend::run_wayland(
                crate::backend::WaylandRunContext {
                    initial_mode: cli.mode,
                    freeze_on_start: cli.freeze,
                    exit_after_capture_mode,
                    named_session_file,
                    session_resume_override: session_override,
                    process_broker: process_broker.clone(),
                    path_resolver: path_resolver.clone(),
                    runtime_paths: runtime_paths
                        .clone()
                        .context("overlay runtime identity was not prepared")?,
                    config_store: config_store.clone(),
                    logger: logger.clone(),
                },
                signal_owner
                    .as_mut()
                    .context("overlay signal owner was not installed")?,
            )?;

            log::info!("Annotation overlay closed.");
        } else {
            // No flags: show usage
            print_usage();
        }

        Ok(())
    })();

    // Both ordinary workers inherited the blocked runtime mask. Join them
    // before the root restores that mask so no application thread becomes
    // signal-eligible during teardown.
    let run_result = finish_daemon_watchdog(run_result, &mut daemon_watchdog);
    drop(daemon_watchdog);
    drop(process_broker);
    drop(process_broker_owner);
    if let Err(error) = &run_result {
        logger.error(
            "wayscriber::entry",
            format!("ordinary run failed: {error:#}"),
        );
    }
    logger.info("wayscriber::app", "ordinary application root finished");
    drop(logger);
    let run_result = finish_logger_owner(run_result, &mut logger_owner);
    finish_signal_owner(run_result, &mut signal_owner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_out_about_still_requires_its_explicit_process_broker() {
        assert!(update_modes_need_process_broker(true, false, true));
    }

    #[test]
    fn compiled_out_explicit_update_check_reaches_the_message_without_a_broker() {
        assert!(!update_modes_need_process_broker(false, true, true));
        assert!(matches!(
            prepare_explicit_update_check(true, None),
            Ok(ExplicitUpdateCheck::CompiledOut(message))
                if message == crate::update_check::COMPILED_OUT_MESSAGE
        ));
    }

    #[test]
    fn daemon_request_session_file_rejects_unanchored_relative_paths() {
        let error = daemon_request_session_file(Some(PathBuf::from("meeting.wayscriber-session")))
            .expect_err("internal requests must already carry absolute session paths");

        assert!(error.to_string().contains("was not anchored"));
    }

    #[test]
    fn daemon_request_session_file_preserves_absolute_paths() {
        let path = PathBuf::from("/tmp/meeting.wayscriber-session");
        let anchored = daemon_request_session_file(Some(path.clone()))
            .expect("absolute internal session path should be accepted");

        assert_eq!(anchored, Some(path));
    }

    #[test]
    fn signal_owner_is_created_only_for_runtime_modes() {
        assert_eq!(runtime_signal_profile(&Cli::default()), None);

        let daemon_toggle = Cli {
            daemon_toggle: true,
            freeze: true,
            ..Cli::default()
        };
        assert_eq!(runtime_signal_profile(&daemon_toggle), None);

        let daemon = Cli {
            daemon: true,
            ..Cli::default()
        };
        assert_eq!(
            runtime_signal_profile(&daemon),
            Some(crate::unix_signals::SignalProfile::Daemon)
        );

        let overlay = Cli {
            active: true,
            ..Cli::default()
        };
        assert_eq!(
            runtime_signal_profile(&overlay),
            Some(crate::unix_signals::SignalProfile::Overlay)
        );
    }
}
