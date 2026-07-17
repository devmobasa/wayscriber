use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};

use super::super::wire::{
    AdmissionRecord, CommandControl, DAEMON_COMMAND_PROTOCOL_VERSION, MAX_ADMISSION_RECORD_BYTES,
    MAX_CONTROL_RECORD_BYTES, NamespaceIdentityV2, canonical_json, fresh_id, parse_canonical_json,
};
use super::super::{BootClock, BootDeadline, BootIdentity, NamespaceIdentity};
use super::{
    LOCK_RETRY, MAX_COMMAND_CONTROLS, MAX_COMMAND_GC_DIRECTORIES, MAX_COMMAND_QUARANTINE_ENTRIES,
    MAX_COMMAND_QUEUE_REFERENCES, MAX_COMMAND_ROOT_ENTRIES, MAX_COMMAND_STAGING_DIRECTORIES,
};

pub(crate) fn command_root() -> PathBuf {
    crate::paths::daemon_command_dir().join("v2")
}

pub(super) fn creating_dir(root: &Path) -> PathBuf {
    root.join(".creating")
}

pub(super) fn controls_dir(root: &Path) -> PathBuf {
    root.join("controls")
}

pub(super) fn queue_dir(root: &Path) -> PathBuf {
    root.join("queue")
}

pub(super) fn gc_dir(root: &Path) -> PathBuf {
    root.join(".gc")
}

pub(super) fn quarantine_dir(root: &Path) -> PathBuf {
    root.join("quarantine")
}

pub(super) fn control_path(root: &Path, identity: &str) -> PathBuf {
    controls_dir(root).join(identity)
}

pub(super) fn control_record_path(control: &Path) -> PathBuf {
    control.join("control.json")
}

pub(super) fn queue_name(order: u64, identity: &str) -> String {
    format!("{order:016x}-{identity}.request")
}

pub(super) fn queue_path(root: &Path, order: u64, identity: &str) -> PathBuf {
    queue_dir(root).join(queue_name(order, identity))
}

pub(super) fn create_private_directory(path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                bail!("{} is not a no-follow protocol directory", path.display());
            }
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn prepare_layout(root: &Path) -> Result<()> {
    if let Some(parent) = root.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create protocol parent {}", parent.display()))?;
        create_private_directory(parent)?;
    }
    create_private_directory(root)?;
    for path in [
        creating_dir(root),
        controls_dir(root),
        queue_dir(root),
        gc_dir(root),
        quarantine_dir(root),
        quarantine_dir(root).join("queue"),
        quarantine_dir(root).join("control"),
    ] {
        create_private_directory(&path)?;
    }
    validate_root_shape(root)
}

pub(super) fn validate_root_shape(root: &Path) -> Result<()> {
    let allowed = BTreeSet::from([
        ".creating",
        ".gc",
        "actions",
        "admission.json",
        "admission.lock",
        "controls",
        "children",
        "quarantine",
        "queue",
    ]);
    let entries = read_dir_bounded(root, MAX_COMMAND_ROOT_ENTRIES)?;
    for entry in entries {
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow!("v2 root contains a non-UTF-8 entry"))?;
        if !allowed.contains(name.as_str()) && !is_atomic_temp(&name, "admission.json") {
            bail!("unexpected v2 command-root entry {name}");
        }
    }
    Ok(())
}

pub(super) fn is_atomic_temp(name: &str, target: &str) -> bool {
    name.starts_with(&format!(".{target}.tmp-"))
}

pub(super) fn read_dir_bounded(path: &Path, cap: usize) -> Result<Vec<fs::DirEntry>> {
    let mut result = Vec::new();
    for entry in fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))? {
        if result.len() >= cap {
            bail!(
                "protocol directory {} exceeds its entry cap",
                path.display()
            );
        }
        result.push(entry.with_context(|| format!("failed to enumerate {}", path.display()))?);
    }
    Ok(result)
}

