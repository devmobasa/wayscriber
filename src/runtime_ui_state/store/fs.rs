use std::ffi::{CString, OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::PathBuf;

use super::RuntimeStateWriterNamespace;
use crate::runtime_ui_state::{
    RuntimeStateFileIdentity, RuntimeStateResolvedParent, SourceMutationId,
};

#[derive(Debug)]
pub(super) struct PinnedParent {
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
            file,
            observed_path: expected.path().to_path_buf(),
            identity,
        })
    }

    pub(super) fn join(&self, leaf: impl Into<OsString>) -> io::Result<PinnedPath> {
        let leaf = leaf.into();
        validate_leaf(&leaf)?;
        Ok(PinnedPath {
            parent: Self {
                file: self.file.try_clone()?,
                observed_path: self.observed_path.clone(),
                identity: self.identity,
            },
            leaf,
        })
    }

    fn fd(&self) -> libc::c_int {
        self.file.as_raw_fd()
    }

    fn sync(&self) -> io::Result<()> {
        self.file.sync_all()
    }

    fn current_path(&self) -> io::Result<PathBuf> {
        if fs::metadata(&self.observed_path)
            .ok()
            .is_some_and(|metadata| file_identity(&metadata) == self.identity)
        {
            return Ok(self.observed_path.clone());
        }

        let proc_path = PathBuf::from(format!("/proc/self/fd/{}", self.fd()));
        let current = fs::read_link(proc_path)?;
        if !fs::metadata(&current)
            .ok()
            .is_some_and(|metadata| file_identity(&metadata) == self.identity)
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "pinned runtime-state parent no longer has a stable path",
            ));
        }
        Ok(current)
    }
}

#[derive(Debug)]
pub(super) struct PinnedPath {
    parent: PinnedParent,
    leaf: OsString,
}

impl PinnedPath {
    pub(super) fn open_read(&self) -> io::Result<File> {
        let leaf = os_str_c_string(&self.leaf)?;
        // SAFETY: the parent fd and CString remain valid for the duration of the call.
        let fd = unsafe {
            libc::openat(
                self.parent.fd(),
                leaf.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            )
        };
        file_from_fd(fd)
    }

    pub(super) fn reported_path(&self) -> io::Result<PathBuf> {
        Ok(self.parent.current_path()?.join(&self.leaf))
    }

    pub(super) fn sync_parent(&self) -> io::Result<()> {
        self.parent.sync()
    }

    fn create_new(&self, mode: libc::mode_t) -> io::Result<File> {
        let leaf = os_str_c_string(&self.leaf)?;
        // SAFETY: the parent fd and CString remain valid for the duration of the call.
        let fd = unsafe {
            libc::openat(
                self.parent.fd(),
                leaf.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                mode,
            )
        };
        file_from_fd(fd)
    }

