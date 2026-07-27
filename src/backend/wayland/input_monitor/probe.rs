//! Capability probe for system-wide input capture.
//!
//! Cheap and side-effect free: no thread, no libinput context, no device open.
//! Mode resolution and `--runtime-capabilities` both read it, so `auto` can
//! silently pick overlay mode and `system` can warn with actionable guidance
//! before anything is spawned.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

const DEV_INPUT_DIR: &str = "/dev/input";
/// Seat to follow when the session does not name one. libinput's udev backend
/// groups devices by seat and `seat0` is the single-seat default.
#[cfg_attr(not(feature = "input-monitor"), allow(dead_code))]
const DEFAULT_SEAT: &str = "seat0";

/// Seat this session belongs to.
///
/// Lives with the probe rather than the reader thread because the preflight
/// path names the seat in its failure message without ever starting a reader,
/// and both must agree on which seat they are talking about. Only those
/// callers need it, so it is dead code without the feature.
#[cfg_attr(not(feature = "input-monitor"), allow(dead_code))]
pub(in crate::backend::wayland) fn current_seat() -> String {
    seat_name(std::env::var_os(crate::env_vars::XDG_SEAT_ENV))
}

/// Resolve a seat name from the environment value, falling back to the
/// single-seat default. On a multi-seat machine `seat0` may be another user's
/// seat (or hold no devices at all), so the session's value wins when it is
/// set to something usable.
#[cfg_attr(not(feature = "input-monitor"), allow(dead_code))]
fn seat_name(configured: Option<std::ffi::OsString>) -> String {
    configured
        .and_then(|seat| seat.into_string().ok())
        .map(|seat| seat.trim().to_string())
        .filter(|seat| !seat.is_empty())
        .unwrap_or_else(|| DEFAULT_SEAT.to_string())
}

/// Whether this build can capture system-wide input *and* this process may
/// read at least one evdev node on the session's seat.
///
/// Both halves matter: without the `input-monitor` feature there is no reader
/// thread to start, and without read access libinput would open no devices and
/// report a seat with nothing on it.
pub(crate) fn system_input_available() -> bool {
    match event_node_access() {
        EventNodeAccess::Readable => true,
        EventNodeAccess::None | EventNodeAccess::Unreadable => false,
        // udev could not answer. Rather than deny a capability that may well
        // work, fall back to the flat directory scan: it cannot attribute a
        // node to a seat, but on the single-seat machines this fallback
        // actually happens on, every node belongs to the one seat anyway.
        EventNodeAccess::Unknown => {
            cfg!(feature = "input-monitor")
                && classify_event_nodes(Path::new(DEV_INPUT_DIR)) == EventNodeAccess::Readable
        }
    }
}

/// What the session's seat looks like to this process.
///
/// The distinction drives the message a failed system-mode request shows:
/// nodes that exist but cannot be opened are a permission problem the `input`
/// group fixes, while a seat with no `event*` nodes at all has nothing to
/// capture, and no amount of group membership changes that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum EventNodeAccess {
    /// No `event*` nodes on this seat.
    None,
    /// Nodes exist on this seat, but this process can read none of them.
    Unreadable,
    /// At least one node on this seat is readable.
    Readable,
    /// The seat's devices could not be enumerated, so which of the above
    /// holds is genuinely unknown. Reported neutrally rather than guessed at.
    Unknown,
}

impl EventNodeAccess {
    /// Fold one node lookup into the seat-wide result.
    ///
    /// A readable node makes capture possible. A concrete permission denial is
    /// actionable even if another node could not be classified; otherwise an
    /// indeterminate lookup stays neutral. Absent/stale udev paths contribute
    /// nothing because there is no node in this process's device namespace to
    /// grant access to.
    fn include(self, node: NodePathAccess) -> Self {
        match (self, node) {
            (_, NodePathAccess::Readable) => Self::Readable,
            (Self::Readable, _) => Self::Readable,
            (_, NodePathAccess::PermissionDenied) => Self::Unreadable,
            (Self::Unreadable, _) => Self::Unreadable,
            (_, NodePathAccess::Unknown) => Self::Unknown,
            (current, NodePathAccess::Absent) => current,
        }
    }
}

