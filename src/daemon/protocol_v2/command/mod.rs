use std::fs::File;
use std::path::PathBuf;
use std::time::Duration;

use super::BootDeadline;
use super::wire::CommandControl;

mod action_link;
mod claimed;
mod client;
mod layout;
mod owner;
mod recovery;
mod staging;

#[cfg(test)]
mod tests;

pub(super) use action_link::{
    CommandActionClaim, CommandActionResult, finish_command_action,
    finish_command_action_indeterminate, try_claim_command_action, try_finish_command_action,
};
pub(crate) use layout::command_root;

#[cfg(test)]
pub(crate) use layout::prepare_layout;

pub(super) const MAX_COMMAND_STAGING_DIRECTORIES: usize = 1024;
pub(super) const MAX_COMMAND_CONTROLS: usize = 1024;
pub(super) const MAX_COMMAND_QUEUE_REFERENCES: usize = 1024;
pub(super) const MAX_COMMAND_GC_DIRECTORIES: usize = 1024;
pub(super) const MAX_COMMAND_QUARANTINE_ENTRIES: usize = 1024;
pub(super) const MAX_COMMAND_ROOT_ENTRIES: usize = 16;
pub(super) const MAX_DIRECTORY_ENTRIES_PER_SCAN: usize = 1025;

pub(super) const AUTHORIZATION_WINDOW: Duration = Duration::from_secs(5);
pub(super) const RESPONSE_WINDOW: Duration = Duration::from_secs(8);
pub(super) const LOCK_RETRY: Duration = Duration::from_millis(5);

#[derive(Debug)]
pub(crate) struct ClientCommand {
    pub(super) identity: String,
    pub(super) control_path: PathBuf,
    pub(super) decision_lock: File,
    pub(super) caller_lease: File,
    pub(super) response_deadline: BootDeadline,
    pub(super) publication_indeterminate_reason: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ClaimedCommand {
    pub(super) control_path: PathBuf,
    pub(super) decision_lock: File,
    pub(super) control: CommandControl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TerminalCommandResult {
    Succeeded,
    Canceled,
    FailedNoEffect(String),
    AdmittedIndeterminate(String),
    CommittedIndeterminate(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FinalEffect {
    Completed,
    Indeterminate,
}

#[derive(Debug)]
pub(crate) struct CommandOwner {
    pub(super) root: PathBuf,
    pub(super) daemon_token: String,
}
