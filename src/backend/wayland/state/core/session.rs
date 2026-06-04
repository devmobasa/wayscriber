use std::path::Path;
use std::time::Instant;

use anyhow::Result;

use super::super::*;
use crate::backend::wayland::session::{
    self as runtime_session, RuntimeClearSessionReport, RuntimeOpenSessionReport,
    RuntimeSaveAsSessionReport,
};
use crate::session::SaveAsOverwrite;

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
}
