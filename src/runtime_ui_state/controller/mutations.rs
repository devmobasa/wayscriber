use super::*;

impl RuntimeUiStateController {
    pub(crate) fn begin_mutation(
        &self,
        scope: RuntimeUiMutationScope,
    ) -> Result<RuntimeUiMutationPermit, BeginMutationError> {
        if self.shutting_down {
            return Err(BeginMutationError::ShuttingDown);
        }
        if let Some(barrier) = &self.active_barrier {
            return Err(BeginMutationError::ControllerBusy(barrier.id));
        }
        if matches!(
            self.file_status,
            RuntimeUiFileStatus::UnsupportedReadOnly { .. } | RuntimeUiFileStatus::Invalid
        ) {
            return Err(BeginMutationError::UnsupportedVersion);
        }
        let targets = scope
            .canonical_targets()
            .map_err(BeginMutationError::InvalidScope)?;
        let guards = self
            .seeds
            .guards(&targets)
            .map_err(BeginMutationError::Seed)?;
        Ok(RuntimeUiMutationPermit {
            controller_id: self.id,
            authority_epoch: self.authority_epoch,
            mutation_id: self
                .allocate_mutation_id()
                .ok_or(BeginMutationError::MutationIdExhausted)?,
            guards,
        })
    }

    pub(crate) fn commit(
        &mut self,
        permit: RuntimeUiMutationPermit,
        desired_values: RuntimeUiMutationValues,
    ) -> CommitResult {
        self.drain_lifecycle_controls();
        if let Some(barrier) = &self.active_barrier {
            return CommitResult::RejectedControllerBusy {
                permit,
                barrier: barrier.id,
            };
        }
        if self.shutting_down {
            return CommitResult::RejectedShuttingDown;
        }
        if permit.controller_id != self.id {
            return CommitResult::RejectedWrongController;
        }
        if matches!(
            self.file_status,
            RuntimeUiFileStatus::UnsupportedReadOnly { .. } | RuntimeUiFileStatus::Invalid
        ) {
            return CommitResult::RejectedUnsupportedVersion;
        }
        if permit.authority_epoch != self.authority_epoch {
            return CommitResult::RejectedStaleAuthorityEpoch;
        }
        if desired_values.targets() != permit.targets() {
            return CommitResult::RejectedInvalidValues(
                MutationShapeError::ValuesDoNotMatchPermitScope,
            );
        }
        let changed_targets = permit
            .guards
            .iter()
            .filter(|guard| !self.seeds.guard_is_current(guard))
            .map(|guard| guard.target.clone())
            .collect::<Vec<_>>();
        if !changed_targets.is_empty() {
            return CommitResult::RejectedSeedChanged {
                targets: changed_targets,
            };
        }

        let previous_model = self.model.clone();
        let previous_passthrough = self.passthrough.clone();
        if !self.model.apply(&permit.guards, &desired_values) {
            return CommitResult::NoChange;
        }
        self.passthrough.reconcile_entries(&self.model);
        self.rebuild_live_state();
        let snapshot = self.canonical_wire();
        match self.pipeline.accept_replace(snapshot, self.authority_epoch) {
            Ok(through) => CommitResult::Accepted { through },
            Err(error) => {
                self.model = previous_model;
                self.passthrough = previous_passthrough;
                self.rebuild_live_state();
                CommitResult::RejectedPersistence(error)
            }
        }
    }

    pub(crate) fn update_seeds(&mut self, seeds: ValidatedInteractionSeeds) -> UpdateSeedsResult {
        self.drain_lifecycle_controls();
        if self.shutting_down {
            return UpdateSeedsResult::RejectedShuttingDown;
        }
        if let Some(barrier) = &self.active_barrier {
            if let Some(incident) = &mut self.incident {
                let staged = match StagedSeedReload::stage(
                    incident.staged_reload.as_ref(),
                    &self.seeds,
                    seeds,
                ) {
                    Ok(staged) => staged,
                    Err(error) => return UpdateSeedsResult::Rejected(error),
                };
                let replaced_older_staged_reload = incident.staged_reload.replace(staged).is_some();
                incident.cleanup.mark_recompute();
                return UpdateSeedsResult::StagedBehindBarrier {
                    barrier: barrier.id,
                    replaced_older_staged_reload,
                };
            }
            let staged =
                match StagedSeedReload::stage(self.staged_reload.as_ref(), &self.seeds, seeds) {
                    Ok(staged) => staged,
                    Err(error) => return UpdateSeedsResult::Rejected(error),
                };
            let replaced_older_staged_reload = self.staged_reload.replace(staged).is_some();
            return UpdateSeedsResult::StagedBehindBarrier {
                barrier: barrier.id,
                replaced_older_staged_reload,
            };
        }
        self.apply_seed_update(seeds)
    }
}
