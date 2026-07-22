use std::path::{Path, PathBuf};

use super::{
    PersistenceIncidentId, RecoveryInspection, RuntimeStateFailurePathEffect,
    RuntimeStateInspectionError, RuntimeStateIoError, RuntimeStateObservedPathEffect,
    RuntimeStateSourceObservation, RuntimeStateSourceRevision, RuntimeUiFileStatus,
    RuntimeUiStateController, RuntimeUiWireState, ValidatedInteractionSeeds,
};

mod fs;
mod inspection;
mod mutation;

const MAX_RUNTIME_UI_FILE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeUiStateStore {
    path: PathBuf,
}

impl RuntimeUiStateStore {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn inspect(&self) -> Result<RuntimeUiStateInspection, RuntimeStateInspectionError> {
        inspection::inspect_path(&self.path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeUiStateInspection {
    pub(crate) observation: RuntimeStateSourceObservation,
    pub(crate) status: RuntimeUiFileStatus,
    pub(crate) supported_wire: Option<RuntimeUiWireState>,
}

impl RuntimeUiStateInspection {
    pub(crate) fn into_recovery_inspection(self) -> RecoveryInspection {
        RecoveryInspection::new(self.observation, self.supported_wire)
    }

    pub(crate) fn into_controller_bootstrap(
        self,
        seeds: ValidatedInteractionSeeds,
    ) -> RuntimeUiStateControllerBootstrap {
        if matches!(self.status, RuntimeUiFileStatus::Invalid) {
            let (controller, incident) = RuntimeUiStateController::new_startup_unhealthy(
                seeds,
                self.observation,
                RuntimeStateIoError::new("startup runtime-state file is malformed"),
                Vec::new(),
                RuntimeStateFailurePathEffect::Known(RuntimeStateObservedPathEffect::Untouched),
            );
            return RuntimeUiStateControllerBootstrap {
                controller,
                startup_incident: Some(incident),
            };
        }
        RuntimeUiStateControllerBootstrap {
            controller: RuntimeUiStateController::new_with_authority(
                seeds,
                self.observation.revision,
                self.status,
                self.supported_wire.unwrap_or_default(),
            ),
            startup_incident: None,
        }
    }

    fn missing(revision: RuntimeStateSourceRevision) -> Self {
        Self {
            observation: RuntimeStateSourceObservation::missing(revision),
            status: RuntimeUiFileStatus::Missing,
            supported_wire: None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RuntimeUiStateControllerBootstrap {
    pub(crate) controller: RuntimeUiStateController,
    pub(crate) startup_incident: Option<PersistenceIncidentId>,
}

#[cfg(test)]
mod tests;
