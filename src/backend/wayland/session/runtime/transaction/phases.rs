use super::*;

impl ExplicitSessionTransaction {
    pub(super) fn complete_start(
        &mut self,
        context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        if outcome.is_some() {
            return Err(anyhow!(
                "unexpected completion before session command started"
            ));
        }
        self.current = context.session.options().cloned();
        match &self.command {
            SessionCommand::ClearTools(defaults) if self.current.is_none() => {
                let defaults = defaults.clone();
                self.apply_default_tools(context, *defaults);
                Ok(TransactionStep::Complete(Box::new(
                    SessionCommandReport::ClearTools(RuntimeClearToolStateReport {
                        session_path: None,
                        outcome: None,
                    }),
                )))
            }
            SessionCommand::Forget(path) => self.work(
                Phase::Forget,
                PersistenceOperation::ForgetNamedSessionByPath { path: path.clone() },
            ),
            _ => {
                let options = self
                    .current
                    .clone()
                    .ok_or_else(|| anyhow!("no active persisted session target"))?;
                match &self.command {
                    SessionCommand::Open(path) => {
                        let mut target = options;
                        target.set_named_file_target(path.clone());
                        target.force_resume_persistence();
                        self.target = Some(target);
                        self.work(
                            Phase::OpenPreflight,
                            PersistenceOperation::ValidateNamedOpen { path: path.clone() },
                        )
                    }
                    SessionCommand::SaveAs(path, _) | SessionCommand::CheckOverwrite(path) => {
                        let current_path = options.session_file_path();
                        let mut target = options;
                        target.set_named_file_target(path.clone());
                        target.force_resume_persistence();
                        self.target = Some(target.clone());
                        self.work(
                            Phase::SaveAsPreflight,
                            PersistenceOperation::SaveAsOverwritePreflight {
                                current_path,
                                options: target,
                            },
                        )
                    }
                    SessionCommand::Clear => {
                        self.capture_input_generation(context);
                        self.work(
                            Phase::Clear,
                            PersistenceOperation::Save {
                                snapshot: empty_snapshot(context.input_state),
                                options,
                                strategy: SaveStrategy::Normal,
                                contentless_clear_boundary: true,
                            },
                        )
                    }
                    SessionCommand::ClearTools(_) => {
                        self.capture_input_generation(context);
                        self.work(
                            Phase::ClearTools,
                            PersistenceOperation::ClearToolState { options },
                        )
                    }
                    SessionCommand::Inspect => {
                        self.work(Phase::Inspect, PersistenceOperation::Inspect { options })
                    }
                    SessionCommand::Forget(_) => unreachable!(),
                }
            }
        }
    }

    pub(super) fn complete_open_preflight(
        &mut self,
        context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        accept_open_preflight(context.session, required_outcome(outcome)?)?;
        self.save_current_or_continue(context)
    }

    pub(super) fn complete_save_current(
        &mut self,
        context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        let PersistenceOutcome::Save(save) = required_outcome(outcome)? else {
            return Err(anyhow!("unexpected save-before-target-change outcome"));
        };
        if !save.committed() {
            return Err(anyhow!(
                "current session had unsaved changes but no session file was written"
            ));
        }
        context.input_state.clear_session_dirty();
        context
            .session
            .mark_saved(Instant::now(), save.committed_board_data);
        self.saved_current = true;
        self.after_current_save(context)
    }

    pub(super) fn complete_load(
        &mut self,
        context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        let PersistenceOutcome::Load(load) = required_outcome(outcome)? else {
            return Err(anyhow!("unexpected named-session load outcome"));
        };
        let target = self.target.as_ref().expect("open has target");
        let snapshot = named_candidate_snapshot(load, target)?;
        self.loaded_board_data = snapshot.has_board_data();
        stored_session::apply_snapshot_replacing_boards(
            context.input_state,
            context.measurer,
            snapshot,
            target,
        )?;
        context
            .input_state
            .set_session_preflight_options(Some(target.clone()));
        context.input_state.clear_session_dirty();
        context
            .session
            .commit_runtime_open(target.clone(), self.loaded_board_data);
        self.epoch = context.session.target_epoch();
        // The open is committed. Later edits belong to the opened target;
        // a catalog completion must never roll that state back.
        self.generation = None;
        self.interaction = None;
        self.work(
            Phase::RecordOpen,
            PersistenceOperation::RecordNamedOpened {
                options: target.clone(),
            },
        )
    }

    pub(super) fn complete_record_open(
        &mut self,
        _context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        if !matches!(required_outcome(outcome)?, PersistenceOutcome::Unit) {
            log::warn!("Named session opened, but catalog returned an unexpected outcome");
        }
        Ok(TransactionStep::Complete(Box::new(
            SessionCommandReport::Open(self.open_report()),
        )))
    }

