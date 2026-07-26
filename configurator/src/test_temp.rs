use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::{fs, io};

pub(crate) struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(crate) fn tempdir() -> io::Result<TempDir> {
    let base = std::env::temp_dir();
    let pid = std::process::id();

    for _ in 0..100 {
        let candidate = RandomState::new().build_hasher().finish();
        let path = base.join(format!(
            "wayscriber-configurator-test-{pid}-{candidate:016x}"
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(TempDir { path }),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "failed to create a unique temporary test directory",
    ))
}

pub(crate) fn path_resolver() -> wayscriber::paths::PathResolver {
    wayscriber::paths::PathResolver::from_environment(
        wayscriber::paths::PathEnvironment::from_values(&[(
            wayscriber::env_vars::HOME_ENV,
            std::ffi::OsStr::new("/tmp"),
        )]),
    )
}
