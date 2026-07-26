mod action;
mod child;
mod command;
mod digest;
mod linux;
mod mode;
mod runtime;
mod wire;

pub(crate) use child::{DaemonWatchdogOwner, OverlayChildOwner, PreparedDaemonWatchdog};
pub(crate) use child::{
    open_daemon_watchdog, prepare_daemon_watchdog_from_environment, recover_stale_child_records,
};
pub(crate) use child::{publish_ready_from_environment, publish_signal_ready_from_environment};
pub(crate) use command::{ClientCommand, CommandOwner, FinalEffect, TerminalCommandResult};
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

pub(crate) fn try_claim_overlay_action(
    runtime_paths: &crate::paths::PreparedRuntimePaths,
) -> anyhow::Result<ActionClaimOutcome> {
    let root = runtime_paths.protocol_v2_root();
    let active = child::active_generation_from_environment(&root)?;
    try_claim_overlay_action_for_active(runtime_paths, root, active)
}

#[cfg(test)]
fn try_claim_overlay_action_for_generation(
    runtime_paths: &crate::paths::PreparedRuntimePaths,
    generation: Option<&str>,
) -> anyhow::Result<ActionClaimOutcome> {
    let root = runtime_paths.protocol_v2_root();
    let active = child::active_generation(&root, generation)?;
    try_claim_overlay_action_for_active(runtime_paths, root, active)
}

fn try_claim_overlay_action_for_active(
    runtime_paths: &crate::paths::PreparedRuntimePaths,
    root: std::path::PathBuf,
    active: child::ActiveGeneration,
) -> anyhow::Result<ActionClaimOutcome> {
    let enabled_daemon_token = match active {
        child::ActiveGeneration::Inactive => return Ok(ActionClaimOutcome::Idle),
        child::ActiveGeneration::Pending => return Ok(ActionClaimOutcome::Deferred),
        child::ActiveGeneration::Enabled { daemon_token } => daemon_token,
    };
    let runtime_path = runtime_paths.daemon_pid_file();
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
    let journal = ActionJournal::open(root.clone())?;
    journal.try_claim_next(&runtime.v2_instance_token, |identity, prepared| {
        command::try_claim_command_action(&root, identity, prepared)
    })
}

pub(crate) fn prepare_rollback_compatibility(
    runtime_paths: &crate::paths::PreparedRuntimePaths,
) -> anyhow::Result<()> {
    prepare_rollback_compatibility_root(runtime_paths.protocol_v2_root())
}

#[cfg(test)]
fn prepare_rollback_compatibility_at_root(root: &std::path::Path) -> anyhow::Result<()> {
    prepare_rollback_compatibility_root(root.to_path_buf())
}

fn prepare_rollback_compatibility_root(root: std::path::PathBuf) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => anyhow::bail!("v2 command root is not a no-follow directory"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    let token = ProtocolToken::generate()?.to_string();
    let owner = CommandOwner::open(&token, root.clone())?;
    child::recover_stale_child_records(&root)?;
    let journal = ActionJournal::open(root)?;
    journal.quiesce_for_rollback(&token)?;
    owner.assert_rollback_quiescent()
}