/// Result of checking one udev or directory-provided event-node path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodePathAccess {
    Readable,
    PermissionDenied,
    /// The path is not present in this process's device namespace. This is
    /// common when a container can see host sysfs/udev data but `/dev/input`
    /// was not passed through, and is not fixed by changing group membership.
    Absent,
    Unknown,
}

/// Classify the session's seat for availability and failure reporting.
#[cfg(feature = "input-monitor")]
pub(in crate::backend::wayland) fn event_node_access() -> EventNodeAccess {
    classify_seat_nodes(&current_seat())
}

/// Without the reader there is nothing to classify: system capture cannot run
/// in this build at all.
#[cfg(not(feature = "input-monitor"))]
pub(in crate::backend::wayland) fn event_node_access() -> EventNodeAccess {
    EventNodeAccess::None
}

/// Flat scan of `/dev/input`, used only when udev cannot attribute nodes to a
/// seat. Cannot distinguish seats, so it is never the primary classifier.
fn classify_event_nodes(dir: &Path) -> EventNodeAccess {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        // A directory this process may not list is itself the permission
        // case; reporting it as "no devices" would send the user looking for
        // hardware instead of at their group membership.
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            return EventNodeAccess::Unreadable;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return EventNodeAccess::None,
        Err(_) => return EventNodeAccess::Unknown,
    };
    let mut access = EventNodeAccess::None;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                access = access.include(NodePathAccess::Unknown);
                continue;
            }
        };
        if !entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("event"))
        {
            continue;
        }
        access = access.include(node_path_access(&entry.path()));
        if access == EventNodeAccess::Readable {
            return access;
        }
    }
    access
}

/// Classify the `event*` nodes udev assigns to `seat`.
///
/// `/dev/input` is a flat namespace shared by every seat, so a plain directory
/// scan can let another seat's readable device mask a permission failure on
/// this one (or the reverse). udev is the only thing that knows the mapping,
/// and it is the same mapping libinput itself applies: the device's `ID_SEAT`
/// property, defaulting to `seat0`.
#[cfg(feature = "input-monitor")]
fn classify_seat_nodes(seat: &str) -> EventNodeAccess {
    let Ok(mut enumerator) = udev::Enumerator::new() else {
        return EventNodeAccess::Unknown;
    };
    if enumerator.match_subsystem("input").is_err() {
        return EventNodeAccess::Unknown;
    }
    let Ok(devices) = enumerator.scan_devices() else {
        return EventNodeAccess::Unknown;
    };

    let mut access = EventNodeAccess::None;
    for device in devices {
        let Some(devnode) = device.devnode() else {
            continue;
        };
        if !devnode
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("event"))
        {
            continue;
        }
        let device_seat = device
            .property_value("ID_SEAT")
            .and_then(|seat| seat.to_str())
            .unwrap_or(DEFAULT_SEAT);
        if device_seat != seat {
            continue;
        }
        access = access.include(node_path_access(devnode));
        if access == EventNodeAccess::Readable {
            return access;
        }
    }
    access
}

