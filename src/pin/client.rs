use std::ffi::OsStr;
use std::time::{Duration, Instant};

use anyhow::Context;

use super::limits::{validate_source, validate_source_dimensions as validate_dimensions};
use super::protocol::{PinCreateResponse, PinCreateWire};
use super::transport::{PinConnection, PinRuntimePaths, StarterLock};
use super::{PinCreateAck, PinCreateError, PinCreateRequest, PinRefusal};
use crate::process_broker::{HelperKind, HelperLifetime};

const HOST_START_TIMEOUT: Duration = Duration::from_secs(2);
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_RETRY: Duration = Duration::from_millis(20);

pub(crate) fn pin_available() -> bool {
    PinRuntimePaths::eligible_from_env()
}

pub(crate) fn validate_source_dimensions(width: u32, height: u32) -> Result<(), PinCreateError> {
    validate_dimensions(width, height).map_err(Into::into)
}

pub(crate) fn create_pin(request: PinCreateRequest) -> Result<PinCreateAck, PinCreateError> {
    validate_request(&request)?;
    let paths = PinRuntimePaths::secure_from_env()?;
    let connection = connect_or_start(&paths)?;
    let png_length = u64::try_from(request.image.bytes.len())
        .map_err(|_| PinCreateError::Refused(PinRefusal::LimitExceeded))?;
    let wire = PinCreateWire {
        request_id: request.request_id,
        png_length,
        width: request.image.width,
        height: request.image.height,
        output: request.output,
        placement: request.placement,
    };
    let response = connection
        .create_client(wire, &request.image.bytes)
        .map_err(map_transport)?;
    if response.request_id() != request.request_id {
        return Err(PinCreateError::Transport(
            "pin host response identity mismatch".into(),
        ));
    }
    match response {
        PinCreateResponse::Ready { request_id, pin_id } => Ok(PinCreateAck { request_id, pin_id }),
        PinCreateResponse::Refused { reason, .. } => Err(reason.into()),
        PinCreateResponse::Failed { message, .. } => Err(PinCreateError::Host(message)),
    }
}

fn validate_request(request: &PinCreateRequest) -> Result<(), PinCreateError> {
    if request.image.format.mime_type != "image/png" || request.image.format.extension != "png" {
        return Err(PinRefusal::InvalidImage.into());
    }
    validate_source(
        request.image.bytes.len(),
        request.image.width,
        request.image.height,
    )?;
    if !request.placement.is_valid() {
        return Err(PinRefusal::InvalidPlacement.into());
    }
    Ok(())
}

fn connect_or_start(paths: &PinRuntimePaths) -> Result<PinConnection, PinCreateError> {
    connect_or_start_with(paths, spawn_host)
}

fn connect_or_start_with<F>(
    paths: &PinRuntimePaths,
    spawn: F,
) -> Result<PinConnection, PinCreateError>
where
    F: FnOnce() -> Result<(), PinCreateError>,
{
    match connect_and_negotiate(paths) {
        Ok(connection) => return Ok(connection),
        Err(error) if startup_failure(&error) != StartupFailure::Terminal => {}
        Err(error) => return Err(map_transport(error)),
    }

    let deadline = Instant::now() + HOST_START_TIMEOUT;
    loop {
        let starter = StarterLock::try_acquire(paths).map_err(map_transport)?;
        if let Some(starter) = starter {
            // Recheck after serializing starters: another client may have
            // completed startup before this one acquired the starter lock.
            match connect_and_negotiate(paths) {
                Ok(connection) => return Ok(connection),
                Err(error) if startup_failure(&error) != StartupFailure::Terminal => {}
                Err(error) => return Err(map_transport(error)),
            }
            spawn()?;
            return wait_for_started_host(paths, deadline, starter);
        }
        match connect_and_negotiate(paths) {
            Ok(connection) => return Ok(connection),
            Err(error) if startup_failure(&error) != StartupFailure::Terminal => {}
            Err(error) => return Err(map_transport(error)),
        }
        if Instant::now() >= deadline {
            return Err(PinCreateError::Timeout);
        }
        std::thread::sleep(CONNECT_RETRY);
    }
}

