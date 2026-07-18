mod action;
mod child;
mod command;
mod digest;
mod linux;
mod mode;
mod runtime;
mod wire;

pub(crate) use child::OverlayChildOwner;
pub(crate) use child::{
    open_daemon_watchdog, recover_stale_child_records, start_daemon_watchdog_from_environment,
};
pub(crate) use child::{publish_ready_from_environment, publish_signal_ready_from_environment};
pub(crate) use command::{
    ClientCommand, CommandOwner, FinalEffect, TerminalCommandResult, command_root,
};
pub(crate) use linux::{
    BootClock, BootDeadline, BootDeadlineSource, BootIdentity, CommandQueueWatcher,
    NamespaceIdentity, ProtocolId, ProtocolToken,
};
pub(crate) use mode::DaemonControlProtocolMode;
pub(crate) use runtime::{ClassifiedRuntimeRecord, read_runtime_record, write_runtime_record_v2};
pub(crate) use wire::EffectKind;
pub(crate) use wire::{DaemonRequestV2, DaemonRuntimeRecordV2};

#[cfg(test)]
mod tests;
pub(crate) use action::{ActionClaimOutcome, ActionFinishOutcome, ActionJournal, ClaimedAction};

pub(crate) fn try_claim_overlay_action() -> anyhow::Result<ActionClaimOutcome> {
    let enabled_daemon_token = match child::active_generation_from_environment()? {
        child::ActiveGeneration::Inactive => return Ok(ActionClaimOutcome::Idle),
        child::ActiveGeneration::Pending => return Ok(ActionClaimOutcome::Deferred),
        child::ActiveGeneration::Enabled { daemon_token } => daemon_token,
    };
    let runtime_path = crate::paths::daemon_pid_file();
    match std::fs::symlink_metadata(&runtime_path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ActionClaimOutcome::Idle);
        }
        Err(error) => return Err(error.into()),
    }
    let runtime = match read_runtime_record(&runtime_path)? {
        ClassifiedRuntimeRecord::V2(runtime) => runtime,
        ClassifiedRuntimeRecord::LegacyV1 { .. } => return Ok(ActionClaimOutcome::Idle),
    };
    if runtime.v2_instance_token != enabled_daemon_token {
        anyhow::bail!("overlay action enable belongs to a different daemon generation");
    }
    let journal = ActionJournal::open()?;
    journal.try_claim_next(&runtime.v2_instance_token, |identity, prepared| {
        command::try_claim_command_action(identity, prepared)
    })
}

pub(crate) fn prepare_rollback_compatibility() -> anyhow::Result<()> {
    let root = command_root();
    match std::fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => anyhow::bail!("v2 command root is not a no-follow directory"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    let token = ProtocolToken::generate()?.to_string();
    let owner = CommandOwner::open(&token)?;
    child::recover_stale_child_records()?;
    let journal = ActionJournal::open()?;
    journal.quiesce_for_rollback(&token)?;
    owner.assert_rollback_quiescent()
}
