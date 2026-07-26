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

const MAX_ACTIONS: usize = 2048;
const MAX_ACTION_QUARANTINE: usize = 1024;

#[cfg(test)]
const ANONYMOUS_PUBLISH_FAILURE_FILE: &str = ".test-anonymous-publish-failures";

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
    Claimed(Box<ClaimedAction>),
    Idle,
    Deferred,
}

#[derive(Debug)]
pub(crate) enum ActionFinishOutcome {
    Complete,
    Deferred(Box<ClaimedAction>),
}

#[derive(Debug, Clone)]
pub(crate) struct ActionJournal {
    command_root: PathBuf,
    root: PathBuf,
}

fn action_root(command_root: &Path) -> PathBuf {
    command_root.join("actions")
}

#[cfg(test)]
fn consume_anonymous_publish_failure(root: &Path) -> Result<bool> {
    let path = root.join(ANONYMOUS_PUBLISH_FAILURE_FILE);
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("failed to read action failure fixture"),
    };
    let remaining: usize = raw
        .parse()
        .context("action failure fixture contains an invalid count")?;
    if remaining <= 1 {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("failed to clear action failure fixture"),
        }
    } else {
        fs::write(&path, (remaining - 1).to_string())
            .context("failed to update action failure fixture")?;
    }
    Ok(true)
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
    if fs::read_dir(&quarantine)?
        .take(MAX_ACTION_QUARANTINE + 1)
        .count()
        >= MAX_ACTION_QUARANTINE
    {
        bail!("action quarantine capacity exhausted");
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

impl ActionJournal {
    pub(crate) fn open(command_root: PathBuf) -> Result<Self> {
        let root = action_root(&command_root);
        create_private_directory(&root)?;
        create_private_directory(&queue_dir(&root))?;
        create_private_directory(&quarantine_dir(&root))?;
        if fs::read_dir(quarantine_dir(&root))?
            .take(MAX_ACTION_QUARANTINE + 1)
            .count()
            >= MAX_ACTION_QUARANTINE
        {
            bail!("action quarantine capacity exhausted");
        }
        Ok(Self { command_root, root })
    }

    #[cfg(test)]
    pub(crate) fn fail_next_anonymous_publications(&self, count: usize) -> Result<()> {
        let path = self.root.join(ANONYMOUS_PUBLISH_FAILURE_FILE);
        if count == 0 {
            match fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error).context("failed to clear action failure fixture"),
            }
        } else {
            fs::write(path, count.to_string()).context("failed to write action failure fixture")
        }
    }

    pub(crate) fn quiesce_for_rollback(&self, compatibility_token: &str) -> Result<()> {
        validate_token(compatibility_token)?;
        let lock = open_journal_lock(&self.root)?;
        let mut entries = BTreeMap::new();
        for entry in fs::read_dir(queue_dir(&self.root))?.take(MAX_ACTIONS + 1) {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow!("action filename is not UTF-8"))?;
            let (order, identity) = parse_action_name(&name)?;
            if entries.insert(order, (identity, entry.path())).is_some() {
                unlock(&lock)?;
                bail!("duplicate action journal order during rollback");
            }
        }
        if entries.len() > MAX_ACTIONS {
            unlock(&lock)?;
            bail!("action journal capacity prevents bounded rollback");
        }
        for (order, (identity, path)) in entries {
            let mut record: ActionRecord = read_record(&path)?;
            validate_record(&record)?;
            if record.action_id != identity || record.action_order != order {
                unlock(&lock)?;
                bail!("action filename changed during rollback");
            }
            let owner_token = match &record.owner {
                ActionOwner::Anonymous { daemon_token }
                | ActionOwner::Command { daemon_token, .. } => daemon_token,
            };
            if owner_token == compatibility_token
                && !matches!(
                    record.state,
                    JournalState::Applied
                        | JournalState::Abandoned { .. }
                        | JournalState::Indeterminate { .. }
                )
            {
                unlock(&lock)?;
                bail!("rollback compatibility generation owns live v2 action work");
            }
            if !matches!(
                record.state,
                JournalState::Applied
                    | JournalState::Abandoned { .. }
                    | JournalState::Indeterminate { .. }
            ) {
                let reason = if matches!(record.state, JournalState::Claimed { .. }) {
                    "rollback preserved a claimed action with indeterminate delivery"
                } else {
                    "rollback rejected an action before delivery"
                };
                record.record_revision = record
                    .record_revision
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("action revision overflow"))?;
                record.state = if matches!(record.state, JournalState::Claimed { .. }) {
                    JournalState::Indeterminate {
                        reason: reason.into(),
                    }
                } else {
                    JournalState::Abandoned {
                        reason: reason.into(),
                    }
                };
                write_record(&path, &record)?;
            }
        }
        unlock(&lock)
    }

    fn allocate_order(&self) -> Result<u64> {
        let path = self.root.join("high-water.json");
        let now = BootClock::now()?.as_nanos();
        let boot_id = BootIdentity::read()?.as_str().to_owned();
        let namespace = NamespaceIdentity::current_time()?;
        let previous = match fs::symlink_metadata(&path) {
            Ok(_) => Some(read_record::<JournalHighWater>(&path)?),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let last = match previous {
            Some(previous)
                if previous.protocol_version == ACTION_ENVELOPE_PROTOCOL_VERSION
                    && previous.boot_id == boot_id
                    && previous.time_namespace_dev == namespace.dev
                    && previous.time_namespace_ino == namespace.ino =>
            {
                previous.last_order
            }
            Some(_) => bail!("action journal boot identity changed"),
            None => {
                if fs::read_dir(queue_dir(&self.root))?.next().is_some() {
                    bail!("missing action high-water record for nonempty journal");
                }
                0
            }
        };
        let order = now.max(
            last.checked_add(1)
                .ok_or_else(|| anyhow!("action order overflow"))?,
        );
        write_record(
            &path,
            &JournalHighWater {
                protocol_version: ACTION_ENVELOPE_PROTOCOL_VERSION,
                boot_id,
                time_namespace_dev: namespace.dev,
                time_namespace_ino: namespace.ino,
                last_order: order,
            },
        )?;
        Ok(order)
    }

    fn publish(
        &self,
        owner: ActionOwner,
        action: TrayAction,
        state: JournalState,
    ) -> Result<PreparedAction> {
        let lock = open_journal_lock(&self.root)?;
        let count = fs::read_dir(queue_dir(&self.root))?
            .take(MAX_ACTIONS + 1)
            .count();
        if count >= MAX_ACTIONS {
            unlock(&lock)?;
            bail!("action journal capacity exhausted");
        }
        let order = self.allocate_order()?;
        let action_id = fresh_id()?;
        let digest = digest_payload(&action_id, order, &owner, action)?;
        let path = queue_dir(&self.root).join(action_name(order, &action_id));
        let record = ActionRecord {
            protocol_version: ACTION_ENVELOPE_PROTOCOL_VERSION,
            record_revision: 1,
            action_id: action_id.clone(),
            action_order: order,
            owner,
            action,
            payload_digest: digest.clone(),
            state,
        };
        validate_record(&record)?;
        write_record(&path, &record)?;
        unlock(&lock)?;
        Ok(PreparedAction {
            action_id,
            action_order: order,
            digest,
            path,
        })
    }

    pub(crate) fn prepare_command(
        &self,
        command_identity: &str,
        daemon_token: &str,
        action: TrayAction,
    ) -> Result<PreparedAction> {
        validate_id(command_identity)?;
        validate_token(daemon_token)?;
        self.publish(
            ActionOwner::Command {
                command_identity: command_identity.to_owned(),
                daemon_token: daemon_token.to_owned(),
            },
            action,
            JournalState::Prepared,
        )
    }

    pub(crate) fn publish_anonymous(
        &self,
        daemon_token: &str,
        action: TrayAction,
    ) -> Result<PreparedAction> {
        #[cfg(test)]
        if consume_anonymous_publish_failure(&self.root)? {
            bail!("injected anonymous action admission failure");
        }
        validate_token(daemon_token)?;
        self.publish(
            ActionOwner::Anonymous {
                daemon_token: daemon_token.to_owned(),
            },
            action,
            JournalState::Eligible,
        )
    }

    #[cfg(test)]
    pub(crate) fn claim_next(
        &self,
        expected_daemon_token: &str,
        mut command_eligible: impl FnMut(&str, &PreparedAction) -> Result<bool>,
    ) -> Result<Option<ClaimedAction>> {
        match self.claim_next_inner(expected_daemon_token, false, |identity, prepared| {
            command_eligible(identity, prepared).map(|eligible| {
                Some(if eligible {
                    super::command::CommandActionClaim::Claimed
                } else {
                    super::command::CommandActionClaim::Barrier
                })
            })
        })? {
            ActionClaimOutcome::Claimed(action) => Ok(Some(*action)),
            ActionClaimOutcome::Idle => Ok(None),
            ActionClaimOutcome::Deferred => {
                bail!("blocking action claim unexpectedly deferred")
            }
        }
    }

    pub(in crate::daemon::protocol_v2) fn try_claim_next(
        &self,
        expected_daemon_token: &str,
        command_eligible: impl FnMut(
            &str,
            &PreparedAction,
        ) -> Result<Option<super::command::CommandActionClaim>>,
    ) -> Result<ActionClaimOutcome> {
        self.claim_next_inner(expected_daemon_token, true, command_eligible)
    }

    fn claim_next_inner(
        &self,
        expected_daemon_token: &str,
        nonblocking: bool,
        mut command_eligible: impl FnMut(
            &str,
            &PreparedAction,
        ) -> Result<Option<super::command::CommandActionClaim>>,
    ) -> Result<ActionClaimOutcome> {
        validate_token(expected_daemon_token)?;
        let Some(lock) = try_open_journal_lock(&self.root, nonblocking)? else {
            return Ok(ActionClaimOutcome::Deferred);
        };
        let raw_entries = fs::read_dir(queue_dir(&self.root))?
            .take(MAX_ACTIONS + 1)
            .collect::<io::Result<Vec<_>>>()?;
        if raw_entries.len() > MAX_ACTIONS {
            unlock(&lock)?;
            bail!("action journal exceeds its bounded capacity");
        }
        let mut entries = BTreeMap::new();
        let mut duplicate_orders = BTreeSet::new();
        for entry in raw_entries {
            let path = entry.path();
            let identity_on_disk = inode_identity(&path)?;
            let name = match entry.file_name().into_string() {
                Ok(name) => name,
                Err(_) => {
                    quarantine_action(&self.root, &path, identity_on_disk)?;
                    continue;
                }
            };
            let (order, identity) = match parse_action_name(&name) {
                Ok(parsed) => parsed,
                Err(_) => {
                    quarantine_action(&self.root, &path, identity_on_disk)?;
                    continue;
                }
            };
            if duplicate_orders.contains(&order) {
                quarantine_action(&self.root, &path, identity_on_disk)?;
                continue;
            }
            if let Some((_identity, previous_path, previous_inode)) =
                entries.insert(order, (identity, path.clone(), identity_on_disk))
            {
                entries.remove(&order);
                duplicate_orders.insert(order);
                quarantine_action(&self.root, &previous_path, previous_inode)?;
                quarantine_action(&self.root, &path, identity_on_disk)?;
            }
        }
        for (order, (identity, path, identity_on_disk)) in entries {
            let mut record: ActionRecord = match read_record(&path) {
                Ok(record) => record,
                Err(_) => {
                    quarantine_action(&self.root, &path, identity_on_disk)?;
                    continue;
                }
            };
            if validate_record(&record).is_err() {
                quarantine_action(&self.root, &path, identity_on_disk)?;
                continue;
            }
            if record.action_id != identity || record.action_order != order {
                quarantine_action(&self.root, &path, identity_on_disk)?;
                continue;
            }
            let prepared = PreparedAction {
                action_id: record.action_id.clone(),
                action_order: record.action_order,
                digest: record.payload_digest.clone(),
                path: path.clone(),
            };
            let owner_token = match &record.owner {
                ActionOwner::Anonymous { daemon_token }
                | ActionOwner::Command { daemon_token, .. } => daemon_token,
            };
            if owner_token != expected_daemon_token {
                if matches!(
                    record.state,
                    JournalState::Applied
                        | JournalState::Abandoned { .. }
                        | JournalState::Indeterminate { .. }
                ) {
                    fs::remove_file(&path)?;
                } else {
                    record.record_revision = record
                        .record_revision
                        .checked_add(1)
                        .ok_or_else(|| anyhow!("action revision overflow"))?;
                    record.state = if matches!(record.state, JournalState::Claimed { .. }) {
                        JournalState::Indeterminate {
                            reason: "claimed action belonged to a previous daemon generation"
                                .into(),
                        }
                    } else {
                        JournalState::Abandoned {
                            reason: "action belonged to a previous daemon generation".into(),
                        }
                    };
                    write_record(&path, &record)?;
                }
                continue;
            }

            if nonblocking && matches!(record.state, JournalState::Claimed { .. }) {
                let reason =
                    "previous overlay delivery ended after durable claim; outcome is indeterminate";
                record.record_revision = record
                    .record_revision
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("action revision overflow"))?;
                record.state = JournalState::Indeterminate {
                    reason: reason.into(),
                };
                write_record(&path, &record)?;
            }

            let eligible = match (&record.owner, &record.state) {
                (ActionOwner::Anonymous { .. }, JournalState::Eligible) => true,
                (
                    ActionOwner::Command {
                        command_identity, ..
                    },
                    JournalState::Prepared,
                ) => match command_eligible(command_identity, &prepared)? {
                    Some(super::command::CommandActionClaim::Claimed) => true,
                    Some(super::command::CommandActionClaim::Barrier) => {
                        unlock(&lock)?;
                        return Ok(ActionClaimOutcome::Idle);
                    }
                    Some(super::command::CommandActionClaim::Abandon(reason)) => {
                        record.record_revision = record
                            .record_revision
                            .checked_add(1)
                            .ok_or_else(|| anyhow!("action revision overflow"))?;
                        record.state = JournalState::Abandoned {
                            reason: reason.clone(),
                        };
                        write_record(&path, &record)?;
                        let finished = if nonblocking {
                            super::command::try_finish_command_action(
                                &self.command_root,
                                command_identity,
                                &prepared,
                                super::command::CommandActionResult::NoEffect,
                                Some(&reason),
                            )?
                        } else {
                            super::command::finish_command_action(
                                &self.command_root,
                                command_identity,
                                &prepared,
                                false,
                                Some(&reason),
                            )?;
                            true
                        };
                        if !finished {
                            unlock(&lock)?;
                            return Ok(ActionClaimOutcome::Deferred);
                        }
                        fs::remove_file(&path)?;
                        continue;
                    }
                    None => {
                        unlock(&lock)?;
                        return Ok(ActionClaimOutcome::Deferred);
                    }
                },
                (
                    ActionOwner::Command {
                        command_identity, ..
                    },
                    JournalState::Applied,
                ) => {
                    let finished = if nonblocking {
                        super::command::try_finish_command_action(
                            &self.command_root,
                            command_identity,
                            &prepared,
                            super::command::CommandActionResult::Applied,
                            None,
                        )?
                    } else {
                        super::command::finish_command_action(
                            &self.command_root,
                            command_identity,
                            &prepared,
                            true,
                            None,
                        )?;
                        true
                    };
                    if !finished {
                        unlock(&lock)?;
                        return Ok(ActionClaimOutcome::Deferred);
                    }
                    fs::remove_file(&path)?;
                    continue;
                }
                (
                    ActionOwner::Command {
                        command_identity, ..
                    },
                    JournalState::Abandoned { reason },
                ) => {
                    let finished = if nonblocking {
                        super::command::try_finish_command_action(
                            &self.command_root,
                            command_identity,
                            &prepared,
                            super::command::CommandActionResult::NoEffect,
                            Some(reason),
                        )?
                    } else {
                        super::command::finish_command_action(
                            &self.command_root,
                            command_identity,
                            &prepared,
                            false,
                            Some(reason),
                        )?;
                        true
                    };
                    if !finished {
                        unlock(&lock)?;
                        return Ok(ActionClaimOutcome::Deferred);
                    }
                    fs::remove_file(&path)?;
                    continue;
                }
                (
                    ActionOwner::Command {
                        command_identity, ..
                    },
                    JournalState::Indeterminate { reason },
                ) => {
                    let finished = if nonblocking {
                        super::command::try_finish_command_action(
                            &self.command_root,
                            command_identity,
                            &prepared,
                            super::command::CommandActionResult::Indeterminate,
                            Some(reason),
                        )?
                    } else {
                        super::command::finish_command_action_indeterminate(
                            &self.command_root,
                            command_identity,
                            &prepared,
                            reason,
                        )?;
                        true
                    };
                    if !finished {
                        unlock(&lock)?;
                        return Ok(ActionClaimOutcome::Deferred);
                    }
                    fs::remove_file(&path)?;
                    continue;
                }
                (
                    _,
                    JournalState::Abandoned { .. }
                    | JournalState::Indeterminate { .. }
                    | JournalState::Applied,
                ) => {
                    fs::remove_file(&path)?;
                    continue;
                }
                _ => false,
            };
            if !eligible {
                // Prepared command actions are global-order barriers until the
                // command owner makes the exact action eligible or records a
                // terminal tombstone. Later anonymous actions must not pass.
                unlock(&lock)?;
                return Ok(ActionClaimOutcome::Idle);
            }
            record.record_revision = record
                .record_revision
                .checked_add(1)
                .ok_or_else(|| anyhow!("action revision overflow"))?;
            record.state = JournalState::Claimed {
                claim_generation: fresh_id()?,
            };
            write_record(&path, &record)?;
            unlock(&lock)?;
            return Ok(ActionClaimOutcome::Claimed(Box::new(ClaimedAction {
                journal: self.clone(),
                record,
                path,
            })));
        }
        unlock(&lock)?;
        Ok(ActionClaimOutcome::Idle)
    }

    pub(crate) fn abandon(&self, prepared: &PreparedAction, reason: &str) -> Result<()> {
        let lock = open_journal_lock(&self.root)?;
        let mut record: ActionRecord = read_record(&prepared.path)?;
        if record.action_id != prepared.action_id
            || record.action_order != prepared.action_order
            || record.payload_digest != prepared.digest
            || !matches!(
                record.state,
                JournalState::Prepared | JournalState::Eligible
            )
        {
            unlock(&lock)?;
            bail!("cannot abandon changed or claimed action");
        }
        record.record_revision = record
            .record_revision
            .checked_add(1)
            .ok_or_else(|| anyhow!("action revision overflow"))?;
        record.state = JournalState::Abandoned {
            reason: bounded_reason(reason, 1024),
        };
        write_record(&prepared.path, &record)?;
        unlock(&lock)
    }

    pub(crate) fn abandon_command(
        &self,
        command_identity: &str,
        prepared: &PreparedAction,
        reason: &str,
    ) -> Result<()> {
        self.abandon(prepared, reason)?;
        super::command::finish_command_action(
            &self.command_root,
            command_identity,
            prepared,
            false,
            Some(reason),
        )?;
        let lock = open_journal_lock(&self.root)?;
        let terminal: ActionRecord = read_record(&prepared.path)?;
        if terminal.action_id != prepared.action_id
            || terminal.action_order != prepared.action_order
            || terminal.payload_digest != prepared.digest
            || !matches!(terminal.state, JournalState::Abandoned { .. })
        {
            unlock(&lock)?;
            bail!("abandoned command action changed before collection");
        }
        fs::remove_file(&prepared.path)?;
        unlock(&lock)
    }
}

