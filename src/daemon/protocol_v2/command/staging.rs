use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;

use super::super::{BootClock, BootIdentity};
use super::MAX_COMMAND_STAGING_DIRECTORIES;
use super::layout::{admission_lock, creating_dir, flock, open_lock, read_dir_bounded, unlock};

pub(super) fn recover_staging(root: &Path) -> Result<()> {
    let deadline = BootClock::now()?.checked_add(Duration::from_millis(200))?;
    let admission = admission_lock(root, deadline)?;
    let current_boot = BootIdentity::read()?.as_str().to_owned();
    for entry in read_dir_bounded(&creating_dir(root), MAX_COMMAND_STAGING_DIRECTORIES + 1)? {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&current_boot) {
            let _ = fs::remove_dir_all(path);
            continue;
        }
        let lease_path = path.join("caller.lease");
        let Ok(lease) = open_lock(&lease_path, false) else {
            let _ = fs::remove_dir_all(path);
            continue;
        };
        if flock(&lease, libc::LOCK_EX | libc::LOCK_NB).is_ok() {
            fs::remove_dir_all(path)?;
        }
    }
    unlock(&admission)
}
