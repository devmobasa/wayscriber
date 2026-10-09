//! One retained retry on the backend that failed admission or produced stale pixels.
use super::*;
use std::time::{Duration, Instant};

const LAYOUT_QUIET_PERIOD: Duration = Duration::from_millis(100);
const LAYOUT_SETTLE_TIMEOUT: Duration = Duration::from_secs(1);
const INCOMPLETE_LAYOUT_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum LayoutRetryError {
    OutputChanged,
    LayoutDidNotSettle,
}

/// One retry retained by the original request while admission or portal pixels settle.
#[derive(Debug, Clone, Copy, Default)]
pub(in crate::backend::wayland) enum CaptureLayoutRetry<B> {
    #[default]
    Available,
    Pending {
        backend: B,
        output_id: Option<u32>,
        generation: u64,
        not_before: Instant,
        deadline: Instant,
    },
    Spent,
}

impl<B: CaptureBackend> CaptureLayoutRetry<B> {
    fn log_decision(&self, backend: B, eligible: bool, queued: bool) {
        let state = match self {
            Self::Available => "available",
            Self::Pending { .. } => "settling",
            Self::Spent => "spent",
        };
        let reason = backend.suppression_reason();

        log::info!(target: CAPTURE_LOG_TARGET,
            "capture.preflight component=layout reason={reason:?} phase=retry-decision backend={backend:?} eligible={eligible} queued={queued} retry_state={state} budget_remaining={}",
            u8::from(matches!(self, Self::Available))
        );
    }

    pub(in crate::backend::wayland) fn schedule(
        &mut self,
        backend: B,
        captured: Option<u32>,
        active: Option<u32>,
        layout_changed: bool,
        generation: u64,
        now: Instant,
    ) -> bool {
        if captured.is_none() || active != captured || !layout_changed {
            return false;
        }

        self.begin_once(backend, captured, generation, now)
    }

    /// Only preflight admission may wait without an identity. A portal result
    /// with no target is invalid and goes through `schedule`, which rejects it.
    pub(in crate::backend::wayland) fn queue_preflight(
        &mut self,
        preflight: &CapturePreflight<B>,
        active: Option<u32>,
        generations: CaptureLayoutGenerations,
        now: Instant,
    ) -> bool {
        let Some(backend) = preflight.backend() else {
            return false;
        };

        let generation = backend.layout_generation(generations);
        let eligible = preflight.can_retry_on_output(active, generation);
        let queued = eligible && self.begin_once(backend, active, generation, now);

        self.log_decision(backend, eligible, queued);

        queued
    }

    /// Spends the single retry budget; a pending or spent retry never restarts.
    fn begin_once(
        &mut self,
        backend: B,
        output_id: Option<u32>,
        generation: u64,
        now: Instant,
    ) -> bool {
        if !matches!(self, Self::Available) {
            return false;
        }

        *self = Self::Pending {
            backend,
            output_id,
            generation,
            not_before: now + LAYOUT_QUIET_PERIOD,
            deadline: now + LAYOUT_SETTLE_TIMEOUT,
        };

        true
    }

    pub(in crate::backend::wayland) fn restart_preflight(
        &mut self,
        preflight: &mut CapturePreflight<B>,
        active: Option<u32>,
        generations: CaptureLayoutGenerations,
        geometry: Option<&OutputGeometry>,
        now: Instant,
    ) -> Result<bool, CapturePreflightError> {
        let Some((backend, output_id)) = self.take_ready(active, generations, geometry, now)?
        else {
            return Ok(false);
        };

        preflight.begin(
            backend,
            Some(output_id),
            backend.layout_generation(generations),
        );
        preflight.log_layout("retry-preflight", None, active, generations);

        Ok(true)
    }

    pub(in crate::backend::wayland) fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub(in crate::backend::wayland) fn timeout(&self, now: Instant) -> Option<Duration> {
        let Self::Pending {
            not_before,
            deadline,
            ..
        } = *self
        else {
            return None;
        };

        Some(not_before.min(deadline).saturating_duration_since(now))
    }

    /// Admit only a complete snapshot after a quiet period. Output events can
    /// arrive in multiple batches; a deadline bounds metadata that never settles.
    pub(in crate::backend::wayland) fn take_ready(
        &mut self,
        active_output: Option<u32>,
        generations: CaptureLayoutGenerations,
        geometry: Option<&OutputGeometry>,
        now: Instant,
    ) -> Result<Option<(B, u32)>, LayoutRetryError> {
        let Self::Pending {
            backend,
            output_id,
            generation,
            not_before,
            deadline,
        } = self
        else {
            return Ok(None);
        };

        if output_id.is_some() && active_output != *output_id {
            *self = Self::Spent;
            return Err(LayoutRetryError::OutputChanged);
        }

        // Admission itself is bounded, even if dispatch wakes late after the
        // geometry's quiet period. Never start a new barrier after this deadline.
        if now >= *deadline {
            *self = Self::Spent;
            return Err(LayoutRetryError::LayoutDidNotSettle);
        }

        if output_id.is_none() {
            let Some(active) = active_output else {
                *not_before = now + INCOMPLETE_LAYOUT_POLL_INTERVAL;
                return Ok(None);
            };

            *output_id = Some(active);
            *not_before = now + LAYOUT_QUIET_PERIOD;
        }

        let active_generation = backend.layout_generation(generations);
        if active_generation != *generation {
            *generation = active_generation;
            *not_before = now + LAYOUT_QUIET_PERIOD;
        }

        if now < *not_before {
            return Ok(None);
        }

        if !geometry.is_some_and(|geometry| backend.layout_scope().is_complete(geometry)) {
            *not_before = now + INCOMPLETE_LAYOUT_POLL_INTERVAL;
            return Ok(None);
        }

        let target = output_id.map(|output_id| (*backend, output_id));
        *self = Self::Spent;

        Ok(target)
    }
}
