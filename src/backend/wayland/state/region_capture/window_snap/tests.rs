use super::*;
use crate::backend::wayland::state::screen_image::{ScreenImageKind, ScreenSourceToken};
use crate::capture::window_geometry::{WindowQueryContext, WindowTarget};
use crate::util::Rect;
use wayland_client::protocol::wl_output;

fn source() -> ScreenSourceToken {
    ScreenSourceToken {
        output_id: 7,
        output_layout_generation: 11,
        kind: ScreenImageKind::Frozen,
        image_generation: 13,
        image_size: (1_500, 1_200),
        stride: 6_000,
        surface: (1_000, 800),
        output_scale: 2,
        output_transform: wl_output::Transform::Normal,
        zoom_transformed: false,
        zoom_scale: 1.0,
        zoom_view_offset: (0.0, 0.0),
    }
}

fn target(rect: Rect) -> WindowTarget {
    WindowTarget {
        id: String::new(),
        title: String::new(),
        logical_rect: rect,
    }
}

fn context() -> WindowQueryContext {
    WindowQueryContext {
        output_name: "DP-1".to_string(),
        output_logical_rect: Rect::new(0, 0, 1_000, 800).unwrap(),
    }
}

#[test]
fn hover_chooses_the_topmost_window_in_provider_order() {
    let correlation = WindowSnapCorrelation::new(3, source());
    let mut session = Some(WindowSnapSession::pending(correlation));
    assert_eq!(
        apply_window_query_completion(
            &mut session,
            correlation,
            vec![
                target(Rect::new(10, 10, 200, 200).unwrap()),
                target(Rect::new(50, 50, 100, 100).unwrap()),
            ],
        ),
        WindowQueryApply::Ready
    );
    let session = session.as_mut().unwrap();
    assert!(session.toggle_mode());

    assert!(session.update_hover((75.0, 75.0)));
    assert_eq!(session.hovered_index(), Some(1));
    assert_eq!(
        session.hovered_target().map(WindowSnapTarget::screen_rect),
        Some(Rect::new(50, 50, 100, 100).unwrap())
    );
}

#[test]
fn directional_walk_uses_omasnap_along_plus_weighted_across_score() {
    let correlation = WindowSnapCorrelation::new(3, source());
    let mut session = Some(WindowSnapSession::pending(correlation));
    assert_eq!(
        apply_window_query_completion(
            &mut session,
            correlation,
            vec![
                target(Rect::new(0, 0, 100, 100).unwrap()),
                target(Rect::new(100, 100, 100, 100).unwrap()),
                target(Rect::new(200, 0, 100, 100).unwrap()),
            ],
        ),
        WindowQueryApply::Ready
    );
    let session = session.as_mut().unwrap();
    assert!(session.toggle_mode());
    assert!(session.update_hover((50.0, 50.0)));

    assert!(session.navigate(WindowSnapDirection::Right, (999.0, 999.0)));
    assert_eq!(session.hovered_index(), Some(2));
    assert_eq!(
        session.hovered_target().map(WindowSnapTarget::screen_rect),
        Some(Rect::new(200, 0, 100, 100).unwrap())
    );
}

#[test]
fn output_logical_window_maps_to_authoritative_pixels_then_zoomed_screen() {
    let mut zoomed = source();
    zoomed.zoom_transformed = true;
    zoomed.zoom_scale = 1.5;
    zoomed.zoom_view_offset = (100.0, 50.0);
    let correlation = WindowSnapCorrelation::new(8, zoomed);
    let mut session = Some(WindowSnapSession::pending(correlation));

    assert_eq!(
        apply_window_query_completion(
            &mut session,
            correlation,
            vec![target(Rect::new(200, 100, 300, 200).unwrap())],
        ),
        WindowQueryApply::Ready
    );
    let session = session.unwrap();
    let mapped = &session.targets()[0];
    assert_eq!(mapped.image_rect().size(), (450, 300));
    assert_eq!(mapped.screen_rect(), Rect::new(150, 75, 450, 300).unwrap());
}

#[test]
fn stale_completion_cannot_replace_a_fresh_generation() {
    let stale = WindowSnapCorrelation::new(3, source());
    let fresh = WindowSnapCorrelation::new(4, source());
    let mut session = Some(WindowSnapSession::pending(fresh));

    assert_eq!(
        apply_window_query_completion(
            &mut session,
            stale,
            vec![target(Rect::new(0, 0, 100, 100).unwrap())],
        ),
        WindowQueryApply::Stale
    );
    assert_eq!(session.unwrap().correlation(), fresh);
}

#[test]
fn empty_completion_removes_window_snap_availability_quietly() {
    let correlation = WindowSnapCorrelation::new(3, source());
    let mut session = Some(WindowSnapSession::pending(correlation));

    assert_eq!(
        apply_window_query_completion(&mut session, correlation, Vec::new()),
        WindowQueryApply::Unavailable
    );
    assert_eq!(session, None);
}

#[test]
fn cancel_then_immediate_reopen_retries_after_the_old_terminal() {
    let old = WindowSnapCorrelation::new(3, source());
    // Cancelling discarded the old session while its capacity-one broker job
    // was still running. Reopening installs the new request as queued.
    let fresh = WindowSnapCorrelation::new(4, source());
    let mut session = Some(WindowSnapSession::queued(fresh, context()));

    assert_eq!(
        apply_window_query_completion(
            &mut session,
            old,
            vec![target(Rect::new(0, 0, 100, 100).unwrap())],
        ),
        WindowQueryApply::Stale
    );
    let retry = session
        .as_ref()
        .and_then(WindowSnapSession::queued_query)
        .expect("the reopened picker remains queued for retry");
    assert_eq!(retry.correlation, fresh);
    assert!(
        session
            .as_mut()
            .unwrap()
            .mark_query_started(retry.correlation)
    );
    assert_eq!(
        apply_window_query_completion(
            &mut session,
            fresh,
            vec![target(Rect::new(20, 20, 100, 100).unwrap())],
        ),
        WindowQueryApply::Ready
    );
    let session = session.unwrap();
    let mapped = &session.targets()[0];
    assert_eq!(mapped.image_rect().size(), (150, 150));
    assert_eq!(mapped.screen_rect(), Rect::new(20, 20, 100, 100).unwrap());
}

#[test]
fn repeated_reopen_and_cancel_only_retries_the_latest_live_picker() {
    let old = WindowSnapCorrelation::new(1, source());
    let mut session = None;
    assert!(session.is_none());
    for generation in 2..=5 {
        let correlation = WindowSnapCorrelation::new(generation, source());
        session = Some(WindowSnapSession::queued(correlation, context()));
        assert_eq!(session.as_ref().unwrap().correlation(), correlation);
        session = None;
        assert!(session.is_none());
    }
    let latest = WindowSnapCorrelation::new(6, source());
    session = Some(WindowSnapSession::queued(latest, context()));

    assert_eq!(
        apply_window_query_completion(
            &mut session,
            old,
            vec![target(Rect::new(0, 0, 100, 100).unwrap())],
        ),
        WindowQueryApply::Stale
    );
    let retry = session
        .as_ref()
        .and_then(WindowSnapSession::queued_query)
        .expect("only the latest live picker is retried");
    assert_eq!(retry.correlation, latest);
}
