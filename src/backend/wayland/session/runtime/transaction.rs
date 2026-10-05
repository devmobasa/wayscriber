//! Session commands advance only when their disk phase completes.
use super::*;

mod output;
mod phases;

#[derive(Debug)]
pub(in crate::backend::wayland) struct SessionCommandAborted(&'static str);

impl std::fmt::Display for SessionCommandAborted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for SessionCommandAborted {}

#[derive(Debug)]
pub(in crate::backend::wayland) enum SessionCommand {
    Open(PathBuf),
    Output {
        transition: Box<super::super::PendingOutputTransition>,
        remembered: Option<PathBuf>,
        home: Option<Box<SessionOptions>>,
    },
    /// Return to the home session, whose options for the current output this
    /// carries; `None` when home has persistence disabled.
    OpenHome(Option<Box<SessionOptions>>),
    SaveAs(PathBuf, SaveAsOverwrite),
    CheckOverwrite(PathBuf),
    Clear,
    ClearTools(Box<ToolStateSnapshot>),
    Inspect,
    Forget(PathBuf),
}

pub(in crate::backend::wayland) enum SessionCommandReport {
    Open(RuntimeOpenSessionReport),
    /// The overlay is back in its home session.
    Home,
    Output {
        abandoned: Option<(PathBuf, anyhow::Error)>,
        too_large: Option<super::super::ExpandedTooLarge>,
        first_output_resolved: bool,
    },
    SaveAs(RuntimeSaveAsSessionReport),
    Overwrite(PathBuf, bool),
    Clear(RuntimeClearSessionReport),
    ClearTools(RuntimeClearToolStateReport),
    Inspection(stored_session::SessionInspection),
    Forgotten(PathBuf, bool),
}

pub(in crate::backend::wayland) enum TransactionStep {
    Work(Box<PersistenceOperation>),
    Complete(Box<SessionCommandReport>),
}

#[derive(Debug, Clone, Copy)]
enum Phase {
    Start,
    OutputCheckSource,
    OutputArtifacts,
    OutputLoad,
    OutputHome,
    OpenPreflight,
    SaveCurrent,
    Load,
    RecordOpen,
    LoadHome,
    SaveAsPreflight,
    SaveAs,
    Clear,
    ClearTools,
    Inspect,
    Forget,
}

pub(in crate::backend::wayland) struct QueuedSessionCommand {
    pub command: SessionCommand,
    pub epoch: u64,
    /// The edit generation the command was requested over.
    pub generation: u64,
}

impl SessionCommand {
    pub(in crate::backend::wayland::session) fn matches_request(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Open(a), Self::Open(b))
            | (Self::CheckOverwrite(a), Self::CheckOverwrite(b))
            | (Self::Forget(a), Self::Forget(b)) => a == b,
            (Self::SaveAs(a, policy), Self::SaveAs(b, other_policy)) => {
                a == b && policy == other_policy
            }
            (Self::OpenHome(_), Self::OpenHome(_))
            | (Self::Clear, Self::Clear)
            | (Self::ClearTools(_), Self::ClearTools(_))
            | (Self::Inspect, Self::Inspect) => true,
            _ => false,
        }
    }
}

pub(in crate::backend::wayland) struct SessionCommandTransaction {
    command: SessionCommand,
    pub(in crate::backend::wayland::session) queued_commands:
        std::collections::VecDeque<QueuedSessionCommand>,
    phase: Phase,
    current: Option<SessionOptions>,
    target: Option<SessionOptions>,
    epoch: u64,
    generation: Option<u64>,
    interaction: Option<(u64, bool)>,
    saved_current: bool,
    loaded_board_data: bool,
    output_snapshot: Option<SessionSnapshot>,
    abandoned: Option<(PathBuf, anyhow::Error)>,
    pub request_id: Option<RequestId>,
}

impl SessionCommandTransaction {
    pub fn new(command: SessionCommand, epoch: u64, interaction: (u64, bool)) -> Self {
        Self {
            command,
            queued_commands: std::collections::VecDeque::new(),
            phase: Phase::Start,
            current: None,
            target: None,
            epoch,
            generation: None,
            interaction: Some(interaction),
            saved_current: false,
            loaded_board_data: false,
            output_snapshot: None,
            abandoned: None,
            request_id: None,
        }
    }

    pub fn command(&self) -> &SessionCommand {
        &self.command
    }

    pub fn has_committed_open(&self) -> bool {
        matches!(self.phase, Phase::RecordOpen)
    }

