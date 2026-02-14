use anyhow::Result;
use log::warn;
use std::env;
use std::os::fd::AsRawFd;
use std::time::Duration;
use wayland_client::{EventQueue, backend::ReadEventsGuard, backend::WaylandError};

use super::super::state::WaylandState;
use crate::{RESUME_SESSION_ENV, runtime_session_override};

pub(super) fn friendly_capture_error(error: &str) -> String {
    let lower = error.to_lowercase();

    if is_missing_tool(&lower, "slurp") {
        return "Missing screenshot tool: slurp. Install slurp + grim and try again.".to_string();
    }
    if is_missing_tool(&lower, "grim") {
        return "Missing screenshot tool: grim. Install grim and try again.".to_string();
    }
    if is_missing_tool(&lower, "wl-copy") {
        return "Missing clipboard tool: wl-clipboard (wl-copy). Install it and try again."
            .to_string();
    }
    if lower.contains("requestcancelled") || lower.contains("cancelled") {
        "Screen capture cancelled by user".to_string()
    } else if lower.contains("permission") {
        "Permission denied. Enable screen sharing in system settings.".to_string()
    } else if lower.contains("portal returned error code") {
        "Portal screenshot failed. If you use wlroots/Hyprland/Niri, install grim + slurp. Otherwise check xdg-desktop-portal."
            .to_string()
    } else if lower.contains("busy") {
        "Screen capture in progress. Try again in a moment.".to_string()
    } else {
        "Screen capture failed. Please try again.".to_string()
    }
}

fn is_missing_tool(lower: &str, tool: &str) -> bool {
    lower.contains(tool)
        && (lower.contains("no such file")
            || lower.contains("not found")
            || lower.contains("failed to run")
            || lower.contains("failed to spawn"))
}

fn timeout_to_poll_ms(timeout: Option<Duration>) -> i32 {
    timeout
        .map(|dur| dur.as_millis().min(i32::MAX as u128) as i32)
        .unwrap_or(-1)
}

fn normalize_read_result(result: Result<usize, WaylandError>) -> Result<usize, WaylandError> {
    match result {
        Ok(n) => Ok(n),
        Err(WaylandError::Io(err)) if err.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
        Err(e) => Err(e),
    }
}

pub(super) fn read_events_with_timeout(
    guard: ReadEventsGuard,
    timeout: Option<Duration>,
) -> Result<usize, WaylandError> {
    let mut pollfd = libc::pollfd {
        fd: guard.connection_fd().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let timeout_ms = timeout_to_poll_ms(timeout);

    loop {
        // SAFETY: pollfd points to valid memory and the file descriptor belongs
        // to the prepared Wayland read guard for the duration of this call.
        let ready = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
        if ready == 0 {
            // Dropping the guard cancels the prepared read.
            return Ok(0);
        }
        if ready < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(WaylandError::Io(err));
        }
        break;
    }

    normalize_read_result(guard.read())
}

pub(super) fn dispatch_with_timeout(
    event_queue: &mut EventQueue<WaylandState>,
    state: &mut WaylandState,
    timeout: Option<Duration>,
) -> Result<(), anyhow::Error> {
    let dispatched = event_queue
        .dispatch_pending(state)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    if dispatched > 0 {
        return Ok(());
    }

    event_queue
        .flush()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    if let Some(guard) = event_queue.prepare_read() {
        let _ =
            read_events_with_timeout(guard, timeout).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        event_queue
            .dispatch_pending(state)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }

    Ok(())
}

