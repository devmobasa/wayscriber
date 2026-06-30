use std::{
    collections::VecDeque,
    fmt,
    time::{Duration, Instant},
};

use log::info;

use crate::{
    env_vars::PERF_LOG_ENV,
    input::{DrawingState, Tool},
    util::Rect,
};

use super::WaylandState;

const MAX_PENDING_INPUT_SAMPLES: usize = 4096;
const MAX_RECENT_LATENCIES: usize = 2048;
const SUMMARY_FRAME_INTERVAL: u64 = 120;
const SUMMARY_INTERVAL: Duration = Duration::from_secs(5);
const SLOW_INPUT_TO_COMMIT: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(tablet), allow(dead_code))]
pub(in crate::backend::wayland) enum PerfInputSource {
    Pointer,
    Touch,
    Stylus,
}

impl fmt::Display for PerfInputSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pointer => f.write_str("pointer"),
            Self::Touch => f.write_str("touch"),
            Self::Stylus => f.write_str("stylus"),
        }
    }
}

#[derive(Clone, Debug)]
struct PerfInputSample {
    received_at: Instant,
    source: PerfInputSource,
    tool: Tool,
    point_count: usize,
    screen_x: i32,
    screen_y: i32,
    canvas_x: i32,
    canvas_y: i32,
    pressure_sample: bool,
}

