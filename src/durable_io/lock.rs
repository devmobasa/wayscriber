//! Advisory locks shared by durable file owners.

use std::fs::File;
use std::io;

#[cfg(unix)]
use rustix::fs::{self as rustix_fs, FlockOperation};

#[cfg(unix)]
fn flock(file: &File, operation: FlockOperation) -> io::Result<()> {
    rustix_fs::flock(file, operation).map_err(io::Error::from)
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
        flock(file, FlockOperation::LockShared)
    }
    #[cfg(not(unix))]
    {
        flock(file, 0)
    }
}

pub fn lock_exclusive(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        flock(file, FlockOperation::LockExclusive)
    }
    #[cfg(not(unix))]
    {
        flock(file, 0)
    }
}

pub fn try_lock_exclusive(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        flock(file, FlockOperation::NonBlockingLockExclusive)
    }
    #[cfg(not(unix))]
    {
        flock(file, 0)
    }
}

pub fn unlock(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        flock(file, FlockOperation::Unlock)
    }
    #[cfg(not(unix))]
    {
        flock(file, 0)
    }
}
