use std::fs::File;
use std::io;

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