impl ClaimedAction {
    pub(crate) fn action(&self) -> TrayAction {
        self.record.action
    }

    #[cfg(test)]
    pub(crate) fn owner(&self) -> &ActionOwner {
        &self.record.owner
    }

    #[cfg(test)]
    pub(crate) fn finish(mut self, applied: bool, reason: Option<&str>) -> Result<()> {
        let lock = open_journal_lock(&self.journal.root)?;
        let current: ActionRecord = read_record(&self.path)?;
        if current != self.record {
            unlock(&lock)?;
            bail!("claimed action changed before completion");
        }
        self.record.record_revision = self
            .record
            .record_revision
            .checked_add(1)
            .ok_or_else(|| anyhow!("action revision overflow"))?;
        self.record.state = if applied {
            JournalState::Applied
        } else {
            JournalState::Abandoned {
                reason: bounded_reason(reason.unwrap_or("handler proved no effect"), 1024),
            }
        };
        write_record(&self.path, &self.record)?;
        unlock(&lock)?;
        if let ActionOwner::Command {
            command_identity, ..
        } = &self.record.owner
        {
            let prepared = PreparedAction {
                action_id: self.record.action_id.clone(),
                action_order: self.record.action_order,
                digest: self.record.payload_digest.clone(),
                path: self.path.clone(),
            };
            super::command::finish_command_action(
                &self.journal.command_root,
                command_identity,
                &prepared,
                applied,
                reason,
            )?;
        }
        let lock = open_journal_lock(&self.journal.root)?;
        let terminal: ActionRecord = read_record(&self.path)?;
        if terminal != self.record {
            unlock(&lock)?;
            bail!("terminal action changed before collection");
        }
        fs::remove_file(&self.path)?;
        unlock(&lock)
    }

