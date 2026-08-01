use super::*;

impl ActionJournal {
    pub(crate) fn abandon(&self, prepared: &PreparedAction, reason: &str) -> Result<()> {
        let lock = open_journal_lock(&self.root)?;
        let mut record: ActionRecord = read_record(&prepared.path)?;
        if record.action_id != prepared.action_id
            || record.action_order != prepared.action_order
            || record.payload_digest != prepared.digest
            || !matches!(
                record.state,
                JournalState::Prepared | JournalState::Eligible
            )
        {
            unlock(&lock)?;
            bail!("cannot abandon changed or claimed action");
        }
        record.record_revision = record
            .record_revision
            .checked_add(1)
            .ok_or_else(|| anyhow!("action revision overflow"))?;
        record.state = JournalState::Abandoned {
            reason: bounded_reason(reason, 1024),
        };
        write_record(&prepared.path, &record)?;
        unlock(&lock)
    }

    pub(crate) fn abandon_command(
        &self,
        command_identity: &str,
        prepared: &PreparedAction,
        reason: &str,
    ) -> Result<()> {
        self.abandon(prepared, reason)?;
        super::super::command::finish_command_action(
            command_identity,
            prepared,
            false,
            Some(reason),
        )?;
        let lock = open_journal_lock(&self.root)?;
        let terminal: ActionRecord = read_record(&prepared.path)?;
        if terminal.action_id != prepared.action_id
            || terminal.action_order != prepared.action_order
            || terminal.payload_digest != prepared.digest
            || !matches!(terminal.state, JournalState::Abandoned { .. })
        {
            unlock(&lock)?;
            bail!("abandoned command action changed before collection");
        }
        fs::remove_file(&prepared.path)?;
        unlock(&lock)
    }
}
