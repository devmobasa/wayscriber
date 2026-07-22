use super::*;

impl RuntimeUiStateStore {
    pub(super) fn restore_or_retain_changed_claim(
        &self,
        id: SourceMutationId,
        target: &MutationTarget,
        claimed: ClaimedSource,
    ) -> SourceMutationResult {
        let ClaimedSource {
            quarantine,
            inspection: claimed,
        } = claimed;
        match self.inspect() {
            Ok(active) if is_expected_missing(&active, &target.expected_path) => {}
            Ok(active) => {
                return observation_changed_retained(id, active.observation, quarantine, claimed);
            }
            Err(error) => {
                return self.retained_after_claim(
                    id,
                    quarantine,
                    claimed,
                    format!(
                        "could not verify the path before restoring changed runtime state: {}",
                        error.message()
                    ),
                );
            }
        }
        match store_fs::rename_noreplace(&quarantine, &target.operation_path) {
            Ok(()) => {
                if let Err(error) = target.operation_path.sync_parent() {
                    return failed(
                        id,
                        format!("restored runtime state has an uncertain outcome: {error}"),
                        self.inspect().ok().map(|inspection| inspection.observation),
                        Vec::new(),
                        RuntimeStateFailurePathEffect::UnknownAfterMutation,
                    );
                }
                match self.inspect() {
                    Ok(active) => SourceMutationResult::ObservationChangedAfterClaim {
                        id,
                        path_effect: RuntimeStatePostClaimPathEffect::QuarantinedThenRestored {
                            restored_source: active.observation.revision.clone(),
                        },
                        active: active.observation,
                        recovery_artifacts: Vec::new(),
                    },
                    Err(error) => failed(
                        id,
                        format!(
                            "could not verify restored runtime state: {}",
                            error.message()
                        ),
                        None,
                        Vec::new(),
                        RuntimeStateFailurePathEffect::UnknownAfterMutation,
                    ),
                }
            }
            Err(error) => self.retained_after_claim(
                id,
                quarantine,
                claimed,
                format!("could not restore changed claimed source without replacement: {error}"),
            ),
        }
    }

    pub(super) fn retained_after_claim(
        &self,
        id: SourceMutationId,
        quarantine: store_fs::PinnedPath,
        claimed: RuntimeUiStateInspection,
        reason: impl Into<String>,
    ) -> SourceMutationResult {
        let artifacts = artifact_for(quarantine.clone(), claimed);
        if artifacts.is_empty() {
            return failed(
                id,
                format!("{}; claimed artifact disappeared", reason.into()),
                self.inspect().ok().map(|inspection| inspection.observation),
                Vec::new(),
                RuntimeStateFailurePathEffect::UnknownAfterMutation,
            );
        }
        if let Err(error) = quarantine.sync_parent() {
            return failed(
                id,
                format!("retained runtime-state artifact has an uncertain outcome: {error}"),
                self.inspect().ok().map(|inspection| inspection.observation),
                artifacts,
                RuntimeStateFailurePathEffect::UnknownAfterMutation,
            );
        }
        let recovery_path = artifacts[0].path.clone();
        match self.inspect() {
            Ok(active) => SourceMutationResult::ObservationChangedAfterClaim {
                id,
                active: active.observation,
                recovery_artifacts: artifacts,
                path_effect: RuntimeStatePostClaimPathEffect::QuarantinedAndRetained {
                    recovery_path,
                },
            },
            Err(_) => failed(
                id,
                reason,
                None,
                artifacts,
                RuntimeStateFailurePathEffect::Known(RuntimeStateObservedPathEffect::PostClaim(
                    RuntimeStatePostClaimPathEffect::QuarantinedAndRetained { recovery_path },
                )),
            ),
        }
    }
}
