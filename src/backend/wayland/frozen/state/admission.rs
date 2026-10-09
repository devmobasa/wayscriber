//! Backend selection and admission of the retained Freeze request.
use super::*;
use crate::backend::wayland::capture_preflight::{CaptureBackend, CaptureLayoutScope};
use crate::backend::wayland::state::OverlaySuppression;

impl CaptureBackend for FrozenCaptureBackend {
    fn layout_scope(self) -> CaptureLayoutScope {
        match self {
            Self::Portal => CaptureLayoutScope::Desktop,
            Self::WlrScreencopy | Self::ExtImageCopy => CaptureLayoutScope::ActiveOutput,
        }
    }

    fn suppression_reason(self) -> OverlaySuppression {
        OverlaySuppression::Frozen
    }
}

impl FrozenState {
    pub(in crate::backend::wayland) fn take_preflight_pending(
        &mut self,
    ) -> Option<FrozenCaptureBackend> {
        self.preflight.take_pending()
    }

    #[cfg(test)]
    pub(in crate::backend::wayland::frozen) fn snapshot_preflight_layout(&mut self) {
        self.preflight.begin(
            FrozenCaptureBackend::Portal,
            self.active_output_id,
            self.layout_generations.desktop,
        );
    }

    pub(in crate::backend::wayland::frozen) fn capture_layout_generation(
        &self,
        backend: FrozenCaptureBackend,
    ) -> u64 {
        backend.layout_generation(self.layout_generations)
    }

    pub(in crate::backend::wayland::frozen) fn ensure_preflight_layout_current(
        &self,
    ) -> Result<(), CapturePreflightError> {
        self.preflight.ensure_layout_current(
            self.active_output_id,
            self.preflight.generation(self.layout_generations),
        )
    }

    /// Admission needs a known active output and the layout saved when the
    /// request began; an unknown output is retried, never captured.
    pub(in crate::backend::wayland::frozen) fn ensure_preflight_admission(
        &self,
    ) -> Result<(), CapturePreflightError> {
        self.preflight.ensure_admission_current(
            self.active_output_id,
            self.preflight.generation(self.layout_generations),
        )
    }

    pub(in crate::backend::wayland) fn log_preflight_layout(&self, barrier_id: Option<u64>) {
        self.preflight.log_layout(
            "snapshot",
            barrier_id,
            self.active_output_id,
            self.layout_generations,
        );
    }

    #[cfg(test)]
    pub fn preflight_layout_is_current(&self) -> bool {
        self.ensure_preflight_layout_current().is_ok()
    }
}
