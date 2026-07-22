use super::*;

impl RuntimeUiStateStore {
    pub(super) fn finish_reset_after_claim(
        &self,
        id: SourceMutationId,
        applied_through: AcceptedStateRevision,
        target: &MutationTarget,
        claimed: ClaimedSource,
        retain: bool,
        hook: &mut dyn FnMut(MutationPoint),
    ) -> SourceMutationResult {
        let ClaimedSource {
            quarantine,
            inspection: claimed,
        } = claimed;
        let active = match self.inspect() {
            Ok(active) => active,
            Err(error) => {
                return failed(
                    id,
                    format!(
                        "could not inspect reset path after claim: {}",
                        error.message()
                    ),
                    None,
                    artifact_for(quarantine, claimed),
                    RuntimeStateFailurePathEffect::UnknownAfterMutation,
                );
            }
        };
        if !is_expected_missing(&active, &target.expected_path) {
            return observation_changed_retained(id, active.observation, quarantine, claimed);
        }
        if let Err(error) = target.operation_path.sync_parent() {
            return failed(
                id,
                format!("runtime-state reset has an uncertain durability outcome: {error}"),
                Some(active.observation),
                artifact_for(quarantine, claimed),
                RuntimeStateFailurePathEffect::UnknownAfterMutation,
            );
        }

        if retain {
            hook(MutationPoint::BeforeFinalInspection);
            let final_active = match self.inspect() {
                Ok(final_active) => final_active,
                Err(error) => {
                    return failed(
                        id,
                        format!(
                            "could not finally verify preserved reset: {}",
                            error.message()
                        ),
                        None,
                        artifact_for(quarantine, claimed),
                        RuntimeStateFailurePathEffect::UnknownAfterMutation,
                    );
                }
            };
            if !is_expected_missing(&final_active, &target.expected_path) {
                return observation_changed_retained(
                    id,
                    final_active.observation,
                    quarantine,
                    claimed,
                );
            }
            let artifacts = artifact_for(quarantine, claimed);
            if artifacts.is_empty() {
                return failed(
                    id,
                    "preserved runtime-state artifact disappeared before acknowledgement",
                    Some(final_active.observation),
                    Vec::new(),
                    RuntimeStateFailurePathEffect::UnknownAfterMutation,
                );
            }
            return applied(
                id,
                applied_through,
                final_active.observation.revision,
                artifacts,
            );
        }

        let cleanup_artifact = match store_fs::remove_file(&quarantine) {
            Ok(()) => Vec::new(),
            Err(_) => artifact_for(quarantine, claimed),
        };
        if let Err(error) = target.operation_path.sync_parent() {
            return failed(
                id,
                format!("runtime-state reset cleanup has an uncertain outcome: {error}"),
                Some(active.observation),
                cleanup_artifact,
                RuntimeStateFailurePathEffect::UnknownAfterMutation,
            );
        }
        hook(MutationPoint::BeforeFinalInspection);
        match self.inspect() {
            Ok(final_active) if is_expected_missing(&final_active, &target.expected_path) => {
                applied(
                    id,
                    applied_through,
                    final_active.observation.revision,
                    cleanup_artifact,
                )
            }
            Ok(final_active) => failed(
                id,
                "runtime-state path changed during final reset verification",
                Some(final_active.observation),
                cleanup_artifact,
                RuntimeStateFailurePathEffect::UnknownAfterMutation,
            ),
            Err(error) => failed(
                id,
                format!(
                    "could not finally verify runtime-state reset: {}",
                    error.message()
                ),
                None,
                cleanup_artifact,
                RuntimeStateFailurePathEffect::UnknownAfterMutation,
            ),
        }
    }
}