    pub(crate) fn try_finish(
        mut self,
        applied: bool,
        reason: Option<&str>,
    ) -> Result<ActionFinishOutcome> {
        if matches!(self.record.state, JournalState::Claimed { .. }) {
            let Some(lock) = try_open_journal_lock(&self.journal.root, true)? else {
                return Ok(ActionFinishOutcome::Deferred(Box::new(self)));
            };
            let current: ActionRecord = read_record(&self.path)?;
            if current != self.record {
                unlock(&lock)?;
                bail!("claimed action changed before completion");
            }
            self.record.record_revision = self
                .record
                .record_revision
                .checked_add(1)
                .ok_or_else(|| anyhow!("action revision overflow"))?;
            self.record.state = if applied {
                JournalState::Applied
            } else {
                JournalState::Abandoned {
                    reason: bounded_reason(reason.unwrap_or("handler proved no effect"), 1024),
                }
            };
            write_record(&self.path, &self.record)?;
            unlock(&lock)?;
        }

        if let ActionOwner::Command {
            command_identity, ..
        } = &self.record.owner
        {
            let prepared = PreparedAction {
                action_id: self.record.action_id.clone(),
                action_order: self.record.action_order,
                digest: self.record.payload_digest.clone(),
                path: self.path.clone(),
            };
            let result = if applied {
                super::command::CommandActionResult::Applied
            } else {
                super::command::CommandActionResult::NoEffect
            };
            if !super::command::try_finish_command_action(
                &self.journal.command_root,
                command_identity,
                &prepared,
                result,
                reason,
            )? {
                return Ok(ActionFinishOutcome::Deferred(Box::new(self)));
            }
        }

        let Some(lock) = try_open_journal_lock(&self.journal.root, true)? else {
            return Ok(ActionFinishOutcome::Deferred(Box::new(self)));
        };
        let terminal: ActionRecord = read_record(&self.path)?;
        if terminal != self.record {
            unlock(&lock)?;
            bail!("terminal action changed before collection");
        }
        fs::remove_file(&self.path)?;
        unlock(&lock)?;
        Ok(ActionFinishOutcome::Complete)
    }
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
mod tests {
    use super::*;

