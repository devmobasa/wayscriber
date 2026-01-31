use anyhow::Result;
use log::warn;
use std::env;
use std::os::fd::AsRawFd;
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

pub(super) fn read_events_with_timeout(
    guard: ReadEventsGuard,
    timeout: Option<std::time::Duration>,
) -> Result<usize, WaylandError> {
    let mut pollfd = libc::pollfd {
        fd: guard.connection_fd().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let timeout_ms = timeout
        .map(|dur| dur.as_millis().min(i32::MAX as u128) as i32)
        .unwrap_or(-1);

    loop {
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

    match guard.read() {
        Ok(n) => Ok(n),
        Err(WaylandError::Io(err)) if err.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
        Err(e) => Err(e),
    }
}

pub(super) fn dispatch_with_timeout(
    event_queue: &mut EventQueue<WaylandState>,
    state: &mut WaylandState,
    timeout: Option<std::time::Duration>,
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