    pub(super) fn complete_save_as_preflight(
        &mut self,
        context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        let outcome = required_outcome(outcome)?;
        if let SessionCommand::CheckOverwrite(path) = &self.command {
            let PersistenceOutcome::SaveAsPreflight {
                same_target,
                overwrite_required,
            } = outcome
            else {
                return Err(anyhow!("unexpected Save As preflight outcome"));
            };
            return Ok(TransactionStep::Complete(Box::new(
                SessionCommandReport::Overwrite(path.clone(), !same_target && overwrite_required),
            )));
        }
        let SessionCommand::SaveAs(path, overwrite) = &self.command else {
            unreachable!()
        };
        match accept_save_as_preflight(context.session, outcome, *overwrite, path)? {
            SaveAsPreflightDecision::SameTarget => self.save_current_or_continue(context),
            SaveAsPreflightDecision::SwitchTarget => {
                let overwrite = *overwrite;
                let options = self.target.as_ref().expect("save as has target").clone();
                let snapshot = context
                    .input_state
                    .with_active_interaction_canceled_for_capture_with(context.measurer, |input| {
                        stored_session::snapshot_from_input(input, &options)
                    })
                    .ok_or_else(|| anyhow!("Save Session As has no session data to write"))?;
                self.capture_input_generation(context);
                self.work(
                    Phase::SaveAs,
                    PersistenceOperation::SaveAs {
                        snapshot,
                        options,
                        overwrite,
                    },
                )
            }
        }
    }

    pub(super) fn complete_save_as(
        &mut self,
        context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        let PersistenceOutcome::SaveAs {
            report,
            committed_board_data,
        } = required_outcome(outcome)?
        else {
            return Err(anyhow!("unexpected Save As worker outcome"));
        };
        let target = self.target.as_ref().expect("save as has target").clone();
        let saved_path = target.session_file_path();
        context
            .input_state
            .set_session_preflight_options(Some(target.clone()));
        context.input_state.clear_session_dirty();
        context
            .session
            .commit_runtime_save_as(target, Instant::now(), committed_board_data);
        Ok(TransactionStep::Complete(Box::new(
            SessionCommandReport::SaveAs(RuntimeSaveAsSessionReport {
                previous_path: self.current_path(),
                saved_path,
                switched_target: true,
                saved: true,
                saved_board_data: committed_board_data,
                outcome: Some(report.outcome),
                written_size: Some(report.written_size),
            }),
        )))
    }

    pub(super) fn complete_clear(
        &mut self,
        context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        let PersistenceOutcome::Save(save) = required_outcome(outcome)? else {
            return Err(anyhow!("unexpected clear-session worker outcome"));
        };
        if !save.committed() {
            return Err(anyhow!(
                "current session clear did not write a committed clear boundary"
            ));
        }
        let options = self.current.as_ref().expect("clear has options");
        stored_session::apply_snapshot_replacing_boards(
            context.input_state,
            context.measurer,
            empty_snapshot(context.input_state),
            options,
        )?;
        context
            .input_state
            .set_session_preflight_options(Some(options.clone()));
        context.input_state.clear_session_dirty();
        context.session.commit_runtime_clear(Instant::now());
        Ok(TransactionStep::Complete(Box::new(
            SessionCommandReport::Clear(RuntimeClearSessionReport {
                cleared_path: options.session_file_path(),
                persisted: true,
            }),
        )))
    }

    pub(super) fn complete_clear_tools(
        &mut self,
        context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        let PersistenceOutcome::ToolStateCleared(outcome) = required_outcome(outcome)? else {
            return Err(anyhow!("unexpected clear-tool-state worker outcome"));
        };
        let SessionCommand::ClearTools(defaults) = &self.command else {
            unreachable!()
        };
        self.apply_default_tools(context, *defaults.clone());
        Ok(TransactionStep::Complete(Box::new(
            SessionCommandReport::ClearTools(RuntimeClearToolStateReport {
                session_path: Some(self.current_path()),
                outcome: Some(outcome),
            }),
        )))
    }

    pub(super) fn complete_inspect(
        &mut self,
        _context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        let PersistenceOutcome::Inspection(inspection) = required_outcome(outcome)? else {
            return Err(anyhow!("unexpected inspection outcome"));
        };
        Ok(TransactionStep::Complete(Box::new(
            SessionCommandReport::Inspection(inspection),
        )))
    }

    pub(super) fn complete_forget(
        &mut self,
        _context: &mut SessionTransaction<'_>,
        outcome: Option<PersistenceOutcome>,
    ) -> Result<TransactionStep> {
        let PersistenceOutcome::CatalogForgotten(forgotten) = required_outcome(outcome)? else {
            return Err(anyhow!("unexpected forget outcome"));
        };
        let SessionCommand::Forget(path) = &self.command else {
            unreachable!()
        };
        Ok(TransactionStep::Complete(Box::new(
            SessionCommandReport::Forgotten(path.clone(), forgotten),
        )))
    }
}
