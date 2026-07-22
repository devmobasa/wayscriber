use super::*;

impl RuntimeUiStateStore {
    pub(super) fn install_at_missing(
        &self,
        id: SourceMutationId,
        applied_through: AcceptedStateRevision,
        target: &MutationTarget,
        replacement: PreparedReplacement<'_>,
        hook: &mut dyn FnMut(MutationPoint),
    ) -> SourceMutationResult {
        hook(MutationPoint::BeforeInstall);
        match self.inspect() {
            Ok(active) if is_expected_missing(&active, &target.expected_path) => {}
            Ok(active) => {
                return SourceMutationResult::SourceChangedBeforeMutation {
                    id,
                    active: active.observation,
                };
            }
            Err(error) => {
                return failed(
                    id,
                    format!(
                        "could not verify the missing runtime-state path before install: {}",
                        error.message()
                    ),
                    None,
                    Vec::new(),
                    untouched(),
                );
            }
        }
        if let Err(error) =
            store_fs::rename_noreplace(replacement.temp.path(), &target.operation_path)
        {
            return match self.inspect() {
                Ok(active) => SourceMutationResult::SourceChangedBeforeMutation {
                    id,
                    active: active.observation,
                },
                Err(inspect_error) => failed(
                    id,
                    format!(
                        "could not install runtime state ({error}) or inspect the active path: {}",
                        inspect_error.message()
                    ),
                    None,
                    Vec::new(),
                    untouched(),
                ),
            };
        }
        replacement.temp.disarm();
        hook(MutationPoint::AfterInstall);
        if let Err(error) = target.operation_path.sync_parent() {
            return failed(
                id,
                format!("runtime-state install has an uncertain durability outcome: {error}"),
                self.inspect().ok().map(|inspection| inspection.observation),
                Vec::new(),
                RuntimeStateFailurePathEffect::UnknownAfterMutation,
            );
        }
        hook(MutationPoint::BeforeFinalInspection);
        self.finish_replacement_without_artifact(
            id,
            applied_through,
            &target.expected_path,
            replacement.bytes,
        )
    }

    pub(super) fn install_after_claim(
        &self,
        id: SourceMutationId,
        applied_through: AcceptedStateRevision,
        target: &MutationTarget,
        claimed: ClaimedSource,
        replacement: PreparedReplacement<'_>,
        hook: &mut dyn FnMut(MutationPoint),
    ) -> SourceMutationResult {
        let ClaimedSource {
            quarantine,
            inspection: claimed,
        } = claimed;
        hook(MutationPoint::BeforeInstall);
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
                        "could not verify the claimed runtime-state path before install: {}",
                        error.message()
                    ),
                );
            }
        }
        if let Err(error) =
            store_fs::rename_noreplace(replacement.temp.path(), &target.operation_path)
        {
            return self.retained_after_claim(
                id,
                quarantine,
                claimed,
                format!("active path changed before runtime-state install: {error}"),
            );
        }
        replacement.temp.disarm();
        hook(MutationPoint::AfterInstall);
        if let Err(error) = target.operation_path.sync_parent() {
            return failed(
                id,
                format!("runtime-state replacement has an uncertain durability outcome: {error}"),
                self.inspect().ok().map(|inspection| inspection.observation),
                artifact_for(quarantine, claimed),
                RuntimeStateFailurePathEffect::UnknownAfterMutation,
            );
        }

        let active = match self.inspect() {
            Ok(active) => active,
            Err(error) => {
                return failed(
                    id,
                    format!(
                        "could not verify installed runtime state: {}",
                        error.message()
                    ),
                    None,
                    artifact_for(quarantine, claimed),
                    RuntimeStateFailurePathEffect::UnknownAfterMutation,
                );
            }
        };
        let installed_is_supported = matches!(active.status, RuntimeUiFileStatus::Supported);
        if !installed_is_supported
            || active.observation.revision.path_identity() != &target.expected_path
            || active.observation.revision.bytes() != Some(replacement.bytes)
        {
            return observation_changed_retained(id, active.observation, quarantine, claimed);
        }

        let cleanup_artifact = match store_fs::remove_file(&quarantine) {
            Ok(()) => Vec::new(),
            Err(_) => artifact_for(quarantine.clone(), claimed),
        };
        if let Err(error) = target.operation_path.sync_parent() {
            return failed(
                id,
                format!("runtime-state replacement cleanup has an uncertain outcome: {error}"),
                Some(active.observation),
                cleanup_artifact,
                RuntimeStateFailurePathEffect::UnknownAfterMutation,
            );
        }
        hook(MutationPoint::BeforeFinalInspection);
        let final_active = match self.inspect() {
            Ok(final_active) => final_active,
            Err(error) => {
                return failed(
                    id,
                    format!(
                        "could not finally verify runtime state: {}",
                        error.message()
                    ),
                    None,
                    cleanup_artifact,
                    RuntimeStateFailurePathEffect::UnknownAfterMutation,
                );
            }
        };
        if final_active.observation.revision.path_identity() != &target.expected_path
            || final_active.observation.revision.bytes() != Some(replacement.bytes)
            || !matches!(final_active.status, RuntimeUiFileStatus::Supported)
        {
            return failed(
                id,
                "runtime-state source changed during final replacement verification",
                Some(final_active.observation),
                cleanup_artifact,
                RuntimeStateFailurePathEffect::UnknownAfterMutation,
            );
        }
        applied(
            id,
            applied_through,
            final_active.observation.revision,
            cleanup_artifact,
        )
    }

    fn finish_replacement_without_artifact(
        &self,
        id: SourceMutationId,
        applied_through: AcceptedStateRevision,
        expected_path: &RuntimeStatePathIdentity,
        expected_bytes: &[u8],
    ) -> SourceMutationResult {
        match self.inspect() {
            Ok(active)
                if matches!(active.status, RuntimeUiFileStatus::Supported)
                    && active.observation.revision.path_identity() == expected_path
                    && active.observation.revision.bytes() == Some(expected_bytes) =>
            {
                applied(id, applied_through, active.observation.revision, Vec::new())
            }
            Ok(active) => failed(
                id,
                "installed runtime state was replaced before verification",
                Some(active.observation),
                Vec::new(),
                RuntimeStateFailurePathEffect::UnknownAfterMutation,
            ),
            Err(error) => failed(
                id,
                format!(
                    "could not verify installed runtime state: {}",
                    error.message()
                ),
                None,
                Vec::new(),
                RuntimeStateFailurePathEffect::UnknownAfterMutation,
            ),
        }
    }
}
