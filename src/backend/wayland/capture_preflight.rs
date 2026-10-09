//! Capture admission and the layout identity retained through fallback.
mod retry;

pub(super) use retry::{CaptureLayoutRetry, LayoutRetryError};

use super::frozen_geometry::OutputGeometry;
use super::state::OverlaySuppression;

/// Stable target for capture diagnostics. It stays under the crate-wide
/// `wayscriber` filter without matching the `wayscriber::capture` module filter.
pub(super) const CAPTURE_LOG_TARGET: &str = "wayscriber::capture_diagnostics";

#[derive(Clone, Copy)]
pub(super) enum CaptureLayoutScope {
    ActiveOutput,
    Desktop,
}

impl CaptureLayoutScope {
    pub(super) fn generation(self, generations: CaptureLayoutGenerations) -> u64 {
        match self {
            Self::ActiveOutput => generations.active_output,
            Self::Desktop => generations.desktop,
        }
    }

    fn is_complete(self, geometry: &OutputGeometry) -> bool {
        match self {
            Self::ActiveOutput => geometry.verified_pixel_size().is_some(),
            Self::Desktop => geometry.portal_layout_is_complete(),
        }
    }
}

pub(super) trait CaptureBackend: Copy + std::fmt::Debug {
    fn layout_scope(self) -> CaptureLayoutScope;
    fn suppression_reason(self) -> OverlaySuppression;

    fn layout_generation(self, generations: CaptureLayoutGenerations) -> u64 {
        self.layout_scope().generation(generations)
    }
}

/// Formats an optional log value as the bare value or `none`, keeping
/// `key=value` diagnostics free of `Some(..)` wrappers.
pub(super) struct LogField<T>(pub(super) Option<T>);

impl<T: std::fmt::Display> std::fmt::Display for LogField<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(value) => value.fmt(formatter),
            None => formatter.write_str("none"),
        }
    }
}

/// Direct/installed pixels depend on the active viewport; desktop captures also
/// depend on other outputs and the screenshot's crop bounds.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct CaptureLayoutGenerations {
    pub(super) active_output: u64,
    pub(super) desktop: u64,
}

impl CaptureLayoutGenerations {
    pub(super) fn update(
        &mut self,
        before: Option<&OutputGeometry>,
        after: Option<&OutputGeometry>,
    ) {
        let desktop_changed = before != after;
        if desktop_changed {
            self.desktop = self.desktop.wrapping_add(1);
        }
        if !OutputGeometry::same_active_output(before, after) {
            self.active_output = self.active_output.wrapping_add(1);
        }

        if desktop_changed {
            log::trace!(target: CAPTURE_LOG_TARGET,
                "capture.layout phase=changed active_output_generation={} desktop_generation={} before={before:?} after={after:?}",
                self.active_output,
                self.desktop
            );
        }
    }
}

/// Failure category survives until the domain owner publishes its terminal.
/// Display text is produced only for diagnostics and the user-facing report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CapturePreflightError {
    StaleLayout,
    LayoutDidNotSettle,
    LostSuppression,
    Backend(String),
}

impl CapturePreflightError {
    pub(super) fn message(&self, domain: &str) -> String {
        match self {
            Self::StaleLayout => format!("{domain} failed after the display layout changed"),
            Self::LayoutDidNotSettle => {
                format!("{domain} failed because the display layout did not settle")
            }
            Self::LostSuppression => "Capture retry lost overlay suppression".to_string(),
            Self::Backend(message) => message.clone(),
        }
    }

    /// Layout churn that outlasted the settling bound is still a layout-change
    /// terminal, not a backend failure that owners may route to a fallback.
    pub(super) fn is_stale_layout(&self) -> bool {
        matches!(self, Self::StaleLayout | Self::LayoutDidNotSettle)
    }

    pub(super) fn from_backend(error: anyhow::Error) -> Self {
        match error.downcast::<Self>() {
            Ok(error) => error,
            Err(error) => Self::Backend(error.to_string()),
        }
    }
}

impl std::fmt::Display for CapturePreflightError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message("Screen capture"))
    }
}

impl std::error::Error for CapturePreflightError {}