    #[test]
    fn action_digests_match_protocol_v2_golden_values() {
        const ACTION_ID: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        const COMMAND_ID: &str = "cccccccccccccccccccccccccccccccc";
        const DAEMON_TOKEN: &str =
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        let anonymous = ActionOwner::Anonymous {
            daemon_token: DAEMON_TOKEN.into(),
        };
        assert_eq!(
            digest_payload(ACTION_ID, 42, &anonymous, TrayAction::CaptureRegion)
                .expect("fixture anonymous action fields are canonical"),
            "53a40b0ef73b768dfa543746835ac6704255db3b9c9c1ad6b06a38f98a13a9c1"
        );

        let command = ActionOwner::Command {
            command_identity: COMMAND_ID.into(),
            daemon_token: DAEMON_TOKEN.into(),
        };
        assert_eq!(
            digest_payload(ACTION_ID, 42, &command, TrayAction::ToggleHelp)
                .expect("fixture command action fields are canonical"),
            "c8aa91f67acd22621252e0e95cee6221cc02a9f1ca72cdc78e46eb3fd0dabf33"
        );
    }

    fn with_runtime<T>(run: impl FnOnce(&Path) -> T) -> T {
        let temp = crate::test_temp::tempdir().expect("fixture creates its runtime directory");
        let root = temp.path().join("daemon-commands").join("v2");
        super::super::command::prepare_layout(&root)
            .expect("fixture prepares its protocol-v2 command layout");
        run(&root)
    }

