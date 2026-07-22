use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoardSeedUpdate {
    Applied,
    Staged,
    Rejected,
}

impl ToolbarRuntimeState {
    fn update_board_seeds(&mut self, config: &Config) -> BoardSeedUpdate {
        let seeds = match runtime_seeds_from_config(config, &self.board_pin_seeds) {
            Ok(seeds) => seeds,
            Err(error) => {
                log::error!("Board runtime seed update was invalid: {error:#}");
                return BoardSeedUpdate::Rejected;
            }
        };
        let outcome = match self.controller.update_seeds(seeds) {
            UpdateSeedsResult::Applied { .. } => BoardSeedUpdate::Applied,
            UpdateSeedsResult::StagedBehindBarrier { barrier, .. } => {
                log::debug!("Board runtime seed update staged behind barrier {barrier:?}");
                BoardSeedUpdate::Staged
            }
            UpdateSeedsResult::Rejected(error) => {
                log::warn!("Board runtime seed update rejected: {error:?}");
                BoardSeedUpdate::Rejected
            }
            UpdateSeedsResult::RejectedPersistence(error) => {
                log::warn!("Board runtime seed cleanup rejected: {error:?}");
                BoardSeedUpdate::Rejected
            }
            UpdateSeedsResult::RejectedShuttingDown => {
                log::debug!("Board runtime seed update ignored during shutdown");
                BoardSeedUpdate::Rejected
            }
        };
        self.dispatch_writer_command();
        outcome
    }

    pub(in crate::backend::wayland) fn remove_board_identity(
        &mut self,
        config: &Config,
        board_id: &str,
    ) {
        self.deferred_board_pin_restores.remove(board_id);
        let previous = self.board_pin_seeds.remove(board_id);
        if self.update_board_seeds(config) == BoardSeedUpdate::Rejected
            && let Some(seed) = previous
        {
            self.board_pin_seeds.insert(board_id.to_string(), seed);
        }
    }

    pub(in crate::backend::wayland) fn restore_board_identity(
        &mut self,
        config: &Config,
        input: &mut InputState,
        board_id: String,
        pin_seed: bool,
        pinned: bool,
    ) -> Option<ToolbarRuntimeFinish> {
        // This path is used for identities created by live board operations,
        // including undo restoration. An entry retained provisionally for a
        // later session restore belongs to an older identity even when its ID
        // and seed happen to match, so force a remove/re-add transition before
        // capturing the new identity's retained pin.
        self.remove_board_identity(config, &board_id);
        let previous = self.board_pin_seeds.insert(board_id.clone(), pin_seed);
        match self.update_board_seeds(config) {
            BoardSeedUpdate::Applied => {}
            BoardSeedUpdate::Staged => {
                if pinned != pin_seed {
                    self.deferred_board_pin_restores.insert(
                        board_id.clone(),
                        DeferredBoardPinRestore {
                            board_id,
                            board_identity_generation: input.boards.board_identity_generation(),
                            pin_seed,
                            pinned,
                            authority_epoch: self.controller.authority_epoch(),
                        },
                    );
                } else {
                    input.apply_board_pinned_runtime(&board_id, pin_seed);
                }
                return None;
            }
            BoardSeedUpdate::Rejected => {
                match previous {
                    Some(previous) => {
                        self.board_pin_seeds.insert(board_id.clone(), previous);
                    }
                    None => {
                        self.board_pin_seeds.remove(&board_id);
                    }
                }
                input.apply_board_pinned_runtime(&board_id, pin_seed);
                return None;
            }
        }
        if pinned == pin_seed {
            input.apply_board_pinned_runtime(&board_id, pin_seed);
            return None;
        }
        self.commit_board_pin_value(input, board_id, pin_seed, pinned)
    }

    fn commit_board_pin_value(
        &mut self,
        input: &mut InputState,
        board_id: String,
        current: bool,
        desired: bool,
    ) -> Option<ToolbarRuntimeFinish> {
        let target = InteractionSeedTarget::BoardPin(board_id.clone());
        let rollback = PreviewRollbackSnapshot {
            values: [(target.clone(), InteractionSeedValue::Bool(current))]
                .into_iter()
                .collect(),
        };
        let session = match self
            .controller
            .begin_runtime_preview(RuntimeUiMutationScope::one(target.clone()), rollback)
        {
            Ok(session) => session,
            Err(error) => {
                log::warn!("Restored board pin runtime mutation blocked: {error:?}");
                input.apply_board_pinned_runtime(&board_id, current);
                return None;
            }
        };
        input.apply_board_pinned_runtime(&board_id, desired);
        let values = RuntimeUiMutationValues::one(target, InteractionSeedValue::Bool(desired))
            .expect("board pin value matches its runtime target");
        let result = self.controller.finish_preview(
            PreviewFinishRequest::RuntimeUi {
                session,
                intent: RuntimePreviewFinishIntent::Commit(values),
            },
            |_, _| unreachable!("runtime board mutation cannot write config"),
        );
        Some(self.finish_result(result))
    }