    pub fn advance(
        &mut self,
        context: &mut SessionTransaction<'_>,
        result: Option<Result<PersistenceOutcome>>,
    ) -> Result<TransactionStep> {
        if self.epoch != context.session.target_epoch() {
            return Err(SessionCommandAborted(
                "session target changed while the command was pending",
            )
            .into());
        }
        if let SessionCommand::Output { transition, .. } = &self.command
            && !context
                .session
                .pending_output_transition()
                .is_some_and(|pending| {
                    pending.revision == transition.revision && pending.source_epoch == self.epoch
                })
        {
            return Err(SessionCommandAborted(
                "output destination changed while persistence was pending",
            )
            .into());
        }
        let outcome = result.transpose()?;
        if self
            .generation
            .is_some_and(|generation| generation != context.session.edit_generation())
        {
            return Err(SessionCommandAborted(
                "session was edited while the command was pending; retry the command",
            )
            .into());
        }

        if let Some((revision, active)) = self.interaction {
            let (current_revision, current_active) =
                context.input_state.session_interaction_state();
            if revision != current_revision || (!active && current_active) {
                return Err(SessionCommandAborted("input interaction changed while the session command was pending; retry the command").into());
            }
        }

        match self.phase {
            Phase::OutputCheckSource => self.complete_output_check_source(context, outcome),
            Phase::OutputArtifacts => self.complete_output_artifacts(context, outcome),
            Phase::OutputLoad | Phase::OutputHome => self.complete_output_load(context, outcome),
            Phase::Start => self.complete_start(context, outcome),
            Phase::OpenPreflight => self.complete_open_preflight(context, outcome),
            Phase::SaveCurrent => self.complete_save_current(context, outcome),
            Phase::Load => self.complete_load(context, outcome),
            Phase::RecordOpen => self.complete_record_open(context, outcome),
            Phase::LoadHome => self.complete_load_home(context, outcome),
            Phase::SaveAsPreflight => self.complete_save_as_preflight(context, outcome),
            Phase::SaveAs => self.complete_save_as(context, outcome),
            Phase::Clear => self.complete_clear(context, outcome),
            Phase::ClearTools => self.complete_clear_tools(context, outcome),
            Phase::Inspect => self.complete_inspect(context, outcome),
            Phase::Forget => self.complete_forget(context, outcome),
        }
    }

    // Catalog bookkeeping is best effort after a successful open.
    pub fn catalog_failure_report(&self, error: anyhow::Error) -> SessionCommandReport {
        debug_assert!(self.has_committed_open());
        let mut report = self.open_report();
        report.catalog_error = Some(error);

        SessionCommandReport::Open(report)
    }

    fn work(&mut self, phase: Phase, operation: PersistenceOperation) -> Result<TransactionStep> {
        self.phase = phase;
        Ok(TransactionStep::Work(Box::new(operation)))
    }

    fn capture_input_generation(&mut self, context: &SessionTransaction<'_>) {
        self.generation = Some(context.session.edit_generation());
    }

    fn save_current_or_continue(
        &mut self,
        context: &mut SessionTransaction<'_>,
    ) -> Result<TransactionStep> {
        if !context.input_state.is_session_dirty() && !context.session.is_dirty() {
            return self.after_current_save(context);
        }
        let options = self
            .current
            .as_ref()
            .expect("target change has current options")
            .clone();
        if !context
            .session
            .validate_source_write(context.input_state.is_session_dirty())?
        {
            return self.after_current_save(context);
        }
        let snapshot = context
            .input_state
            .with_active_interaction_canceled_for_capture_with(context.measurer, |input| {
                stored_session::snapshot_from_input(input, &options)
            });
        let snapshot = if let Some(snapshot) = snapshot {
            snapshot
        } else if session_persistence_enabled(&options) {
            empty_snapshot(context.input_state)
        } else {
            return Err(anyhow!(
                "current session has unsaved changes but persistence is disabled"
            ));
        };
        self.capture_input_generation(context);
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

    fn after_current_save(
        &mut self,
        context: &mut SessionTransaction<'_>,
    ) -> Result<TransactionStep> {
        match &self.command {
            SessionCommand::Output { .. } => self.load_output_target(),
            SessionCommand::Open(_) => {
                self.capture_input_generation(context);
                self.work(
                    Phase::Load,
                    PersistenceOperation::LoadNamedCandidate {
                        options: self.target.as_ref().expect("open target").clone(),
                    },
                )
            }
            // Home loads the way a launch would, so its recovery, clear and
            // tool-restore rules apply rather than those of a runtime Open.
            SessionCommand::OpenHome(Some(options)) => {
                let options = (**options).clone();
                self.capture_input_generation(context);
                self.work(Phase::LoadHome, PersistenceOperation::LoadHome { options })
            }
            SessionCommand::OpenHome(None) => {
                self.leave_persistence_for_home(context)?;
                Ok(TransactionStep::Complete(Box::new(
                    SessionCommandReport::Home,
                )))
            }
            SessionCommand::SaveAs(_, _) => Ok(TransactionStep::Complete(Box::new(
                SessionCommandReport::SaveAs(RuntimeSaveAsSessionReport {
                    previous_path: self.current_path(),
                    saved_path: self.current_path(),
                    switched_target: false,
                    saved: self.saved_current,
                    saved_board_data: context.session.has_loaded_board_data(),
                    outcome: None,
                    written_size: None,
                }),
            ))),
            _ => unreachable!(),
        }
    }

    fn current_path(&self) -> PathBuf {
        self.current
            .as_ref()
            .expect("command has current options")
            .session_file_path()
    }

    fn open_report(&self) -> RuntimeOpenSessionReport {
        RuntimeOpenSessionReport {
            previous_path: self.current_path(),
            opened_path: self
                .target
                .as_ref()
                .expect("open target")
                .session_file_path(),
            saved_current: self.saved_current,
            loaded_board_data: self.loaded_board_data,
            catalog_error: None,
        }
    }

    fn apply_default_tools(
        &self,
        context: &mut SessionTransaction<'_>,
        defaults: ToolStateSnapshot,
    ) {
        stored_session::apply_tool_state_snapshot(context.input_state, context.measurer, defaults);
        context.input_state.mark_session_dirty();
        context.session.record_input_dirty(Instant::now(), true);
    }
}

fn empty_snapshot(input: &InputState) -> SessionSnapshot {
    SessionSnapshot {
        active_board_id: input.board_id().to_string(),
        boards: Vec::new(),
        tool_state: None,
    }
}
fn required_outcome(outcome: Option<PersistenceOutcome>) -> Result<PersistenceOutcome> {
    outcome.ok_or_else(|| anyhow!("session command phase requires a completion"))
}
