use std::fs;
use std::time::Duration;

use anyhow::{Context, Result, bail};

#[cfg(test)]
use anyhow::anyhow;

use super::super::wire::{
    ActionStatus, CallerDisposition, CallerIdentity, ChildReportStatus, CommandControl,
    CommandDecision, CommandResponse, DAEMON_COMMAND_PROTOCOL_VERSION, DaemonRequestV2,
    MAX_QUEUE_REFERENCE_BYTES, PublicationState, QueueReference, ReconciliationCursor,
    ReconciliationStatus, SubmissionClock, fresh_id, response_digest, validate_token,
};
use super::super::{BootClock, BootIdentity, NamespaceIdentity};
use super::layout::{
    admission_lock, allocate_order, bump_revision, command_root, control_path,
    create_private_directory, creating_dir, ensure_capacity, lock_until, open_lock, prepare_layout,
    queue_path, read_control, try_lock_until, unlock, validate_root_shape, write_control,
    write_record,
};
use super::{AUTHORIZATION_WINDOW, ClientCommand, RESPONSE_WINDOW, TerminalCommandResult};

impl ClientCommand {
    pub(crate) fn publish(request: &DaemonRequestV2, daemon_token: &str) -> Result<Self> {
        request.validate()?;
        validate_token(daemon_token)?;
        let root = command_root();
        prepare_layout(&root)?;
        let now = BootClock::now()?;
        let authorization_deadline = now.checked_add(AUTHORIZATION_WINDOW)?;
        let response_deadline = now.checked_add(RESPONSE_WINDOW)?;
        let admission = admission_lock(&root, authorization_deadline)?;
        validate_root_shape(&root)?;
        ensure_capacity(&root)?;

        let order = allocate_order(&root)?;
        let identity = fresh_id()?;
        let caller_nonce = fresh_id()?;
        let boot_id = BootIdentity::read()?.as_str().to_owned();
        let stage =
            creating_dir(&root).join(format!("{boot_id}-{:016x}-{identity}", now.as_nanos()));
        create_private_directory(&stage)?;
        let decision_lock = open_lock(&stage.join("decision.lock"), true)?;
        let caller_lease = open_lock(&stage.join("caller.lease"), true)?;
        lock_until(&caller_lease, libc::LOCK_SH, authorization_deadline)?;
        lock_until(&decision_lock, libc::LOCK_EX, authorization_deadline)?;

        let mut control = CommandControl {
            protocol_version: DAEMON_COMMAND_PROTOCOL_VERSION,
            record_revision: 1,
            identity: identity.clone(),
            target_daemon_token: daemon_token.to_owned(),
            caller_identity: CallerIdentity {
                pid: std::process::id(),
                process_start_ticks: super::super::linux::current_process_start_ticks()?,
                nonce: caller_nonce,
            },
            submission_clock: SubmissionClock {
                boot_id,
                time_namespace: NamespaceIdentity::current_time()?.into(),
                requested_boottime_ns: now.as_nanos(),
                authorization_deadline_boottime_ns: authorization_deadline.as_nanos(),
                response_deadline_boottime_ns: response_deadline.as_nanos(),
            },
            queue_order: order,
            request: request.clone(),
            publication_state: PublicationState::VisibleUnqueued,
            decision: CommandDecision::Open,
            action_status: ActionStatus::None,
            reconciliation_cursor: ReconciliationCursor {
                highest_required_revision: 1,
                highest_applied_revision: 1,
            },
            reconciliation_status: ReconciliationStatus::None,
            child_report_status: ChildReportStatus::None,
            response: None,
            caller_disposition: CallerDisposition::Waiting,
        };
        write_control(&stage, &control)?;
        let published_control = control_path(&root, &identity);
        fs::rename(&stage, &published_control).with_context(|| {
            format!(
                "failed to publish command control {}",
                published_control.display()
            )
        })?;

        let reference = QueueReference {
            protocol_version: DAEMON_COMMAND_PROTOCOL_VERSION,
            queue_order: order,
            command_identity: identity.clone(),
            target_daemon_token: daemon_token.to_owned(),
        };
        write_record(
            &queue_path(&root, order, &identity),
            &reference,
            MAX_QUEUE_REFERENCE_BYTES,
        )?;
        bump_revision(&mut control)?;
        control.publication_state = PublicationState::Queued;
        write_control(&published_control, &control)?;
        unlock(&decision_lock)?;
        unlock(&admission)?;

        Ok(Self {
            identity,
            control_path: published_control,
            decision_lock,
            caller_lease,
            response_deadline,
        })
    }

