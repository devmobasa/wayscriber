//! Direct capture attempt resources, output snapshot, and timeout.
use super::super::capture::CaptureSession;
use super::super::ext_image_copy::ExtImageCopySession;
use super::FrozenCaptureBackend;
use crate::backend::wayland::capture::CaptureLayoutContext;
use crate::backend::wayland::frozen_geometry::OutputGeometry;
use std::time::{Duration, Instant};

pub(in crate::backend::wayland::frozen) enum DirectCaptureAttempt {
    WlrScreencopy {
        session: Box<CaptureSession>,
        context: DirectCaptureContext,
    },
    ExtImageCopy {
        session: Box<ExtImageCopySession>,
        context: DirectCaptureContext,
    },
}

impl DirectCaptureAttempt {
    pub(super) fn backend(&self) -> FrozenCaptureBackend {
        match self {
            Self::WlrScreencopy { .. } => FrozenCaptureBackend::WlrScreencopy,
            Self::ExtImageCopy { .. } => FrozenCaptureBackend::ExtImageCopy,
        }
    }

    pub(super) fn context(&self) -> &DirectCaptureContext {
        match self {
            Self::WlrScreencopy { context, .. } | Self::ExtImageCopy { context, .. } => context,
        }
    }

    pub(super) fn destroy(self) {
        match self {
            Self::WlrScreencopy { session, .. } => session.frame.destroy(),
            Self::ExtImageCopy { session, .. } => (*session).destroy(),
        }
    }
}

pub(in crate::backend::wayland::frozen) const DIRECT_CAPTURE_TIMEOUT: Duration =
    Duration::from_secs(3);

pub(in crate::backend::wayland::frozen) struct DirectCaptureContext {
    pub(in crate::backend::wayland::frozen) layout: CaptureLayoutContext,
    pub(in crate::backend::wayland::frozen) source_geometry: OutputGeometry,
    started_at: Instant,
}

impl DirectCaptureContext {
    pub(in crate::backend::wayland::frozen) fn new(
        layout: CaptureLayoutContext,
        source_geometry: OutputGeometry,
    ) -> Self {
        Self::new_at(layout, source_geometry, Instant::now())
    }

    fn new_at(
        layout: CaptureLayoutContext,
        source_geometry: OutputGeometry,
        started_at: Instant,
    ) -> Self {
        Self {
            layout,
            source_geometry,
            started_at,
        }
    }

    pub(super) fn timeout(&self, now: Instant) -> Duration {
        self.started_at
            .checked_add(DIRECT_CAPTURE_TIMEOUT)
            .map(|deadline| deadline.saturating_duration_since(now))
            .unwrap_or(Duration::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wayland_client::protocol::wl_output;

    #[test]
    fn direct_capture_context_tracks_its_deadline_and_output_identity() {
        let started_at = Instant::now();
        let geometry = OutputGeometry::update_from(
            Some((0, 0)),
            Some((1, 1)),
            (1, 1),
            1,
            wl_output::Transform::Normal,
            Some((1, 1)),
        )
        .expect("geometry");
        let capture =
            DirectCaptureContext::new_at(CaptureLayoutContext::new(7, 3), geometry, started_at);

        assert_eq!(capture.timeout(started_at), DIRECT_CAPTURE_TIMEOUT);
        assert_eq!(
            capture.timeout(started_at + DIRECT_CAPTURE_TIMEOUT),
            Duration::ZERO
        );
        assert!(capture.layout.matches(Some(7), 3));
        assert!(!capture.layout.matches(Some(8), 3));
        assert!(!capture.layout.matches(None, 3));
        assert!(!capture.layout.matches(Some(7), 4));
    }
}
