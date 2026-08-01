use super::*;

impl ActionJournal {
    pub(super) fn claim_next_inner(
        &self,
        expected_daemon_token: &str,
        nonblocking: bool,
        mut command_eligible: impl FnMut(
            &str,
            &PreparedAction,
        )
            -> Result<Option<super::super::command::CommandActionClaim>>,
    ) -> Result<ActionClaimOutcome> {
        validate_token(expected_daemon_token)?;
        let Some(lock) = try_open_journal_lock(&self.root, nonblocking)? else {
            return Ok(ActionClaimOutcome::Deferred);
        };
        let raw_entries = fs::read_dir(queue_dir(&self.root))?
            .take(MAX_ACTIONS + 1)
            .collect::<io::Result<Vec<_>>>()?;
        if raw_entries.len() > MAX_ACTIONS {
            unlock(&lock)?;
            bail!("action journal exceeds its bounded capacity");
        }
        let mut entries = BTreeMap::new();
        let mut duplicate_orders = BTreeSet::new();
        for entry in raw_entries {
            let path = entry.path();
            let identity_on_disk = inode_identity(&path)?;
            let name = match entry.file_name().into_string() {
                Ok(name) => name,
                Err(_) => {
                    quarantine_action(&self.root, &path, identity_on_disk)?;
                    continue;
                }
            };
            let (order, identity) = match parse_action_name(&name) {
                Ok(parsed) => parsed,
                Err(_) => {
                    quarantine_action(&self.root, &path, identity_on_disk)?;
                    continue;
                }
            };
            if duplicate_orders.contains(&order) {
                quarantine_action(&self.root, &path, identity_on_disk)?;
                continue;
            }
            if let Some((_identity, previous_path, previous_inode)) =
                entries.insert(order, (identity, path.clone(), identity_on_disk))
            {
                entries.remove(&order);
                duplicate_orders.insert(order);
                quarantine_action(&self.root, &previous_path, previous_inode)?;
                quarantine_action(&self.root, &path, identity_on_disk)?;
            }
        }
        for (order, (identity, path, identity_on_disk)) in entries {
            let mut record: ActionRecord = match read_record(&path) {
                Ok(record) => record,
                Err(_) => {
                    quarantine_action(&self.root, &path, identity_on_disk)?;
                    continue;
                }
            };
            if validate_record(&record).is_err() {
                quarantine_action(&self.root, &path, identity_on_disk)?;
                continue;
            }
            if record.action_id != identity || record.action_order != order {
                quarantine_action(&self.root, &path, identity_on_disk)?;
                continue;
            }
            let prepared = PreparedAction {
                action_id: record.action_id.clone(),
                action_order: record.action_order,
                digest: record.payload_digest.clone(),
                path: path.clone(),
            };
            let owner_token = match &record.owner {
                ActionOwner::Anonymous { daemon_token }
                | ActionOwner::Command { daemon_token, .. } => daemon_token,
            };
            if owner_token != expected_daemon_token {
                if matches!(
                    record.state,
                    JournalState::Applied
                        | JournalState::Abandoned { .. }
                        | JournalState::Indeterminate { .. }
                ) {
                    fs::remove_file(&path)?;
                } else {
                    record.record_revision = record
                        .record_revision
                        .checked_add(1)
                        .ok_or_else(|| anyhow!("action revision overflow"))?;
                    record.state = if matches!(record.state, JournalState::Claimed { .. }) {
                        JournalState::Indeterminate {
                            reason: "claimed action belonged to a previous daemon generation"
                                .into(),
                        }
                    } else {
                        JournalState::Abandoned {
                            reason: "action belonged to a previous daemon generation".into(),
                        }
                    };
                    write_record(&path, &record)?;
                }
                continue;
            }

            if nonblocking && matches!(record.state, JournalState::Claimed { .. }) {
                let reason =
                    "previous overlay delivery ended after durable claim; outcome is indeterminate";
                record.record_revision = record
                    .record_revision
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("action revision overflow"))?;
                record.state = JournalState::Indeterminate {
                    reason: reason.into(),
                };
                write_record(&path, &record)?;
            }

            let eligible = match (&record.owner, &record.state) {
                (ActionOwner::Anonymous { .. }, JournalState::Eligible) => true,
                (
                    ActionOwner::Command {
                        command_identity, ..
                    },
                    JournalState::Prepared,
                ) => match command_eligible(command_identity, &prepared)? {
                    Some(super::super::command::CommandActionClaim::Claimed) => true,
                    Some(super::super::command::CommandActionClaim::Barrier) => {
                        unlock(&lock)?;
                        return Ok(ActionClaimOutcome::Idle);
                    }
                    Some(super::super::command::CommandActionClaim::Abandon(reason)) => {
                        record.record_revision = record
                            .record_revision
                            .checked_add(1)
                            .ok_or_else(|| anyhow!("action revision overflow"))?;
                        record.state = JournalState::Abandoned {
                            reason: reason.clone(),
                        };
                        write_record(&path, &record)?;
                        let finished = if nonblocking {
                            super::super::command::try_finish_command_action(
                                command_identity,
                                &prepared,
                                super::super::command::CommandActionResult::NoEffect,
                                Some(&reason),
                            )?
                        } else {
                            super::super::command::finish_command_action(
                                command_identity,
                                &prepared,
                                false,
                                Some(&reason),
                            )?;
                            true
                        };
                        if !finished {
                            unlock(&lock)?;
                            return Ok(ActionClaimOutcome::Deferred);
                        }
                        fs::remove_file(&path)?;
                        continue;
                    }
                    None => {
                        unlock(&lock)?;
                        return Ok(ActionClaimOutcome::Deferred);
                    }
                },
                (
                    ActionOwner::Command {
                        command_identity, ..
                    },
                    JournalState::Applied,
                ) => {
                    let finished = if nonblocking {
                        super::super::command::try_finish_command_action(
                            command_identity,
                            &prepared,
                            super::super::command::CommandActionResult::Applied,
                            None,
                        )?
                    } else {
                        super::super::command::finish_command_action(
                            command_identity,
                            &prepared,
                            true,
                            None,
                        )?;
                        true
                    };
                    if !finished {
                        unlock(&lock)?;
                        return Ok(ActionClaimOutcome::Deferred);
                    }
                    fs::remove_file(&path)?;
                    continue;
                }
                (
                    ActionOwner::Command {
                        command_identity, ..
                    },
                    JournalState::Abandoned { reason },
                ) => {
                    let finished = if nonblocking {
                        super::super::command::try_finish_command_action(
                            command_identity,
                            &prepared,
                            super::super::command::CommandActionResult::NoEffect,
                            Some(reason),
                        )?
                    } else {
                        super::super::command::finish_command_action(
                            command_identity,
                            &prepared,
                            false,
                            Some(reason),
                        )?;
                        true
                    };
                    if !finished {
                        unlock(&lock)?;
                        return Ok(ActionClaimOutcome::Deferred);
                    }
                    fs::remove_file(&path)?;
                    continue;
                }
                (
                    ActionOwner::Command {
                        command_identity, ..
                    },
                    JournalState::Indeterminate { reason },
                ) => {
                    let finished = if nonblocking {
                        super::super::command::try_finish_command_action(
                            command_identity,
                            &prepared,
                            super::super::command::CommandActionResult::Indeterminate,
                            Some(reason),
                        )?
                    } else {
                        super::super::command::finish_command_action_indeterminate(
                            command_identity,
                            &prepared,
                            reason,
                        )?;
                        true
                    };
                    if !finished {
                        unlock(&lock)?;
                        return Ok(ActionClaimOutcome::Deferred);
                    }
                    fs::remove_file(&path)?;
                    continue;
                }
                (
                    _,
                    JournalState::Abandoned { .. }
                    | JournalState::Indeterminate { .. }
                    | JournalState::Applied,
                ) => {
                    fs::remove_file(&path)?;
                    continue;
                }
                _ => false,
            };
            if !eligible {
                // Prepared command actions are global-order barriers until the
                // command owner makes the exact action eligible or records a
                // terminal tombstone. Later anonymous actions must not pass.
                unlock(&lock)?;
                return Ok(ActionClaimOutcome::Idle);
            }
            record.record_revision = record
                .record_revision
                .checked_add(1)
                .ok_or_else(|| anyhow!("action revision overflow"))?;
            record.state = JournalState::Claimed {
                claim_generation: fresh_id()?,
            };
            write_record(&path, &record)?;
            unlock(&lock)?;
            return Ok(ActionClaimOutcome::Claimed(ClaimedAction {
                journal: self.clone(),
                record,
                path,
            }));
        }
        unlock(&lock)?;
        Ok(ActionClaimOutcome::Idle)
    }
}