#[derive(Clone, Copy, Debug)]
struct PerfFrameContext {
    render_duration: Option<Duration>,
    dirty_area_pct: f64,
    full_damage: bool,
    damage_rects: usize,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
struct PerfCommitReport {
    sample_count: usize,
    max_latency_ms: u64,
    slow_frame: Option<PerfSlowFrame>,
    summary: Option<PerfSummary>,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
struct PerfSlowFrame {
    latency_ms: u64,
    source: PerfInputSource,
    tool: Tool,
    point_count: usize,
    screen_x: i32,
    screen_y: i32,
    canvas_x: i32,
    canvas_y: i32,
    pressure_sample: bool,
    render_ms: Option<u64>,
    dirty_area_pct: f64,
    full_damage: bool,
    damage_rects: usize,
    dropped_input_samples: u64,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct PerfSummary {
    frames: u64,
    samples: u64,
    window_samples: usize,
    p50_ms: u64,
    p95_ms: u64,
    p99_ms: u64,
    max_ms: u64,
    dropped_input_samples: u64,
}

#[derive(Debug)]
pub(super) struct PerfMetrics {
    enabled: bool,
    pending_input_samples: VecDeque<PerfInputSample>,
    recent_latencies_ms: VecDeque<u64>,
    render_started_at: Option<Instant>,
    frames_since_summary: u64,
    samples_since_summary: u64,
    dropped_input_samples: u64,
    last_summary_at: Option<Instant>,
}

impl PerfMetrics {
    pub(super) fn from_env() -> Self {
        let enabled = perf_log_enabled_from_env();
        if enabled {
            info!("Performance logging enabled via {PERF_LOG_ENV}=1");
        }
        Self::new(enabled)
    }

    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            pending_input_samples: VecDeque::new(),
            recent_latencies_ms: VecDeque::new(),
            render_started_at: None,
            frames_since_summary: 0,
            samples_since_summary: 0,
            dropped_input_samples: 0,
            last_summary_at: None,
        }
    }

    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn begin_render(&mut self, now: Instant) {
        if !self.enabled {
            return;
        }
        self.render_started_at = Some(now);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn record_input_sample(
        &mut self,
        source: PerfInputSource,
        tool: Tool,
        point_count: usize,
        screen_x: i32,
        screen_y: i32,
        canvas_x: i32,
        canvas_y: i32,
        pressure_sample: bool,
        received_at: Instant,
    ) {
        if !self.enabled {
            return;
        }
        if self.pending_input_samples.len() == MAX_PENDING_INPUT_SAMPLES {
            self.pending_input_samples.pop_front();
            self.dropped_input_samples += 1;
        }
        self.pending_input_samples.push_back(PerfInputSample {
            received_at,
            source,
            tool,
            point_count,
            screen_x,
            screen_y,
            canvas_x,
            canvas_y,
            pressure_sample,
        });
    }

    fn commit_frame(
        &mut self,
        frame: PerfFrameContext,
        commit_at: Instant,
    ) -> Option<PerfCommitReport> {
        if !self.enabled {
            return None;
        }

        let render_duration = frame.render_duration.or_else(|| {
            self.render_started_at
                .map(|start| commit_at.saturating_duration_since(start))
        });
        self.render_started_at = None;

        let mut sample_count: usize = 0;
        let mut max_latency = Duration::ZERO;
        let mut slowest_sample = None;

        while let Some(sample) = self.pending_input_samples.pop_front() {
            let latency = commit_at.saturating_duration_since(sample.received_at);
            self.push_latency_ms(duration_ms(latency));
            sample_count += 1;
            if latency >= max_latency {
                max_latency = latency;
                slowest_sample = Some(sample);
            }
        }

        self.frames_since_summary += 1;
        self.samples_since_summary += sample_count as u64;
        if self.last_summary_at.is_none() {
            self.last_summary_at = Some(commit_at);
        }

        let slow_frame = if max_latency >= SLOW_INPUT_TO_COMMIT {
            slowest_sample.map(|sample| PerfSlowFrame {
                latency_ms: duration_ms(max_latency),
                source: sample.source,
                tool: sample.tool,
                point_count: sample.point_count,
                screen_x: sample.screen_x,
                screen_y: sample.screen_y,
                canvas_x: sample.canvas_x,
                canvas_y: sample.canvas_y,
                pressure_sample: sample.pressure_sample,
                render_ms: render_duration.map(duration_ms),
                dirty_area_pct: frame.dirty_area_pct,
                full_damage: frame.full_damage,
                damage_rects: frame.damage_rects,
                dropped_input_samples: self.dropped_input_samples,
            })
        } else {
            None
        };

        if let Some(slow) = slow_frame.as_ref() {
            info!(
                "perf.slow_input_to_paint proxy=input_to_wayland_commit latency_ms={} source={} tool={:?} points={} pressure_sample={} screen=({}, {}) canvas=({}, {}) render_ms={} dirty_area_pct={:.2} full_damage={} damage_rects={} dropped_input_samples={}",
                slow.latency_ms,
                slow.source,
                slow.tool,
                slow.point_count,
                slow.pressure_sample,
                slow.screen_x,
                slow.screen_y,
                slow.canvas_x,
                slow.canvas_y,
                format_optional_ms(slow.render_ms),
                slow.dirty_area_pct,
                slow.full_damage,
                slow.damage_rects,
                slow.dropped_input_samples
            );
        }

        let summary = if self.summary_due(commit_at) && !self.recent_latencies_ms.is_empty() {
            let summary = self.build_summary();
            info!(
                "perf.input_to_paint_latency proxy=input_to_wayland_commit frames={} samples={} window_samples={} p50_ms={} p95_ms={} p99_ms={} max_ms={} dropped_input_samples={}",
                summary.frames,
                summary.samples,
                summary.window_samples,
                summary.p50_ms,
                summary.p95_ms,
                summary.p99_ms,
                summary.max_ms,
                summary.dropped_input_samples
            );
            self.frames_since_summary = 0;
            self.samples_since_summary = 0;
            self.last_summary_at = Some(commit_at);
            Some(summary)
        } else {
            None
        };

        Some(PerfCommitReport {
            sample_count,
            max_latency_ms: duration_ms(max_latency),
            slow_frame,
            summary,
        })
    }

    fn push_latency_ms(&mut self, latency_ms: u64) {
        if self.recent_latencies_ms.len() == MAX_RECENT_LATENCIES {
            self.recent_latencies_ms.pop_front();
        }
        self.recent_latencies_ms.push_back(latency_ms);
    }

    fn summary_due(&self, now: Instant) -> bool {
        if self.frames_since_summary >= SUMMARY_FRAME_INTERVAL {
            return true;
        }
        self.last_summary_at
            .is_some_and(|last| now.saturating_duration_since(last) >= SUMMARY_INTERVAL)
    }

    fn build_summary(&self) -> PerfSummary {
        let mut sorted = self.recent_latencies_ms.iter().copied().collect::<Vec<_>>();
        sorted.sort_unstable();

        PerfSummary {
            frames: self.frames_since_summary,
            samples: self.samples_since_summary,
            window_samples: sorted.len(),
            p50_ms: percentile_nearest_rank(&sorted, 50).unwrap_or(0),
            p95_ms: percentile_nearest_rank(&sorted, 95).unwrap_or(0),
            p99_ms: percentile_nearest_rank(&sorted, 99).unwrap_or(0),
            max_ms: sorted.last().copied().unwrap_or(0),
            dropped_input_samples: self.dropped_input_samples,
        }
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn begin_perf_render(&mut self, now: Instant) {
        self.perf.begin_render(now);
    }

    pub(in crate::backend::wayland) fn record_perf_input_sample(
        &mut self,
        source: PerfInputSource,
        screen_x: i32,
        screen_y: i32,
        canvas_x: i32,
        canvas_y: i32,
        pressure_sample: bool,
    ) {
        if !self.perf.enabled() {
            return;
        }
        let Some((tool, point_count)) = self.active_drawing_perf_context() else {
            return;
        };
        self.perf.record_input_sample(
            source,
            tool,
            point_count,
            screen_x,
            screen_y,
            canvas_x,
            canvas_y,
            pressure_sample,
            Instant::now(),
        );
    }

    pub(in crate::backend::wayland) fn commit_perf_frame(
        &mut self,
        damage_screen: &[Rect],
        logical_width: u32,
        logical_height: u32,
        damage_rects: usize,
        commit_at: Instant,
    ) {
        if !self.perf.enabled() {
            return;
        }
        let frame = PerfFrameContext {
            render_duration: None,
            dirty_area_pct: damage_area_pct(damage_screen, logical_width, logical_height),
            full_damage: damage_covers_surface(damage_screen, logical_width, logical_height),
            damage_rects,
        };
        let _ = self.perf.commit_frame(frame, commit_at);
    }

    fn active_drawing_perf_context(&self) -> Option<(Tool, usize)> {
        let DrawingState::Drawing { tool, points, .. } = &self.input_state.state else {
            return None;
        };
        Some((*tool, points.len()))
    }
}

fn perf_log_enabled_from_env() -> bool {
    std::env::var(PERF_LOG_ENV)
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on" | "ON"))
        .unwrap_or(false)
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn format_optional_ms(value: Option<u64>) -> String {
    value.map_or_else(|| "n/a".to_string(), |ms| ms.to_string())
}

fn percentile_nearest_rank(sorted_values: &[u64], percentile: u64) -> Option<u64> {
    if sorted_values.is_empty() {
        return None;
    }
    let rank = ((percentile as f64 / 100.0) * sorted_values.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted_values.len() - 1);
    sorted_values.get(index).copied()
}

fn damage_area_pct(damage: &[Rect], logical_width: u32, logical_height: u32) -> f64 {
    let surface_area = u64::from(logical_width).saturating_mul(u64::from(logical_height));
    if surface_area == 0 {
        return 0.0;
    }

    let damage_area = damage
        .iter()
        .map(|rect| clamped_rect_area(*rect, logical_width, logical_height))
        .sum::<u64>();
    ((damage_area as f64 / surface_area as f64) * 100.0).min(100.0)
}

fn damage_covers_surface(damage: &[Rect], logical_width: u32, logical_height: u32) -> bool {
    let width = logical_width.min(i32::MAX as u32) as i32;
    let height = logical_height.min(i32::MAX as u32) as i32;
    damage
        .iter()
        .any(|rect| rect.x <= 0 && rect.y <= 0 && rect.width >= width && rect.height >= height)
}

fn clamped_rect_area(rect: Rect, logical_width: u32, logical_height: u32) -> u64 {
    let max_x = logical_width.min(i32::MAX as u32) as i32;
    let max_y = logical_height.min(i32::MAX as u32) as i32;
    let left = rect.x.clamp(0, max_x);
    let top = rect.y.clamp(0, max_y);
    let right = rect.x.saturating_add(rect.width).clamp(0, max_x);
    let bottom = rect.y.saturating_add(rect.height).clamp(0, max_y);
    if right <= left || bottom <= top {
        return 0;
    }
    (right - left) as u64 * (bottom - top) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

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
            PerfFrameContext {
                render_duration: Some(Duration::from_millis(2)),
                dirty_area_pct: 1.0,
                full_damage: false,
                damage_rects: 1,
            },
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
                PerfFrameContext {
                    render_duration: None,
                    dirty_area_pct: 2.5,
                    full_damage: false,
                    damage_rects: 3,
                },
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
                PerfFrameContext {
                    render_duration: Some(Duration::from_millis(1)),
                    dirty_area_pct: 0.5,
                    full_damage: false,
                    damage_rects: 1,
                },
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
            PerfFrameContext {
                render_duration: Some(Duration::from_millis(1)),
                dirty_area_pct: 0.5,
                full_damage: false,
                damage_rects: 1,
            },
            base + Duration::from_millis(10),
        );
        assert!(first_report.and_then(|report| report.summary).is_none());

        record_sample(&mut metrics, base + Duration::from_secs(5));
        let second_report = metrics
            .commit_frame(
                PerfFrameContext {
                    render_duration: Some(Duration::from_millis(1)),
                    dirty_area_pct: 0.5,
                    full_damage: false,
                    damage_rects: 1,
                },
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
}
