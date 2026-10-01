use crate::input::state::{Toast, ToastPriority};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use super::super::*;
use crate::backend::wayland::{
    backend::event_loop::session_save,
    session::{
        PersistenceOperation, PersistenceOutcome, RuntimeClearSessionReport,
        RuntimeClearToolStateReport, RuntimeOpenSessionReport, RuntimeSaveAsSessionReport,
    },
};
use crate::session::{
    self as stored_session, ClearToolStateOutcome, SaveAsOverwrite, ToolStateSnapshot,
};

impl WaylandState {
    pub(in crate::backend::wayland) fn open_named_session_runtime(
        &mut self,
        target_path: &Path,
    ) -> Result<RuntimeOpenSessionReport> {
        session_save::persistence_barrier(self)?;
        let result = crate::backend::wayland::session::SessionTransaction {
            input_state: &mut self.input_state,
            measurer: self.render.text_measurer(),
            session: &mut self.session,
            persistence: &mut self.persistence,
        }
        .open_named_session_runtime(target_path);
        if result.is_ok() {
            self.refresh_runtime_ui_config_seeds();
        }
        result
    }

    pub(in crate::backend::wayland) fn save_named_session_as_runtime(
        &mut self,
        target_path: &Path,
        overwrite: SaveAsOverwrite,
    ) -> Result<RuntimeSaveAsSessionReport> {
        session_save::persistence_barrier(self)?;

        crate::backend::wayland::session::SessionTransaction {
            input_state: &mut self.input_state,
            measurer: self.render.text_measurer(),
            session: &mut self.session,
            persistence: &mut self.persistence,
        }
        .save_named_session_as_runtime(target_path, overwrite)
    }

    pub(in crate::backend::wayland) fn save_named_session_as_requires_overwrite(
        &mut self,
        target_path: &Path,
    ) -> Result<bool> {
        session_save::persistence_barrier(self)?;

        crate::backend::wayland::session::SessionTransaction {
            input_state: &mut self.input_state,
            measurer: self.render.text_measurer(),
            session: &mut self.session,
            persistence: &mut self.persistence,
        }
        .save_named_session_as_requires_overwrite(target_path)
    }

    pub(in crate::backend::wayland) fn clear_current_session_runtime(
        &mut self,
    ) -> Result<RuntimeClearSessionReport> {
        ensure_destructive_session_config_available(self.session_config_failed)?;
        session_save::persistence_barrier(self)?;
        let result = crate::backend::wayland::session::SessionTransaction {
            input_state: &mut self.input_state,
            measurer: self.render.text_measurer(),
            session: &mut self.session,
            persistence: &mut self.persistence,
        }
        .clear_current_session_runtime();
        if result.is_ok() {
            self.refresh_runtime_ui_config_seeds();
        }
        result
    }

    pub(in crate::backend::wayland) fn clear_saved_tool_state_runtime(
        &mut self,
    ) -> Result<RuntimeClearToolStateReport> {
        ensure_destructive_session_config_available(self.session_config_failed)?;
        let default_tool_state = ToolStateSnapshot::from_config(&self.config);
        session_save::persistence_barrier(self)?;

        crate::backend::wayland::session::SessionTransaction {
            input_state: &mut self.input_state,
            measurer: self.render.text_measurer(),
            session: &mut self.session,
            persistence: &mut self.persistence,
        }
        .clear_saved_tool_state_runtime(default_tool_state)
    }

    pub(in crate::backend::wayland) fn handle_clear_saved_tool_state_action(&mut self) {
        match self.clear_saved_tool_state_runtime() {
            Ok(report) => {
                let message = clear_tool_state_runtime_message(&report);
                log::info!("{message}");
                self.input_state
                    .push_toast(ToastPriority::Info, "session", Toast::info(message));
            }
            Err(err) => {
                let message = format!("Failed to reset tool defaults: {err:#}");
                log::warn!("{message}");
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    "session",
                    Toast::error(message),
                );
            }
        }
    }

    pub(in crate::backend::wayland) fn inspect_active_session(
        &mut self,
    ) -> Result<stored_session::SessionInspection> {
        let options = self
            .session_options()
            .cloned()
            .ok_or_else(|| anyhow!("no active persisted session target"))?;
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::Inspect { options },
        )?;
        let PersistenceOutcome::Inspection(inspection) = outcome else {
            return Err(anyhow!("unexpected session-inspection worker outcome"));
        };
        Ok(inspection)
    }

    pub(in crate::backend::wayland) fn forget_named_session_by_path(
        &mut self,
        path: PathBuf,
    ) -> Result<bool> {
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::ForgetNamedSessionByPath { path },
        )?;
        let PersistenceOutcome::CatalogForgotten(forgotten) = outcome else {
            return Err(anyhow!("unexpected catalog-forget worker outcome"));
        };
        Ok(forgotten)
    }
}

fn ensure_destructive_session_config_available(section_failed: bool) -> Result<()> {
    if section_failed {
        return Err(anyhow!(
            "config.toml [session] could not be read; refusing to modify saved session data that default settings may mistarget - fix the section and retry"
        ));
    }
    Ok(())
}

fn clear_tool_state_runtime_message(report: &RuntimeClearToolStateReport) -> String {
    match report.outcome {
        Some(ClearToolStateOutcome::Cleared {
            preserved_board_data: true,
        }) => {
            "Tool defaults reset from config. Saved boards and history were preserved.".to_string()
        }
        Some(ClearToolStateOutcome::Cleared {
            preserved_board_data: false,
        }) => "Tool defaults reset from config. No board data was present.".to_string(),
        Some(ClearToolStateOutcome::NoToolState) => {
            "Tool defaults reset from config. No saved tool state was stored.".to_string()
        }
        Some(ClearToolStateOutcome::NoSession) => {
            "Tool defaults reset from config. No saved session file was present.".to_string()
        }
        None => "Tool defaults reset from config for this run. No active session file to edit."
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_session_actions_fail_closed_after_session_config_fallback() {
        let err = ensure_destructive_session_config_available(true)
            .expect_err("default-derived session paths must not be mutated");
        assert!(format!("{err:#}").contains("refusing to modify saved session data"));
        ensure_destructive_session_config_available(false)
            .expect("a successfully loaded session section permits mutations");
    }
}
