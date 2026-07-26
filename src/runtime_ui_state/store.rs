use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use super::{
    ControllerId, ControllerStartupError, PersistenceIncidentId, RecoveryInspection,
    RuntimeStateFailurePathEffect, RuntimeStateInspectionError, RuntimeStateIoError,
    RuntimeStateObservedPathEffect, RuntimeStateSourceObservation, RuntimeStateSourceRevision,
    RuntimeUiFileStatus, RuntimeUiStateController, RuntimeUiWireState, ValidatedInteractionSeeds,
};

mod fs;
mod inspection;
mod mutation;

const MAX_RUNTIME_UI_FILE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeStateWriterNamespace([u8; 16]);

impl RuntimeStateWriterNamespace {
    fn generate() -> io::Result<Self> {
        let mut bytes = [0; 16];
        let mut filled = 0;
        while filled < bytes.len() {
            // SAFETY: the remaining slice is writable for its full length.
            // getrandom writes at most that length and retains no pointer.
            let result = unsafe {
                libc::getrandom(bytes[filled..].as_mut_ptr().cast(), bytes.len() - filled, 0)
            };
            if result > 0 {
                filled += result as usize;
                continue;
            }
            if result == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "getrandom returned zero bytes for runtime-state writer namespace",
                ));
            }
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        Ok(Self(bytes))
    }

    #[cfg(test)]
    const fn test_fixture(byte: u8) -> Self {
        Self([byte; 16])
    }
}

impl fmt::Display for RuntimeStateWriterNamespace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeUiStateStore {
    path: PathBuf,
    writer_namespace: RuntimeStateWriterNamespace,
}

impl RuntimeUiStateStore {
    pub(crate) fn try_new(path: impl Into<PathBuf>) -> io::Result<Self> {
        Ok(Self {
            path: path.into(),
            writer_namespace: RuntimeStateWriterNamespace::generate()?,
        })
    }

    #[cfg(test)]
    pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
        Self::with_writer_namespace(path, RuntimeStateWriterNamespace::test_fixture(0))
    }

    #[cfg(test)]
    fn with_writer_namespace(
        path: impl Into<PathBuf>,
        writer_namespace: RuntimeStateWriterNamespace,
    ) -> Self {
        Self {
            path: path.into(),
            writer_namespace,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn controller_id(&self) -> ControllerId {
        ControllerId::new(self.writer_namespace.0, 1)
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
        controller_id: ControllerId,
        seeds: ValidatedInteractionSeeds,
    ) -> Result<RuntimeUiStateControllerBootstrap, ControllerStartupError> {
        if matches!(self.status, RuntimeUiFileStatus::Invalid) {
            let (controller, incident) = RuntimeUiStateController::new_startup_unhealthy(
                controller_id,
                seeds,
                self.observation,
                RuntimeStateIoError::new("startup runtime-state file is malformed"),
                Vec::new(),
                RuntimeStateFailurePathEffect::Known(RuntimeStateObservedPathEffect::Untouched),
            )?;
            return Ok(RuntimeUiStateControllerBootstrap {
                controller,
                startup_incident: Some(incident),
            });
        }
        Ok(RuntimeUiStateControllerBootstrap {
            controller: RuntimeUiStateController::new_with_authority(
                controller_id,
                seeds,
                self.observation.revision,
                self.status,
                self.supported_wire.unwrap_or_default(),
            )?,
            startup_incident: None,
        })
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
mod namespace_tests;
#[cfg(test)]
mod tests;
