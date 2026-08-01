use super::*;
use crate::util::Rect;

fn record_sample(metrics: &mut PerfMetrics, received_at: Instant) {
    metrics.record_input_sample(
        PerfInputSource::Pointer,
        Tool::Pen,
        12,
        10,
        20,
        30,
        40,
        false,
        received_at,
    );
}

fn frame_context(
    render_duration: Option<Duration>,
    dirty_area_pct: f64,
    full_damage: bool,
    damage_rects: usize,
    force_full_reason: Option<FullDamageReason>,
) -> PerfFrameContext {
    PerfFrameContext {
        render_duration,
        dirty_area_pct,
        full_damage,
        damage_rects,
        force_full_reason,
        damage_diagnostics: PerfDamageDiagnostics::default(),
    }
}

#[test]
fn percentile_uses_nearest_rank() {
    let values = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    assert_eq!(percentile_nearest_rank(&values, 50), Some(5));
    assert_eq!(percentile_nearest_rank(&values, 95), Some(10));
    assert_eq!(percentile_nearest_rank(&values, 99), Some(10));
    assert_eq!(percentile_nearest_rank(&[], 95), None);
}

#[test]
fn disabled_metrics_do_not_store_or_report_samples() {
    let base = Instant::now();
    let mut metrics = PerfMetrics::new(false);

    record_sample(&mut metrics, base);
    let report = metrics.commit_frame(
        frame_context(Some(Duration::from_millis(2)), 1.0, false, 1, None),
        base + Duration::from_millis(16),
    );

    assert!(report.is_none());
    assert!(metrics.pending_input_samples.is_empty());
    assert!(metrics.recent_latencies_ms.is_empty());
}

#[test]
fn fake_input_to_commit_flow_records_latency_and_slow_frame_context() {
    let base = Instant::now();
    let mut metrics = PerfMetrics::new(true);
    metrics.begin_render(base + Duration::from_millis(40));
    record_sample(&mut metrics, base);
    metrics.record_input_sample(
        PerfInputSource::Stylus,
        Tool::Marker,
        27,
        100,
        110,
        120,
        130,
        true,
        base + Duration::from_millis(10),
    );

    let report = metrics
        .commit_frame(
            frame_context(None, 2.5, false, 3, None),
            base + Duration::from_millis(70),
        )
        .expect("enabled metrics should report commits");

    assert_eq!(report.sample_count, 2);
    assert_eq!(report.max_latency_ms, 70);
    let slow = report.slow_frame.expect("slow sample should be reported");
    assert_eq!(slow.source, PerfInputSource::Pointer);
    assert_eq!(slow.tool, Tool::Pen);
    assert_eq!(slow.point_count, 12);
    assert_eq!(slow.render_ms, Some(30));
    assert_eq!(
        metrics
            .recent_latencies_ms
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![70, 60]
    );
}

#[test]
fn summary_reports_p95_and_p99_after_frame_interval() {
    let base = Instant::now();
    let mut metrics = PerfMetrics::new(true);

    for frame in 0..SUMMARY_FRAME_INTERVAL {
        let commit_at = base + Duration::from_millis(frame);
        metrics.record_input_sample(
            PerfInputSource::Touch,
            Tool::Eraser,
            frame as usize,
            1,
            2,
            3,
            4,
            false,
            commit_at - Duration::from_millis(frame + 1),
        );
        let report = metrics.commit_frame(
            frame_context(Some(Duration::from_millis(1)), 0.5, false, 1, None),
            commit_at,
        );
        if frame + 1 < SUMMARY_FRAME_INTERVAL {
            assert!(report.and_then(|r| r.summary).is_none());
        } else {
            let summary = report
                .and_then(|r| r.summary)
                .expect("summary at frame interval");
            assert_eq!(summary.frames, SUMMARY_FRAME_INTERVAL);
            assert_eq!(summary.samples, SUMMARY_FRAME_INTERVAL);
            assert_eq!(summary.window_samples, SUMMARY_FRAME_INTERVAL as usize);
            assert_eq!(summary.p95_ms, 114);
            assert_eq!(summary.p99_ms, 119);
        }
    }
}

#[test]
fn summary_reports_after_time_interval_before_frame_interval() {
    let base = Instant::now();
    let mut metrics = PerfMetrics::new(true);
    record_sample(&mut metrics, base);
    let first_report = metrics.commit_frame(
        frame_context(Some(Duration::from_millis(1)), 0.5, false, 1, None),
        base + Duration::from_millis(10),
    );
    assert!(first_report.and_then(|report| report.summary).is_none());

    record_sample(&mut metrics, base + Duration::from_secs(5));
    let second_report = metrics
        .commit_frame(
            frame_context(Some(Duration::from_millis(1)), 0.5, false, 1, None),
            base + Duration::from_secs(5) + Duration::from_millis(20),
        )
        .expect("enabled metrics should report commits");

    let summary = second_report.summary.expect("summary after time interval");
    assert_eq!(summary.frames, 2);
    assert_eq!(summary.samples, 2);
    assert_eq!(summary.p95_ms, 20);
    assert_eq!(summary.p99_ms, 20);
}