fn wait_for_started_host(
    paths: &PinRuntimePaths,
    deadline: Instant,
    _starter: StarterLock,
) -> Result<PinConnection, PinCreateError> {
    let mut retried_eof = false;
    loop {
        match connect_and_negotiate(paths) {
            Ok(connection) => return Ok(connection),
            Err(error) => {
                match startup_failure(&error) {
                    StartupFailure::ConnectRace => {}
                    StartupFailure::EofRace if !retried_eof => retried_eof = true,
                    StartupFailure::EofRace | StartupFailure::Terminal => {
                        return Err(map_transport(error));
                    }
                }
                if Instant::now() >= deadline {
                    return Err(PinCreateError::Timeout);
                }
            }
        }
        std::thread::sleep(CONNECT_RETRY);
    }
}

fn connect_and_negotiate(paths: &PinRuntimePaths) -> anyhow::Result<PinConnection> {
    let mut connection = PinConnection::connect(paths.socket(), EXCHANGE_TIMEOUT)?;
    connection.negotiate_client()?;
    Ok(connection)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupFailure {
    ConnectRace,
    EofRace,
    Terminal,
}

fn startup_failure(error: &anyhow::Error) -> StartupFailure {
    let Some(error) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<std::io::Error>())
    else {
        return StartupFailure::Terminal;
    };
    match error.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
            StartupFailure::ConnectRace
        }
        std::io::ErrorKind::UnexpectedEof
        | std::io::ErrorKind::ConnectionReset
        | std::io::ErrorKind::BrokenPipe => StartupFailure::EofRace,
        _ => StartupFailure::Terminal,
    }
}

fn spawn_host() -> Result<(), PinCreateError> {
    let executable = std::env::current_exe().map_err(|error| {
        PinCreateError::Transport(format!("failed to resolve current executable: {error}"))
    })?;
    crate::process_broker::current()
        .and_then(|broker| {
            broker.spawn(
                HelperKind::PinHost,
                HelperLifetime::DetachedAfterExec,
                executable.as_os_str(),
                [OsStr::new("--pin-host")],
                Vec::new(),
            )
        })
        .context("failed to start pin host through process broker")
        .map(|_| ())
        .map_err(map_transport)
}

