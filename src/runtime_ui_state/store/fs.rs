use std::ffi::{CString, OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_os = "linux")]
use rustix::fs::RenameFlags;
use rustix::fs::{self as rustix_fs, AtFlags, Mode, OFlags};
use rustix::io::Errno;

use crate::runtime_ui_state::{RuntimeStateFileIdentity, RuntimeStateResolvedParent};

static NEXT_SIBLING_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub(super) struct PinnedParent {
    inner: Arc<PinnedParentInner>,
}

#[derive(Debug)]
struct PinnedParentInner {
    file: File,
    observed_path: PathBuf,
    identity: RuntimeStateFileIdentity,
}

impl PinnedParent {
    pub(super) fn open(expected: &RuntimeStateResolvedParent) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true).custom_flags(
            libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK,
        );
        let file = options.open(expected.path())?;
        let metadata = file.metadata()?;
        if !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "runtime-state parent is not a directory",
            ));
        }
        let identity = file_identity(&metadata);
        if identity != expected.identity() {
            return Err(io::Error::other("runtime-state parent identity changed"));
        }
        Ok(Self {
            inner: Arc::new(PinnedParentInner {
                file,
                observed_path: expected.path().to_path_buf(),
                identity,
            }),
        })
    }

    pub(super) fn join(&self, leaf: impl Into<OsString>) -> io::Result<PinnedPath> {
        let leaf = leaf.into();
        validate_leaf(&leaf)?;
        Ok(PinnedPath {
            parent: self.clone(),
            leaf,
        })
    }

    fn fd(&self) -> BorrowedFd<'_> {
        self.inner.file.as_fd()
    }

    fn sync(&self) -> io::Result<()> {
        self.inner.file.sync_all()
    }

    fn current_path(&self) -> io::Result<PathBuf> {
        if fs::metadata(&self.inner.observed_path)
            .is_ok_and(|metadata| file_identity(&metadata) == self.inner.identity)
        {
            return Ok(self.inner.observed_path.clone());
        }

        let proc_path = PathBuf::from(format!("/proc/self/fd/{}", self.fd().as_raw_fd()));
        let current = fs::read_link(proc_path)?;
        if !fs::metadata(&current)
            .is_ok_and(|metadata| file_identity(&metadata) == self.inner.identity)
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "pinned runtime-state parent no longer has a stable path",
            ));
        }
        Ok(current)
    }
}

#[derive(Debug, Clone)]
pub(super) struct PinnedPath {
    parent: PinnedParent,
    leaf: OsString,
}

impl PinnedPath {
    pub(super) fn open_read(&self) -> io::Result<File> {
        let leaf = os_str_c_string(&self.leaf)?;
        let fd = rustix_fs::openat(
            self.parent.fd(),
            &leaf,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )?;
        Ok(File::from(fd))
    }

    pub(super) fn reported_path(&self) -> io::Result<PathBuf> {
        Ok(self.parent.current_path()?.join(&self.leaf))
    }

    pub(super) fn sync_parent(&self) -> io::Result<()> {
        self.parent.sync()
    }

    fn create_new(&self, mode: Mode) -> io::Result<File> {
        let leaf = os_str_c_string(&self.leaf)?;
        let fd = rustix_fs::openat(
            self.parent.fd(),
            &leaf,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            mode,
        )?;
        Ok(File::from(fd))
    }

    fn exists_nofollow(&self) -> io::Result<bool> {
        let leaf = os_str_c_string(&self.leaf)?;
        match rustix_fs::statat(self.parent.fd(), &leaf, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => Ok(true),
            Err(Errno::NOENT) => Ok(false),
            Err(errno) => Err(errno.into()),
        }
    }
}

pub(super) struct CleanupPath {
    path: Option<PinnedPath>,
}

impl CleanupPath {
    pub(super) fn new(path: PinnedPath) -> Self {
        Self { path: Some(path) }
    }

    pub(super) fn path(&self) -> &PinnedPath {
        self.path.as_ref().expect("cleanup path is armed")
    }

    pub(super) fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for CleanupPath {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            let _ = remove_file(path);
        }
    }
}

pub(super) fn create_synced_temp(path: &PinnedPath, bytes: &[u8]) -> io::Result<CleanupPath> {
    let path = create_unique_sibling(path, "tmp", |candidate| {
        match candidate.create_new(Mode::RUSR | Mode::WUSR) {
            Ok(mut file) => {
                let result = file.write_all(bytes).and_then(|()| file.sync_all());
                drop(file);
                if let Err(error) = result {
                    let _ = remove_file(candidate);
                    return Err(error);
                }
                Ok(true)
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
            Err(error) => Err(error),
        }
    })?;
    Ok(CleanupPath::new(path))
}

pub(super) fn unique_recovery_path(path: &PinnedPath) -> io::Result<PinnedPath> {
    create_unique_sibling(path, "recovery", |candidate| {
        Ok(!candidate.exists_nofollow()?)
    })
}

fn create_unique_sibling<F>(
    path: &PinnedPath,
    suffix: &str,
    mut reserve: F,
) -> io::Result<PinnedPath>
where
    F: FnMut(&PinnedPath) -> io::Result<bool>,
{
    for _ in 0..128 {
        let id = NEXT_SIBLING_ID.fetch_add(1, Ordering::Relaxed);
        let candidate = path.parent.join(OsString::from(format!(
            ".{}.wayscriber-{suffix}-{}-{id}",
            path.leaf.to_string_lossy(),
            process::id()
        )))?;
        if reserve(&candidate)? {
            return Ok(candidate);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique sibling path",
    ))
}

pub(super) fn rename_noreplace(source: &PinnedPath, destination: &PinnedPath) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let source_leaf = os_str_c_string(&source.leaf)?;
        let destination_leaf = os_str_c_string(&destination.leaf)?;
        rustix_fs::renameat_with(
            source.parent.fd(),
            &source_leaf,
            destination.parent.fd(),
            &destination_leaf,
            RenameFlags::NOREPLACE,
        )
        .map_err(io::Error::from)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (source, destination);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no-replace rename is unavailable on this platform",
        ))
    }
}

pub(super) fn remove_file(path: &PinnedPath) -> io::Result<()> {
    let leaf = os_str_c_string(&path.leaf)?;
    rustix_fs::unlinkat(path.parent.fd(), &leaf, AtFlags::empty()).map_err(io::Error::from)
}

fn validate_leaf(leaf: &OsStr) -> io::Result<()> {
    if leaf.is_empty() || leaf == "." || leaf == ".." || leaf.as_bytes().contains(&b'/') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime-state path leaf is invalid",
        ));
    }
    Ok(())
}

fn os_str_c_string(value: &OsStr) -> io::Result<CString> {
    CString::new(value.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL byte"))
}

fn file_identity(metadata: &fs::Metadata) -> RuntimeStateFileIdentity {
    RuntimeStateFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}
