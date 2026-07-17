use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use super::super::wire::{
    ActionStatus, CallerDisposition, ChildReportStatus, CommandDecision, CommandResponse,
    EffectStatus, MAX_QUEUE_REFERENCE_BYTES, PublicationState, QueueReference,
    ReconciliationStatus, fresh_id, validate_id, validate_token,
};
use super::super::{BootClock, BootDeadline};
use super::layout::{
    QuarantineKind, admission_lock, bump_revision, command_root, control_path, controls_dir, flock,
    gc_dir, is_atomic_temp, lock_until, open_lock, prepare_layout, quarantine_entry, queue_dir,
    read_control, read_dir_bounded, read_record, unlock, write_control,
};
use super::recovery::{parse_queue_name, recover_previous_generation, validate_reference};
use super::staging::recover_staging;
use super::{
    ClaimedCommand, CommandOwner, MAX_COMMAND_CONTROLS, MAX_COMMAND_QUEUE_REFERENCES,
    MAX_DIRECTORY_ENTRIES_PER_SCAN,
};

impl CommandOwner {
    pub(crate) fn open(daemon_token: &str) -> Result<Self> {
        validate_token(daemon_token)?;
        let root = command_root();
        prepare_layout(&root)?;
        recover_staging(&root)?;
        recover_previous_generation(&root, daemon_token)?;
        Ok(Self {
            root,
            daemon_token: daemon_token.to_owned(),
        })
    }

    pub(crate) fn queue_path(&self) -> PathBuf {
        queue_dir(&self.root)
    }

    pub(crate) fn next_maintenance_deadline(&self) -> Result<Option<BootDeadline>> {
        let now = BootClock::now()?;
        let retry = now.checked_add(Duration::from_millis(50))?;
        let mut next = None;
        for entry in read_dir_bounded(&controls_dir(&self.root), MAX_DIRECTORY_ENTRIES_PER_SCAN)? {
            let control = read_control(&entry.path())?;
            let candidate = if control.response.is_some() {
                Some(retry)
            } else if matches!(control.publication_state, PublicationState::VisibleUnqueued) {
                let authorization = BootDeadline::from_nanos(
                    control.submission_clock.authorization_deadline_boottime_ns,
                );
                Some(if authorization > now {
                    authorization
                } else {
                    retry
                })
            } else {
                None
            };
            if let Some(candidate) = candidate {
                next = Some(next.map_or(candidate, |current: BootDeadline| current.min(candidate)));
            }
        }
        Ok(next)
    }

