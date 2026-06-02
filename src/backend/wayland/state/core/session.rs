use std::path::Path;
use std::time::Instant;

use anyhow::Result;

use super::super::*;
use crate::backend::wayland::session::{self as runtime_session, RuntimeOpenSessionReport};

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
}