    fn exists_nofollow(&self) -> io::Result<bool> {
        let leaf = os_str_c_string(&self.leaf)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: the pointers are valid and stat points to writable storage.
        let result = unsafe {
            libc::fstatat(
                self.parent.fd(),
                leaf.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if result == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::NotFound {
            Ok(false)
        } else {
            Err(error)
        }
    }
}

pub(super) struct CleanupPath {
    path: PinnedPath,
    armed: bool,
}

impl CleanupPath {
    pub(super) fn new(path: PinnedPath) -> Self {
        Self { path, armed: true }
    }

    pub(super) fn path(&self) -> &PinnedPath {
        &self.path
    }

    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CleanupPath {
    fn drop(&mut self) {
        if self.armed {
            let _ = remove_file(&self.path);
        }
    }
}

pub(super) fn create_synced_temp(
    path: &PinnedPath,
    writer_namespace: RuntimeStateWriterNamespace,
    mutation_id: SourceMutationId,
    bytes: &[u8],
) -> io::Result<CleanupPath> {
    let path = create_unique_sibling(path, "tmp", writer_namespace, mutation_id, |candidate| {
        match candidate.create_new(0o600) {
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

pub(super) fn unique_recovery_path(
    path: &PinnedPath,
    writer_namespace: RuntimeStateWriterNamespace,
    mutation_id: SourceMutationId,
) -> io::Result<PinnedPath> {
    create_unique_sibling(
        path,
        "recovery",
        writer_namespace,
        mutation_id,
        |candidate| Ok(!candidate.exists_nofollow()?),
    )
}

fn create_unique_sibling<F>(
    path: &PinnedPath,
    suffix: &str,
    writer_namespace: RuntimeStateWriterNamespace,
    mutation_id: SourceMutationId,
    mut reserve: F,
) -> io::Result<PinnedPath>
where
    F: FnMut(&PinnedPath) -> io::Result<bool>,
{
    for attempt in 0..128 {
        let candidate = path.parent.join(OsString::from(format!(
            ".{}.wayscriber-{suffix}-{}-{writer_namespace}-{}-{attempt}",
            path.leaf.to_string_lossy(),
            std::process::id(),
            mutation_id.get(),
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
        // SAFETY: both parent fds and CStrings remain valid for the call.
        let result = unsafe {
            libc::renameat2(
                source.parent.fd(),
                source_leaf.as_ptr(),
                destination.parent.fd(),
                destination_leaf.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
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
    // SAFETY: the parent fd and CString remain valid for the duration of the call.
    let result = unsafe { libc::unlinkat(path.parent.fd(), leaf.as_ptr(), 0) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
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

fn file_from_fd(fd: libc::c_int) -> io::Result<File> {
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openat returned a new owned descriptor on success.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn file_identity(metadata: &fs::Metadata) -> RuntimeStateFileIdentity {
    RuntimeStateFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_writers_reserve_distinct_operation_local_siblings() {
        let temp = crate::test_temp::tempdir().expect("test owns its runtime-state directory");
        let metadata = fs::metadata(temp.path()).expect("test runtime-state directory exists");
        let expected =
            RuntimeStateResolvedParent::new(temp.path().to_path_buf(), file_identity(&metadata));
        let parent = PinnedParent::open(&expected).expect("test directory can be pinned");
        let target = parent
            .join("runtime-ui.toml")
            .expect("test target has a valid leaf");

        let writer_namespace = RuntimeStateWriterNamespace::test_fixture(7);
        let first = create_synced_temp(&target, writer_namespace, SourceMutationId(7), b"first")
            .expect("first writer reserves its temporary sibling");
        let second = create_synced_temp(&target, writer_namespace, SourceMutationId(7), b"second")
            .expect("second writer skips the first writer's reserved sibling");

        assert_ne!(
            first
                .path()
                .reported_path()
                .expect("first sibling remains reportable"),
            second
                .path()
                .reported_path()
                .expect("second sibling remains reportable")
        );
    }

    #[test]
    fn later_mutation_does_not_reuse_removed_recovery_artifact_identity() {
        let temp = crate::test_temp::tempdir().expect("test owns its runtime-state directory");
        let metadata = fs::metadata(temp.path()).expect("test runtime-state directory exists");
        let expected =
            RuntimeStateResolvedParent::new(temp.path().to_path_buf(), file_identity(&metadata));
        let parent = PinnedParent::open(&expected).expect("test directory can be pinned");
        let target = parent
            .join("runtime-ui.toml")
            .expect("test target has a valid leaf");

        let writer_namespace = RuntimeStateWriterNamespace::test_fixture(9);
        let first = unique_recovery_path(&target, writer_namespace, SourceMutationId(11))
            .expect("first mutation allocates a recovery identity");
        let first_reported = first
            .reported_path()
            .expect("first recovery identity remains reportable");
        let first_file = first
            .create_new(0o600)
            .expect("test retains the first recovery artifact");
        drop(first_file);
        remove_file(&first).expect("test removes the retained recovery artifact");

        let second = unique_recovery_path(&target, writer_namespace, SourceMutationId(12))
            .expect("later mutation allocates a new recovery identity");
        let second_reported = second
            .reported_path()
            .expect("second recovery identity remains reportable");

        assert_ne!(first_reported, second_reported);
    }

    #[test]
    fn joined_path_parent_descriptor_is_close_on_exec() {
        let temp = crate::test_temp::tempdir().expect("test owns its runtime-state directory");
        let metadata = fs::metadata(temp.path()).expect("test runtime-state directory exists");
        let expected =
            RuntimeStateResolvedParent::new(temp.path().to_path_buf(), file_identity(&metadata));
        let parent = PinnedParent::open(&expected).expect("test directory can be pinned");
        let target = parent
            .join("runtime-ui.toml")
            .expect("test target has a valid leaf");

        // SAFETY: target owns this live descriptor for the duration of the query.
        let flags = unsafe { libc::fcntl(target.parent.fd(), libc::F_GETFD) };

        assert!(
            flags >= 0,
            "test can inspect the duplicated descriptor flags"
        );
        assert_ne!(flags & libc::FD_CLOEXEC, 0);
    }
}
