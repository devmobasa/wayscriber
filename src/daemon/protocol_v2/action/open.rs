use super::*;

impl ActionJournal {
    pub(crate) fn open() -> Result<Self> {
        let root = action_root();
        create_private_directory(&root)?;
        create_private_directory(&queue_dir(&root))?;
        create_private_directory(&quarantine_dir(&root))?;
        // Keep only a tail of quarantined garbage: before this collection, a
        // full quarantine made every open fail, and under Restart=on-failure
        // that was a permanent crash-restart loop.
        super::super::linux::gc_quarantine_tail(
            &quarantine_dir(&root),
            super::super::linux::QUARANTINE_RETAINED_ENTRIES,
        )?;
        if fs::read_dir(quarantine_dir(&root))?
            .take(MAX_ACTION_QUARANTINE + 1)
            .count()
            >= MAX_ACTION_QUARANTINE
        {
            bail!("action quarantine capacity exhausted");
        }
        Ok(Self { root })
    }

    #[cfg(test)]
    pub(crate) fn fail_next_anonymous_publications(&self, count: usize) {
        let mut failures = ANONYMOUS_PUBLISH_FAILURES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if count == 0 {
            failures.remove(&self.root);
        } else {
            failures.insert(self.root.clone(), count);
        }
    }

    pub(crate) fn quiesce_for_rollback(&self, compatibility_token: &str) -> Result<()> {
        validate_token(compatibility_token)?;
        let lock = open_journal_lock(&self.root)?;
        let mut entries = BTreeMap::new();
        for entry in fs::read_dir(queue_dir(&self.root))?.take(MAX_ACTIONS + 1) {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow!("action filename is not UTF-8"))?;
            let (order, identity) = parse_action_name(&name)?;
            if entries.insert(order, (identity, entry.path())).is_some() {
                unlock(&lock)?;
                bail!("duplicate action journal order during rollback");
            }
        }
        if entries.len() > MAX_ACTIONS {
            unlock(&lock)?;
            bail!("action journal capacity prevents bounded rollback");
        }
        for (order, (identity, path)) in entries {
            let mut record: ActionRecord = read_record(&path)?;
            validate_record(&record)?;
            if record.action_id != identity || record.action_order != order {
                unlock(&lock)?;
                bail!("action filename changed during rollback");
            }
            let owner_token = match &record.owner {
                ActionOwner::Anonymous { daemon_token }
                | ActionOwner::Command { daemon_token, .. } => daemon_token,
            };
            if owner_token == compatibility_token
                && !matches!(
                    record.state,
                    JournalState::Applied
                        | JournalState::Abandoned { .. }
                        | JournalState::Indeterminate { .. }
                )
            {
                unlock(&lock)?;
                bail!("rollback compatibility generation owns live v2 action work");
            }
            if !matches!(
                record.state,
                JournalState::Applied
                    | JournalState::Abandoned { .. }
                    | JournalState::Indeterminate { .. }
            ) {
                let reason = if matches!(record.state, JournalState::Claimed { .. }) {
                    "rollback preserved a claimed action with indeterminate delivery"
                } else {
                    "rollback rejected an action before delivery"
                };
                record.record_revision = record
                    .record_revision
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("action revision overflow"))?;
                record.state = if matches!(record.state, JournalState::Claimed { .. }) {
                    JournalState::Indeterminate {
                        reason: reason.into(),
                    }
                } else {
                    JournalState::Abandoned {
                        reason: reason.into(),
                    }
                };
                write_record(&path, &record)?;
            }
        }
        unlock(&lock)
    }

    fn allocate_order(&self) -> Result<u64> {
        let path = self.root.join("high-water.json");
        let now = BootClock::now()?.as_nanos();
        let boot_id = BootIdentity::read()?.as_str().to_owned();
        let namespace = NamespaceIdentity::current_time()?;
        let previous = match fs::symlink_metadata(&path) {
            Ok(_) => Some(read_record::<JournalHighWater>(&path)?),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let last = match previous {
            Some(previous)
                if previous.protocol_version == ACTION_ENVELOPE_PROTOCOL_VERSION
                    && previous.boot_id == boot_id
                    && previous.time_namespace_dev == namespace.dev
                    && previous.time_namespace_ino == namespace.ino =>
            {
                previous.last_order
            }
            Some(_) => bail!("action journal boot identity changed"),
            None => {
                if fs::read_dir(queue_dir(&self.root))?.next().is_some() {
                    bail!("missing action high-water record for nonempty journal");
                }
                0
            }
        };
        let order = now.max(
            last.checked_add(1)
                .ok_or_else(|| anyhow!("action order overflow"))?,
        );
        write_record(
            &path,
            &JournalHighWater {
                protocol_version: ACTION_ENVELOPE_PROTOCOL_VERSION,
                boot_id,
                time_namespace_dev: namespace.dev,
                time_namespace_ino: namespace.ino,
                last_order: order,
            },
        )?;
        Ok(order)
    }

    fn publish(
        &self,
        owner: ActionOwner,
        action: TrayAction,
        state: JournalState,
    ) -> Result<PreparedAction> {
        let lock = open_journal_lock(&self.root)?;
        let count = fs::read_dir(queue_dir(&self.root))?
            .take(MAX_ACTIONS + 1)
            .count();
        if count >= MAX_ACTIONS {
            unlock(&lock)?;
            bail!("action journal capacity exhausted");
        }
        let order = self.allocate_order()?;
        let action_id = fresh_id()?;
        let digest = digest_payload(&action_id, order, &owner, action)?;
        let path = queue_dir(&self.root).join(action_name(order, &action_id));
        let record = ActionRecord {
            protocol_version: ACTION_ENVELOPE_PROTOCOL_VERSION,
            record_revision: 1,
            action_id: action_id.clone(),
            action_order: order,
            owner,
            action,
            payload_digest: digest.clone(),
            state,
        };
        validate_record(&record)?;
        write_record(&path, &record)?;
        unlock(&lock)?;
        Ok(PreparedAction {
            action_id,
            action_order: order,
            digest,
            path,
        })
    }

    pub(crate) fn prepare_command(
        &self,
        command_identity: &str,
        daemon_token: &str,
        action: TrayAction,
    ) -> Result<PreparedAction> {
        validate_id(command_identity)?;
        validate_token(daemon_token)?;
        self.publish(
            ActionOwner::Command {
                command_identity: command_identity.to_owned(),
                daemon_token: daemon_token.to_owned(),
            },
            action,
            JournalState::Prepared,
        )
    }

    pub(crate) fn publish_anonymous(
        &self,
        daemon_token: &str,
        action: TrayAction,
    ) -> Result<PreparedAction> {
        #[cfg(test)]
        if consume_anonymous_publish_failure(&self.root) {
            bail!("injected anonymous action admission failure");
        }
        validate_token(daemon_token)?;
        self.publish(
            ActionOwner::Anonymous {
                daemon_token: daemon_token.to_owned(),
            },
            action,
            JournalState::Eligible,
        )
    }

    #[cfg(test)]
    pub(crate) fn claim_next(
        &self,
        expected_daemon_token: &str,
        mut command_eligible: impl FnMut(&str, &PreparedAction) -> Result<bool>,
    ) -> Result<Option<ClaimedAction>> {
        match self.claim_next_inner(expected_daemon_token, false, |identity, prepared| {
            command_eligible(identity, prepared).map(|eligible| {
                Some(if eligible {
                    super::super::command::CommandActionClaim::Claimed
                } else {
                    super::super::command::CommandActionClaim::Barrier
                })
            })
        })? {
            ActionClaimOutcome::Claimed(action) => Ok(Some(action)),
            ActionClaimOutcome::Idle => Ok(None),
            ActionClaimOutcome::Deferred => {
                bail!("blocking action claim unexpectedly deferred")
            }
        }
    }

    pub(in crate::daemon::protocol_v2) fn try_claim_next(
        &self,
        expected_daemon_token: &str,
        command_eligible: impl FnMut(
            &str,
            &PreparedAction,
        )
            -> Result<Option<super::super::command::CommandActionClaim>>,
    ) -> Result<ActionClaimOutcome> {
        self.claim_next_inner(expected_daemon_token, true, command_eligible)
    }
}