pub(super) fn ensure_capacity(root: &Path) -> Result<()> {
    let checks = [
        (creating_dir(root), MAX_COMMAND_STAGING_DIRECTORIES),
        (controls_dir(root), MAX_COMMAND_CONTROLS),
        (queue_dir(root), MAX_COMMAND_QUEUE_REFERENCES),
        (gc_dir(root), MAX_COMMAND_GC_DIRECTORIES),
    ];
    for (path, cap) in checks {
        if read_dir_bounded(&path, cap + 1)?.len() >= cap {
            bail!("v2 command capacity exhausted at {}", path.display());
        }
    }
    let quarantine_count = read_dir_bounded(
        &quarantine_dir(root).join("queue"),
        MAX_COMMAND_QUARANTINE_ENTRIES + 1,
    )?
    .len()
        + read_dir_bounded(
            &quarantine_dir(root).join("control"),
            MAX_COMMAND_QUARANTINE_ENTRIES + 1,
        )?
        .len();
    if quarantine_count >= MAX_COMMAND_QUARANTINE_ENTRIES {
        bail!("v2 command quarantine capacity exhausted");
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) enum QuarantineKind {
    Queue,
    Control,
}

pub(super) fn quarantine_entry(root: &Path, source: &Path, kind: QuarantineKind) -> Result<()> {
    let deadline = BootClock::now()?.checked_add(Duration::from_millis(200))?;
    let admission = admission_lock(root, deadline)?;
    let queue_quarantine = quarantine_dir(root).join("queue");
    let control_quarantine = quarantine_dir(root).join("control");
    let count = read_dir_bounded(&queue_quarantine, MAX_COMMAND_QUARANTINE_ENTRIES + 1)?.len()
        + read_dir_bounded(&control_quarantine, MAX_COMMAND_QUARANTINE_ENTRIES + 1)?.len();
    if count >= MAX_COMMAND_QUARANTINE_ENTRIES {
        unlock(&admission)?;
        bail!("v2 command quarantine capacity exhausted");
    }
    let before = fs::symlink_metadata(source)
        .with_context(|| format!("failed to identify quarantine source {}", source.display()))?;
    let identity = fresh_id()?;
    let target = match kind {
        QuarantineKind::Queue => queue_quarantine.join(format!("invalid-{identity}.request")),
        QuarantineKind::Control => control_quarantine.join(format!("invalid-{identity}")),
    };
    fs::rename(source, &target).with_context(|| {
        format!(
            "failed to quarantine protocol entry {} as {}",
            source.display(),
            target.display()
        )
    })?;
    let after = fs::symlink_metadata(&target)?;
    if before.dev() != after.dev() || before.ino() != after.ino() {
        unlock(&admission)?;
        bail!("quarantined protocol entry identity changed during rename");
    }
    unlock(&admission)
}

pub(super) fn open_lock(path: &Path, create_new: bool) -> Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    if create_new {
        options.create_new(true);
    }
    let file = options
        .open(path)
        .with_context(|| format!("failed to open protocol lock {}", path.display()))?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        bail!("protocol lock {} is not a regular file", path.display());
    }
    Ok(file)
}

