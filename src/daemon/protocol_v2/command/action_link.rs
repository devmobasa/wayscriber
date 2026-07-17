use std::io::ErrorKind;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use super::super::BootClock;
use super::super::action::PreparedAction;
use super::super::wire::{
    ActionStatus, CommandDecision, CommandResponse, EffectKind, EffectStatus, MAX_REASON_BYTES,
    bounded_reason, validate_id,
};
use super::layout::{
    bump_revision, command_root, control_path, flock, lock_until, open_lock, read_control, unlock,
    write_control,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::daemon::protocol_v2) enum CommandActionClaim {
    Claimed,
    Barrier,
    Abandon(String),
}

pub(in crate::daemon::protocol_v2) fn try_claim_command_action(
    command_identity: &str,
    action: &PreparedAction,
) -> Result<Option<CommandActionClaim>> {
    claim_command_action_inner(command_identity, action, true)
}

fn claim_command_action_inner(
    command_identity: &str,
    action: &PreparedAction,
    nonblocking: bool,
) -> Result<Option<CommandActionClaim>> {
    validate_id(command_identity)?;
    let path = control_path(&command_root(), command_identity);
    let decision = open_lock(&path.join("decision.lock"), false)?;
    if nonblocking {
        match flock(&decision, libc::LOCK_EX | libc::LOCK_NB) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(None),
            Err(error) => return Err(error).context("failed to try command-action decision lock"),
        }
    } else {
        let deadline = BootClock::now()?.checked_add(Duration::from_millis(200))?;
        lock_until(&decision, libc::LOCK_EX, deadline)?;
    }
    let mut control = read_control(&path)?;
    let exact = |status: &ActionStatus| match status {
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
        }
        | ActionStatus::Abandoned {
            action_id,
            action_order,
            digest,
            ..
        }
        | ActionStatus::Indeterminate {
            action_id,
            action_order,
            digest,
            ..
        } => {
            action_id == &action.action_id
                && *action_order == action.action_order
                && digest == &action.digest
        }
        _ => false,
    };
    let outcome = match (&control.decision, &control.action_status) {
        (
            CommandDecision::Committed {
                effect_status: EffectStatus::Authorized,
                ..
            },
            ActionStatus::Eligible { .. } | ActionStatus::Claimed { .. },
        ) if exact(&control.action_status) => {
            if matches!(control.action_status, ActionStatus::Eligible { .. }) {
                control.action_status = ActionStatus::Claimed {
                    action_id: action.action_id.clone(),
                    action_order: action.action_order,
                    digest: action.digest.clone(),
                };
                bump_revision(&mut control)?;
                write_control(&path, &control)?;
            }
            CommandActionClaim::Claimed
        }
        (CommandDecision::Open, ActionStatus::None) => CommandActionClaim::Barrier,
        (CommandDecision::Open, ActionStatus::Prepared { .. }) if exact(&control.action_status) => {
            CommandActionClaim::Barrier
        }
        (CommandDecision::Canceled, ActionStatus::None) => {
            CommandActionClaim::Abandon("command was canceled before action commit".into())
        }
        (CommandDecision::Canceled, status) if exact(status) => {
            CommandActionClaim::Abandon("command was canceled before action commit".into())
        }
        (CommandDecision::Rejected { reason }, ActionStatus::None) => {
            CommandActionClaim::Abandon(bounded_reason(reason, 1024))
        }
        (CommandDecision::Rejected { reason }, status) if exact(status) => {
            CommandActionClaim::Abandon(bounded_reason(reason, 1024))
        }
        _ => {
            unlock(&decision)?;
            bail!("command action state does not match its journal envelope");
        }
    };
    unlock(&decision)?;
    Ok(Some(outcome))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::daemon::protocol_v2) enum CommandActionResult {
    Applied,
    NoEffect,
    Indeterminate,
}

pub(in crate::daemon::protocol_v2) fn finish_command_action(
    command_identity: &str,
    action: &PreparedAction,
    applied: bool,
    reason: Option<&str>,
) -> Result<()> {
    let result = if applied {
        CommandActionResult::Applied
    } else {
        CommandActionResult::NoEffect
    };
    if finish_command_action_inner(command_identity, action, result, reason, false)? {
        Ok(())
    } else {
        bail!("blocking command-action finish unexpectedly deferred")
    }
}

pub(in crate::daemon::protocol_v2) fn try_finish_command_action(
    command_identity: &str,
    action: &PreparedAction,
    result: CommandActionResult,
    reason: Option<&str>,
) -> Result<bool> {
    finish_command_action_inner(command_identity, action, result, reason, true)
}

pub(in crate::daemon::protocol_v2) fn finish_command_action_indeterminate(
    command_identity: &str,
    action: &PreparedAction,
    reason: &str,
) -> Result<()> {
    if finish_command_action_inner(
        command_identity,
        action,
        CommandActionResult::Indeterminate,
        Some(reason),
        false,
    )? {
        Ok(())
    } else {
        bail!("blocking indeterminate command-action finish unexpectedly deferred")
    }
}

