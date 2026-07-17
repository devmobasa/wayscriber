use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use super::{BootIdentity, NamespaceIdentity, ProtocolId, ProtocolToken};
use crate::daemon::control::DaemonToggleRequest;
use crate::tray_action::TrayAction;

pub(crate) const DAEMON_COMMAND_PROTOCOL_VERSION: u16 = 2;
pub(crate) const ACTION_ENVELOPE_PROTOCOL_VERSION: u16 = 2;
pub(crate) const DAEMON_CHILD_PROTOCOL_VERSION: u16 = 2;

pub(crate) const MAX_RUNTIME_RECORD_BYTES: usize = 4 * 1024;
pub(crate) const MAX_ADMISSION_RECORD_BYTES: usize = 1024;
pub(crate) const MAX_QUEUE_REFERENCE_BYTES: usize = 1024;
pub(crate) const MAX_CONTROL_RECORD_BYTES: usize = 64 * 1024;
pub(crate) const MAX_ACTION_ENVELOPE_BYTES: usize = 4 * 1024;
pub(crate) const MAX_REASON_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DaemonRuntimeRecordV2 {
    pub(crate) pid: u32,
    pub(crate) runtime_record_version: u16,
    pub(crate) typed_control_protocol_version: u16,
    pub(crate) boot_id: String,
    pub(crate) time_namespace: NamespaceIdentityV2,
    pub(crate) pid_namespace: NamespaceIdentityV2,
    pub(crate) process_start_ticks: u64,
    pub(crate) v2_instance_token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NamespaceIdentityV2 {
    pub(crate) dev: u64,
    pub(crate) ino: u64,
}

impl From<NamespaceIdentity> for NamespaceIdentityV2 {
    fn from(value: NamespaceIdentity) -> Self {
        Self {
            dev: value.dev,
            ino: value.ino,
        }
    }
}

impl DaemonRuntimeRecordV2 {
    pub(crate) fn current(token: ProtocolToken) -> Result<Self> {
        Ok(Self {
            pid: std::process::id(),
            runtime_record_version: DAEMON_COMMAND_PROTOCOL_VERSION,
            typed_control_protocol_version: DAEMON_COMMAND_PROTOCOL_VERSION,
            boot_id: BootIdentity::read()?.as_str().to_owned(),
            time_namespace: NamespaceIdentity::current_time()?.into(),
            pid_namespace: NamespaceIdentity::current_pid()?.into(),
            process_start_ticks: super::linux::current_process_start_ticks()?,
            v2_instance_token: token.to_string(),
        })
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.runtime_record_version != DAEMON_COMMAND_PROTOCOL_VERSION
            || self.typed_control_protocol_version != DAEMON_COMMAND_PROTOCOL_VERSION
        {
            bail!("unsupported daemon v2 protocol version");
        }
        validate_boot_id(&self.boot_id)?;
        validate_token(&self.v2_instance_token)?;
        if self.pid == 0 || self.process_start_ticks == 0 {
            bail!("daemon runtime identity contains a zero value");
        }
        if self.boot_id != BootIdentity::read()?.as_str()
            || self.time_namespace != NamespaceIdentity::current_time()?.into()
            || self.pid_namespace != NamespaceIdentity::current_pid()?.into()
        {
            bail!("daemon runtime identity belongs to a different boot or namespace");
        }
        if super::linux::process_start_ticks(self.pid)? != self.process_start_ticks {
            bail!("daemon runtime pid was reused");
        }
        let _live_pidfd = super::linux::open_pidfd(self.pid)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DaemonRequestV2 {
    pub(crate) mode: Option<String>,
    pub(crate) freeze: bool,
    pub(crate) exit_after_capture: bool,
    pub(crate) no_exit_after_capture: bool,
    pub(crate) resume_session: bool,
    pub(crate) no_resume_session: bool,
    pub(crate) session_file: Option<PathBuf>,
    pub(crate) overlay_action: Option<TrayAction>,
}

impl From<&DaemonToggleRequest> for DaemonRequestV2 {
    fn from(value: &DaemonToggleRequest) -> Self {
        Self {
            mode: value.mode.clone(),
            freeze: value.freeze,
            exit_after_capture: value.exit_after_capture,
            no_exit_after_capture: value.no_exit_after_capture,
            resume_session: value.resume_session,
            no_resume_session: value.no_resume_session,
            session_file: value.session_file.clone(),
            overlay_action: value.overlay_action,
        }
    }
}

impl From<DaemonRequestV2> for DaemonToggleRequest {
    fn from(value: DaemonRequestV2) -> Self {
        Self {
            mode: value.mode,
            freeze: value.freeze,
            exit_after_capture: value.exit_after_capture,
            no_exit_after_capture: value.no_exit_after_capture,
            resume_session: value.resume_session,
            no_resume_session: value.no_resume_session,
            session_file: value.session_file,
            overlay_action: value.overlay_action,
        }
    }
}

impl DaemonRequestV2 {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.exit_after_capture && self.no_exit_after_capture {
            bail!("conflicting capture exit modes");
        }
        if self.resume_session && self.no_resume_session {
            bail!("conflicting session resume modes");
        }
        if self.no_resume_session && self.session_file.is_some() {
            bail!("named session conflicts with disabled session resume");
        }
        if self.mode.as_ref().is_some_and(|mode| mode.len() > 256) {
            bail!("mode exceeds 256 bytes");
        }
        if let Some(path) = &self.session_file {
            let path = path
                .to_str()
                .ok_or_else(|| anyhow!("session path is not valid UTF-8"))?;
            if path.len() > 4096 || !PathBuf::from(path).is_absolute() {
                bail!("session path is not a bounded absolute path");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SubmissionClock {
    pub(crate) boot_id: String,
    pub(crate) time_namespace: NamespaceIdentityV2,
    pub(crate) requested_boottime_ns: u64,
    pub(crate) authorization_deadline_boottime_ns: u64,
    pub(crate) response_deadline_boottime_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CallerIdentity {
    pub(crate) pid: u32,
    pub(crate) process_start_ticks: u64,
    pub(crate) nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", deny_unknown_fields)]
pub(crate) enum PublicationState {
    VisibleUnqueued,
    Queued,
    Claimed {
        daemon_instance_token: String,
        claim_generation: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EffectKind {
    NoOp,
    HideReady,
    DeliverReadyAction,
    StartAndShow,
    StartAndDeliverAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "snake_case",
    tag = "status",
    content = "reason",
    deny_unknown_fields
)]
pub(crate) enum EffectStatus {
    Authorized,
    Completed,
    FailedNoEffect(String),
    Indeterminate(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeliveryOwner {
    pub(crate) daemon_instance_token: String,
    pub(crate) recovery_generation: String,
    pub(crate) child_generation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "decision", deny_unknown_fields)]
pub(crate) enum CommandDecision {
    Open,
    Canceled,
    Rejected {
        reason: String,
    },
    Committed {
        effect_id: String,
        effect_kind: EffectKind,
        effect_status: EffectStatus,
        delivery_owner: Option<DeliveryOwner>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status", deny_unknown_fields)]
pub(crate) enum ActionStatus {
    None,
    Prepared {
        action_id: String,
        action_order: u64,
        digest: String,
    },
    Eligible {
        action_id: String,
        action_order: u64,
        digest: String,
    },
    Claimed {
        action_id: String,
        action_order: u64,
        digest: String,
    },
    Applied {
        action_id: String,
        action_order: u64,
        digest: String,
    },
    Abandoned {
        action_id: String,
        action_order: u64,
        digest: String,
        reason: String,
    },
    Indeterminate {
        action_id: String,
        action_order: u64,
        digest: String,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReconciliationCursor {
    pub(crate) highest_required_revision: u64,
    pub(crate) highest_applied_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status", deny_unknown_fields)]
pub(crate) enum ReconciliationStatus {
    None,
    Pending {
        reconciliation_id: String,
        target: ReconciliationTarget,
        notification_kind: NotificationKind,
        effect_id: Option<String>,
        action_id: Option<String>,
        opened_required_revision: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "target", deny_unknown_fields)]
pub(crate) enum ReconciliationTarget {
    Child {
        daemon_token: String,
        recovery_generation: String,
        child_generation: String,
        channel_nonce: String,
    },
    DaemonCleanup {
        daemon_token: String,
        recovery_generation: String,
        cleanup_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NotificationKind {
    Record,
    State,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status", deny_unknown_fields)]
pub(crate) enum ChildReportStatus {
    None,
    Pending {
        report_id: String,
        source_revision: u64,
        report_delivery_deadline_boottime_ns: u64,
        recovery_deadline_boottime_ns: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "response", deny_unknown_fields)]
pub(crate) enum CommandResponse {
    Succeeded { effect_id: String },
    Canceled,
    FailedNoEffect { reason: String },
    CommittedIndeterminate { effect_id: String, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "disposition", deny_unknown_fields)]
pub(crate) enum CallerDisposition {
    Waiting,
    Acknowledged { response_digest: String },
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommandControl {
    pub(crate) protocol_version: u16,
    pub(crate) record_revision: u64,
    pub(crate) identity: String,
    pub(crate) target_daemon_token: String,
    pub(crate) caller_identity: CallerIdentity,
    pub(crate) submission_clock: SubmissionClock,
    pub(crate) queue_order: u64,
    pub(crate) request: DaemonRequestV2,
    pub(crate) publication_state: PublicationState,
    pub(crate) decision: CommandDecision,
    pub(crate) action_status: ActionStatus,
    pub(crate) reconciliation_cursor: ReconciliationCursor,
    pub(crate) reconciliation_status: ReconciliationStatus,
    pub(crate) child_report_status: ChildReportStatus,
    pub(crate) response: Option<CommandResponse>,
    pub(crate) caller_disposition: CallerDisposition,
}

impl CommandControl {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.protocol_version != DAEMON_COMMAND_PROTOCOL_VERSION
            || self.record_revision == 0
            || self.queue_order == 0
            || self.caller_identity.pid == 0
            || self.caller_identity.process_start_ticks == 0
        {
            bail!("invalid command protocol version or revision");
        }
        validate_id(&self.identity)?;
        validate_token(&self.target_daemon_token)?;
        validate_id(&self.caller_identity.nonce)?;
        validate_boot_id(&self.submission_clock.boot_id)?;
        validate_namespace(&self.submission_clock.time_namespace)?;
        if self.submission_clock.requested_boottime_ns == 0
            || self.submission_clock.requested_boottime_ns
                > self.submission_clock.authorization_deadline_boottime_ns
            || self.submission_clock.authorization_deadline_boottime_ns
                > self.submission_clock.response_deadline_boottime_ns
        {
            bail!("invalid command submission deadlines");
        }
        self.request.validate()?;
        validate_publication_state(&self.publication_state)?;
        validate_action_status(&self.action_status)?;
        validate_decision_and_response(self)?;
        validate_reconciliation(self)?;
        validate_child_report(&self.child_report_status, self.record_revision)?;
        if self.reconciliation_cursor.highest_applied_revision
            > self.reconciliation_cursor.highest_required_revision
            || self.reconciliation_cursor.highest_applied_revision == 0
            || self.reconciliation_cursor.highest_required_revision > self.record_revision
        {
            bail!("invalid reconciliation revision cursor");
        }
        match &self.caller_disposition {
            CallerDisposition::Waiting | CallerDisposition::Abandoned => {}
            CallerDisposition::Acknowledged {
                response_digest: digest,
            } => {
                validate_digest(digest)?;
                let response = self
                    .response
                    .as_ref()
                    .ok_or_else(|| anyhow!("acknowledged command has no response"))?;
                if response_digest(response)? != *digest {
                    bail!("command acknowledgement digest does not match its response");
                }
            }
        }
        validate_reason_fields(self)?;
        Ok(())
    }
}

fn validate_namespace(identity: &NamespaceIdentityV2) -> Result<()> {
    if identity.dev == 0 || identity.ino == 0 {
        bail!("protocol namespace identity contains a zero value");
    }
    Ok(())
}

fn validate_publication_state(state: &PublicationState) -> Result<()> {
    if let PublicationState::Claimed {
        daemon_instance_token,
        claim_generation,
    } = state
    {
        validate_token(daemon_instance_token)?;
        validate_id(claim_generation)?;
    }
    Ok(())
}

fn validate_action_identity(action_id: &str, action_order: u64, digest: &str) -> Result<()> {
    validate_id(action_id)?;
    if action_order == 0 {
        bail!("action order is zero");
    }
    validate_digest(digest)
}

fn validate_action_status(status: &ActionStatus) -> Result<()> {
    match status {
        ActionStatus::None => Ok(()),
        ActionStatus::Prepared {
            action_id,
            action_order,
            digest,
        }
        | ActionStatus::Eligible {
            action_id,
            action_order,
            digest,
        }
        | ActionStatus::Claimed {
            action_id,
            action_order,
            digest,
        }
        | ActionStatus::Applied {
            action_id,
            action_order,
            digest,
        } => validate_action_identity(action_id, *action_order, digest),
        ActionStatus::Abandoned {
            action_id,
            action_order,
            digest,
            reason,
        }
        | ActionStatus::Indeterminate {
            action_id,
            action_order,
            digest,
            reason,
        } => {
            validate_action_identity(action_id, *action_order, digest)?;
            validate_reason(reason)
        }
    }
}

fn validate_delivery_owner(owner: &DeliveryOwner) -> Result<()> {
    validate_token(&owner.daemon_instance_token)?;
    validate_id(&owner.recovery_generation)?;
    if let Some(child_generation) = &owner.child_generation {
        validate_id(child_generation)?;
    }
    Ok(())
}

fn validate_decision_and_response(control: &CommandControl) -> Result<()> {
    let response_matches = match &control.decision {
        CommandDecision::Open => control.response.is_none(),
        CommandDecision::Canceled => matches!(control.response, Some(CommandResponse::Canceled)),
        CommandDecision::Rejected { reason } => matches!(
            &control.response,
            Some(CommandResponse::FailedNoEffect { reason: response_reason })
                if response_reason == reason
        ),
        CommandDecision::Committed {
            effect_id,
            effect_kind,
            effect_status,
            delivery_owner,
        } => {
            validate_id(effect_id)?;
            match (effect_kind, delivery_owner) {
                (EffectKind::NoOp, None) => {}
                (EffectKind::NoOp, Some(_)) | (_, None) => {
                    bail!("committed effect has invalid delivery ownership")
                }
                (_, Some(owner)) => validate_delivery_owner(owner)?,
            }
            match effect_status {
                EffectStatus::Authorized => {
                    if *effect_kind == EffectKind::NoOp {
                        bail!("no-op effects must commit atomically as completed");
                    }
                    control.response.is_none()
                }
                EffectStatus::Completed => matches!(
                    &control.response,
                    Some(CommandResponse::Succeeded { effect_id: response_id })
                        if response_id == effect_id
                ),
                EffectStatus::FailedNoEffect(reason) => matches!(
                    &control.response,
                    Some(CommandResponse::FailedNoEffect { reason: response_reason })
                        if response_reason == reason
                ),
                EffectStatus::Indeterminate(reason) => matches!(
                    &control.response,
                    Some(CommandResponse::CommittedIndeterminate {
                        effect_id: response_id,
                        reason: response_reason,
                    }) if response_id == effect_id && response_reason == reason
                ),
            }
        }
    };
    if !response_matches {
        bail!("command decision and response are inconsistent");
    }

    let has_action = !matches!(control.action_status, ActionStatus::None);
    if has_action && control.request.overlay_action.is_none() {
        bail!("command action state does not match its request");
    }
    if let CommandDecision::Committed {
        effect_kind,
        effect_status,
        ..
    } = &control.decision
    {
        let action_effect = matches!(
            effect_kind,
            EffectKind::DeliverReadyAction | EffectKind::StartAndDeliverAction
        );
        if action_effect != has_action {
            bail!("committed effect kind does not match its action state");
        }
        if matches!(effect_status, EffectStatus::Authorized)
            && matches!(
                control.action_status,
                ActionStatus::Applied { .. }
                    | ActionStatus::Abandoned { .. }
                    | ActionStatus::Indeterminate { .. }
            )
        {
            bail!("authorized action effect already has a terminal action state");
        }
        if !matches!(effect_status, EffectStatus::Authorized)
            && matches!(
                control.action_status,
                ActionStatus::Prepared { .. } | ActionStatus::Eligible { .. }
            )
        {
            bail!("terminal effect retains an unclaimed live action");
        }
    }
    Ok(())
}

fn validate_reconciliation(control: &CommandControl) -> Result<()> {
    let ReconciliationStatus::Pending {
        reconciliation_id,
        target,
        effect_id,
        action_id,
        opened_required_revision,
        ..
    } = &control.reconciliation_status
    else {
        return Ok(());
    };
    validate_id(reconciliation_id)?;
    if *opened_required_revision == 0
        || *opened_required_revision > control.reconciliation_cursor.highest_required_revision
    {
        bail!("invalid reconciliation opening revision");
    }
    match target {
        ReconciliationTarget::Child {
            daemon_token,
            recovery_generation,
            child_generation,
            channel_nonce,
        } => {
            validate_token(daemon_token)?;
            validate_id(recovery_generation)?;
            validate_id(child_generation)?;
            validate_id(channel_nonce)?;
        }
        ReconciliationTarget::DaemonCleanup {
            daemon_token,
            recovery_generation,
            cleanup_id,
        } => {
            validate_token(daemon_token)?;
            validate_id(recovery_generation)?;
            validate_id(cleanup_id)?;
        }
    }
    if let Some(effect_id) = effect_id {
        validate_id(effect_id)?;
    }
    if let Some(action_id) = action_id {
        validate_id(action_id)?;
    }
    Ok(())
}

fn validate_child_report(status: &ChildReportStatus, record_revision: u64) -> Result<()> {
    let ChildReportStatus::Pending {
        report_id,
        source_revision,
        report_delivery_deadline_boottime_ns,
        recovery_deadline_boottime_ns,
    } = status
    else {
        return Ok(());
    };
    validate_id(report_id)?;
    if *source_revision == 0
        || *source_revision > record_revision
        || *report_delivery_deadline_boottime_ns == 0
        || *report_delivery_deadline_boottime_ns >= *recovery_deadline_boottime_ns
    {
        bail!("invalid child report revisions or deadlines");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AdmissionRecord {
    pub(crate) protocol_version: u16,
    pub(crate) boot_id: String,
    pub(crate) time_namespace: NamespaceIdentityV2,
    pub(crate) last_order: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QueueReference {
    pub(crate) protocol_version: u16,
    pub(crate) queue_order: u64,
    pub(crate) command_identity: String,
    pub(crate) target_daemon_token: String,
}

pub(crate) fn canonical_json<T: Serialize>(value: &T, cap: usize) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value).context("failed to serialize v2 protocol record")?;
    if bytes.len() > cap {
        bail!("v2 protocol record exceeds {cap} bytes");
    }
    Ok(bytes)
}

pub(crate) fn parse_canonical_json<T: DeserializeOwned + Serialize>(
    bytes: &[u8],
    cap: usize,
) -> Result<T> {
    if bytes.len() > cap {
        bail!("v2 protocol record exceeds {cap} bytes");
    }
    let value = serde_json::from_slice(bytes).context("invalid v2 protocol JSON")?;
    if canonical_json(&value, cap)? != bytes {
        bail!("v2 protocol JSON is not in canonical encoding");
    }
    Ok(value)
}

pub(crate) fn response_digest(response: &CommandResponse) -> Result<String> {
    let canonical = canonical_json(response, MAX_CONTROL_RECORD_BYTES)?;
    let digest = Sha256::digest(canonical);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub(crate) fn validate_id(value: &str) -> Result<()> {
    validate_lower_hex(value, 32, "protocol identity")
}

pub(crate) fn validate_token(value: &str) -> Result<()> {
    validate_lower_hex(value, 64, "protocol token")
}

pub(crate) fn validate_digest(value: &str) -> Result<()> {
    validate_lower_hex(value, 64, "SHA-256 digest")
}

fn validate_lower_hex(value: &str, length: usize, label: &str) -> Result<()> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        bail!("{label} is not canonical lowercase hexadecimal");
    }
    Ok(())
}

fn validate_boot_id(value: &str) -> Result<()> {
    let bytes = value.as_bytes();
    if bytes.len() != 36
        || !bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'),
        })
    {
        bail!("boot identity is not a canonical lowercase UUID");
    }
    Ok(())
}

pub(crate) fn validate_reason(reason: &str) -> Result<()> {
    if reason.len() > MAX_REASON_BYTES {
        bail!("protocol reason exceeds {MAX_REASON_BYTES} bytes");
    }
    Ok(())
}

pub(crate) fn bounded_reason(reason: &str, max_bytes: usize) -> String {
    let mut end = reason.len().min(max_bytes);
    while !reason.is_char_boundary(end) {
        end -= 1;
    }
    reason[..end].to_owned()
}

fn validate_reason_fields(control: &CommandControl) -> Result<()> {
    match &control.decision {
        CommandDecision::Rejected { reason } => validate_reason(reason)?,
        CommandDecision::Committed { effect_status, .. } => match effect_status {
            EffectStatus::FailedNoEffect(reason) | EffectStatus::Indeterminate(reason) => {
                validate_reason(reason)?
            }
            EffectStatus::Authorized | EffectStatus::Completed => {}
        },
        CommandDecision::Open | CommandDecision::Canceled => {}
    }
    if let Some(response) = &control.response {
        match response {
            CommandResponse::FailedNoEffect { reason }
            | CommandResponse::CommittedIndeterminate { reason, .. } => validate_reason(reason)?,
            CommandResponse::Succeeded { .. } | CommandResponse::Canceled => {}
        }
    }
    Ok(())
}

pub(crate) fn fresh_id() -> Result<String> {
    Ok(ProtocolId::generate()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_record_rejects_unknown_and_noncanonical_input() {
        let token = ProtocolToken::generate().unwrap();
        let record = DaemonRuntimeRecordV2::current(token).unwrap();
        record.validate().unwrap();
        let canonical = canonical_json(&record, MAX_RUNTIME_RECORD_BYTES).unwrap();
        let parsed: DaemonRuntimeRecordV2 =
            parse_canonical_json(&canonical, MAX_RUNTIME_RECORD_BYTES).unwrap();
        assert_eq!(parsed, record);

        let spaced = format!(" {}", String::from_utf8(canonical.clone()).unwrap());
        assert!(
            parse_canonical_json::<DaemonRuntimeRecordV2>(
                spaced.as_bytes(),
                MAX_RUNTIME_RECORD_BYTES
            )
            .is_err()
        );

        let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        value["unknown"] = serde_json::json!(true);
        assert!(serde_json::from_value::<DaemonRuntimeRecordV2>(value).is_err());
    }

    #[test]
    fn response_digest_is_stable_and_sensitive() {
        let response = CommandResponse::FailedNoEffect {
            reason: "no effect".into(),
        };
        let first = response_digest(&response).unwrap();
        let second = response_digest(&response).unwrap();
        assert_eq!(first, second);
        validate_digest(&first).unwrap();
        assert_ne!(first, response_digest(&CommandResponse::Canceled).unwrap());
    }

    #[test]
    fn bounded_reason_limits_utf8_by_bytes_without_splitting_a_character() {
        let reason = "é".repeat(600);
        let bounded = bounded_reason(&reason, 1023);
        assert!(bounded.len() <= 1023);
        assert!(reason.starts_with(&bounded));
        assert_eq!(bounded.chars().count(), 511);
    }
}
