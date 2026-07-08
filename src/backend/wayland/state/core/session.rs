use std::path::Path;
use std::time::Instant;

use anyhow::Result;

use super::super::*;
use crate::backend::wayland::session::{
    self as runtime_session, RuntimeClearSessionReport, RuntimeClearToolStateReport,
    RuntimeOpenSessionReport, RuntimeSaveAsSessionReport,
};
use crate::session::{ClearToolStateOutcome, SaveAsOverwrite, ToolStateSnapshot};

impl WaylandState {
    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn open_named_session_runtime(
        &mut self,
        target_path: &Path,
    ) -> Result<RuntimeOpenSessionReport> {
        runtime_session::open_named_session_runtime(
            &mut self.input_state,
            &mut self.session,
            target_path,
            Instant::now(),
        )
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn save_named_session_as_runtime(
        &mut self,
        target_path: &Path,
        overwrite: SaveAsOverwrite,
    ) -> Result<RuntimeSaveAsSessionReport> {
        runtime_session::save_named_session_as_runtime(
            &mut self.input_state,
            &mut self.session,
            target_path,
            overwrite,
            Instant::now(),
        )
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn save_named_session_as_requires_overwrite(
        &self,
        target_path: &Path,
    ) -> Result<bool> {
        runtime_session::save_named_session_as_requires_overwrite(&self.session, target_path)
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn clear_current_session_runtime(
        &mut self,
    ) -> Result<RuntimeClearSessionReport> {
        runtime_session::clear_current_session_runtime(
            &mut self.input_state,
            &mut self.session,
            Instant::now(),
        )
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn clear_saved_tool_state_runtime(
        &mut self,
    ) -> Result<RuntimeClearToolStateReport> {
        let default_tool_state = ToolStateSnapshot::from_config(&self.config);
        runtime_session::clear_saved_tool_state_runtime(
            &mut self.input_state,
            &mut self.session,
            default_tool_state,
            Instant::now(),
        )
    }

    pub(in crate::backend::wayland) fn handle_clear_saved_tool_state_action(&mut self) {
        match self.clear_saved_tool_state_runtime() {
            Ok(report) => {
                let message = clear_tool_state_runtime_message(&report);
                log::info!("{message}");
                self.input_state
                    .set_ui_toast(crate::input::state::UiToastKind::Info, message);
            }
            Err(err) => {
                let message = format!("Failed to reset tool defaults: {err:#}");
                log::warn!("{message}");
                self.input_state
                    .set_ui_toast(crate::input::state::UiToastKind::Error, message);
            }
        }
    }
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