    #[test]
    fn anonymous_actions_keep_global_order_and_terminal_tombstones() {
        with_runtime(|root| {
            let journal =
                ActionJournal::open(root.to_path_buf()).expect("fixture opens its action journal");
            let token = super::super::ProtocolToken::generate()
                .expect("fixture generates its daemon protocol token")
                .to_string();
            journal
                .publish_anonymous(&token, TrayAction::LightDrawOn)
                .expect("fixture publishes its first anonymous action");
            journal
                .publish_anonymous(&token, TrayAction::LightDrawOff)
                .expect("fixture publishes its second anonymous action");
            let first = journal
                .claim_next(&token, |_, _| Ok(false))
                .expect("fixture scans its anonymous action queue")
                .expect("fixture queue contains its first anonymous action");
            assert_eq!(first.action(), TrayAction::LightDrawOn);
            first
                .finish(true, None)
                .expect("fixture completes its first anonymous action");
            let second = journal
                .claim_next(&token, |_, _| Ok(false))
                .expect("fixture scans its remaining anonymous action queue")
                .expect("fixture queue contains its second anonymous action");
            assert_eq!(second.action(), TrayAction::LightDrawOff);
            second
                .finish(false, Some("not active"))
                .expect("fixture abandons its second anonymous action");
            assert!(
                journal
                    .claim_next(&token, |_, _| Ok(false))
                    .expect("fixture scans its exhausted anonymous action queue")
                    .is_none()
            );
        });
    }

