use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::thread::{self, JoinHandle};

use super::{
    RecoveryIoCommand, RecoveryIoCompletion, RecoveryIoOperation, RecoveryIoResult,
    RuntimeStateFailurePathEffect, RuntimeStateInspectionError, RuntimeStateIoError,
    RuntimeUiStateInspection, RuntimeUiStateStore, SourceMutationId, SourceMutationRequest,
    SourceMutationResult,
};

const WRITER_COMMAND_CAPACITY: usize = 32;

#[derive(Debug)]
pub(crate) enum RuntimeStateWriterCommand {
    SourceMutation(SourceMutationRequest),
    Recovery(RecoveryIoCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeStateWriterCompletion {
    SourceMutation(SourceMutationResult),
    Recovery(RecoveryIoCompletion),
}

#[derive(Debug)]
pub(crate) enum RuntimeStateWriterSubmitError {
    Full(Box<RuntimeStateWriterCommand>),
    Disconnected(Box<RuntimeStateWriterCommand>),
}

#[derive(Debug)]
pub(crate) struct RuntimeUiStateWriter {
    commands: Option<SyncSender<RuntimeStateWriterCommand>>,
    completions: Receiver<RuntimeStateWriterCompletion>,
    worker: Option<JoinHandle<()>>,
}

impl RuntimeUiStateWriter {
    pub(crate) fn spawn(store: RuntimeUiStateStore) -> std::io::Result<Self> {
        Self::spawn_with_completion_notifier(store, || {})
    }

    pub(crate) fn spawn_with_completion_notifier(
        store: RuntimeUiStateStore,
        notify_completion: impl Fn() + Send + 'static,
    ) -> std::io::Result<Self> {
        let (command_tx, command_rx) = sync_channel(WRITER_COMMAND_CAPACITY);
        let (completion_tx, completion_rx) = sync_channel(WRITER_COMMAND_CAPACITY);
        let worker = thread::Builder::new()
            .name("wayscriber-runtime-state-writer".to_string())
            .spawn(move || {
                run_writer(
                    store,
                    command_rx,
                    completion_tx,
                    Box::new(notify_completion),
                )
            })?;
        Ok(Self {
            commands: Some(command_tx),
            completions: completion_rx,
            worker: Some(worker),
        })
    }

    /// A successful return is the acceptance boundary. Every accepted command
    /// is executed serially and causes exactly one completion to be emitted.
    pub(crate) fn submit(
        &self,
        command: RuntimeStateWriterCommand,
    ) -> Result<(), RuntimeStateWriterSubmitError> {
        let Some(commands) = &self.commands else {
            return Err(RuntimeStateWriterSubmitError::Disconnected(Box::new(
                command,
            )));
        };
        match commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(command)) => {
                Err(RuntimeStateWriterSubmitError::Full(Box::new(command)))
            }
            Err(TrySendError::Disconnected(command)) => Err(
                RuntimeStateWriterSubmitError::Disconnected(Box::new(command)),
            ),
        }
    }

    pub(crate) fn recv(&self) -> Result<RuntimeStateWriterCompletion, std::sync::mpsc::RecvError> {
        self.completions.recv()
    }

    pub(crate) fn try_recv(&self) -> Result<RuntimeStateWriterCompletion, TryRecvError> {
        self.completions.try_recv()
    }