#[test]
fn final_summary_flushes_partial_frame_and_input_windows_once() {
    let base = Instant::now();
    let mut metrics = PerfMetrics::new(true);

    record_sample(&mut metrics, base);
    let _ = metrics.commit_frame(
        frame_context(Some(Duration::from_millis(2)), 100.0, true, 1, None),
        base + Duration::from_millis(9),
    );
    let _ = metrics.record_render_complete(
        base + Duration::from_millis(10),
        base + Duration::from_millis(12),
        false,
        120,
        false,
    );

    let report = metrics.flush_pending_summaries(base + Duration::from_millis(20));

    let input = report.input.expect("partial input summary should flush");
    assert_eq!(input.frames, 1);
    assert_eq!(input.samples, 1);
    assert_eq!(input.p95_ms, 9);

    let frame_pacing = report
        .frame_pacing
        .expect("partial frame pacing summary should flush");
    assert_eq!(frame_pacing.frames, 1);
    assert_eq!(frame_pacing.render_p95_ms, 2);
    assert_eq!(frame_pacing.full_damage_count, 1);
    assert_eq!(frame_pacing.full_damage_pct, "100.00");
    assert_eq!(
        frame_pacing.force_full_reasons,
        "damage_regions_cover_surface:1"
    );

    assert_eq!(metrics.frames_since_summary, 0);
    assert_eq!(metrics.samples_since_summary, 0);
    assert_eq!(metrics.render_frames_since_summary, 0);
    assert_eq!(metrics.full_damage_count, 0);
    assert_eq!(
        metrics.flush_pending_summaries(base + Duration::from_millis(30)),
        PerfFinalSummaryReport::default()
    );
}

#[test]
fn slow_frame_reports_render_budget_and_damage_context() {
    let base = Instant::now();
    let mut metrics = PerfMetrics::new(true);
    let _ = metrics.commit_frame(
        frame_context(
            Some(Duration::from_millis(1)),
            42.0,
            true,
            2,
            Some(FullDamageReason::CanvasClear),
        ),
        base,
    );

    let report = metrics
        .record_render_complete(base, base + Duration::from_millis(12), false, 120, false)
        .expect("enabled metrics should report render frames");

    let slow = report.slow_frame.expect("12ms exceeds the 120 FPS budget");
    assert_eq!(slow.frame, 1);
    assert_eq!(slow.render_ms, 12);
    assert_eq!(slow.budget_ms, Some(8));
    assert_eq!(slow.max_fps_no_vsync, 120);
    assert_eq!(slow.dirty_area_pct, 42.0);
    assert!(slow.full_damage);
    assert_eq!(slow.force_full_reason, Some(FullDamageReason::CanvasClear));
    assert_eq!(slow.damage_rects, 2);
}

#[test]
fn frame_pacing_summary_reports_render_percentiles_and_skips() {
    let base = Instant::now();
    let mut metrics = PerfMetrics::new(true);

    metrics.record_render_skip(PerfRenderSkipReason::FrameCallbackPending);
    metrics.record_render_skip(PerfRenderSkipReason::FpsCap);
    metrics.record_render_skip(PerfRenderSkipReason::SurfaceUnconfigured);
    metrics.record_render_skip(PerfRenderSkipReason::NoRedraw);

    for frame in 0..SUMMARY_FRAME_INTERVAL {
        let started_at = base + Duration::from_millis(frame * 2);
        let duration = Duration::from_millis(frame + 1);
        if frame % 40 == 0 {
            let _ = metrics.commit_frame(
                frame_context(
                    Some(Duration::from_millis(1)),
                    100.0,
                    true,
                    1,
                    Some(FullDamageReason::CanvasClear),
                ),
                started_at,
            );
        }
        let report =
            metrics.record_render_complete(started_at, started_at + duration, true, 120, false);
        if frame + 1 < SUMMARY_FRAME_INTERVAL {
            assert!(report.and_then(|r| r.summary).is_none());
        } else {
            let summary = report
                .and_then(|r| r.summary)
                .expect("summary at frame interval");
            assert_eq!(summary.frames, SUMMARY_FRAME_INTERVAL);
            assert_eq!(summary.window_frames, SUMMARY_FRAME_INTERVAL as usize);
            assert_eq!(summary.render_p95_ms, 114);
            assert_eq!(summary.render_p99_ms, 119);
            assert_eq!(summary.render_max_ms, 120);
            assert_eq!(summary.skipped_frame_callback_pending, 1);
            assert_eq!(summary.skipped_fps_cap, 1);
            assert_eq!(summary.skipped_surface_unconfigured, 1);
            assert_eq!(summary.skipped_no_redraw, 1);
            assert_eq!(summary.render_over_50ms, 70);
            assert_eq!(summary.full_damage_count, 3);
            assert_eq!(summary.full_damage_pct, "2.50");
            assert_eq!(summary.force_full_reason, "canvas_clear");
            assert_eq!(summary.force_full_reasons, "canvas_clear:3");
        }
    }
}

