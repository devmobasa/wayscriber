//! Capture admission and the layout identity retained through fallback.
use std::time::{Duration, Instant};

use super::frozen_geometry::OutputGeometry;

const PORTAL_LAYOUT_QUIET_PERIOD: Duration = Duration::from_millis(100);
const PORTAL_LAYOUT_SETTLE_TIMEOUT: Duration = Duration::from_secs(1);
const INCOMPLETE_LAYOUT_POLL_INTERVAL: Duration = Duration::from_millis(50);

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
        if before != after {
            self.desktop = self.desktop.wrapping_add(1);
        }
        if !OutputGeometry::same_active_output(before, after) {
            self.active_output = self.active_output.wrapping_add(1);
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

impl From<PortalRetryError> for CapturePreflightError {
    fn from(error: PortalRetryError) -> Self {
        match error {
            PortalRetryError::OutputChanged => Self::StaleLayout,
            PortalRetryError::LayoutDidNotSettle => Self::LayoutDidNotSettle,
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

    pub(super) fn changed_on_output(&self, output_id: Option<u32>, generation: u64) -> bool {
        let layout = match self {
            Self::Idle => return false,
            Self::Pending { layout, .. } | Self::Capturing { layout, .. } => layout,
        };

        layout.output_id.is_some()
            && layout.output_id == output_id
            && layout.generation != generation
    }

    pub(super) fn layout_matches(&self, output_id: Option<u32>, generation: u64) -> bool {
        let layout = match self {
            Self::Idle => return true,
            Self::Pending { layout, .. } | Self::Capturing { layout, .. } => layout,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PortalRetryError {
    OutputChanged,
    LayoutDidNotSettle,
}

/// One retry retained by the original request while its portal result is discarded.
#[derive(Debug, Clone, Copy, Default)]
pub(super) enum PortalLayoutRetry {
    #[default]
    Available,
    Pending {
        output_id: u32,
        generation: u64,
        not_before: Instant,
        deadline: Instant,
    },
    Spent,
}

impl PortalLayoutRetry {
    pub(super) fn schedule(
        &mut self,
        captured: Option<u32>,
        active: Option<u32>,
        layout_changed: bool,
        generation: u64,
        now: Instant,
    ) -> bool {
        let Some(output_id) = captured else {
            return false;
        };
        if !matches!(self, Self::Available) || active != captured || !layout_changed {
            return false;
        }

        *self = Self::Pending {
            output_id,
            generation,
            not_before: now + PORTAL_LAYOUT_QUIET_PERIOD,
            deadline: now + PORTAL_LAYOUT_SETTLE_TIMEOUT,
        };

        true
    }

    pub(super) fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub(super) fn timeout(&self, now: Instant) -> Option<Duration> {
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
    pub(super) fn take_ready(
        &mut self,
        active_output: Option<u32>,
        active_generation: u64,
        geometry: Option<&OutputGeometry>,
        now: Instant,
    ) -> Result<Option<u32>, PortalRetryError> {
        let Self::Pending {
            output_id,
            generation,
            not_before,
            deadline,
        } = self
        else {
            return Ok(None);
        };
        if active_output != Some(*output_id) {
            *self = Self::Spent;
            return Err(PortalRetryError::OutputChanged);
        }
        // Admission itself is bounded, even if dispatch wakes late after the
        // geometry's quiet period. Never start a new barrier after this deadline.
        if now >= *deadline {
            *self = Self::Spent;
            return Err(PortalRetryError::LayoutDidNotSettle);
        }

        if active_generation != *generation {
            *generation = active_generation;
            *not_before = now + PORTAL_LAYOUT_QUIET_PERIOD;
        }
        if now < *not_before {
            return Ok(None);
        }
        if !geometry.is_some_and(OutputGeometry::portal_layout_is_complete) {
            *not_before = now + INCOMPLETE_LAYOUT_POLL_INTERVAL;
            return Ok(None);
        }

        let output_id = *output_id;
        *self = Self::Spent;
        Ok(Some(output_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut retry = PortalLayoutRetry::default();
        assert!(retry.schedule(Some(1), Some(1), true, 4, now));

        assert_eq!(retry.timeout(now), Some(Duration::from_millis(100)));
        assert_eq!(
            retry.take_ready(Some(1), 4, Some(&geometry), now + Duration::from_millis(99)),
            Ok(None)
        );
        assert_eq!(
            retry.take_ready(
                Some(1),
                5,
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
                5,
                Some(&geometry),
                now + Duration::from_millis(199)
            ),
            Ok(None)
        );
        assert_eq!(
            retry.take_ready(
                Some(1),
                5,
                Some(&geometry),
                now + Duration::from_millis(200)
            ),
            Ok(Some(1))
        );
        assert_eq!(retry.timeout(now + Duration::from_millis(200)), None);
        assert!(!retry.schedule(Some(1), Some(1), true, 6, now));
    }

    #[test]
    fn incomplete_topology_wait_is_paced_and_bounded() {
        let now = Instant::now();
        let incomplete = geometry().with_known_output_count(Some(2));
        let mut retry = PortalLayoutRetry::default();
        assert!(retry.schedule(Some(1), Some(1), true, 4, now));

        assert_eq!(
            retry.take_ready(
                Some(1),
                4,
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
                .take_ready(Some(1), 4, Some(&incomplete), now + Duration::from_secs(1))
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
