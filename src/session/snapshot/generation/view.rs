use super::marker::{MarkerKind, read_marker_generation};
use super::probe::probe_payload_header;
use super::{Generation, MAX_GENERATION};
use crate::session::{
    SessionOptions,
    primary::{is_non_regular_session_artifact, open_session_artifact_for_probe},
};
use anyhow::{Result, anyhow};
use log::warn;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::session::snapshot) enum Slot {
    Primary,
    Backup,
    Recovery,
    Cleared,
    BackupRecoverable,
    RecoveryRecoverable,
}

impl Slot {
    const ALL: [Self; 6] = [
        Self::Primary,
        Self::Backup,
        Self::Recovery,
        Self::Cleared,
        Self::BackupRecoverable,
        Self::RecoveryRecoverable,
    ];

    fn path(self, options: &SessionOptions) -> PathBuf {
        match self {
            Self::Primary => options.session_file_path(),
            Self::Backup => options.backup_file_path(),
            Self::Recovery => options.recovery_file_path(),
            Self::Cleared => options.clear_marker_file_path(),
            Self::BackupRecoverable => options.backup_recovery_marker_file_path(),
            Self::RecoveryRecoverable => options.recovery_recoverable_marker_file_path(),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Primary => "primary session",
            Self::Backup => "session backup",
            Self::Recovery => "session recovery",
            Self::Cleared => "clear marker",
            Self::BackupRecoverable => "backup recoverable marker",
            Self::RecoveryRecoverable => "recovery recoverable marker",
        }
    }

    fn marker_kind(self) -> Option<MarkerKind> {
        match self {
            Self::Cleared => Some(MarkerKind::Cleared),
            Self::BackupRecoverable => Some(MarkerKind::BackupRecoverable),
            Self::RecoveryRecoverable => Some(MarkerKind::RecoveryRecoverable),
            _ => None,
        }
    }

    fn can_hide_newer_data(self) -> bool {
        matches!(self, Self::Recovery | Self::Cleared)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::session::snapshot) enum UnreadablePolicy {
    Refuse,
    Warn,
}

enum SlotState {
    Absent,
    Ignored,
    Unreadable(anyhow::Error),
    Present(Generation),
}

struct SlotProbe {
    slot: Slot,
    path: PathBuf,
    state: SlotState,
}

pub(in crate::session::snapshot) struct ArtifactSetView {
    slots: Vec<SlotProbe>,
}

impl ArtifactSetView {
    /// Caller holds the exclusive save lock, or accepts the Save As preparation race.
    pub(in crate::session::snapshot) fn probe(options: &SessionOptions) -> Self {
        let slots = Slot::ALL
            .into_iter()
            .map(|slot| {
                let path = slot.path(options);
                let state = probe_slot(slot, &path, options.is_named_file());
                SlotProbe { slot, path, state }
            })
            .collect();
        Self { slots }
    }

    pub(in crate::session::snapshot) fn next_generation(
        &self,
        policy: UnreadablePolicy,
    ) -> Result<Option<u64>> {
        for probe in &self.slots {
            let SlotState::Unreadable(err) = &probe.state else {
                continue;
            };
            if probe.slot.can_hide_newer_data() && policy == UnreadablePolicy::Refuse {
                return Err(anyhow!(
                    "not saving: cannot read the save generation of {} {}: {err:#}",
                    probe.slot.label(),
                    probe.path.display()
                ));
            }
            warn!(
                "Allocating a session save generation without unreadable {} {}: {err:#}",
                probe.slot.label(),
                probe.path.display()
            );
        }
        let max = self
            .slots
            .iter()
            .filter_map(|probe| match probe.state {
                SlotState::Present(g) => g.known(),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        if max >= MAX_GENERATION {
            warn!(
                "Session save generation is at its ceiling; saving without one until that artifact rotates out"
            );
            return Ok(None);
        }
        Ok(Some(max + 1))
    }
}

fn probe_slot(slot: Slot, path: &Path, no_follow: bool) -> SlotState {
    let file = match open_session_artifact_for_probe(path, no_follow) {
        Ok(None) => return SlotState::Absent,
        Ok(Some(file)) => file,
        Err(err) if is_non_regular_session_artifact(&err) => {
            warn!(
                "Ignoring non-regular {} {}: {err}",
                slot.label(),
                path.display()
            );
            return SlotState::Ignored;
        }
        Err(err) => return SlotState::Unreadable(err),
    };
    let result = match slot.marker_kind() {
        Some(kind) => read_marker_generation(&file, kind),
        None => probe_payload_header(&file),
    };
    match result {
        Ok(g) => SlotState::Present(g),
        Err(err) => SlotState::Unreadable(err.into()),
    }
}