fn node_path_access(path: &Path) -> NodePathAccess {
    let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
        return NodePathAccess::Unknown;
    };
    // SAFETY: `c_path` is a valid NUL-terminated C string that outlives the
    // call, and `access(2)` only reads it.
    if unsafe { libc::access(c_path.as_ptr(), libc::R_OK) } == 0 {
        return NodePathAccess::Readable;
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EACCES | libc::EPERM) => NodePathAccess::PermissionDenied,
        Some(libc::ENOENT | libc::ENOTDIR | libc::ENODEV) => NodePathAccess::Absent,
        _ => NodePathAccess::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_dev_input_directory_reports_no_devices() {
        assert_eq!(
            classify_event_nodes(Path::new("/nonexistent/wayscriber/dev/input")),
            EventNodeAccess::None
        );
    }

    /// Non-`event*` nodes (mice, js*, by-id symlink directories) never count,
    /// so a seat with only legacy nodes reads as unavailable.
    #[test]
    fn only_event_nodes_count_as_capture_devices() {
        let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
        std::fs::write(temp.path().join("mice"), b"").expect("fixture node");
        assert_eq!(classify_event_nodes(temp.path()), EventNodeAccess::None);

        std::fs::write(temp.path().join("event0"), b"").expect("fixture node");
        assert_eq!(classify_event_nodes(temp.path()), EventNodeAccess::Readable);
    }

    /// An empty seat and an unreadable one are different problems with
    /// different fixes, so the classifier keeps them apart.
    #[test]
    fn node_access_separates_an_empty_directory_from_an_unreadable_one() {
        let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
        assert_eq!(classify_event_nodes(temp.path()), EventNodeAccess::None);

        // Only non-event nodes: still nothing to capture from.
        std::fs::write(temp.path().join("mice"), b"").expect("fixture node");
        assert_eq!(classify_event_nodes(temp.path()), EventNodeAccess::None);

        let node = temp.path().join("event0");
        std::fs::write(&node, b"").expect("fixture node");
        assert_eq!(classify_event_nodes(temp.path()), EventNodeAccess::Readable);

        // A node this process cannot read reports the permission case. Running
        // as root defeats the mode bits, so skip there rather than assert a
        // condition the environment cannot produce.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o000))
            .expect("fixture permissions");
        if node_path_access(&node) == NodePathAccess::Readable {
            eprintln!("running with privileges that bypass file permissions; skipping");
            return;
        }
        assert_eq!(
            classify_event_nodes(temp.path()),
            EventNodeAccess::Unreadable
        );
        assert_eq!(
            classify_event_nodes(Path::new("/nonexistent/wayscriber/dev/input")),
            EventNodeAccess::None
        );
    }

    /// A directory this process may not list is a permission problem, not an
    /// absent one: reporting "no devices" would send the user hunting for
    /// hardware instead of checking their group membership.
    #[test]
    fn an_unlistable_directory_reports_the_permission_case() {
        use std::os::unix::fs::PermissionsExt;

        let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
        let dir = temp.path().join("input");
        std::fs::create_dir(&dir).expect("fixture directory");
        std::fs::write(dir.join("event0"), b"").expect("fixture node");

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o000))
            .expect("fixture permissions");
        let listable = std::fs::read_dir(&dir).is_ok();
        // Restore before any assertion so the tempdir can always be cleaned up.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755))
            .expect("restore permissions");
        if listable {
            eprintln!("running with privileges that bypass directory permissions; skipping");
            return;
        }

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o000))
            .expect("fixture permissions");
        let access = classify_event_nodes(&dir);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755))
            .expect("restore permissions");
        assert_eq!(access, EventNodeAccess::Unreadable);
    }

    /// A udev database entry can outlive (or exist outside the namespace of)
    /// its `/dev/input/event*` node. That is absence, not evidence that joining
    /// a group will make the node readable.
    #[test]
    fn a_missing_devnode_does_not_report_a_permission_failure() {
        let missing = Path::new("/nonexistent/wayscriber/dev/input/event0");
        assert_eq!(node_path_access(missing), NodePathAccess::Absent);
        assert_eq!(
            EventNodeAccess::None.include(node_path_access(missing)),
            EventNodeAccess::None
        );
    }

    #[test]
    fn concrete_access_results_win_over_indeterminate_nodes() {
        assert_eq!(
            EventNodeAccess::None
                .include(NodePathAccess::Unknown)
                .include(NodePathAccess::PermissionDenied),
            EventNodeAccess::Unreadable
        );
        assert_eq!(
            EventNodeAccess::Unreadable.include(NodePathAccess::Readable),
            EventNodeAccess::Readable
        );
    }

    /// The session's seat wins so a multi-seat machine reads the seat the
    /// user is at; a missing or blank value falls back to the single-seat
    /// default rather than asking libinput to open "".
    #[test]
    fn the_seat_follows_the_session_and_falls_back_to_the_default() {
        assert_eq!(seat_name(Some("seat1".into())), "seat1");
        assert_eq!(seat_name(Some(" seat2 ".into())), "seat2");
        assert_eq!(seat_name(None), DEFAULT_SEAT);
        assert_eq!(seat_name(Some("".into())), DEFAULT_SEAT);
        assert_eq!(seat_name(Some("   ".into())), DEFAULT_SEAT);
    }

    /// Without the feature the probe is false regardless of permissions, so
    /// `auto` cannot resolve to a mode this build cannot serve.
    #[test]
    fn availability_requires_the_compiled_feature() {
        if !cfg!(feature = "input-monitor") {
            assert!(!system_input_available());
        }
    }
}