    pub(crate) fn wait(self) -> Result<TerminalCommandResult> {
        loop {
            let now = BootClock::now()?;
            let slice = now
                .checked_add(Duration::from_millis(20))?
                .min(self.response_deadline);
            if !try_lock_until(&self.decision_lock, libc::LOCK_EX, slice)? {
                if slice < self.response_deadline {
                    continue;
                }
                bail!("protocol lock deadline expired while waiting for command response");
            }
            let mut control = read_control(&self.control_path)?;
            if control.identity != self.identity {
                bail!("command control identity changed");
            }
            if let Some(response) = control.response.clone() {
                let result = terminal_result(&response);
                acknowledge(&mut control, response)?;
                write_control(&self.control_path, &control)?;
                unlock(&self.decision_lock)?;
                unlock(&self.caller_lease)?;
                return Ok(result);
            }
            if now >= self.response_deadline {
                let canceled = cancel_open(&mut control)?;
                if canceled {
                    write_control(&self.control_path, &control)?;
                    unlock(&self.decision_lock)?;
                    unlock(&self.caller_lease)?;
                    return Ok(TerminalCommandResult::Canceled);
                }
                bump_revision(&mut control)?;
                control.caller_disposition = CallerDisposition::Abandoned;
                write_control(&self.control_path, &control)?;
                unlock(&self.decision_lock)?;
                unlock(&self.caller_lease)?;
                return Ok(TerminalCommandResult::CommittedIndeterminate(
                    "response deadline expired after the effect was durably committed".into(),
                ));
            }
            unlock(&self.decision_lock)?;
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[cfg(test)]
    pub(crate) fn cancel(self) -> Result<TerminalCommandResult> {
        lock_until(&self.decision_lock, libc::LOCK_EX, self.response_deadline)?;
        let mut control = read_control(&self.control_path)?;
        if cancel_open(&mut control)? {
            write_control(&self.control_path, &control)?;
            unlock(&self.decision_lock)?;
            unlock(&self.caller_lease)?;
            return Ok(TerminalCommandResult::Canceled);
        }
        let response = control
            .response
            .clone()
            .ok_or_else(|| anyhow!("cancellation lost to a nonterminal committed effect"))?;
        let result = terminal_result(&response);
        acknowledge(&mut control, response)?;
        write_control(&self.control_path, &control)?;
        unlock(&self.decision_lock)?;
        unlock(&self.caller_lease)?;
        Ok(result)
    }
}

fn cancel_open(control: &mut CommandControl) -> Result<bool> {
    if !matches!(control.decision, CommandDecision::Open) {
        return Ok(false);
    }
    bump_revision(control)?;
    control.decision = CommandDecision::Canceled;
    control.response = Some(CommandResponse::Canceled);
    let digest = response_digest(control.response.as_ref().unwrap())?;
    control.caller_disposition = CallerDisposition::Acknowledged {
        response_digest: digest,
    };
    Ok(true)
}

fn acknowledge(control: &mut CommandControl, response: CommandResponse) -> Result<()> {
    if !matches!(control.caller_disposition, CallerDisposition::Waiting) {
        bail!("command response was already acknowledged");
    }
    bump_revision(control)?;
    control.caller_disposition = CallerDisposition::Acknowledged {
        response_digest: response_digest(&response)?,
    };
    Ok(())
}

fn terminal_result(response: &CommandResponse) -> TerminalCommandResult {
    match response {
        CommandResponse::Succeeded { .. } => TerminalCommandResult::Succeeded,
        CommandResponse::Canceled => TerminalCommandResult::Canceled,
        CommandResponse::FailedNoEffect { reason } => {
            TerminalCommandResult::FailedNoEffect(reason.clone())
        }
        CommandResponse::CommittedIndeterminate { reason, .. } => {
            TerminalCommandResult::CommittedIndeterminate(reason.clone())
        }
    }
}
