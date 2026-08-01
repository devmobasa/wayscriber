use super::*;

impl ClaimedAction {
    pub(crate) fn action(&self) -> TrayAction {
        self.record.action
    }

    #[cfg(test)]
    pub(crate) fn owner(&self) -> &ActionOwner {
        &self.record.owner
    }

    #[cfg(test)]
    pub(crate) fn finish(mut self, applied: bool, reason: Option<&str>) -> Result<()> {
        let lock = open_journal_lock(&self.journal.root)?;
        let current: ActionRecord = read_record(&self.path)?;
        if current != self.record {
            unlock(&lock)?;
            bail!("claimed action changed before completion");
        }
        self.record.record_revision = self
            .record
            .record_revision
            .checked_add(1)
            .ok_or_else(|| anyhow!("action revision overflow"))?;
        self.record.state = if applied {
            JournalState::Applied
        } else {
            JournalState::Abandoned {
                reason: bounded_reason(reason.unwrap_or("handler proved no effect"), 1024),
            }
        };
        write_record(&self.path, &self.record)?;
        unlock(&lock)?;
        if let ActionOwner::Command {
            command_identity, ..
        } = &self.record.owner
        {
            let prepared = PreparedAction {
                action_id: self.record.action_id.clone(),
                action_order: self.record.action_order,
                digest: self.record.payload_digest.clone(),
                path: self.path.clone(),
            };
            super::super::command::finish_command_action(
                command_identity,
                &prepared,
                applied,
                reason,
            )?;
        }
        let lock = open_journal_lock(&self.journal.root)?;
        let terminal: ActionRecord = read_record(&self.path)?;
        if terminal != self.record {
            unlock(&lock)?;
            bail!("terminal action changed before collection");
        }
        fs::remove_file(&self.path)?;
        unlock(&lock)
    }

    pub(crate) fn try_finish(
        mut self,
        applied: bool,
        reason: Option<&str>,
    ) -> Result<ActionFinishOutcome> {
        if matches!(self.record.state, JournalState::Claimed { .. }) {
            let Some(lock) = try_open_journal_lock(&self.journal.root, true)? else {
                return Ok(ActionFinishOutcome::Deferred(self));
            };
            let current: ActionRecord = read_record(&self.path)?;
            if current != self.record {
                unlock(&lock)?;
                bail!("claimed action changed before completion");
            }
            self.record.record_revision = self
                .record
                .record_revision
                .checked_add(1)
                .ok_or_else(|| anyhow!("action revision overflow"))?;
            self.record.state = if applied {
                JournalState::Applied
            } else {
                JournalState::Abandoned {
                    reason: bounded_reason(reason.unwrap_or("handler proved no effect"), 1024),
                }
            };
            write_record(&self.path, &self.record)?;
            unlock(&lock)?;
        }

        if let ActionOwner::Command {
            command_identity, ..
        } = &self.record.owner
        {
            let prepared = PreparedAction {
                action_id: self.record.action_id.clone(),
                action_order: self.record.action_order,
                digest: self.record.payload_digest.clone(),
                path: self.path.clone(),
            };
            let result = if applied {
                super::super::command::CommandActionResult::Applied
            } else {
                super::super::command::CommandActionResult::NoEffect
            };
            if !super::super::command::try_finish_command_action(
                command_identity,
                &prepared,
                result,
                reason,
            )? {
                return Ok(ActionFinishOutcome::Deferred(self));
            }
        }

        let Some(lock) = try_open_journal_lock(&self.journal.root, true)? else {
            return Ok(ActionFinishOutcome::Deferred(self));
        };
        let terminal: ActionRecord = read_record(&self.path)?;
        if terminal != self.record {
            unlock(&lock)?;
            bail!("terminal action changed before collection");
        }
        fs::remove_file(&self.path)?;
        unlock(&lock)?;
        Ok(ActionFinishOutcome::Complete)
    }
}
