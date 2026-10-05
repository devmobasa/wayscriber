//! Output switches use the same ordered, nonblocking disk phases as session commands.
use super::*;

impl SessionCommandTransaction {
    pub(super) fn start_output(
        &mut self,
        context: &mut SessionTransaction<'_>,
    ) -> Result<TransactionStep> {
        let SessionCommand::Output {
            transition,
            remembered,
            ..
        } = &self.command
        else {
            unreachable!()
        };
        self.target = Some(transition.staged_options.clone());

        let current = self.current.as_ref().expect("output switch has source");
        let check_remembered = remembered
            .as_ref()
            .is_some_and(|path| current.session_file_path() == *path);
        let current_path = current.session_file_path();
        self.capture_input_generation(context);
        if check_remembered {
            // An unavailable remembered source goes straight to the target
            // without a save, so a refused write must stop the switch first.
            context
                .session
                .validate_source_write(context.input_state.is_session_dirty())?;
            return self.work(
                Phase::OutputCheckSource,
                PersistenceOperation::CheckRemembered { path: current_path },
            );
        }
        self.prepare_output_save(context)
    }

    pub(super) fn complete_output_check_source(
        &mut self,
        context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        match required_outcome(outcome)? {
            PersistenceOutcome::Unit => self.prepare_output_save(context),
            PersistenceOutcome::RememberedUnavailable(_) => self.load_output_target(),
            other => Err(anyhow!("unexpected remembered source check: {other:?}")),
        }
    }

    fn prepare_output_save(
        &mut self,
        context: &mut SessionTransaction<'_>,
    ) -> Result<TransactionStep> {
        let options = self
            .current
            .as_ref()
            .expect("output switch has source")
            .clone();
        let dirty = context.session.is_dirty() || context.input_state.is_session_dirty();
        if !context
            .session
            .validate_source_write(context.input_state.is_session_dirty())?
            || !dirty
        {
            return self.load_output_target();
        }
        if context.session.should_skip_save_for_protected_path(
            &options.session_file_path(),
            context.input_state.is_session_dirty(),
        ) {
            return self.load_output_target();
        }
        self.output_snapshot = context
            .input_state
            .snapshot_for_persistence_with(context.measurer, &options);
        if !self
            .output_snapshot
            .as_ref()
            .is_some_and(SessionSnapshot::has_board_data)
            && !context.session.has_loaded_board_data()
            && !context.session.is_dirty()
            && !context.input_state.is_session_dirty()
        {
            return self.work(
                Phase::OutputArtifacts,
                PersistenceOperation::HasArtifacts { options },
            );
        }
        self.submit_output_save(context)
    }

    pub(super) fn complete_output_artifacts(
        &mut self,
        context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        let PersistenceOutcome::HasArtifacts(has_artifacts) = required_outcome(outcome)? else {
            return Err(anyhow!("unexpected output artifact inspection"));
        };
        if has_artifacts {
            self.load_output_target()
        } else {
            self.submit_output_save(context)
        }
    }

    fn submit_output_save(
        &mut self,
        context: &mut SessionTransaction<'_>,
    ) -> Result<TransactionStep> {
        let options = self
            .current
            .as_ref()
            .expect("output switch has source")
            .clone();
        let snapshot = match self.output_snapshot.take() {
            Some(snapshot) => snapshot,
            None if session_persistence_enabled(&options) => empty_snapshot(context.input_state),
            None => return self.load_output_target(),
        };
        self.work(
            Phase::SaveCurrent,
            PersistenceOperation::Save {
                snapshot,
                options,
                strategy: SaveStrategy::Normal,
                contentless_clear_boundary: context.session.has_loaded_board_data(),
            },
        )
    }

    pub(super) fn load_output_target(&mut self) -> Result<TransactionStep> {
        let options = self
            .target
            .as_ref()
            .expect("output switch has target")
            .clone();
        let SessionCommand::Output { remembered, .. } = &self.command else {
            unreachable!()
        };
        let operation = if remembered
            .as_ref()
            .is_some_and(|path| options.session_file_path() == *path)
        {
            PersistenceOperation::LoadRemembered { options }
        } else {
            PersistenceOperation::LoadConfigured { options }
        };
        self.work(Phase::OutputLoad, operation)
    }

    pub(super) fn complete_output_load(
        &mut self,
        context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        let outcome = match required_outcome(outcome)? {
            PersistenceOutcome::Load(outcome) => outcome,
            PersistenceOutcome::RememberedUnavailable(reason)
                if matches!(self.phase, Phase::OutputLoad) =>
            {
                self.abandoned = Some((
                    self.target
                        .as_ref()
                        .expect("remembered target")
                        .session_file_path(),
                    reason,
                ));
                let SessionCommand::Output { home, .. } = &self.command else {
                    unreachable!()
                };
                if let Some(home) = home {
                    let options = (**home).clone();
                    self.target = Some(options.clone());
                    return self.work(
                        Phase::OutputHome,
                        PersistenceOperation::LoadConfigured { options },
                    );
                }
                context.input_state.set_session_preflight_options(None);
                context.session.commit_without_persistence();
                return Ok(TransactionStep::Complete(Box::new(
                    SessionCommandReport::Output {
                        abandoned: self.abandoned.take(),
                        too_large: None,
                        first_output_resolved: true,
                    },
                )));
            }
            other => return Err(anyhow!("unexpected output load outcome: {other:?}")),
        };
        let target = self
            .target
            .as_ref()
            .expect("output load has target")
            .clone();
        let first_output_resolved = !target.per_output || target.output_identity().is_some();
        let too_large = super::super::super::commit_output_load(
            context.input_state,
            context.measurer,
            context.session,
            target,
            outcome,
            "output load",
        )?;
        Ok(TransactionStep::Complete(Box::new(
            SessionCommandReport::Output {
                abandoned: self.abandoned.take(),
                too_large,
                first_output_resolved,
            },
        )))
    }
}