#[test]
fn frame_pacing_summary_separates_unreasoned_full_damage_from_expected_reasons() {
    let base = Instant::now();
    let mut metrics = PerfMetrics::new(true);

    for frame in 0..SUMMARY_FRAME_INTERVAL {
        let started_at = base + Duration::from_millis(frame * 2);
        let force_full_reason = match frame {
            0 => Some(FullDamageReason::CanvasClear),
            1 | 2 => None,
            _ => {
                let _ = metrics.commit_frame(
                    frame_context(Some(Duration::from_millis(1)), 0.5, false, 1, None),
                    started_at,
                );
                let report = metrics.record_render_complete(
                    started_at,
                    started_at + Duration::from_millis(1),
                    false,
                    120,
                    false,
                );
                if frame + 1 == SUMMARY_FRAME_INTERVAL {
                    let summary = report
                        .and_then(|report| report.summary)
                        .expect("summary at frame interval");
                    assert_eq!(summary.full_damage_count, 3);
                    assert_eq!(summary.full_damage_pct, "2.50");
                    assert_eq!(
                        summary.force_full_reasons,
                        "canvas_clear:1,damage_regions_cover_surface:2"
                    );
                }
                continue;
            }
        };

        let _ = metrics.commit_frame(
            frame_context(
                Some(Duration::from_millis(1)),
                100.0,
                true,
                1,
                force_full_reason,
            ),
            started_at,
        );
        let report = metrics.record_render_complete(
            started_at,
            started_at + Duration::from_millis(1),
            false,
            120,
            false,
        );
        assert!(report.and_then(|report| report.summary).is_none());
    }
}

#[test]
fn full_damage_source_prefers_input_full_over_generic_force() {
    let diagnostics = PerfDamageDiagnostics {
        input_full_reason: Some(FullDamageReason::FirstRunOnboarding),
        input_covers_surface: true,
        buffer_covers_surface: true,
        final_single_surface_rect: true,
        ..PerfDamageDiagnostics::default()
    };

    assert_eq!(
        full_damage_source(
            true,
            Some(FullDamageReason::FirstRunOnboarding),
            &diagnostics
        ),
        "input_full"
    );
}

#[test]
fn render_breakdown_summary_reports_stage_culling_and_cache_use() {
    let base = Instant::now();
    let mut metrics = PerfMetrics::new(true);
    metrics.record_render_breakdown(PerfRenderBreakdown {
        stages: PerfRenderStageDurations {
            completed_shapes: Duration::from_millis(9),
            provisional: Duration::from_millis(3),
            ..PerfRenderStageDurations::default()
        },
        surface_px: 2_000_000,
        shapes_total: 20,
        shapes_tested: 12,
        shapes_rendered: 3,
        provisional_points: 42,
        render_profile: PerfRenderProfileKind::Canvas,
        canvas_layer_cache_used: true,
    });
    let _ = metrics.commit_frame(
        frame_context(Some(Duration::from_millis(1)), 1.0, false, 2, None),
        base,
    );
    let _ =
        metrics.record_render_complete(base, base + Duration::from_millis(12), false, 120, false);

    let report = metrics.flush_pending_summaries(base + Duration::from_millis(20));
    let summary = report
        .render_breakdown
        .expect("render breakdown summary should flush");

    assert_eq!(summary.samples, 1);
    assert_eq!(summary.dominant_stage, "completed_shapes");
    assert_eq!(summary.dominant_stage_avg, Duration::from_millis(9));
    assert_eq!(summary.surface_px_max, 2_000_000);
    assert_eq!(summary.shapes_total_max, 20);
    assert_eq!(summary.shapes_tested_avg, 12);
    assert_eq!(summary.shapes_rendered_avg, 3);
    assert_eq!(summary.shape_cull_pct, "75.00");
    assert_eq!(summary.provisional_points_max, 42);
    assert_eq!(summary.render_profile_frames, 1);
    assert_eq!(summary.canvas_layer_cache_used_frames, 1);
}

#[test]
fn damage_percentage_clamps_to_surface_bounds() {
    let damage = [
        Rect::new(-10, -10, 20, 20).unwrap(),
        Rect::new(50, 50, 100, 100).unwrap(),
    ];

    assert_eq!(damage_area_pct(&damage, 100, 100), 26.0);
    assert!(damage_covers_surface(
        &[Rect::new(0, 0, 100, 100).unwrap()],
        100,
        100
    ));
}