    #[test]
    fn command_action_stays_ineligible_until_exact_commit_predicate() {
        with_runtime(|root| {
            let journal =
                ActionJournal::open(root.to_path_buf()).expect("fixture opens its action journal");
            let token = super::super::ProtocolToken::generate()
                .expect("fixture generates its daemon protocol token")
                .to_string();
            let command = super::super::ProtocolId::generate()
                .expect("fixture generates its command identity")
                .to_string();
            let prepared = journal
                .prepare_command(&command, &token, TrayAction::ToggleFreeze)
                .expect("fixture prepares its command-owned action");
            journal
                .publish_anonymous(&token, TrayAction::CaptureRegion)
                .expect("fixture publishes its later anonymous action");
            assert!(
                journal
                    .claim_next(&token, |_, _| Ok(false))
                    .expect("fixture scans while its head command is uncommitted")
                    .is_none()
            );
            let claimed = journal
                .claim_next(&token, |identity, candidate| {
                    Ok(identity == command && candidate.action_id == prepared.action_id)
                })
                .expect("fixture scans with its exact commit predicate")
                .expect("exact predicate makes the command action claimable");
            assert_eq!(
                claimed.owner(),
                &ActionOwner::Command {
                    command_identity: command,
                    daemon_token: token,
                }
            );
            assert_eq!(claimed.action(), TrayAction::ToggleFreeze);
        });
    }

    #[test]
    fn event_loop_claim_and_finish_defer_instead_of_waiting_for_locks() {
        with_runtime(|root| {
            let token = super::super::ProtocolToken::generate()
                .expect("fixture generates its daemon protocol token")
                .to_string();
            let owner = super::super::command::CommandOwner::open(&token, root.to_path_buf())
                .expect("fixture opens its v2 command owner");
            let journal =
                ActionJournal::open(root.to_path_buf()).expect("fixture opens its action journal");
            let request = super::super::wire::DaemonRequestV2 {
                mode: None,
                freeze: false,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: Some(TrayAction::ToggleFreeze),
            };
            let _client = super::super::command::ClientCommand::publish(&request, &token, root)
                .expect("fixture publishes its command-owned action request");
            let mut command = owner
                .claim_next()
                .expect("fixture scans its v2 command queue")
                .expect("fixture queue contains its action command");
            let command_identity = command.identity().to_owned();
            let _prepared = command
                .prepare_action(&journal)
                .expect("fixture prepares its command action")
                .expect("open fixture command produces an action envelope");
            command
                .commit(super::super::wire::EffectKind::DeliverReadyAction)
                .expect("fixture commits its command action");

            let held_journal_lock = open_journal_lock(&journal.root)
                .expect("fixture exclusively holds its action journal lock");
            assert!(matches!(
                journal
                    .try_claim_next(&token, |identity, candidate| {
                        super::super::command::try_claim_command_action(root, identity, candidate)
                    })
                    .expect("fixture performs a nonblocking journal claim"),
                ActionClaimOutcome::Deferred
            ));
            unlock(&held_journal_lock).expect("fixture releases its action journal lock");

            // The command claim still owns decision.lock, so the action claimant
            // must defer without sleeping on the Wayland event-loop thread.
            assert!(matches!(
                journal
                    .try_claim_next(&token, |identity, candidate| {
                        super::super::command::try_claim_command_action(root, identity, candidate)
                    })
                    .expect("fixture performs a nonblocking command-action claim"),
                ActionClaimOutcome::Deferred
            ));
            command
                .defer()
                .expect("fixture releases its command decision lock");

            let claim_outcome = journal
                .try_claim_next(&token, |identity, candidate| {
                    super::super::command::try_claim_command_action(root, identity, candidate)
                })
                .expect("fixture retries its nonblocking command-action claim");
            let action = match claim_outcome {
                ActionClaimOutcome::Claimed(action) => action,
                other => {
                    assert!(
                        matches!(other, ActionClaimOutcome::Claimed(_)),
                        "released command lock should make the action claimable: {other:?}"
                    );
                    return;
                }
            };

            let decision_path = root
                .join("controls")
                .join(command_identity)
                .join("decision.lock");
            let held_decision = OpenOptions::new()
                .read(true)
                .write(true)
                .open(decision_path)
                .expect("fixture opens its command decision lock");
            assert_eq!(
                unsafe { libc::flock(held_decision.as_raw_fd(), libc::LOCK_EX) },
                0
            );
            let finish_outcome = action
                .try_finish(true, None)
                .expect("fixture attempts a nonblocking action finish");
            let action = match finish_outcome {
                ActionFinishOutcome::Deferred(action) => action,
                other => {
                    assert!(
                        matches!(other, ActionFinishOutcome::Deferred(_)),
                        "contended command finish should defer: {other:?}"
                    );
                    return;
                }
            };
            assert_eq!(
                unsafe { libc::flock(held_decision.as_raw_fd(), libc::LOCK_UN) },
                0
            );
            assert!(matches!(
                action
                    .try_finish(true, None)
                    .expect("fixture retries action finish after releasing contention"),
                ActionFinishOutcome::Complete
            ));
        });
    }

