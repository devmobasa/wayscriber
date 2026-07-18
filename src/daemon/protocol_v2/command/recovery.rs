use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};

use super::super::BootClock;
use super::super::wire::{
    ActionStatus, CommandDecision, CommandResponse, DAEMON_COMMAND_PROTOCOL_VERSION, EffectStatus,
    MAX_QUEUE_REFERENCE_BYTES, QueueReference, validate_id, validate_token,
};
use super::MAX_DIRECTORY_ENTRIES_PER_SCAN;
use super::layout::{
    QuarantineKind, bump_revision, controls_dir, lock_until, open_lock, quarantine_entry,
    queue_dir, read_control, read_dir_bounded, read_record, unlock, write_control,
};

pub(super) fn recover_previous_generation(root: &Path, daemon_token: &str) -> Result<()> {
    let reason = "previous daemon generation ended before terminal effect proof";
    for entry in read_dir_bounded(&controls_dir(root), MAX_DIRECTORY_ENTRIES_PER_SCAN)? {
        let path = entry.path();
        let identity_valid = entry
            .file_name()
            .into_string()
            .ok()
            .is_some_and(|identity| validate_id(&identity).is_ok());
        if !identity_valid {
            quarantine_entry(root, &path, QuarantineKind::Control)?;
            continue;
        }
        let decision_lock = match open_lock(&path.join("decision.lock"), false) {
            Ok(lock) => lock,
            Err(_) => {
                quarantine_entry(root, &path, QuarantineKind::Control)?;
                continue;
            }
        };
        let deadline = BootClock::now()?.checked_add(Duration::from_millis(200))?;
        lock_until(&decision_lock, libc::LOCK_EX, deadline)?;
        let mut control = match read_control(&path) {
            Ok(control) => control,
            Err(_) => {
                unlock(&decision_lock)?;
                quarantine_entry(root, &path, QuarantineKind::Control)?;
                continue;
            }
        };
        if control.target_daemon_token == daemon_token || control.response.is_some() {
            unlock(&decision_lock)?;
            continue;
        }
        let response = match &mut control.decision {
            CommandDecision::Open => {
                control.decision = CommandDecision::Rejected {
                    reason: reason.to_owned(),
                };
                CommandResponse::FailedNoEffect {
                    reason: reason.to_owned(),
                }
            }
            CommandDecision::Canceled => CommandResponse::Canceled,
            CommandDecision::Rejected { reason } => CommandResponse::FailedNoEffect {
                reason: reason.clone(),
            },
            CommandDecision::Committed {
                effect_id,
                effect_status,
                ..
            } => match effect_status {
                EffectStatus::Authorized => {
                    *effect_status = EffectStatus::Indeterminate(reason.to_owned());
                    CommandResponse::CommittedIndeterminate {
                        effect_id: effect_id.clone(),
                        reason: reason.to_owned(),
                    }
                }
                EffectStatus::Completed => CommandResponse::Succeeded {
                    effect_id: effect_id.clone(),
                },
                EffectStatus::FailedNoEffect(reason) => CommandResponse::FailedNoEffect {
                    reason: reason.clone(),
                },
                EffectStatus::Indeterminate(reason) => CommandResponse::CommittedIndeterminate {
                    effect_id: effect_id.clone(),
                    reason: reason.clone(),
                },
            },
        };
        if matches!(
            control.action_status,
            ActionStatus::Prepared { .. }
                | ActionStatus::Eligible { .. }
                | ActionStatus::Claimed { .. }
        ) {
            control.action_status = match control.action_status.clone() {
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
                } => ActionStatus::Indeterminate {
                    action_id,
                    action_order,
                    digest,
                    reason: reason.to_owned(),
                },
                _ => unreachable!("matched action state changed without mutation"),
            };
        }
        bump_revision(&mut control)?;
        control.response = Some(response);
        write_control(&path, &control)?;
        unlock(&decision_lock)?;
    }

    for entry in read_dir_bounded(&queue_dir(root), MAX_DIRECTORY_ENTRIES_PER_SCAN)? {
        let name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => {
                quarantine_entry(root, &entry.path(), QuarantineKind::Queue)?;
                continue;
            }
        };
        if name.starts_with('.') {
            continue;
        }
        let (_order, identity) = match parse_queue_name(&name) {
            Ok(parsed) => parsed,
            Err(_) => {
                quarantine_entry(root, &entry.path(), QuarantineKind::Queue)?;
                continue;
            }
        };
        let reference: QueueReference = match read_record(&entry.path(), MAX_QUEUE_REFERENCE_BYTES)
        {
            Ok(reference) => reference,
            Err(_) => {
                quarantine_entry(root, &entry.path(), QuarantineKind::Queue)?;
                continue;
            }
        };
        if reference.command_identity != identity {
            quarantine_entry(root, &entry.path(), QuarantineKind::Queue)?;
            continue;
        }
        if reference.target_daemon_token != daemon_token {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

pub(super) fn validate_reference(
    reference: &QueueReference,
    order: u64,
    identity: &str,
    token: &str,
) -> Result<()> {
    if reference.protocol_version != DAEMON_COMMAND_PROTOCOL_VERSION
        || reference.queue_order != order
        || reference.command_identity != identity
        || reference.target_daemon_token != token
    {
        bail!("invalid or cross-generation command queue reference");
    }
    validate_id(&reference.command_identity)?;
    validate_token(&reference.target_daemon_token)
}

pub(super) fn parse_queue_name(name: &str) -> Result<(u64, String)> {
    let stem = name
        .strip_suffix(".request")
        .ok_or_else(|| anyhow!("invalid command queue filename"))?;
    let (order, identity) = stem
        .split_once('-')
        .ok_or_else(|| anyhow!("invalid command queue filename"))?;
    if order.len() != 16
        || !order
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        bail!("command queue order is not canonical");
    }
    validate_id(identity)?;
    Ok((u64::from_str_radix(order, 16)?, identity.to_owned()))
}
