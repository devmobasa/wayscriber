use super::*;

pub(super) fn status_allowed(mode: MutationMode, status: &RuntimeUiFileStatus) -> bool {
    match mode {
        MutationMode::Replace => matches!(
            status,
            RuntimeUiFileStatus::Missing | RuntimeUiFileStatus::Supported
        ),
        MutationMode::ResetSupported => matches!(
            status,
            RuntimeUiFileStatus::Missing | RuntimeUiFileStatus::Supported
        ),
        MutationMode::ResetPreservingUnsupported => {
            matches!(status, RuntimeUiFileStatus::UnsupportedReadOnly { .. })
        }
        MutationMode::ResetPreservingInvalid => matches!(status, RuntimeUiFileStatus::Invalid),
    }
}

pub(super) fn artifact_for(
    path: store_fs::PinnedPath,
    source_inspection: RuntimeUiStateInspection,
) -> Vec<RuntimeStateRecoveryArtifact> {
    let source_path = source_inspection
        .observation
        .revision
        .path_identity()
        .clone();
    inspection::inspect_pinned(&path).map_or_else(
        |_| Vec::new(),
        |mut inspection| {
            if inspection.observation.revision.bytes().is_none() {
                return Vec::new();
            }
            inspection.observation.revision = inspection
                .observation
                .revision
                .with_path_identity(source_path);
            let Ok(path) = path.reported_path() else {
                return Vec::new();
            };
            vec![RuntimeStateRecoveryArtifact {
                path,
                observation: inspection.observation,
            }]
        },
    )
}

pub(super) fn observation_changed_retained(
    id: SourceMutationId,
    active: RuntimeStateSourceObservation,
    recovery_path: store_fs::PinnedPath,
    claimed: RuntimeUiStateInspection,
) -> SourceMutationResult {
    let recovery_artifacts = artifact_for(recovery_path.clone(), claimed);
    if recovery_artifacts.is_empty() {
        return failed(
            id,
            "claimed runtime-state artifact disappeared before acknowledgement",
            Some(active),
            Vec::new(),
            RuntimeStateFailurePathEffect::UnknownAfterMutation,
        );
    }
    if let Err(error) = recovery_path.sync_parent() {
        return failed(
            id,
            format!("retained runtime-state artifact has an uncertain outcome: {error}"),
            Some(active),
            recovery_artifacts,
            RuntimeStateFailurePathEffect::UnknownAfterMutation,
        );
    }
    let recovery_path = recovery_artifacts[0].path.clone();
    SourceMutationResult::ObservationChangedAfterClaim {
        id,
        active,
        recovery_artifacts,
        path_effect: RuntimeStatePostClaimPathEffect::QuarantinedAndRetained { recovery_path },
    }
}

pub(super) fn is_expected_missing(
    inspection: &RuntimeUiStateInspection,
    expected_path: &RuntimeStatePathIdentity,
) -> bool {
    inspection.observation.revision.bytes().is_none()
        && inspection.observation.revision.path_identity() == expected_path
}

pub(super) fn applied(
    id: SourceMutationId,
    applied_through: AcceptedStateRevision,
    new_source: RuntimeStateSourceRevision,
    recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
) -> SourceMutationResult {
    SourceMutationResult::Applied {
        id,
        applied_through,
        new_source,
        recovery_artifacts,
    }
}

pub(super) fn failed(
    id: SourceMutationId,
    message: impl Into<String>,
    active: Option<RuntimeStateSourceObservation>,
    recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
    path_effect: RuntimeStateFailurePathEffect,
) -> SourceMutationResult {
    SourceMutationResult::Failed {
        id,
        error: RuntimeStateIoError::new(message),
        active,
        recovery_artifacts,
        path_effect,
    }
}

pub(super) fn untouched() -> RuntimeStateFailurePathEffect {
    RuntimeStateFailurePathEffect::Known(RuntimeStateObservedPathEffect::Untouched)
}