    #[test]
    fn cancellation_during_action_preparation_leaves_a_collectable_tombstone() {
        with_runtime(|root| {
            let token = super::super::ProtocolToken::generate()
                .expect("fixture generates its daemon protocol token")
                .to_string();
            let owner = super::super::command::CommandOwner::open(&token, root.to_path_buf())
                .expect("fixture opens its v2 command owner");
            let journal =
                ActionJournal::open(root.to_path_buf()).expect("fixture opens its action journal");
            let client = super::super::command::ClientCommand::publish(
                &super::super::wire::DaemonRequestV2 {
                    mode: None,
                    freeze: false,
                    exit_after_capture: false,
                    no_exit_after_capture: false,
                    resume_session: false,
                    no_resume_session: false,
                    session_file: None,
                    overlay_action: Some(TrayAction::ToggleFreeze),
                },
                &token,
                root,
            )
            .expect("fixture publishes its cancelable action command");
            let command = owner
                .claim_next()
                .expect("fixture scans its v2 command queue")
                .expect("fixture queue contains its cancelable command");
            let held_journal_lock = open_journal_lock(&journal.root)
                .expect("fixture holds its action journal lock during preparation");
            let worker_journal = journal.clone();
            let worker = std::thread::spawn(move || {
                let mut command = command;
                let outcome = command.prepare_action(&worker_journal);
                (outcome, command)
            });

            assert_eq!(
                client
                    .cancel()
                    .expect("fixture cancels its uncommitted action command"),
                super::super::command::TerminalCommandResult::Canceled
            );
            unlock(&held_journal_lock).expect("fixture releases its action journal lock");
            let (outcome, command) = worker
                .join()
                .expect("fixture action-preparation worker exits normally");
            assert!(
                outcome
                    .expect("fixture preparation reconciles its canceled command")
                    .is_none()
            );
            command
                .defer()
                .expect("fixture releases its canceled command claim");

            assert!(
                journal
                    .claim_next(&token, |_, _| Ok(false))
                    .expect("fixture scans after canceled action preparation")
                    .is_none()
            );
            assert_eq!(
                owner
                    .collect_terminal()
                    .expect("fixture collects its canceled command tombstone"),
                1
            );
        });
    }

    #[test]
    fn canceled_command_reconciles_prepared_and_crash_left_action_envelopes() {
        for record_preparation in [false, true] {
            with_runtime(|root| {
                let token = super::super::ProtocolToken::generate()
                    .expect("fixture generates its daemon protocol token")
                    .to_string();
                let owner = super::super::command::CommandOwner::open(&token, root.to_path_buf())
                    .expect("fixture opens its v2 command owner");
                let journal = ActionJournal::open(root.to_path_buf())
                    .expect("fixture opens its action journal");
                let client = super::super::command::ClientCommand::publish(
                    &super::super::wire::DaemonRequestV2 {
                        mode: None,
                        freeze: false,
                        exit_after_capture: false,
                        no_exit_after_capture: false,
                        resume_session: false,
                        no_resume_session: false,
                        session_file: None,
                        overlay_action: Some(TrayAction::ToggleFreeze),
                    },
                    &token,
                    root,
                )
                .expect("fixture publishes its cancelable action command");
                let mut command = owner
                    .claim_next()
                    .expect("fixture scans its v2 command queue")
                    .expect("fixture queue contains its cancelable command");
                if record_preparation {
                    command
                        .prepare_action(&journal)
                        .expect("fixture records command action preparation")
                        .expect("open fixture command produces an action envelope");
                } else {
                    journal
                        .prepare_command(command.identity(), &token, TrayAction::ToggleFreeze)
                        .expect("fixture leaves a crash-style prepared action envelope");
                }
                command
                    .defer()
                    .expect("fixture releases its uncommitted command claim");
                assert_eq!(
                    client
                        .cancel()
                        .expect("fixture cancels its uncommitted action command"),
                    super::super::command::TerminalCommandResult::Canceled
                );

                assert!(matches!(
                    journal
                        .try_claim_next(&token, |identity, candidate| {
                            super::super::command::try_claim_command_action(
                                root, identity, candidate,
                            )
                        })
                        .expect("fixture reconciles its canceled action envelope"),
                    ActionClaimOutcome::Idle
                ));
                assert_eq!(
                    owner
                        .collect_terminal()
                        .expect("fixture collects its canceled command tombstone"),
                    1
                );
            });
        }
    }

    #[test]
    fn orphaned_claim_becomes_committed_indeterminate_without_replay() {
        with_runtime(|root| {
            let token = super::super::ProtocolToken::generate()
                .expect("fixture generates its daemon protocol token")
                .to_string();
            let owner = super::super::command::CommandOwner::open(&token, root.to_path_buf())
                .expect("fixture opens its v2 command owner");
            let journal =
                ActionJournal::open(root.to_path_buf()).expect("fixture opens its action journal");
            let request = super::super::wire::DaemonRequestV2 {
                mode: None,
                freeze: false,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: Some(TrayAction::ToggleFreeze),
            };
            let client = super::super::command::ClientCommand::publish(&request, &token, root)
                .expect("fixture publishes its command-owned action request");
            let mut command = owner
                .claim_next()
                .expect("fixture scans its v2 command queue")
                .expect("fixture queue contains its action command");
            command
                .prepare_action(&journal)
                .expect("fixture prepares its command action")
                .expect("open fixture command produces an action envelope");
            command
                .commit(super::super::wire::EffectKind::DeliverReadyAction)
                .expect("fixture commits its command action");
            command
                .defer()
                .expect("fixture releases its committed command claim");

            let claim_outcome = journal
                .try_claim_next(&token, |identity, candidate| {
                    super::super::command::try_claim_command_action(root, identity, candidate)
                })
                .expect("fixture claims its committed command action");
            let orphaned = match claim_outcome {
                ActionClaimOutcome::Claimed(action) => action,
                other => {
                    assert!(
                        matches!(other, ActionClaimOutcome::Claimed(_)),
                        "committed action should be claimable: {other:?}"
                    );
                    return;
                }
            };
            drop(orphaned);

            assert!(matches!(
                journal
                    .try_claim_next(&token, |identity, candidate| {
                        super::super::command::try_claim_command_action(root, identity, candidate)
                    })
                    .expect("fixture reconciles its orphaned committed action"),
                ActionClaimOutcome::Idle
            ));
            assert!(matches!(
                client
                    .wait()
                    .expect("fixture client observes its reconciled terminal result"),
                super::super::command::TerminalCommandResult::CommittedIndeterminate(_)
            ));
        });
    }

