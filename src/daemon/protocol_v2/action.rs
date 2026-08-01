use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use super::digest::sha256_hex;
use super::wire::{
    ACTION_ENVELOPE_PROTOCOL_VERSION, MAX_ACTION_ENVELOPE_BYTES, bounded_reason, canonical_json,
    fresh_id, parse_canonical_json, validate_digest, validate_id, validate_reason, validate_token,
};
use super::{BootClock, BootIdentity, NamespaceIdentity};
use crate::tray_action::TrayAction;

mod abandon;
mod claim;
mod claimed;
mod open;

const MAX_ACTIONS: usize = 2048;
const MAX_ACTION_QUARANTINE: usize = 1024;

#[cfg(test)]
static ANONYMOUS_PUBLISH_FAILURES: std::sync::LazyLock<std::sync::Mutex<BTreeMap<PathBuf, usize>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(BTreeMap::new()));

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "owner", deny_unknown_fields)]
pub(crate) enum ActionOwner {
    Anonymous {
        daemon_token: String,
    },
    Command {
        command_identity: String,
        daemon_token: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", deny_unknown_fields)]
enum JournalState {
    Prepared,
    Eligible,
    Claimed { claim_generation: String },
    Applied,
    Abandoned { reason: String },
    Indeterminate { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionRecord {
    protocol_version: u16,
    record_revision: u64,
    action_id: String,
    action_order: u64,
    owner: ActionOwner,
    action: TrayAction,
    payload_digest: String,
    state: JournalState,
}

#[derive(Serialize)]
struct ActionDigestPayload<'a> {
    protocol_version: u16,
    action_id: &'a str,
    action_order: u64,
    owner: &'a ActionOwner,
    action: TrayAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalHighWater {
    protocol_version: u16,
    boot_id: String,
    time_namespace_dev: u64,
    time_namespace_ino: u64,
    last_order: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedAction {
    pub(crate) action_id: String,
    pub(crate) action_order: u64,
    pub(crate) digest: String,
    path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct ClaimedAction {
    journal: ActionJournal,
    record: ActionRecord,
    path: PathBuf,
}

#[derive(Debug)]
pub(crate) enum ActionClaimOutcome {
    Claimed(ClaimedAction),
    Idle,
    Deferred,
}

#[derive(Debug)]
pub(crate) enum ActionFinishOutcome {
    Complete,
    Deferred(ClaimedAction),
}

#[derive(Debug, Clone)]
pub(crate) struct ActionJournal {
    root: PathBuf,
}

fn action_root() -> PathBuf {
    super::command_root().join("actions")
}

#[cfg(test)]
fn consume_anonymous_publish_failure(root: &Path) -> bool {
    let mut failures = ANONYMOUS_PUBLISH_FAILURES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let remove = match failures.get_mut(root) {
        Some(remaining) => {
            *remaining -= 1;
            *remaining == 0
        }
        None => return false,
    };
    if remove {
        failures.remove(root);
    }
    true
}

fn queue_dir(root: &Path) -> PathBuf {
    root.join("queue")
}

fn quarantine_dir(root: &Path) -> PathBuf {
    root.join("quarantine")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InodeIdentity {
    device: u64,
    inode: u64,
}

fn inode_identity(path: &Path) -> Result<InodeIdentity> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to identify action entry {}", path.display()))?;
    Ok(InodeIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn quarantine_action(root: &Path, path: &Path, expected: InodeIdentity) -> Result<()> {
    let quarantine = quarantine_dir(root);
    // Collect when this insertion *would* reach the cap, not once it has: the
    // entry about to be renamed in counts too, and a quarantine left sitting
    // exactly at the cap fails every capacity check until the next open.
    // Collecting here (rather than failing) also keeps the error out of the
    // claim path, where it would kill a running daemon over garbage entries.
    if fs::read_dir(&quarantine)?
        .take(MAX_ACTION_QUARANTINE + 1)
        .count()
        .saturating_add(1)
        >= MAX_ACTION_QUARANTINE
    {
        super::linux::gc_quarantine_tail(&quarantine, super::linux::QUARANTINE_RETAINED_ENTRIES)?;
    }
    if inode_identity(path)? != expected {
        bail!("action entry changed before quarantine");
    }
    let target = quarantine.join(format!("invalid-{}.action", fresh_id()?));
    fs::rename(path, &target).with_context(|| {
        format!(
            "failed to quarantine action entry {} as {}",
            path.display(),
            target.display()
        )
    })?;
    if inode_identity(&target)? != expected {
        bail!("action entry identity changed during quarantine");
    }
    Ok(())
}

fn action_name(order: u64, identity: &str) -> String {
    format!("{order:016x}-{identity}.action")
}

fn create_private_directory(path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
        Err(err) => return Err(err.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        bail!("{} is not a no-follow action directory", path.display());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn open_journal_lock(root: &Path) -> Result<File> {
    try_open_journal_lock(root, false)?
        .ok_or_else(|| anyhow!("blocking action journal lock unexpectedly deferred"))
}

fn try_open_journal_lock(root: &Path, nonblocking: bool) -> Result<Option<File>> {
    let path = root.join("journal.lock");
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    let file = options.open(&path)?;
    if !file.metadata()?.is_file() {
        bail!("action journal lock is not a regular file");
    }
    // SAFETY: file owns the descriptor; flock retains no pointer.
    let operation = libc::LOCK_EX | if nonblocking { libc::LOCK_NB } else { 0 };
    if unsafe { libc::flock(file.as_raw_fd(), operation) } != 0 {
        let error = io::Error::last_os_error();
        if nonblocking && error.kind() == ErrorKind::WouldBlock {
            return Ok(None);
        }
        return Err(error).context("failed to lock action journal");
    }
    Ok(Some(file))
}

fn unlock(file: &File) -> Result<()> {
    // SAFETY: file owns the descriptor; flock retains no pointer.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error()).context("failed to unlock action journal")
    }
}

fn write_record<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = canonical_json(value, MAX_ACTION_ENVELOPE_BYTES)?;
    crate::durable_io::write_atomic(
        path,
        &bytes,
        crate::durable_io::AtomicWriteOptions::private_runtime_file(),
    )
    .with_context(|| format!("failed to write action record {}", path.display()))
}

fn read_record<T: serde::de::DeserializeOwned + Serialize>(path: &Path) -> Result<T> {
    let bytes = super::linux::read_bounded_regular_file(path, MAX_ACTION_ENVELOPE_BYTES)?;
    parse_canonical_json(&bytes, MAX_ACTION_ENVELOPE_BYTES)
}

fn digest_payload(
    action_id: &str,
    order: u64,
    owner: &ActionOwner,
    action: TrayAction,
) -> Result<String> {
    let payload = canonical_json(
        &ActionDigestPayload {
            protocol_version: ACTION_ENVELOPE_PROTOCOL_VERSION,
            action_id,
            action_order: order,
            owner,
            action,
        },
        MAX_ACTION_ENVELOPE_BYTES,
    )?;
    sha256_hex(&payload)
}

fn validate_record(record: &ActionRecord) -> Result<()> {
    if record.protocol_version != ACTION_ENVELOPE_PROTOCOL_VERSION
        || record.record_revision == 0
        || record.action_order == 0
    {
        bail!("invalid action protocol version or revision");
    }
    validate_id(&record.action_id)?;
    validate_digest(&record.payload_digest)?;
    match &record.owner {
        ActionOwner::Anonymous { daemon_token } => validate_token(daemon_token)?,
        ActionOwner::Command {
            command_identity,
            daemon_token,
        } => {
            validate_id(command_identity)?;
            validate_token(daemon_token)?;
        }
    }
    if digest_payload(
        &record.action_id,
        record.action_order,
        &record.owner,
        record.action,
    )? != record.payload_digest
    {
        bail!("action payload digest mismatch");
    }
    match &record.state {
        JournalState::Claimed { claim_generation } => validate_id(claim_generation)?,
        JournalState::Abandoned { reason } | JournalState::Indeterminate { reason } => {
            validate_reason(reason)?
        }
        JournalState::Prepared | JournalState::Eligible | JournalState::Applied => {}
    }
    Ok(())
}

fn parse_action_name(name: &str) -> Result<(u64, String)> {
    let stem = name
        .strip_suffix(".action")
        .ok_or_else(|| anyhow!("invalid action filename"))?;
    let (order, identity) = stem
        .split_once('-')
        .ok_or_else(|| anyhow!("invalid action filename"))?;
    if order.len() != 16
        || !order
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        bail!("action order is not canonical");
    }
    validate_id(identity)?;
    Ok((u64::from_str_radix(order, 16)?, identity.to_owned()))
}

#[cfg(test)]
mod tests;