pub(super) fn resume_override_from_env() -> Option<bool> {
    if let Some(runtime) = runtime_session_override() {
        return Some(runtime);
    }
    match env::var(RESUME_SESSION_ENV) {
        Ok(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" | "on" | "resume" | "enable" | "enabled" => Some(true),
                "0" | "false" | "no" | "off" | "disable" | "disabled" => Some(false),
                _ => {
                    warn!(
                        "Ignoring invalid {} value '{}'; expected on/off/true/false",
                        RESUME_SESSION_ENV, raw
                    );
                    None
                }
            }
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;
    use crate::set_runtime_session_override;

    fn env_mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn timeout_to_poll_ms_supports_none_and_caps_large_values() {
        assert_eq!(timeout_to_poll_ms(None), -1);
        assert_eq!(timeout_to_poll_ms(Some(Duration::from_millis(15))), 15);

        let huge = Duration::from_millis(i32::MAX as u64 + 1000);
        assert_eq!(timeout_to_poll_ms(Some(huge)), i32::MAX);
    }

    #[test]
    fn normalize_read_result_maps_would_block_to_zero() {
        let err = WaylandError::Io(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        assert_eq!(normalize_read_result(Err(err)).unwrap(), 0);
    }

    #[test]
    fn normalize_read_result_preserves_other_errors() {
        let err = WaylandError::Io(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
        let actual = normalize_read_result(Err(err)).unwrap_err();
        match actual {
            WaylandError::Io(io_err) => {
                assert_eq!(io_err.kind(), std::io::ErrorKind::BrokenPipe);
            }
            other => panic!("expected io error, got {other}"),
        }
    }

    #[test]
    fn friendly_capture_error_covers_known_classes() {
        assert_eq!(
            friendly_capture_error("failed to spawn slurp: No such file"),
            "Missing screenshot tool: slurp. Install slurp + grim and try again."
        );
        assert_eq!(
            friendly_capture_error("grim not found"),
            "Missing screenshot tool: grim. Install grim and try again."
        );
        assert_eq!(
            friendly_capture_error("wl-copy failed to run"),
            "Missing clipboard tool: wl-clipboard (wl-copy). Install it and try again."
        );
        assert_eq!(
            friendly_capture_error("RequestCancelled by user"),
            "Screen capture cancelled by user"
        );
        assert_eq!(
            friendly_capture_error("permission denied"),
            "Permission denied. Enable screen sharing in system settings."
        );
        assert_eq!(
            friendly_capture_error("portal returned error code 2"),
            "Portal screenshot failed. If you use wlroots/Hyprland/Niri, install grim + slurp. Otherwise check xdg-desktop-portal."
        );
        assert_eq!(
            friendly_capture_error("resource busy"),
            "Screen capture in progress. Try again in a moment."
        );
        assert_eq!(
            friendly_capture_error("something unexpected"),
            "Screen capture failed. Please try again."
        );
    }

    #[test]
    fn resume_override_from_env_prefers_runtime_override() {
        let _guard = env_mutex().lock().unwrap();

        // SAFETY: test serialized by env mutex.
        unsafe {
            std::env::set_var(RESUME_SESSION_ENV, "off");
        }
        set_runtime_session_override(Some(true));

        assert_eq!(resume_override_from_env(), Some(true));

        set_runtime_session_override(None);
        // SAFETY: test serialized by env mutex.
        unsafe {
            std::env::remove_var(RESUME_SESSION_ENV);
        }
    }

    #[test]
    fn resume_override_from_env_parses_expected_values() {
        let _guard = env_mutex().lock().unwrap();
        set_runtime_session_override(None);

        // SAFETY: test serialized by env mutex.
        unsafe {
            std::env::set_var(RESUME_SESSION_ENV, "enabled");
        }
        assert_eq!(resume_override_from_env(), Some(true));

        // SAFETY: test serialized by env mutex.
        unsafe {
            std::env::set_var(RESUME_SESSION_ENV, "0");
        }
        assert_eq!(resume_override_from_env(), Some(false));

        // SAFETY: test serialized by env mutex.
        unsafe {
            std::env::set_var(RESUME_SESSION_ENV, "maybe");
        }
        assert_eq!(resume_override_from_env(), None);

        // SAFETY: test serialized by env mutex.
        unsafe {
            std::env::remove_var(RESUME_SESSION_ENV);
        }
    }
}