pub(super) fn flock(file: &File, operation: libc::c_int) -> io::Result<()> {
    // SAFETY: file owns a valid descriptor and flock does not retain it.
    if unsafe { libc::flock(file.as_raw_fd(), operation) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(super) fn unlock(file: &File) -> Result<()> {
    flock(file, libc::LOCK_UN).context("failed to unlock protocol descriptor")
}

pub(super) fn lock_until(
    file: &File,
    operation: libc::c_int,
    deadline: BootDeadline,
) -> Result<()> {
    if try_lock_until(file, operation, deadline)? {
        Ok(())
    } else {
        bail!("protocol lock deadline expired")
    }
}

pub(super) fn try_lock_until(
    file: &File,
    operation: libc::c_int,
    deadline: BootDeadline,
) -> Result<bool> {
    loop {
        match flock(file, operation | libc::LOCK_NB) {
            Ok(()) => return Ok(true),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if BootClock::now()? >= deadline {
                    return Ok(false);
                }
                std::thread::sleep(LOCK_RETRY);
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error).context("failed to acquire protocol lock"),
        }
    }
}

pub(super) fn admission_lock(root: &Path, deadline: BootDeadline) -> Result<File> {
    let path = root.join("admission.lock");
    let file = match open_lock(&path, true) {
        Ok(file) => file,
        Err(error)
            if error
                .downcast_ref::<io::Error>()
                .is_some_and(|source| source.kind() == ErrorKind::AlreadyExists) =>
        {
            open_lock(&path, false)?
        }
        Err(error) => return Err(error),
    };
    lock_until(&file, libc::LOCK_EX, deadline)?;
    Ok(file)
}

pub(super) fn write_record<T: serde::Serialize>(path: &Path, value: &T, cap: usize) -> Result<()> {
    let bytes = canonical_json(value, cap)?;
    crate::durable_io::write_atomic(
        path,
        &bytes,
        crate::durable_io::AtomicWriteOptions::private_runtime_file(),
    )
    .with_context(|| format!("failed to publish protocol record {}", path.display()))
}

pub(super) fn read_record<T: serde::de::DeserializeOwned + serde::Serialize>(
    path: &Path,
    cap: usize,
) -> Result<T> {
    let bytes = super::super::linux::read_bounded_regular_file(path, cap)
        .with_context(|| format!("failed to read protocol record {}", path.display()))?;
    parse_canonical_json(&bytes, cap)
}

pub(super) fn allocate_order(root: &Path) -> Result<u64> {
    let path = root.join("admission.json");
    let now = BootClock::now()?.as_nanos();
    let boot_id = BootIdentity::read()?.as_str().to_owned();
    let namespace: NamespaceIdentityV2 = NamespaceIdentity::current_time()?.into();
    let previous = match fs::symlink_metadata(&path) {
        Ok(_) => Some(read_record::<AdmissionRecord>(
            &path,
            MAX_ADMISSION_RECORD_BYTES,
        )?),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let last_order = match previous {
        Some(previous) => {
            if previous.protocol_version != DAEMON_COMMAND_PROTOCOL_VERSION
                || previous.boot_id != boot_id
                || previous.time_namespace != namespace
            {
                bail!("v2 command admission identity changed");
            }
            previous.last_order
        }
        None => {
            if !read_dir_bounded(&controls_dir(root), 1)?.is_empty()
                || !read_dir_bounded(&queue_dir(root), 1)?.is_empty()
            {
                bail!("missing admission high-water record for nonempty v2 tree");
            }
            0
        }
    };
    let next = now.max(
        last_order
            .checked_add(1)
            .ok_or_else(|| anyhow!("command order overflow"))?,
    );
    write_record(
        &path,
        &AdmissionRecord {
            protocol_version: DAEMON_COMMAND_PROTOCOL_VERSION,
            boot_id,
            time_namespace: namespace,
            last_order: next,
        },
        MAX_ADMISSION_RECORD_BYTES,
    )?;
    let proved: AdmissionRecord = read_record(&path, MAX_ADMISSION_RECORD_BYTES)?;
    if proved.last_order != next {
        bail!("command high-water allocation was not durable");
    }
    Ok(next)
}

pub(super) fn bump_revision(control: &mut CommandControl) -> Result<()> {
    control.record_revision = control
        .record_revision
        .checked_add(1)
        .ok_or_else(|| anyhow!("command record revision overflow"))?;
    Ok(())
}

pub(super) fn write_control(path: &Path, control: &CommandControl) -> Result<()> {
    control.validate()?;
    write_record(
        &control_record_path(path),
        control,
        MAX_CONTROL_RECORD_BYTES,
    )
}

pub(super) fn read_control(path: &Path) -> Result<CommandControl> {
    let control: CommandControl =
        read_record(&control_record_path(path), MAX_CONTROL_RECORD_BYTES)?;
    control.validate()?;
    Ok(control)
}