fn finish_command_action_inner(
    command_identity: &str,
    action: &PreparedAction,
    result: CommandActionResult,
    reason: Option<&str>,
    nonblocking: bool,
) -> Result<bool> {
    validate_id(command_identity)?;
    let path = control_path(&command_root(), command_identity);
    let decision = open_lock(&path.join("decision.lock"), false)?;
    if nonblocking {
        match flock(&decision, libc::LOCK_EX | libc::LOCK_NB) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(false),
            Err(error) => return Err(error).context("failed to try command-action decision lock"),
        }
    } else {
        let deadline = BootClock::now()?.checked_add(Duration::from_millis(200))?;
        lock_until(&decision, libc::LOCK_EX, deadline)?;
    }
    let mut control = read_control(&path)?;
    let applied = result == CommandActionResult::Applied;
    let exact_claim = matches!(
        &control.action_status,
        ActionStatus::Claimed {
            action_id,
            action_order,
            digest,
        } if action_id == &action.action_id
            && *action_order == action.action_order
            && digest == &action.digest
    );
    let exact_unclaimed_failure = !applied
        && matches!(
            &control.action_status,
            ActionStatus::Prepared {
                action_id,
                action_order,
                digest,
            } | ActionStatus::Eligible {
                action_id,
                action_order,
                digest,
            } if action_id == &action.action_id
                && *action_order == action.action_order
                && digest == &action.digest
        );
    let terminal_before_commit = result == CommandActionResult::NoEffect
        && matches!(
            control.decision,
            CommandDecision::Canceled | CommandDecision::Rejected { .. }
        )
        && (matches!(control.action_status, ActionStatus::None) || exact_unclaimed_failure);
    if terminal_before_commit {
        let reason = bounded_reason(reason.unwrap_or("command ended before action commit"), 1024);
        control.action_status = ActionStatus::Abandoned {
            action_id: action.action_id.clone(),
            action_order: action.action_order,
            digest: action.digest.clone(),
            reason,
        };
        bump_revision(&mut control)?;
        write_control(&path, &control)?;
        unlock(&decision)?;
        return Ok(true);
    }
    let already_applied = applied
        && matches!(
            &control.action_status,
            ActionStatus::Applied {
                action_id,
                action_order,
                digest,
            } if action_id == &action.action_id
                && *action_order == action.action_order
                && digest == &action.digest
        )
        && matches!(
            (&control.decision, &control.response),
            (
                CommandDecision::Committed {
                    effect_status: EffectStatus::Completed,
                    ..
                },
                Some(CommandResponse::Succeeded { .. })
            )
        );
    let already_failed = !applied
        && matches!(
            &control.action_status,
            ActionStatus::Abandoned {
                action_id,
                action_order,
                digest,
                ..
            } | ActionStatus::Indeterminate {
                action_id,
                action_order,
                digest,
                ..
            } if action_id == &action.action_id
                && *action_order == action.action_order
                && digest == &action.digest
        )
        && control.response.is_some();
    if already_applied || already_failed {
        unlock(&decision)?;
        return Ok(true);
    }
    if !exact_claim && !exact_unclaimed_failure {
        unlock(&decision)?;
        bail!("command action claim no longer matches");
    }
    let (effect_id, effect_kind) = match &control.decision {
        CommandDecision::Committed {
            effect_id,
            effect_kind,
            effect_status: EffectStatus::Authorized,
            ..
        } => (effect_id.clone(), *effect_kind),
        _ => {
            unlock(&decision)?;
            bail!("command action effect is not authorized");
        }
    };
    let reason = bounded_reason(
        reason.unwrap_or("action handler proved no effect"),
        MAX_REASON_BYTES,
    );
    if result == CommandActionResult::Applied {
        control.action_status = ActionStatus::Applied {
            action_id: action.action_id.clone(),
            action_order: action.action_order,
            digest: action.digest.clone(),
        };
        if let CommandDecision::Committed { effect_status, .. } = &mut control.decision {
            *effect_status = EffectStatus::Completed;
        }
        control.response = Some(CommandResponse::Succeeded { effect_id });
    } else if result == CommandActionResult::NoEffect
        && effect_kind == EffectKind::DeliverReadyAction
    {
        control.action_status = ActionStatus::Abandoned {
            action_id: action.action_id.clone(),
            action_order: action.action_order,
            digest: action.digest.clone(),
            reason: reason.clone(),
        };
        if let CommandDecision::Committed { effect_status, .. } = &mut control.decision {
            *effect_status = EffectStatus::FailedNoEffect(reason.clone());
        }
        control.response = Some(CommandResponse::FailedNoEffect { reason });
    } else {
        control.action_status = ActionStatus::Indeterminate {
            action_id: action.action_id.clone(),
            action_order: action.action_order,
            digest: action.digest.clone(),
            reason: reason.clone(),
        };
        if let CommandDecision::Committed { effect_status, .. } = &mut control.decision {
            *effect_status = EffectStatus::Indeterminate(reason.clone());
        }
        control.response = Some(CommandResponse::CommittedIndeterminate { effect_id, reason });
    }
    bump_revision(&mut control)?;
    write_control(&path, &control)?;
    unlock(&decision)?;
    Ok(true)
}
