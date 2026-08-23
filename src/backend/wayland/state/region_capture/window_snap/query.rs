use crate::backend::wayland::runtime_operation::RuntimeOperationSubmitError;
use crate::backend::wayland::{
    RuntimeOperationController, RuntimeOperationPoll, state::WaylandState,
};
use crate::capture::window_geometry::{
    WindowQueryContext, WindowQueryResult, detect_backend, query_window_targets,
};
use crate::input::state::RegionPurposeTag;
use crate::util::Rect;

use super::{
    ScreenSourceToken, WindowQueryApply, WindowSnapCorrelation, WindowSnapQuery, WindowSnapSession,
    apply_window_query_completion,
};
use crate::backend::wayland::state::region_capture::FreezeOwnership;

fn source_has_correlated_window_layout(
    purpose: RegionPurposeTag,
    freeze_ownership: FreezeOwnership,
) -> bool {
    purpose.is_capture() && matches!(freeze_ownership, FreezeOwnership::PickerOwned { .. })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowQuerySubmit {
    NoneQueued,
    Started,
    Busy,
    Failed,
}

fn submit_queued_window_query_with(
    controller: &mut RuntimeOperationController<
        WindowSnapQuery,
        Result<WindowQueryResult, crate::capture::window_geometry::WindowGeometryError>,
    >,
    session: &mut Option<WindowSnapSession>,
    operation: impl FnOnce(
        WindowQueryContext,
    ) -> Result<
        WindowQueryResult,
        crate::capture::window_geometry::WindowGeometryError,
    > + Send
    + 'static,
) -> WindowQuerySubmit {
    let Some(query) = session.as_ref().and_then(WindowSnapSession::queued_query) else {
        return WindowQuerySubmit::NoneQueued;
    };
    let correlation = query.correlation;
    let worker_context = query.context.clone();
    match controller.try_submit(query, "wayscriber-window-query", move || {
        operation(worker_context)
    }) {
        Ok(_) => {
            let marked = session
                .as_mut()
                .is_some_and(|session| session.mark_query_started(correlation));
            debug_assert!(marked, "submitted window query must retain its session");
            WindowQuerySubmit::Started
        }
        Err(failure) => {
            let (error, _) = failure.into_parts();
            if matches!(error, RuntimeOperationSubmitError::Busy { .. }) {
                log::debug!("Window snap query queued behind the active query: {error}");
                return WindowQuerySubmit::Busy;
            }
            log::debug!("Window snapping unavailable: {error}");
            clear_matching_window_session(session, correlation);
            WindowQuerySubmit::Failed
        }
    }
}

fn poll_window_query_with(
    controller: &mut RuntimeOperationController<
        WindowSnapQuery,
        Result<WindowQueryResult, crate::capture::window_geometry::WindowGeometryError>,
    >,
    session: &mut Option<WindowSnapSession>,
    retry: impl FnOnce(
        WindowQueryContext,
    ) -> Result<
        WindowQueryResult,
        crate::capture::window_geometry::WindowGeometryError,
    > + Send
    + 'static,
) -> bool {
    let (consumed_terminal, changed) = match controller.poll() {
        RuntimeOperationPoll::Idle | RuntimeOperationPoll::Pending { .. } => (false, false),
        RuntimeOperationPoll::Ready {
            context,
            outcome: Ok(WindowQueryResult { backend, targets }),
            ..
        } => {
            log::debug!(
                "Window snap provider {backend:?} returned {} target(s)",
                targets.len()
            );
            let applied = apply_window_query_completion(session, context.correlation, targets);
            (true, applied != WindowQueryApply::Stale)
        }
        RuntimeOperationPoll::Ready {
            context,
            outcome: Err(error),
            ..
        } => {
            log::debug!("Window snapping unavailable: {error}");
            (
                true,
                clear_matching_window_session(session, context.correlation),
            )
        }
        RuntimeOperationPoll::ProducerFailed {
            context, reason, ..
        } => {
            log::debug!("Window snap query worker failed: {reason}");
            (
                true,
                clear_matching_window_session(session, context.correlation),
            )
        }
        RuntimeOperationPoll::Disconnected { context, .. } => {
            log::debug!("Window snap query worker disconnected");
            (
                true,
                clear_matching_window_session(session, context.correlation),
            )
        }
    };
    if consumed_terminal {
        let _ = submit_queued_window_query_with(controller, session, retry);
    }
    changed
}

impl WaylandState {
    pub(in crate::backend::wayland::state) fn start_region_window_query(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        source: ScreenSourceToken,
        freeze_ownership: FreezeOwnership,
    ) {
        self.clear_region_window_snap();
        if !source_has_correlated_window_layout(purpose, freeze_ownership)
            || detect_backend().is_none()
        {
            return;
        }
        let Some(provider) = self.region_window_query_context(source) else {
            return;
        };
        let correlation = WindowSnapCorrelation::new(generation, source);
        self.data.window_snap = Some(WindowSnapSession::queued(correlation, provider));
        let _ = submit_queued_window_query_with(
            &mut self.window_query,
            &mut self.data.window_snap,
            |provider| query_window_targets(&provider),
        );
    }

    fn region_window_query_context(&self, source: ScreenSourceToken) -> Option<WindowQueryContext> {
        let output = self.surface.current_output()?;
        let info = self.output_state.info(&output)?;
        if info.id != source.output_id {
            return None;
        }
        let output_name = info.name.filter(|name| !name.is_empty())?;
        let (x, y) = info.logical_position?;
        let (width, height) = info.logical_size?;
        let surface = (u32::try_from(width).ok()?, u32::try_from(height).ok()?);
        if surface != source.surface {
            return None;
        }
        Some(WindowQueryContext {
            output_name,
            output_logical_rect: Rect::new(x, y, width, height)?,
        })
    }

    pub(in crate::backend::wayland) fn poll_region_window_query_completion(&mut self) {
        if poll_window_query_with(
            &mut self.window_query,
            &mut self.data.window_snap,
            |provider| query_window_targets(&provider),
        ) {
            self.mark_region_window_snap_dirty();
        }
    }

    pub(in crate::backend::wayland::state) fn clear_region_window_snap(&mut self) {
        self.data.window_snap = None;
    }
}

fn clear_matching_window_session(
    session: &mut Option<WindowSnapSession>,
    correlation: WindowSnapCorrelation,
) -> bool {
    if session
        .as_ref()
        .is_none_or(|session| session.correlation() != correlation)
    {
        return false;
    }
    *session = None;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::{
        RuntimeOperationIdSource, RuntimeWakeSource,
        state::screen_image::{ScreenImageKind, ScreenSourceToken},
    };
    use crate::capture::window_geometry::{WindowGeometryBackend, WindowTarget};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    use wayland_client::protocol::wl_output;

    fn source() -> ScreenSourceToken {
        ScreenSourceToken {
            output_id: 7,
            output_layout_generation: 11,
            kind: ScreenImageKind::Frozen,
            image_generation: 13,
            image_size: (1_000, 800),
            stride: 4_000,
            surface: (1_000, 800),
            output_scale: 1,
            output_transform: wl_output::Transform::Normal,
            zoom_transformed: false,
            zoom_scale: 1.0,
            zoom_view_offset: (0.0, 0.0),
        }
    }

    fn provider() -> WindowQueryContext {
        WindowQueryContext {
            output_name: "DP-1".to_string(),
            output_logical_rect: Rect::new(0, 0, 1_000, 800).unwrap(),
        }
    }

    fn result(
        id: &str,
    ) -> Result<WindowQueryResult, crate::capture::window_geometry::WindowGeometryError> {
        Ok(WindowQueryResult {
            backend: WindowGeometryBackend::Hyprland,
            targets: vec![WindowTarget {
                id: id.to_string(),
                title: id.to_string(),
                logical_rect: Rect::new(20, 20, 100, 80).unwrap(),
            }],
        })
    }

    #[test]
    fn only_a_picker_owned_fresh_freeze_can_offer_window_snapping() {
        assert!(source_has_correlated_window_layout(
            RegionPurposeTag::CaptureDeliver,
            FreezeOwnership::PickerOwned {
                image_generation: 7
            }
        ));
        assert!(source_has_correlated_window_layout(
            RegionPurposeTag::CaptureInteractive,
            FreezeOwnership::PickerOwned {
                image_generation: 8
            }
        ));
        assert!(!source_has_correlated_window_layout(
            RegionPurposeTag::CaptureDeliver,
            FreezeOwnership::PreExisting
        ));
        assert!(!source_has_correlated_window_layout(
            RegionPurposeTag::Ocr,
            FreezeOwnership::PickerOwned {
                image_generation: 9
            }
        ));
    }

    #[test]
    fn real_capacity_one_terminal_routes_the_reopened_picker_into_retry() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller =
            RuntimeOperationController::new(RuntimeOperationIdSource::new(), wake.handle());
        let old = WindowSnapCorrelation::new(1, source());
        let middle = WindowSnapCorrelation::new(2, source());
        let fresh = WindowSnapCorrelation::new(3, source());
        let mut session = Some(WindowSnapSession::queued(old, provider()));
        let (release_tx, release_rx) = mpsc::channel();
        assert_eq!(
            submit_queued_window_query_with(&mut controller, &mut session, move |_| {
                release_rx.recv().unwrap();
                result("old")
            }),
            WindowQuerySubmit::Started
        );

        // Cancel drops only the UI session; immediate reopen queues behind the
        // real controller's still-active old worker.
        session = Some(WindowSnapSession::queued(middle, provider()));
        assert_eq!(
            submit_queued_window_query_with(&mut controller, &mut session, |_| {
                panic!("a busy submission must not run")
            }),
            WindowQuerySubmit::Busy
        );
        // A second cancel/reopen replaces only the queued UI request. The
        // single old worker remains the controller's sole active operation.
        session = Some(WindowSnapSession::queued(fresh, provider()));
        assert_eq!(
            submit_queued_window_query_with(&mut controller, &mut session, |_| {
                panic!("a repeated busy submission must not run")
            }),
            WindowQuerySubmit::Busy
        );
        release_tx.send(()).unwrap();

        let deadline = Instant::now() + Duration::from_secs(1);
        while !session.as_ref().is_some_and(WindowSnapSession::is_ready) {
            poll_window_query_with(&mut controller, &mut session, |_| result("fresh"));
            assert!(Instant::now() < deadline, "reopened query did not retry");
            std::thread::yield_now();
        }
        let session = session.unwrap();
        assert_eq!(session.correlation(), fresh);
        assert_eq!(session.targets()[0].image_rect().size(), (100, 80));
        assert_eq!(
            session.targets()[0].screen_rect(),
            Rect::new(20, 20, 100, 80).unwrap()
        );
    }
}