    pub(crate) fn shutdown(mut self) {
        self.commands.take();
        while self.completions.recv().is_ok() {}
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for RuntimeUiStateWriter {
    fn drop(&mut self) {
        self.commands.take();
        while self.completions.recv().is_ok() {}
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_writer(
    store: RuntimeUiStateStore,
    commands: Receiver<RuntimeStateWriterCommand>,
    completions: SyncSender<RuntimeStateWriterCompletion>,
    notify_completion: Box<dyn Fn() + Send>,
) {
    while let Ok(command) = commands.recv() {
        let completion = execute_catching_panics(&store, command);
        if completions.send(completion).is_err() {
            break;
        }
        if catch_unwind(AssertUnwindSafe(&notify_completion)).is_err() {
            log::error!("Runtime UI completion notifier panicked; writer will continue");
        }
    }
}

fn execute_catching_panics(
    store: &RuntimeUiStateStore,
    command: RuntimeStateWriterCommand,
) -> RuntimeStateWriterCompletion {
    match command {
        RuntimeStateWriterCommand::SourceMutation(request) => {
            let id = request.id;
            let result = catch_unwind(AssertUnwindSafe(|| store.execute_source_mutation(request)))
                .unwrap_or_else(|_| panic_source_failure(store, id));
            RuntimeStateWriterCompletion::SourceMutation(result)
        }
        RuntimeStateWriterCommand::Recovery(command) => {
            let identity = RecoveryCompletionIdentity::from(&command);
            let result = catch_unwind(AssertUnwindSafe(|| execute_recovery(store, command)))
                .unwrap_or_else(|_| identity.panic_result(store));
            RuntimeStateWriterCompletion::Recovery(identity.complete(result))
        }
    }
}

fn execute_recovery(store: &RuntimeUiStateStore, command: RecoveryIoCommand) -> RecoveryIoResult {
    match command.operation {
        RecoveryIoOperation::Inspect => RecoveryIoResult::Inspected(
            store
                .inspect()
                .map(RuntimeUiStateInspection::into_recovery_inspection),
        ),
        RecoveryIoOperation::PreserveInvalidIfUnchanged {
            mutation_id,
            confirmation,
        } => RecoveryIoResult::SourceMutation(
            store.execute_preserve_invalid(mutation_id, confirmation.revision().clone()),
        ),
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => {
            RecoveryIoResult::SourceMutation(store.execute_source_mutation(request))
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RecoveryCompletionIdentity {
    controller_id: super::ControllerId,
    incident: super::PersistenceIncidentId,
    barrier: super::ControllerBarrierId,
    attempt: super::RecoveryAttemptId,
    command_id: super::RecoveryCommandId,
    mutation_id: Option<SourceMutationId>,
}

impl From<&RecoveryIoCommand> for RecoveryCompletionIdentity {
    fn from(command: &RecoveryIoCommand) -> Self {
        let mutation_id = match &command.operation {
            RecoveryIoOperation::Inspect => None,
            RecoveryIoOperation::PreserveInvalidIfUnchanged { mutation_id, .. } => {
                Some(*mutation_id)
            }
            RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => Some(request.id),
        };
        Self {
            controller_id: command.controller_id,
            incident: command.incident,
            barrier: command.barrier,
            attempt: command.attempt,
            command_id: command.command_id,
            mutation_id,
        }
    }
}

impl RecoveryCompletionIdentity {
    fn complete(self, result: RecoveryIoResult) -> RecoveryIoCompletion {
        RecoveryIoCompletion {
            controller_id: self.controller_id,
            incident: self.incident,
            barrier: self.barrier,
            attempt: self.attempt,
            command_id: self.command_id,
            result,
        }
    }

    fn panic_result(self, store: &RuntimeUiStateStore) -> RecoveryIoResult {
        match self.mutation_id {
            Some(id) => RecoveryIoResult::SourceMutation(panic_source_failure(store, id)),
            None => RecoveryIoResult::Inspected(Err(RuntimeStateInspectionError::new(
                "runtime-state writer panicked while inspecting",
            ))),
        }
    }
}

fn panic_source_failure(store: &RuntimeUiStateStore, id: SourceMutationId) -> SourceMutationResult {
    SourceMutationResult::Failed {
        id,
        error: RuntimeStateIoError::new("runtime-state writer panicked while mutating storage"),
        active: store
            .inspect()
            .ok()
            .map(|inspection| inspection.observation),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::UnknownAfterMutation,
    }
}

#[cfg(test)]
mod tests;
