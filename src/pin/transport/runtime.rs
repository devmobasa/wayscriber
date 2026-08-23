use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use super::{PinConnection, authorize_peer_uid, effective_uid, seqpacket_socket, set_nonblocking};
use crate::pin::PinCreateError;

pub(super) const PIN_DIR: &str = "wayscriber";
pub(super) const PIN_SOCKET: &str = "pin-host-v1.sock";
pub(super) const PIN_LOCK: &str = "pin-host-v1.lock";
pub(super) const PIN_START_LOCK: &str = "pin-host-v1.start.lock";

#[derive(Debug, Clone)]
pub(crate) struct PinRuntimePaths {
    pub(super) socket: PathBuf,
    pub(super) lock: PathBuf,
    pub(super) start_lock: PathBuf,
}

impl PinRuntimePaths {
    pub(crate) fn secure_from_env() -> Result<Self, PinCreateError> {
        let root = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or(PinCreateError::SecureRuntimeUnavailable)?;
        validate_private_directory(&root).map_err(|_| PinCreateError::SecureRuntimeUnavailable)?;
        let directory = root.join(PIN_DIR);
        ensure_private_subdirectory(&directory)
            .map_err(|_| PinCreateError::SecureRuntimeUnavailable)?;
        Ok(Self {
            socket: directory.join(PIN_SOCKET),
            lock: directory.join(PIN_LOCK),
            start_lock: directory.join(PIN_START_LOCK),
        })
    }

    pub(crate) fn eligible_from_env() -> bool {
        let Some(root) = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
        else {
            return false;
        };
        if validate_private_directory(&root).is_err() {
            return false;
        }
        let directory = root.join(PIN_DIR);
        match std::fs::symlink_metadata(directory) {
            Ok(metadata) => private_directory_metadata(&metadata),
            Err(error) => error.kind() == io::ErrorKind::NotFound,
        }
    }

    pub(crate) fn socket(&self) -> &Path {
        &self.socket
    }

    pub(crate) fn lock(&self) -> &Path {
        &self.lock
    }

    pub(crate) fn start_lock(&self) -> &Path {
        &self.start_lock
    }
}

pub(crate) struct StarterLock {
    _file: std::fs::File,
}

impl StarterLock {
    pub(crate) fn try_acquire(paths: &PinRuntimePaths) -> Result<Option<Self>> {
        let file = open_lock(paths.start_lock())?;
        // SAFETY: flock operates on the live starter-serialization descriptor.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == 0 {
            return Ok(Some(Self { _file: file }));
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::WouldBlock {
            Ok(None)
        } else {
            Err(error).context("failed to acquire pin host starter lock")
        }
    }
}

pub(crate) struct HostLock {
    _file: std::fs::File,
    path: PathBuf,
}

impl HostLock {
    pub(crate) fn try_acquire(paths: &PinRuntimePaths) -> Result<Option<Self>> {
        let file = open_lock(paths.lock())?;
        // SAFETY: flock operates on the live lock descriptor.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == 0 {
            return Ok(Some(Self {
                _file: file,
                path: paths.lock().to_owned(),
            }));
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::WouldBlock {
            Ok(None)
        } else {
            Err(error).context("failed to acquire pin host lock")
        }
    }

    pub(crate) fn acquire_for(paths: &PinRuntimePaths, timeout: Duration) -> Result<Self> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(lock) = Self::try_acquire(paths)? {
                return Ok(lock);
            }
            if Instant::now() >= deadline {
                bail!("timed out acquiring pin host singleton lock");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

pub(crate) struct PinListener {
    descriptor: OwnedFd,
    socket_path: PathBuf,
}

impl PinListener {
    pub(crate) fn bind(paths: &PinRuntimePaths, ownership: &HostLock) -> Result<Self> {
        if ownership.path != paths.lock() {
            bail!("pin listener lock does not cover its runtime paths");
        }
        remove_stale_socket(paths.socket())?;
        let descriptor = seqpacket_socket()?;
        bind_path(descriptor.as_raw_fd(), paths.socket())?;
        std::fs::set_permissions(paths.socket(), std::fs::Permissions::from_mode(0o600))?;
        // SAFETY: descriptor is a bound connection-oriented Unix socket.
        if unsafe { libc::listen(descriptor.as_raw_fd(), 16) } != 0 {
            return Err(io::Error::last_os_error()).context("failed to listen on pin socket");
        }
        Ok(Self {
            descriptor,
            socket_path: paths.socket().to_owned(),
        })
    }

    pub(crate) fn set_nonblocking(&self, enabled: bool) -> io::Result<()> {
        set_nonblocking(self.as_raw_fd(), enabled)
    }

    pub(crate) fn accept(&self) -> Result<Option<PinConnection>> {
        loop {
            // SAFETY: the listener is live; no peer address is requested.
            let raw = unsafe {
                libc::accept4(
                    self.as_raw_fd(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    libc::SOCK_CLOEXEC,
                )
            };
            if raw >= 0 {
                // SAFETY: accept4 returned a new owned descriptor.
                let descriptor = unsafe { OwnedFd::from_raw_fd(raw) };
                let peer_uid = match crate::unix_transport::peer_uid(descriptor.as_raw_fd()) {
                    Ok(uid) => uid,
                    Err(error) => {
                        log::warn!("Rejected pin client without valid peer credentials: {error}");
                        continue;
                    }
                };
                if !authorize_peer_uid(peer_uid, effective_uid()) {
                    log::warn!("Rejected pin client owned by another uid");
                    continue;
                }
                return Ok(Some(PinConnection::accepted(descriptor)));
            }
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if error.kind() == io::ErrorKind::WouldBlock {
                return Ok(None);
            }
            return Err(error).context("failed to accept pin client");
        }
    }
}

impl AsRawFd for PinListener {
    fn as_raw_fd(&self) -> RawFd {
        self.descriptor.as_raw_fd()
    }
}

impl Drop for PinListener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

fn validate_private_directory(path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !private_directory_metadata(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "runtime directory is not private and owned",
        ));
    }
    Ok(())
}

fn private_directory_metadata(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_dir()
        && !metadata.file_type().is_symlink()
        && metadata.uid() == effective_uid()
        && metadata.mode() & 0o077 == 0
}

fn ensure_private_subdirectory(path: &Path) -> io::Result<()> {
    match std::fs::create_dir(path) {
        Ok(()) => std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    validate_private_directory(path)
}

fn open_lock(path: &Path) -> Result<std::fs::File> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file()
        || metadata.uid() != effective_uid()
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
    {
        bail!("pin host lock is not a private owned regular file");
    }
    Ok(file)
}

fn remove_stale_socket(path: &Path) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_socket()
        || metadata.uid() != effective_uid()
        || metadata.mode() & 0o077 != 0
    {
        bail!("refusing to unlink an unsafe pin host socket entry");
    }
    std::fs::remove_file(path)?;
    Ok(())
}

fn bind_path(fd: RawFd, path: &Path) -> io::Result<()> {
    let (address, length) = super::socket_address(path)?;
    // SAFETY: address/length describe a fully initialized sockaddr_un.
    if unsafe { libc::bind(fd, (&address as *const libc::sockaddr_un).cast(), length) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
