use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

#[cfg(unix)]
fn flock(file: &File, op: libc::c_int) -> io::Result<()> {
    let fd = file.as_raw_fd();
    let result = unsafe { libc::flock(fd, op) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
fn flock(_file: &File, _op: i32) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "file locking is not supported on this platform",
    ))
}

pub fn lock_shared(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        flock(file, libc::LOCK_SH)
    }
    #[cfg(not(unix))]
    {
        flock(file, 0)
    }
}

pub fn lock_exclusive(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        flock(file, libc::LOCK_EX)
    }
    #[cfg(not(unix))]
    {
        flock(file, 0)
    }
}

pub fn try_lock_exclusive(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        flock(file, libc::LOCK_EX | libc::LOCK_NB)
    }
    #[cfg(not(unix))]
    {
        flock(file, 0)
    }
}

pub fn unlock(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        flock(file, libc::LOCK_UN)
    }
    #[cfg(not(unix))]
    {
        flock(file, 0)
    }
}

pub(crate) fn open_runtime_lock_file(lock_path: &Path, named: bool) -> io::Result<File> {
    if named {
        return open_or_create_named_lock_file(lock_path);
    }

    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(lock_path)
}

fn open_or_create_named_lock_file(lock_path: &Path) -> io::Result<File> {
    for _ in 0..2 {
        match create_named_lock_file(lock_path) {
            Ok(file) => return Ok(file),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                match open_existing_named_lock_file(lock_path) {
                    Ok(file) => return Ok(file),
                    Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                    Err(err) => return Err(err),
                }
            }
            Err(err) => return Err(err),
        }
    }

    open_existing_named_lock_file(lock_path)
}

fn create_named_lock_file(lock_path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    options
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);

    let file = options.open(lock_path)?;
    ensure_regular_lock_file(lock_path, file)
}

fn open_existing_named_lock_file(lock_path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);

    let file = options.open(lock_path)?;
    ensure_regular_lock_file(lock_path, file)
}

fn ensure_regular_lock_file(lock_path: &Path, file: File) -> io::Result<File> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "session lock file is not a regular file: {}",
                lock_path.display()
            ),
        ));
    }

    Ok(file)
}