impl From<LayoutRetryError> for CapturePreflightError {
    fn from(error: LayoutRetryError) -> Self {
        match error {
            LayoutRetryError::OutputChanged => Self::StaleLayout,
            LayoutRetryError::LayoutDidNotSettle => Self::LayoutDidNotSettle,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CaptureLayout {
    output_id: Option<u32>,
    generation: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) enum CapturePreflight<B> {
    #[default]
    Idle,
    Pending {
        backend: B,
        layout: CaptureLayout,
    },
    Capturing {
        backend: B,
        layout: CaptureLayout,
    },
}

impl<B: Copy> CapturePreflight<B> {
    pub(super) fn begin(&mut self, backend: B, output_id: Option<u32>, generation: u64) {
        *self = Self::Pending {
            backend,
            layout: CaptureLayout {
                output_id,
                generation,
            },
        };
    }

    pub(super) fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub(super) fn awaiting_output(&self) -> bool {
        self.layout()
            .is_some_and(|layout| layout.output_id.is_none())
    }

    fn layout(&self) -> Option<&CaptureLayout> {
        match self {
            Self::Idle => None,
            Self::Pending { layout, .. } | Self::Capturing { layout, .. } => Some(layout),
        }
    }

    pub(super) fn take_pending(&mut self) -> Option<B> {
        let Self::Pending { backend, layout } = *self else {
            return None;
        };

        *self = Self::Capturing { backend, layout };

        Some(backend)
    }

    pub(super) fn backend(&self) -> Option<B> {
        match *self {
            Self::Idle => None,
            Self::Pending { backend, .. } | Self::Capturing { backend, .. } => Some(backend),
        }
    }

    pub(super) fn generation(&self, generations: CaptureLayoutGenerations) -> u64
    where
        B: CaptureBackend,
    {
        self.backend().map_or(generations.active_output, |backend| {
            backend.layout_generation(generations)
        })
    }

    /// Before acquisition starts, an unknown target may bind to its first
    /// output. A request already bound to an output must never move to another.
    pub(super) fn can_retry_on_output(&self, output_id: Option<u32>, generation: u64) -> bool {
        let Some(layout) = self.layout() else {
            return false;
        };

        match (layout.output_id, output_id) {
            (None, _) => true,
            (Some(captured), Some(active)) => captured == active && layout.generation != generation,
            _ => false,
        }
    }

    pub(super) fn log_layout(
        &self,
        phase: &'static str,
        barrier_id: Option<u64>,
        active_output: Option<u32>,
        generations: CaptureLayoutGenerations,
    ) where
        B: CaptureBackend,
    {
        if let (Some(backend), Some(layout)) = (self.backend(), self.layout()) {
            let reason = backend.suppression_reason();
            let current_generation = backend.layout_generation(generations);

            log::info!(target: CAPTURE_LOG_TARGET,
                "capture.preflight id={} component=layout reason={reason:?} phase={phase} backend={backend:?} captured_output={} active_output={} captured_generation={} current_generation={current_generation}",
                LogField(barrier_id),
                LogField(layout.output_id),
                LogField(active_output),
                layout.generation
            );
        }
    }

    pub(super) fn layout_matches(&self, output_id: Option<u32>, generation: u64) -> bool {
        let Some(layout) = self.layout() else {
            return true;
        };

        super::portal_capture::layout_token_matches(
            layout.output_id,
            layout.generation,
            output_id,
            generation,
        )
    }

    pub(super) fn ensure_layout_current(
        &self,
        output_id: Option<u32>,
        generation: u64,
    ) -> Result<(), CapturePreflightError> {
        if self.layout_matches(output_id, generation) {
            Ok(())
        } else {
            Err(CapturePreflightError::StaleLayout)
        }
    }

    pub(super) fn ensure_admission_current(
        &self,
        output_id: Option<u32>,
        generation: u64,
    ) -> Result<(), CapturePreflightError> {
        if output_id.is_none() {
            return Err(CapturePreflightError::StaleLayout);
        }

        self.ensure_layout_current(output_id, generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TestBackend {
        Portal,
        Direct,
    }

    impl CaptureBackend for TestBackend {
        fn layout_scope(self) -> CaptureLayoutScope {
            match self {
                Self::Portal => CaptureLayoutScope::Desktop,
                Self::Direct => CaptureLayoutScope::ActiveOutput,
            }
        }

        fn suppression_reason(self) -> OverlaySuppression {
            OverlaySuppression::Frozen
        }
    }

    fn generations(generation: u64) -> CaptureLayoutGenerations {
        CaptureLayoutGenerations {
            active_output: generation,
            desktop: generation,
        }
    }

    #[test]
    fn direct_retry_does_not_wait_for_unrelated_output_metadata() {
        let now = Instant::now();
        let incomplete_desktop = geometry().with_known_output_count(Some(2));
        let mut retry = CaptureLayoutRetry::default();
        assert!(retry.schedule(TestBackend::Direct, Some(1), Some(1), true, 4, now));

        assert_eq!(
            retry.take_ready(
                Some(1),
                CaptureLayoutGenerations {
                    active_output: 4,
                    desktop: 5
                },
                Some(&incomplete_desktop),
                now + Duration::from_millis(100)
            ),
            Ok(Some((TestBackend::Direct, 1)))
        );
    }

    #[test]
    fn preflight_retry_keeps_known_targets_and_allows_output_discovery() {
        for (captured, active, generation, expected) in [
            (None, None, 4, true),
            (None, Some(1), 4, true),
            (Some(1), None, 5, false),
            (Some(1), Some(2), 5, false),
            (Some(1), Some(1), 4, false),
            (Some(1), Some(1), 5, true),
        ] {
            let mut preflight = CapturePreflight::default();
            preflight.begin((), captured, 4);

            assert_eq!(preflight.can_retry_on_output(active, generation), expected);

            preflight.take_pending();
            assert_eq!(preflight.can_retry_on_output(active, generation), expected);
        }

        assert!(!CapturePreflight::<()>::Idle.can_retry_on_output(Some(1), 5));
    }

    #[test]
    fn retry_waits_for_a_first_output_then_keeps_that_identity() {
        let now = Instant::now();
        let geometry = geometry();
        let mut unbound = CapturePreflight::default();
        unbound.begin(TestBackend::Portal, None, 4);
        let mut retry = CaptureLayoutRetry::default();
        assert!(retry.queue_preflight(&unbound, None, generations(4), now));

        assert_eq!(retry.take_ready(None, generations(4), None, now), Ok(None));
        assert_eq!(
            retry.take_ready(Some(1), generations(5), Some(&geometry), now),
            Ok(None)
        );
        assert_eq!(
            retry.take_ready(
                Some(2),
                generations(5),
                Some(&geometry),
                now + Duration::from_millis(150)
            ),
            Err(LayoutRetryError::OutputChanged)
        );

        let mut retry = CaptureLayoutRetry::default();
        assert!(retry.queue_preflight(&unbound, None, generations(4), now));

        assert_eq!(
            retry.take_ready(None, generations(4), None, now + Duration::from_secs(1)),
            Err(LayoutRetryError::LayoutDidNotSettle)
        );
    }

    fn geometry() -> OutputGeometry {
        OutputGeometry::update_from(
            Some((0, 0)),
            Some((2, 1)),
            (2, 1),
            1,
            wayland_client::protocol::wl_output::Transform::Normal,
            Some((2, 1)),
        )
        .unwrap()
        .with_known_output_count(Some(1))
    }

    #[test]
    fn retry_waits_for_both_output_update_batches_and_spends_the_budget_once() {
        let now = Instant::now();
        let geometry = geometry();
        let mut retry = CaptureLayoutRetry::default();
        assert!(retry.schedule(TestBackend::Portal, Some(1), Some(1), true, 4, now));

        assert_eq!(retry.timeout(now), Some(Duration::from_millis(100)));
        assert_eq!(
            retry.take_ready(
                Some(1),
                generations(4),
                Some(&geometry),
                now + Duration::from_millis(99)
            ),
            Ok(None)
        );
        assert_eq!(
            retry.take_ready(
                Some(1),
                generations(5),
                Some(&geometry),
                now + Duration::from_millis(100)
            ),
            Ok(None)
        );
        assert_eq!(
            retry.timeout(now + Duration::from_millis(100)),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            retry.take_ready(
                Some(1),
                generations(5),
                Some(&geometry),
                now + Duration::from_millis(199)
            ),
            Ok(None)
        );
        assert_eq!(
            retry.take_ready(
                Some(1),
                generations(5),
                Some(&geometry),
                now + Duration::from_millis(200)
            ),
            Ok(Some((TestBackend::Portal, 1)))
        );
        assert_eq!(retry.timeout(now + Duration::from_millis(200)), None);
        assert!(!retry.schedule(TestBackend::Portal, Some(1), Some(1), true, 6, now));
    }

    #[test]
    fn incomplete_topology_wait_is_paced_and_bounded() {
        let now = Instant::now();
        let incomplete = geometry().with_known_output_count(Some(2));
        let mut retry = CaptureLayoutRetry::default();
        assert!(retry.schedule(TestBackend::Portal, Some(1), Some(1), true, 4, now));

        assert_eq!(
            retry.take_ready(
                Some(1),
                generations(4),
                Some(&incomplete),
                now + Duration::from_millis(100)
            ),
            Ok(None)
        );
        assert_eq!(
            retry.timeout(now + Duration::from_millis(100)),
            Some(Duration::from_millis(50))
        );
        assert!(
            retry
                .take_ready(
                    Some(1),
                    generations(4),
                    Some(&incomplete),
                    now + Duration::from_secs(1)
                )
                .is_err()
        );
        assert!(!retry.is_pending());
        assert_eq!(retry.timeout(now + Duration::from_secs(1)), None);
    }

    #[test]
    fn dispatch_retains_layout_until_reset() {
        let mut phase = CapturePreflight::default();
        phase.begin(7, Some(3), 10);
        assert!(phase.is_pending());
        assert_eq!(phase.take_pending(), Some(7));
        assert!(!phase.is_pending());
        assert_eq!(phase.take_pending(), None);
        assert!(phase.layout_matches(Some(3), 10));
        assert!(!phase.layout_matches(Some(4), 10));
        assert!(!phase.layout_matches(Some(3), 11));
        phase = CapturePreflight::Idle;
        assert!(phase.layout_matches(Some(4), 11));
    }
}