fn map_transport(error: anyhow::Error) -> PinCreateError {
    if let Some(reason) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<PinRefusal>())
    {
        return (*reason).into();
    }
    if error.chain().any(|cause| {
        cause.downcast_ref::<std::io::Error>().is_some_and(|error| {
            matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            )
        })
    }) {
        PinCreateError::Timeout
    } else {
        PinCreateError::Transport(format!("{error:#}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Arc, Barrier, Mutex, atomic};

    fn secure_paths() -> (crate::test_temp::TempDir, PinRuntimePaths) {
        let root = crate::test_temp::tempdir().unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let previous = std::env::var_os("XDG_RUNTIME_DIR");
        // SAFETY: callers retain test_env's process-environment lock.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", root.path()) };
        let paths = PinRuntimePaths::secure_from_env().unwrap();
        if let Some(previous) = previous {
            // SAFETY: callers retain test_env's process-environment lock.
            unsafe { std::env::set_var("XDG_RUNTIME_DIR", previous) };
        } else {
            // SAFETY: callers retain test_env's process-environment lock.
            unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        }
        (root, paths)
    }

    #[test]
    fn availability_has_no_directory_creation_side_effect() {
        let _guard = crate::test_env::lock();
        let root = crate::test_temp::tempdir().unwrap();
        std::fs::set_permissions(
            root.path(),
            <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
        )
        .unwrap();
        let previous = std::env::var_os("XDG_RUNTIME_DIR");
        // SAFETY: test_env serializes process environment mutation.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", root.path()) };

        assert!(pin_available());
        assert!(!root.path().join("wayscriber").exists());

        if let Some(previous) = previous {
            // SAFETY: test_env serializes process environment mutation.
            unsafe { std::env::set_var("XDG_RUNTIME_DIR", previous) };
        } else {
            // SAFETY: test_env serializes process environment mutation.
            unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        }
    }

    #[test]
    fn insecure_runtime_directory_is_ineligible() {
        let _guard = crate::test_env::lock();
        let root = crate::test_temp::tempdir().unwrap();
        std::fs::set_permissions(
            root.path(),
            <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
        )
        .unwrap();
        let previous = std::env::var_os("XDG_RUNTIME_DIR");
        // SAFETY: test_env serializes process environment mutation.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", root.path()) };
        assert!(!pin_available());
        if let Some(previous) = previous {
            // SAFETY: test_env serializes process environment mutation.
            unsafe { std::env::set_var("XDG_RUNTIME_DIR", previous) };
        } else {
            // SAFETY: test_env serializes process environment mutation.
            unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        }
    }

    #[test]
    fn unsupported_version_is_terminal_and_never_spawns() {
        let _guard = crate::test_env::lock();
        let (_root, paths) = secure_paths();
        let host_paths = paths.clone();
        let host = std::thread::spawn(move || {
            use super::super::transport::{HostLock, PinListener, ReceivedPacket};

            let lock = HostLock::try_acquire(&host_paths).unwrap().unwrap();
            let listener = PinListener::bind(&host_paths, &lock).unwrap();
            let mut connection = listener.accept().unwrap().unwrap();
            let ReceivedPacket::Hello { .. } = connection.receive().unwrap() else {
                panic!("expected hello")
            };
            connection.send_hello(u16::MAX).unwrap();
        });
        while !paths.socket().exists() {
            std::thread::yield_now();
        }
        let spawns = atomic::AtomicUsize::new(0);
        let error = connect_or_start_with(&paths, || {
            spawns.fetch_add(1, atomic::Ordering::SeqCst);
            Ok(())
        });
        let Err(error) = error else {
            panic!("unsupported protocol version must be terminal")
        };
        assert!(matches!(
            error,
            PinCreateError::Refused(PinRefusal::UnsupportedVersion)
        ));
        assert_eq!(spawns.load(atomic::Ordering::SeqCst), 0);
        host.join().unwrap();
    }

    #[test]
    fn concurrent_connect_or_start_spawns_one_owned_host_candidate() {
        let _guard = crate::test_env::lock();
        let (_root, paths) = secure_paths();
        let barrier = Arc::new(Barrier::new(3));
        let spawns = Arc::new(atomic::AtomicUsize::new(0));
        let host_thread = Arc::new(Mutex::new(None));
        let clients: Vec<_> = (0..2)
            .map(|_| {
                let paths = paths.clone();
                let barrier = Arc::clone(&barrier);
                let spawns = Arc::clone(&spawns);
                let host_thread = Arc::clone(&host_thread);
                std::thread::spawn(move || {
                    barrier.wait();
                    let host_paths = paths.clone();
                    let handle_slot = Arc::clone(&host_thread);
                    let connection = connect_or_start_with(&paths, move || {
                        use super::super::transport::{HostLock, PinListener, ReceivedPacket};

                        spawns.fetch_add(1, atomic::Ordering::SeqCst);
                        let handle = std::thread::spawn(move || {
                            let lock = HostLock::try_acquire(&host_paths).unwrap().unwrap();
                            let listener = PinListener::bind(&host_paths, &lock).unwrap();
                            for _ in 0..2 {
                                let mut connection = listener.accept().unwrap().unwrap();
                                let ReceivedPacket::Hello { version } =
                                    connection.receive().unwrap()
                                else {
                                    panic!("expected hello")
                                };
                                assert!(connection.send_hello(version).unwrap());
                            }
                        });
                        *handle_slot.lock().unwrap() = Some(handle);
                        Ok(())
                    })
                    .unwrap();
                    drop(connection);
                })
            })
            .collect();
        barrier.wait();
        for client in clients {
            client.join().unwrap();
        }
        host_thread.lock().unwrap().take().unwrap().join().unwrap();
        assert_eq!(spawns.load(atomic::Ordering::SeqCst), 1);
        assert!(
            super::super::transport::HostLock::try_acquire(&paths)
                .unwrap()
                .is_some(),
            "the sole host candidate must release ownership after exit"
        );
    }
}