    #[test]
    fn digest_and_filename_tampering_fail_closed() {
        with_runtime(|root| {
            let journal =
                ActionJournal::open(root.to_path_buf()).expect("fixture opens its action journal");
            let token = super::super::ProtocolToken::generate()
                .expect("fixture generates its daemon protocol token")
                .to_string();
            let action = journal
                .publish_anonymous(&token, TrayAction::CaptureRegion)
                .expect("fixture publishes its anonymous action");
            let bytes = fs::read(&action.path).expect("fixture reads its published action record");
            let mut record: serde_json::Value =
                serde_json::from_slice(&bytes).expect("fixture parses its published action record");
            record["action"] = serde_json::json!("capture_full");
            let tampered =
                serde_json::to_vec(&record).expect("fixture serializes its tampered action record");
            fs::write(&action.path, tampered).expect("fixture writes its tampered action record");
            assert!(
                journal
                    .claim_next(&token, |_, _| Ok(false))
                    .expect("fixture scans its tampered action queue")
                    .is_none()
            );
            assert_eq!(
                fs::read_dir(quarantine_dir(&journal.root))
                    .expect("fixture reads its action quarantine")
                    .count(),
                1
            );
        });
    }

    #[test]
    fn filename_order_tampering_and_revision_overflow_fail_closed() {
        with_runtime(|root| {
            let journal =
                ActionJournal::open(root.to_path_buf()).expect("fixture opens its action journal");
            let token = super::super::ProtocolToken::generate()
                .expect("fixture generates its daemon protocol token")
                .to_string();
            let action = journal
                .publish_anonymous(&token, TrayAction::CaptureRegion)
                .expect("fixture publishes its anonymous action");
            let changed_path = queue_dir(&journal.root).join(action_name(
                action
                    .action_order
                    .checked_add(1)
                    .expect("fixture action order has room for one increment"),
                &action.action_id,
            ));
            fs::rename(&action.path, changed_path)
                .expect("fixture changes its action filename order");
            assert!(
                journal
                    .claim_next(&token, |_, _| Ok(false))
                    .expect("fixture scans its filename-tampered action queue")
                    .is_none()
            );
            assert_eq!(
                fs::read_dir(quarantine_dir(&journal.root))
                    .expect("fixture reads its action quarantine")
                    .count(),
                1
            );
        });

        with_runtime(|root| {
            let journal =
                ActionJournal::open(root.to_path_buf()).expect("fixture opens its action journal");
            let token = super::super::ProtocolToken::generate()
                .expect("fixture generates its daemon protocol token")
                .to_string();
            let action = journal
                .publish_anonymous(&token, TrayAction::CaptureRegion)
                .expect("fixture publishes its anonymous action");
            let mut record: ActionRecord =
                read_record(&action.path).expect("fixture reads its published action record");
            record.record_revision = u64::MAX;
            write_record(&action.path, &record)
                .expect("fixture writes its maximum-revision action record");
            assert!(journal.abandon(&action, "not applied").is_err());
        });
    }

    #[test]
    fn rollback_keeps_an_indeterminate_command_action_tombstone() {
        with_runtime(|root| {
            let token = super::super::ProtocolToken::generate()
                .expect("fixture generates its daemon protocol token")
                .to_string();
            let owner = super::super::command::CommandOwner::open(&token, root.to_path_buf())
                .expect("fixture opens its v2 command owner");
            let journal =
                ActionJournal::open(root.to_path_buf()).expect("fixture opens its action journal");
            let client = super::super::ClientCommand::publish(
                &super::super::DaemonRequestV2 {
                    mode: None,
                    freeze: false,
                    exit_after_capture: false,
                    no_exit_after_capture: false,
                    resume_session: false,
                    no_resume_session: false,
                    session_file: None,
                    overlay_action: Some(TrayAction::ToggleFreeze),
                },
                &token,
                root,
            )
            .expect("fixture publishes its command-owned action request");
            let mut claim = owner
                .claim_next()
                .expect("fixture scans its v2 command queue")
                .expect("fixture queue contains its action command");
            let prepared = claim
                .prepare_action(&journal)
                .expect("fixture prepares its command action")
                .expect("open fixture command produces an action envelope");
            claim
                .commit(super::super::EffectKind::StartAndDeliverAction)
                .expect("fixture commits its command action");
            claim
                .defer()
                .expect("fixture releases its committed command claim");

            super::super::prepare_rollback_compatibility_at_root(root)
                .expect("fixture prepares protocol-v2 rollback compatibility");
            let tombstone: ActionRecord =
                read_record(&prepared.path).expect("fixture reads its rollback action tombstone");
            assert!(matches!(tombstone.state, JournalState::Abandoned { .. }));
            assert!(matches!(
                client
                    .wait()
                    .expect("fixture client observes its rollback terminal result"),
                super::super::TerminalCommandResult::CommittedIndeterminate(_)
            ));
        });
    }
}