    pub(crate) fn claim_next(&self) -> Result<Option<ClaimedCommand>> {
        let mut references = BTreeMap::new();
        let mut duplicate_orders = BTreeSet::new();
        for entry in read_dir_bounded(&queue_dir(&self.root), MAX_DIRECTORY_ENTRIES_PER_SCAN)? {
            let name = match entry.file_name().into_string() {
                Ok(name) => name,
                Err(_) => {
                    quarantine_entry(&self.root, &entry.path(), QuarantineKind::Queue)?;
                    continue;
                }
            };
            if is_atomic_temp(&name, "") || name.starts_with('.') {
                continue;
            }
            let (order, identity) = match parse_queue_name(&name) {
                Ok(parsed) => parsed,
                Err(_) => {
                    quarantine_entry(&self.root, &entry.path(), QuarantineKind::Queue)?;
                    continue;
                }
            };
            let reference: QueueReference =
                match read_record(&entry.path(), MAX_QUEUE_REFERENCE_BYTES) {
                    Ok(reference) => reference,
                    Err(_) => {
                        quarantine_entry(&self.root, &entry.path(), QuarantineKind::Queue)?;
                        continue;
                    }
                };
            if validate_reference(&reference, order, &identity, &self.daemon_token).is_err() {
                if reference.command_identity == identity {
                    self.reject_queue_integrity_failure(
                        &reference,
                        "command queue filename and payload disagree",
                    )?;
                }
                quarantine_entry(&self.root, &entry.path(), QuarantineKind::Queue)?;
                continue;
            }
            if duplicate_orders.contains(&order) {
                self.reject_queue_integrity_failure(&reference, "duplicate command queue order")?;
                quarantine_entry(&self.root, &entry.path(), QuarantineKind::Queue)?;
                continue;
            }
            let current_reference = reference.clone();
            if let Some((_identity, previous_path, previous_reference)) =
                references.insert(order, (identity, entry.path(), reference))
            {
                references.remove(&order);
                duplicate_orders.insert(order);
                self.reject_queue_integrity_failure(
                    &previous_reference,
                    "duplicate command queue order",
                )?;
                self.reject_queue_integrity_failure(
                    &current_reference,
                    "duplicate command queue order",
                )?;
                quarantine_entry(&self.root, &previous_path, QuarantineKind::Queue)?;
                quarantine_entry(&self.root, &entry.path(), QuarantineKind::Queue)?;
            }
        }
        let referenced_controls = references
            .iter()
            .map(|(order, (identity, _, _))| (*order, identity.clone()))
            .collect::<BTreeSet<_>>();
        let reference = references.into_iter().next();
        let claimed = self.earliest_ref_less_control(&referenced_controls)?;
        let use_claimed = match (&reference, &claimed) {
            (None, Some(_)) => true,
            (Some(_), None) | (None, None) => false,
            (
                Some((reference_order, (reference_identity, _, _))),
                Some((claim_order, claim_identity, _)),
            ) => {
                if reference_order == claim_order && reference_identity != claim_identity {
                    bail!("duplicate v2 command order names different identities");
                }
                claim_order < reference_order
            }
        };
        if use_claimed {
            let (_order, identity, path) = claimed.expect("selected claimed command exists");
            return self.claim_ref_less_control(identity, path);
        }
        let Some((_order, (identity, reference_path, reference))) = reference else {
            return Ok(None);
        };
        let path = control_path(&self.root, &identity);
        let decision_lock = match open_lock(&path.join("decision.lock"), false) {
            Ok(lock) => lock,
            Err(error)
                if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|source| source.kind() == ErrorKind::NotFound) =>
            {
                quarantine_entry(&self.root, &reference_path, QuarantineKind::Queue)?;
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let retry_deadline = BootClock::now()?.checked_add(Duration::from_millis(200))?;
        lock_until(&decision_lock, libc::LOCK_EX, retry_deadline)?;
        let mut control = match read_control(&path) {
            Ok(control) => control,
            Err(_) => {
                unlock(&decision_lock)?;
                quarantine_entry(&self.root, &reference_path, QuarantineKind::Queue)?;
                quarantine_entry(&self.root, &path, QuarantineKind::Control)?;
                return Ok(None);
            }
        };
        if control.identity != reference.command_identity
            || control.queue_order != reference.queue_order
            || control.target_daemon_token != reference.target_daemon_token
        {
            unlock(&decision_lock)?;
            quarantine_entry(&self.root, &reference_path, QuarantineKind::Queue)?;
            quarantine_entry(&self.root, &path, QuarantineKind::Control)?;
            return Ok(None);
        }
        match &control.decision {
            CommandDecision::Open => {
                bump_revision(&mut control)?;
                control.publication_state = PublicationState::Claimed {
                    daemon_instance_token: self.daemon_token.clone(),
                    claim_generation: fresh_id()?,
                };
                write_control(&path, &control)?;
                fs::remove_file(&reference_path).with_context(|| {
                    format!(
                        "failed to remove claimed queue reference {}",
                        reference_path.display()
                    )
                })?;
                Ok(Some(ClaimedCommand {
                    control_path: path,
                    decision_lock,
                    control,
                }))
            }
            CommandDecision::Canceled | CommandDecision::Rejected { .. } => {
                fs::remove_file(&reference_path)?;
                unlock(&decision_lock)?;
                self.collect_terminal()?;
                Ok(None)
            }
            CommandDecision::Committed { .. } => {
                fs::remove_file(&reference_path)?;
                Ok(Some(ClaimedCommand {
                    control_path: path,
                    decision_lock,
                    control,
                }))
            }
        }
    }

    fn earliest_ref_less_control(
        &self,
        referenced_controls: &BTreeSet<(u64, String)>,
    ) -> Result<Option<(u64, String, PathBuf)>> {
        let mut candidates = Vec::new();
        for entry in read_dir_bounded(&controls_dir(&self.root), MAX_DIRECTORY_ENTRIES_PER_SCAN)? {
            let identity = match entry.file_name().into_string() {
                Ok(identity) if validate_id(&identity).is_ok() => identity,
                _ => {
                    quarantine_entry(&self.root, &entry.path(), QuarantineKind::Control)?;
                    continue;
                }
            };
            let path = entry.path();
            let control = match read_control(&path) {
                Ok(control) if control.identity == identity => control,
                Ok(_) | Err(_) => {
                    quarantine_entry(&self.root, &path, QuarantineKind::Control)?;
                    continue;
                }
            };
            if matches!(control.publication_state, PublicationState::VisibleUnqueued)
                && !referenced_controls.contains(&(control.queue_order, control.identity.clone()))
            {
                self.recover_abandoned_unqueued(&path, &control)?;
                continue;
            }
            let recoverable = matches!(control.decision, CommandDecision::Open)
                || matches!(
                    (&control.decision, &control.action_status),
                    (
                        CommandDecision::Committed {
                            effect_status: EffectStatus::Authorized,
                            ..
                        },
                        ActionStatus::None
                    )
                );
            let ref_less_queued = matches!(control.publication_state, PublicationState::Queued)
                && !referenced_controls.contains(&(control.queue_order, control.identity.clone()));
            if (matches!(control.publication_state, PublicationState::Claimed { .. })
                || ref_less_queued)
                && control.response.is_none()
                && recoverable
            {
                candidates.push((control.queue_order, identity, path));
            }
        }
        candidates.sort_by_key(|candidate| candidate.0);
        Ok(candidates.into_iter().next())
    }

    fn reject_queue_integrity_failure(
        &self,
        reference: &QueueReference,
        reason: &str,
    ) -> Result<()> {
        let path = control_path(&self.root, &reference.command_identity);
        let decision = match open_lock(&path.join("decision.lock"), false) {
            Ok(decision) => decision,
            Err(error)
                if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|source| source.kind() == ErrorKind::NotFound) =>
            {
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        let deadline = BootClock::now()?.checked_add(Duration::from_millis(200))?;
        lock_until(&decision, libc::LOCK_EX, deadline)?;
        let mut control = read_control(&path)?;
        if control.identity == reference.command_identity
            && control.queue_order == reference.queue_order
            && control.target_daemon_token == reference.target_daemon_token
            && matches!(control.decision, CommandDecision::Open)
        {
            bump_revision(&mut control)?;
            control.decision = CommandDecision::Rejected {
                reason: reason.to_owned(),
            };
            control.response = Some(CommandResponse::FailedNoEffect {
                reason: reason.to_owned(),
            });
            write_control(&path, &control)?;
        }
        unlock(&decision)
    }

    fn recover_abandoned_unqueued(
        &self,
        path: &std::path::Path,
        observed: &super::super::wire::CommandControl,
    ) -> Result<()> {
        let caller_lease = open_lock(&path.join("caller.lease"), false)?;
        match flock(&caller_lease, libc::LOCK_EX | libc::LOCK_NB) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error).context("failed to inspect unqueued caller lease"),
        }
        let decision = open_lock(&path.join("decision.lock"), false)?;
        match flock(&decision, libc::LOCK_EX | libc::LOCK_NB) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                unlock(&caller_lease)?;
                return Ok(());
            }
            Err(error) => {
                unlock(&caller_lease)?;
                return Err(error).context("failed to inspect unqueued command decision");
            }
        }
        let mut control = read_control(path)?;
        if &control == observed
            && matches!(control.publication_state, PublicationState::VisibleUnqueued)
            && matches!(control.decision, CommandDecision::Open)
            && control.response.is_none()
        {
            let reason = "caller ended before command queue publication".to_owned();
            bump_revision(&mut control)?;
            control.decision = CommandDecision::Rejected {
                reason: reason.clone(),
            };
            control.response = Some(CommandResponse::FailedNoEffect { reason });
            control.caller_disposition = CallerDisposition::Abandoned;
            write_control(path, &control)?;
        }
        unlock(&decision)?;
        unlock(&caller_lease)
    }

    fn claim_ref_less_control(
        &self,
        identity: String,
        path: PathBuf,
    ) -> Result<Option<ClaimedCommand>> {
        if !matches!(
            fs::symlink_metadata(&path),
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink()
        ) {
            bail!("ref-less command control is not a no-follow directory");
        }
        let decision_lock = open_lock(&path.join("decision.lock"), false)?;
        let deadline = BootClock::now()?.checked_add(Duration::from_millis(200))?;
        lock_until(&decision_lock, libc::LOCK_EX, deadline)?;
        let mut control = read_control(&path)?;
        let recoverable = matches!(control.decision, CommandDecision::Open)
            || matches!(
                (&control.decision, &control.action_status),
                (
                    CommandDecision::Committed {
                        effect_status: EffectStatus::Authorized,
                        ..
                    },
                    ActionStatus::None
                )
            );
        if control.identity != identity
            || !matches!(
                control.publication_state,
                PublicationState::Queued | PublicationState::Claimed { .. }
            )
            || control.response.is_some()
            || !recoverable
        {
            unlock(&decision_lock)?;
            return Ok(None);
        }
        if matches!(control.publication_state, PublicationState::Queued) {
            bump_revision(&mut control)?;
            control.publication_state = PublicationState::Claimed {
                daemon_instance_token: self.daemon_token.clone(),
                claim_generation: fresh_id()?,
            };
            write_control(&path, &control)?;
        }
        Ok(Some(ClaimedCommand {
            control_path: path,
            decision_lock,
            control,
        }))
    }

    pub(crate) fn collect_terminal(&self) -> Result<usize> {
        let deadline = BootClock::now()?.checked_add(Duration::from_millis(200))?;
        let admission = admission_lock(&self.root, deadline)?;
        let mut collected = 0;
        for entry in read_dir_bounded(&controls_dir(&self.root), MAX_DIRECTORY_ENTRIES_PER_SCAN)? {
            let path = entry.path();
            let lease = open_lock(&path.join("caller.lease"), false)?;
            if flock(&lease, libc::LOCK_EX | libc::LOCK_NB).is_err() {
                continue;
            }
            let decision = open_lock(&path.join("decision.lock"), false)?;
            if flock(&decision, libc::LOCK_EX | libc::LOCK_NB).is_err() {
                unlock(&lease)?;
                continue;
            }
            let mut control = read_control(&path)?;
            if control.response.is_none()
                || !matches!(control.reconciliation_status, ReconciliationStatus::None)
                || !matches!(control.child_report_status, ChildReportStatus::None)
            {
                unlock(&decision)?;
                unlock(&lease)?;
                continue;
            }
            if matches!(control.caller_disposition, CallerDisposition::Waiting) {
                bump_revision(&mut control)?;
                control.caller_disposition = CallerDisposition::Abandoned;
                write_control(&path, &control)?;
            }
            if !matches!(
                control.caller_disposition,
                CallerDisposition::Acknowledged { .. } | CallerDisposition::Abandoned
            ) {
                unlock(&decision)?;
                unlock(&lease)?;
                continue;
            }
            let gc_identity = fresh_id()?;
            let tombstone = gc_dir(&self.root).join(format!("{}-{gc_identity}", control.identity));
            fs::rename(&path, &tombstone)?;
            fs::remove_dir_all(&tombstone)?;
            collected += 1;
        }
        unlock(&admission)?;
        Ok(collected)
    }

    pub(crate) fn assert_rollback_quiescent(&self) -> Result<()> {
        if !read_dir_bounded(&queue_dir(&self.root), MAX_COMMAND_QUEUE_REFERENCES + 1)?.is_empty() {
            bail!("v2 command queue is not quiescent for rollback");
        }
        let children = self.root.join("children");
        match fs::read_dir(&children) {
            Ok(entries) => {
                if entries.take(1).count() != 0 {
                    bail!("overlay child proof remains unresolved for rollback");
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("failed to inspect v2 child proofs"),
        }
        for entry in read_dir_bounded(&controls_dir(&self.root), MAX_COMMAND_CONTROLS + 1)? {
            let control = read_control(&entry.path())?;
            if matches!(control.decision, CommandDecision::Open)
                || matches!(
                    control.decision,
                    CommandDecision::Committed {
                        effect_status: EffectStatus::Authorized,
                        ..
                    }
                )
                || control.response.is_none()
                || !matches!(control.reconciliation_status, ReconciliationStatus::None)
                || !matches!(control.child_report_status, ChildReportStatus::None)
            {
                bail!("v2 command state is not quiescent for rollback");
            }
        }
        Ok(())
    }
}
