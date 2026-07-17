use std::time::Duration;

use anyhow::{Result, anyhow, bail};

#[cfg(test)]
use super::super::wire::CommandControl;

use super::super::action::{ActionJournal, PreparedAction};
use super::super::wire::{
    ActionStatus, CommandDecision, CommandResponse, DaemonRequestV2, DeliveryOwner, EffectKind,
    EffectStatus, MAX_REASON_BYTES, bounded_reason, fresh_id,
};
use super::super::{BootClock, BootDeadline};
use super::layout::{bump_revision, flock, lock_until, read_control, unlock, write_control};
use super::{ClaimedCommand, FinalEffect};

impl ClaimedCommand {
    pub(crate) fn identity(&self) -> &str {
        &self.control.identity
    }

    pub(crate) fn request(&self) -> DaemonRequestV2 {
        self.control.request.clone()
    }

    pub(crate) fn is_open(&self) -> bool {
        matches!(self.control.decision, CommandDecision::Open)
    }

    pub(crate) fn authorized_effect(&self) -> Option<EffectKind> {
        match self.control.decision {
            CommandDecision::Committed {
                effect_kind,
                effect_status: EffectStatus::Authorized,
                ..
            } => Some(effect_kind),
            _ => None,
        }
    }

    pub(crate) fn prepare_action(
        &mut self,
        journal: &ActionJournal,
    ) -> Result<Option<PreparedAction>> {
        let action = self
            .control
            .request
            .overlay_action
            .ok_or_else(|| anyhow!("command has no overlay action"))?;
        if !matches!(self.control.decision, CommandDecision::Open)
            || !matches!(self.control.action_status, ActionStatus::None)
        {
            bail!("command is not eligible for action preparation");
        }
        let expected = self.control.clone();
        unlock(&self.decision_lock)?;
        let prepared =
            journal.prepare_command(&expected.identity, &expected.target_daemon_token, action)?;
        let deadline =
            BootDeadline::from_nanos(expected.submission_clock.authorization_deadline_boottime_ns);
        if let Err(error) = lock_until(&self.decision_lock, libc::LOCK_EX, deadline) {
            let _ = journal.abandon(&prepared, "command decision lock was not reacquired");
            return Err(error);
        }
        let current = read_control(&self.control_path)?;
        if current != expected {
            unlock(&self.decision_lock)?;
            let reason = "command was canceled during action preparation";
            journal.abandon(&prepared, reason)?;
            let recovery_deadline = BootClock::now()?.checked_add(Duration::from_millis(200))?;
            lock_until(&self.decision_lock, libc::LOCK_EX, recovery_deadline)?;
            let mut latest = read_control(&self.control_path)?;
            if latest != current
                || !matches!(
                    latest.decision,
                    CommandDecision::Canceled | CommandDecision::Rejected { .. }
                )
                || !matches!(latest.action_status, ActionStatus::None)
            {
                self.control = latest;
                bail!("command changed incompatibly during action preparation");
            }
            latest.action_status = ActionStatus::Abandoned {
                action_id: prepared.action_id.clone(),
                action_order: prepared.action_order,
                digest: prepared.digest.clone(),
                reason: reason.to_owned(),
            };
            bump_revision(&mut latest)?;
            write_control(&self.control_path, &latest)?;
            self.control = latest;
            return Ok(None);
        }
        self.control.action_status = ActionStatus::Prepared {
            action_id: prepared.action_id.clone(),
            action_order: prepared.action_order,
            digest: prepared.digest.clone(),
        };
        bump_revision(&mut self.control)?;
        write_control(&self.control_path, &self.control)?;
        Ok(Some(prepared))
    }

    pub(crate) fn commit(&mut self, effect_kind: EffectKind) -> Result<String> {
        if !matches!(self.control.decision, CommandDecision::Open) {
            bail!("command decision is no longer open");
        }
        if BootClock::now()?.as_nanos()
            > self
                .control
                .submission_clock
                .authorization_deadline_boottime_ns
        {
            return self.reject("command authorization deadline expired");
        }
        let effect_id = fresh_id()?;
        let recovery_generation = fresh_id()?;
        if let ActionStatus::Prepared {
            action_id,
            action_order,
            digest,
        } = &self.control.action_status
        {
            self.control.action_status = ActionStatus::Eligible {
                action_id: action_id.clone(),
                action_order: *action_order,
                digest: digest.clone(),
            };
        }
        bump_revision(&mut self.control)?;
        let effect_status = if effect_kind == EffectKind::NoOp {
            EffectStatus::Completed
        } else {
            EffectStatus::Authorized
        };
        self.control.decision = CommandDecision::Committed {
            effect_id: effect_id.clone(),
            effect_kind,
            effect_status,
            delivery_owner: (effect_kind != EffectKind::NoOp).then(|| DeliveryOwner {
                daemon_instance_token: self.control.target_daemon_token.clone(),
                recovery_generation,
                child_generation: None,
            }),
        };
        if effect_kind == EffectKind::NoOp {
            self.control.response = Some(CommandResponse::Succeeded {
                effect_id: effect_id.clone(),
            });
        }
        write_control(&self.control_path, &self.control)?;
        Ok(effect_id)
    }

    pub(crate) fn reject(&mut self, reason: &str) -> Result<String> {
        if !matches!(self.control.decision, CommandDecision::Open) {
            bail!("only an open command can be rejected");
        }
        let reason = bounded_reason(reason, MAX_REASON_BYTES);
        bump_revision(&mut self.control)?;
        self.control.decision = CommandDecision::Rejected {
            reason: reason.clone(),
        };
        self.control.response = Some(CommandResponse::FailedNoEffect { reason });
        write_control(&self.control_path, &self.control)?;
        Ok(String::new())
    }

    pub(crate) fn finalize(mut self, result: FinalEffect, reason: Option<&str>) -> Result<()> {
        let effect_id = match &self.control.decision {
            CommandDecision::Committed {
                effect_id,
                effect_status: EffectStatus::Authorized,
                ..
            } => effect_id.clone(),
            _ => bail!("only an authorized committed effect can be finalized"),
        };
        let (status, response) = match result {
            FinalEffect::Completed => (
                EffectStatus::Completed,
                CommandResponse::Succeeded {
                    effect_id: effect_id.clone(),
                },
            ),
            FinalEffect::Indeterminate => {
                let reason = bounded_reason(
                    reason.unwrap_or("effect outcome is indeterminate"),
                    MAX_REASON_BYTES,
                );
                (
                    EffectStatus::Indeterminate(reason.clone()),
                    CommandResponse::CommittedIndeterminate {
                        effect_id: effect_id.clone(),
                        reason,
                    },
                )
            }
        };
        if let CommandDecision::Committed { effect_status, .. } = &mut self.control.decision {
            *effect_status = status;
        }
        bump_revision(&mut self.control)?;
        self.control.response = Some(response);
        write_control(&self.control_path, &self.control)?;
        unlock(&self.decision_lock)
    }

    pub(crate) fn defer(self) -> Result<()> {
        unlock(&self.decision_lock)
    }

    #[cfg(test)]
    pub(crate) fn control(&self) -> &CommandControl {
        &self.control
    }
}

impl Drop for ClaimedCommand {
    fn drop(&mut self) {
        let _ = flock(&self.decision_lock, libc::LOCK_UN);
    }
}