    pub(in crate::backend::wayland) fn finish_deferred_board_pin_restores(
        &mut self,
        input: &mut InputState,
    ) -> Vec<ToolbarRuntimeFinish> {
        if self.controller.active_barrier().is_some() {
            return Vec::new();
        }

        let current_generation = input.boards.board_identity_generation();
        let current_epoch = self.controller.authority_epoch();
        let deferred = std::mem::take(&mut self.deferred_board_pin_restores);
        let mut finishes = Vec::new();
        for restore in deferred.into_values() {
            if restore.board_identity_generation != current_generation
                || restore.authority_epoch != current_epoch
                || self.board_pin_seeds.get(&restore.board_id).copied() != Some(restore.pin_seed)
                || input.boards.pin_seed(&restore.board_id) != Some(restore.pin_seed)
                || !input.boards.has_board(&restore.board_id)
            {
                continue;
            }
            let target = InteractionSeedTarget::BoardPin(restore.board_id.clone());
            let Some(InteractionSeedValue::Bool(current)) =
                self.controller.live_state().get(&target)
            else {
                continue;
            };
            let current = *current;
            if current == restore.pinned {
                input.apply_board_pinned_runtime(&restore.board_id, restore.pinned);
                continue;
            }
            if let Some(finish) =
                self.commit_board_pin_value(input, restore.board_id, current, restore.pinned)
            {
                finishes.push(finish);
            }
        }
        finishes
    }

    pub(in crate::backend::wayland) fn begin_board_pin_toggle(
        &mut self,
        config: &Config,
        board_id: String,
        pin_seed: bool,
        current: bool,
    ) -> Option<PreparedBoardPinMutation> {
        match self.board_pin_seeds.get(&board_id).copied() {
            Some(seed) if seed != pin_seed => {
                log::debug!("Ignoring board pin request captured under an old authored seed");
                return None;
            }
            Some(_) => {}
            None => {
                self.board_pin_seeds.insert(board_id.clone(), pin_seed);
                match self.update_board_seeds(config) {
                    BoardSeedUpdate::Applied => {}
                    BoardSeedUpdate::Staged => return None,
                    BoardSeedUpdate::Rejected => {
                        self.board_pin_seeds.remove(&board_id);
                        return None;
                    }
                }
            }
        }
        let target = InteractionSeedTarget::BoardPin(board_id.clone());
        let rollback = PreviewRollbackSnapshot {
            values: [(target.clone(), InteractionSeedValue::Bool(current))]
                .into_iter()
                .collect(),
        };
        let session = match self
            .controller
            .begin_runtime_preview(RuntimeUiMutationScope::one(target), rollback)
        {
            Ok(session) => session,
            Err(error) => {
                log::warn!("Board pin runtime mutation blocked: {error:?}");
                return None;
            }
        };
        Some(PreparedBoardPinMutation {
            board_id,
            desired: !current,
            session,
        })
    }

    pub(in crate::backend::wayland) fn finish_board_pin_toggle(
        &mut self,
        prepared: PreparedBoardPinMutation,
        applied: bool,
    ) -> ToolbarRuntimeFinish {
        let intent = if applied {
            let target = InteractionSeedTarget::BoardPin(prepared.board_id);
            let values =
                RuntimeUiMutationValues::one(target, InteractionSeedValue::Bool(prepared.desired))
                    .expect("board pin value matches its runtime target");
            RuntimePreviewFinishIntent::Commit(values)
        } else {
            RuntimePreviewFinishIntent::Cancel
        };
        let result = self.controller.finish_preview(
            PreviewFinishRequest::RuntimeUi {
                session: prepared.session,
                intent,
            },
            |_, _| unreachable!("runtime board mutation cannot write config"),
        );
        self.finish_result(result)
    }
}
