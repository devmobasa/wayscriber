use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    time::{Duration, Instant},
};

use log::info;

use crate::{
    env_vars::PERF_LOG_ENV,
    input::{DrawingState, Tool},
};

use super::{FullDamageReason, WaylandState};

#[path = "perf_modules/damage_diagnostics.rs"]
mod damage_diagnostics;
#[path = "perf_modules/render_breakdown.rs"]
mod render_breakdown;

pub(in crate::backend::wayland) use damage_diagnostics::{
    PerfDamageDiagnostics, PerfFrameDamageContext, damage_covers_logical_surface,
};
use damage_diagnostics::{
    damage_area_pct, damage_covers_surface, effective_full_damage_reason,
    format_effective_full_damage_reason, format_pct_hundredths, full_damage_source,
    largest_region_area_pct_hundredths,
};
#[cfg(test)]
use render_breakdown::PerfRenderStageDurations;
pub(in crate::backend::wayland) use render_breakdown::{
    PerfRenderBreakdown, PerfRenderProfileKind,
};
use render_breakdown::{
    PerfRenderBreakdownAccumulator, PerfRenderBreakdownSummary, log_render_stage_frame,
    log_render_stage_summary,
};

const MAX_PENDING_INPUT_SAMPLES: usize = 4096;
const MAX_RECENT_LATENCIES: usize = 2048;
const MAX_RECENT_RENDER_DURATIONS: usize = 2048;
const SUMMARY_FRAME_INTERVAL: u64 = 120;
const SUMMARY_INTERVAL: Duration = Duration::from_secs(5);
const SLOW_INPUT_TO_COMMIT: Duration = Duration::from_millis(50);
const VSYNC_ASSUMED_FRAME_BUDGET: Duration = Duration::from_micros(16_667);
const SLOW_RENDER_FALLBACK: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(feature = "tablet-input"), allow(dead_code))]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::backend::wayland) enum PerfRenderSkipReason {
    FrameCallbackPending,
    FpsCap,
    SurfaceUnconfigured,
    NoRedraw,
}

impl fmt::Display for PerfRenderSkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameCallbackPending => f.write_str("frame_callback_pending"),
            Self::FpsCap => f.write_str("fps_cap"),
            Self::SurfaceUnconfigured => f.write_str("surface_unconfigured"),
            Self::NoRedraw => f.write_str("no_redraw"),
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
    force_full_reason: Option<FullDamageReason>,
    damage_diagnostics: PerfDamageDiagnostics,
}

impl Default for PerfFrameContext {
    fn default() -> Self {
        Self {
            render_duration: None,
            dirty_area_pct: 0.0,
            full_damage: false,
            damage_rects: 0,
            force_full_reason: None,
            damage_diagnostics: PerfDamageDiagnostics::default(),
        }
    }
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
    full_damage_reason: Option<FullDamageReason>,
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

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
struct PerfFramePacingReport {
    slow_frame: Option<PerfSlowRenderFrame>,
    summary: Option<PerfFramePacingSummary>,
    render_breakdown_summary: Option<PerfRenderBreakdownSummary>,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, Default, PartialEq)]
struct PerfFinalSummaryReport {
    input: Option<PerfSummary>,
    frame_pacing: Option<PerfFramePacingSummary>,
    render_breakdown: Option<PerfRenderBreakdownSummary>,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
struct PerfSlowRenderFrame {
    frame: u64,
    render_ms: u64,
    budget_ms: Option<u64>,
    vsync_enabled: bool,
    max_fps_no_vsync: u32,
    dirty_area_pct: f64,
    render_breakdown: Option<PerfRenderBreakdown>,
    full_damage: bool,
    damage_rects: usize,
    force_full_reason: Option<FullDamageReason>,
    damage_diagnostics: PerfDamageDiagnostics,
    keep_rendering: bool,
    skipped_frame_callback_pending: u64,
    skipped_fps_cap: u64,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct PerfFramePacingSummary {
    frames: u64,
    window_frames: usize,
    render_p50_ms: u64,
    render_p95_ms: u64,
    render_p99_ms: u64,
    render_max_ms: u64,
    render_over_8ms: u64,
    render_over_16ms: u64,
    render_over_33ms: u64,
    render_over_50ms: u64,
    full_damage_count: u64,
    full_damage_pct: String,
    force_full_reason: String,
    force_full_reasons: String,
    skipped_frame_callback_pending: u64,
    skipped_fps_cap: u64,
    skipped_surface_unconfigured: u64,
    skipped_no_redraw: u64,
}

#[derive(Debug)]
pub(super) struct PerfMetrics {
    enabled: bool,
    pending_input_samples: VecDeque<PerfInputSample>,
    recent_latencies_ms: VecDeque<u64>,
    recent_render_ms: VecDeque<u64>,
    render_started_at: Option<Instant>,
    last_frame_context: Option<PerfFrameContext>,
    last_render_breakdown: Option<PerfRenderBreakdown>,
    frames_since_summary: u64,
    samples_since_summary: u64,
    render_frame_id: u64,
    render_frames_since_summary: u64,
    render_over_8ms: u64,
    render_over_16ms: u64,
    render_over_33ms: u64,
    render_over_50ms: u64,
    full_damage_count: u64,
    full_damage_reasons: BTreeMap<FullDamageReason, u64>,
    render_breakdown: PerfRenderBreakdownAccumulator,
    skipped_frame_callback_pending: u64,
    skipped_fps_cap: u64,
    skipped_surface_unconfigured: u64,
    skipped_no_redraw: u64,
    dropped_input_samples: u64,
    last_summary_at: Option<Instant>,
    last_frame_pacing_summary_at: Option<Instant>,
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
            recent_render_ms: VecDeque::new(),
            render_started_at: None,
            last_frame_context: None,
            last_render_breakdown: None,
            frames_since_summary: 0,
            samples_since_summary: 0,
            render_frame_id: 0,
            render_frames_since_summary: 0,
            render_over_8ms: 0,
            render_over_16ms: 0,
            render_over_33ms: 0,
            render_over_50ms: 0,
            full_damage_count: 0,
            full_damage_reasons: BTreeMap::new(),
            render_breakdown: PerfRenderBreakdownAccumulator::default(),
            skipped_frame_callback_pending: 0,
            skipped_fps_cap: 0,
            skipped_surface_unconfigured: 0,
            skipped_no_redraw: 0,
            dropped_input_samples: 0,
            last_summary_at: None,
            last_frame_pacing_summary_at: None,
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
        self.last_render_breakdown = None;
    }

    fn record_render_breakdown(&mut self, breakdown: PerfRenderBreakdown) {
        if !self.enabled {
            return;
        }
        self.last_render_breakdown = Some(breakdown);
    }

    fn record_render_skip(&mut self, reason: PerfRenderSkipReason) {
        if !self.enabled {
            return;
        }
        match reason {
            PerfRenderSkipReason::FrameCallbackPending => {
                self.skipped_frame_callback_pending += 1;
            }
            PerfRenderSkipReason::FpsCap => {
                self.skipped_fps_cap += 1;
            }
            PerfRenderSkipReason::SurfaceUnconfigured => {
                self.skipped_surface_unconfigured += 1;
            }
            PerfRenderSkipReason::NoRedraw => {
                self.skipped_no_redraw += 1;
            }
        }
    }

    fn record_render_complete(
        &mut self,
        render_started_at: Instant,
        render_finished_at: Instant,
        vsync_enabled: bool,
        max_fps_no_vsync: u32,
        keep_rendering: bool,
    ) -> Option<PerfFramePacingReport> {
        if !self.enabled {
            return None;
        }

        let render_duration = render_finished_at.saturating_duration_since(render_started_at);
        let render_ms = duration_ms(render_duration);
        self.push_render_ms(render_ms);
        self.render_frame_id += 1;
        self.render_frames_since_summary += 1;
        self.count_render_duration(render_duration);
        if self.last_frame_pacing_summary_at.is_none() {
            self.last_frame_pacing_summary_at = Some(render_finished_at);
        }

        let frame = self.last_frame_context.take().unwrap_or_default();
        let render_breakdown = self.last_render_breakdown.take();
        self.count_full_damage(&frame);
        if let Some(breakdown) = render_breakdown.as_ref() {
            self.count_render_breakdown(breakdown);
        }
        let budget = frame_budget_duration(vsync_enabled, max_fps_no_vsync);
        let slow_frame = if budget.is_some_and(|budget| render_duration > budget)
            || render_duration > SLOW_RENDER_FALLBACK
        {
            Some(PerfSlowRenderFrame {
                frame: self.render_frame_id,
                render_ms,
                budget_ms: budget.map(duration_ms),
                vsync_enabled,
                max_fps_no_vsync,
                dirty_area_pct: frame.dirty_area_pct,
                render_breakdown,
                full_damage: frame.full_damage,
                damage_rects: frame.damage_rects,
                force_full_reason: frame.force_full_reason,
                damage_diagnostics: frame.damage_diagnostics,
                keep_rendering,
                skipped_frame_callback_pending: self.skipped_frame_callback_pending,
                skipped_fps_cap: self.skipped_fps_cap,
            })
        } else {
            None
        };

        if let Some(slow) = slow_frame.as_ref() {
            info!(
                "perf.slow_frame frame={} render_ms={} budget_ms={} vsync={} max_fps_no_vsync={} dirty_area_pct={:.2} full_damage={} full_damage_reason={} force_full_reason={} full_damage_source={} damage_rects={} input_damage_rects={} input_full_reason={} input_covers_surface={} buffer_damage_rects_before_merge={} buffer_damage_rects_after_merge={} buffer_covers_surface={} final_single_surface_rect={} largest_damage_rect_pct={} keep_rendering={} skipped_frame_callback_pending={} skipped_fps_cap={}",
                slow.frame,
                slow.render_ms,
                format_optional_ms(slow.budget_ms),
                slow.vsync_enabled,
                slow.max_fps_no_vsync,
                slow.dirty_area_pct,
                slow.full_damage,
                format_effective_full_damage_reason(slow.full_damage, slow.force_full_reason),
                format_force_full_reason(slow.force_full_reason),
                full_damage_source(
                    slow.full_damage,
                    slow.force_full_reason,
                    &slow.damage_diagnostics
                ),
                slow.damage_rects,
                slow.damage_diagnostics.input_regions,
                format_force_full_reason(slow.damage_diagnostics.input_full_reason),
                slow.damage_diagnostics.input_covers_surface,
                slow.damage_diagnostics.buffer_regions_before_merge,
                slow.damage_diagnostics.buffer_regions_after_merge,
                slow.damage_diagnostics.buffer_covers_surface,
                slow.damage_diagnostics.final_single_surface_rect,
                format_pct_hundredths(slow.damage_diagnostics.largest_region_area_pct_hundredths),
                slow.keep_rendering,
                slow.skipped_frame_callback_pending,
                slow.skipped_fps_cap
            );
            if let Some(breakdown) = slow.render_breakdown.as_ref() {
                log_render_stage_frame(slow.frame, slow.render_ms, breakdown);
            }
        }

        let (summary, render_breakdown_summary) =
            if self.frame_pacing_summary_due(render_finished_at) {
                let summary = self.build_frame_pacing_summary();
                let render_breakdown_summary = self.build_render_breakdown_summary();
                log_frame_pacing_summary(&summary, false);
                if let Some(summary) = render_breakdown_summary.as_ref() {
                    log_render_stage_summary(summary, false);
                }
                self.reset_frame_pacing_summary(render_finished_at);
                (Some(summary), render_breakdown_summary)
            } else {
                (None, None)
            };

        Some(PerfFramePacingReport {
            slow_frame,
            summary,
            render_breakdown_summary,
        })
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
        self.last_frame_context = Some(frame);

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
                full_damage_reason: effective_full_damage_reason(
                    frame.full_damage,
                    frame.force_full_reason,
                ),
                damage_rects: frame.damage_rects,
                dropped_input_samples: self.dropped_input_samples,
            })
        } else {
            None
        };

        if let Some(slow) = slow_frame.as_ref() {
            info!(
                "perf.slow_input_to_paint proxy=input_to_wayland_commit latency_ms={} source={} tool={:?} points={} pressure_sample={} screen=({}, {}) canvas=({}, {}) render_ms={} dirty_area_pct={:.2} full_damage={} full_damage_reason={} damage_rects={} dropped_input_samples={}",
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
                format_force_full_reason(slow.full_damage_reason),
                slow.damage_rects,
                slow.dropped_input_samples
            );
        }

        let summary = if self.summary_due(commit_at) && !self.recent_latencies_ms.is_empty() {
            let summary = self.build_summary();
            log_input_summary(&summary, false);
            self.reset_input_summary(commit_at);
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

    fn push_render_ms(&mut self, render_ms: u64) {
        if self.recent_render_ms.len() == MAX_RECENT_RENDER_DURATIONS {
            self.recent_render_ms.pop_front();
        }
        self.recent_render_ms.push_back(render_ms);
    }

    fn count_render_duration(&mut self, render_duration: Duration) {
        if render_duration > Duration::from_millis(8) {
            self.render_over_8ms += 1;
        }
        if render_duration > Duration::from_millis(16) {
            self.render_over_16ms += 1;
        }
        if render_duration > Duration::from_millis(33) {
            self.render_over_33ms += 1;
        }
        if render_duration > Duration::from_millis(50) {
            self.render_over_50ms += 1;
        }
    }

    fn count_full_damage(&mut self, frame: &PerfFrameContext) {
        if !frame.full_damage {
            return;
        }
        self.full_damage_count += 1;
        let reason = frame
            .force_full_reason
            .unwrap_or(FullDamageReason::DamageRegionsCoverSurface);
        *self.full_damage_reasons.entry(reason).or_default() += 1;
    }

    fn count_render_breakdown(&mut self, breakdown: &PerfRenderBreakdown) {
        self.render_breakdown.record(breakdown);
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

    fn reset_input_summary(&mut self, now: Instant) {
        self.frames_since_summary = 0;
        self.samples_since_summary = 0;
        self.last_summary_at = Some(now);
    }

    fn frame_pacing_summary_due(&self, now: Instant) -> bool {
        if self.render_frames_since_summary >= SUMMARY_FRAME_INTERVAL {
            return true;
        }
        self.last_frame_pacing_summary_at
            .is_some_and(|last| now.saturating_duration_since(last) >= SUMMARY_INTERVAL)
    }

    fn build_frame_pacing_summary(&self) -> PerfFramePacingSummary {
        let mut sorted = self.recent_render_ms.iter().copied().collect::<Vec<_>>();
        sorted.sort_unstable();

        PerfFramePacingSummary {
            frames: self.render_frames_since_summary,
            window_frames: sorted.len(),
            render_p50_ms: percentile_nearest_rank(&sorted, 50).unwrap_or(0),
            render_p95_ms: percentile_nearest_rank(&sorted, 95).unwrap_or(0),
            render_p99_ms: percentile_nearest_rank(&sorted, 99).unwrap_or(0),
            render_max_ms: sorted.last().copied().unwrap_or(0),
            render_over_8ms: self.render_over_8ms,
            render_over_16ms: self.render_over_16ms,
            render_over_33ms: self.render_over_33ms,
            render_over_50ms: self.render_over_50ms,
            full_damage_count: self.full_damage_count,
            full_damage_pct: format_pct(self.full_damage_count, self.render_frames_since_summary),
            force_full_reason: dominant_full_damage_reason(&self.full_damage_reasons)
                .map_or_else(|| "none".to_string(), |reason| reason.as_str().to_string()),
            force_full_reasons: format_full_damage_reasons(&self.full_damage_reasons),
            skipped_frame_callback_pending: self.skipped_frame_callback_pending,
            skipped_fps_cap: self.skipped_fps_cap,
            skipped_surface_unconfigured: self.skipped_surface_unconfigured,
            skipped_no_redraw: self.skipped_no_redraw,
        }
    }

    fn build_render_breakdown_summary(&self) -> Option<PerfRenderBreakdownSummary> {
        self.render_breakdown
            .build_summary(self.render_frames_since_summary)
    }

    fn reset_frame_pacing_summary(&mut self, now: Instant) {
        self.render_frames_since_summary = 0;
        self.render_over_8ms = 0;
        self.render_over_16ms = 0;
        self.render_over_33ms = 0;
        self.render_over_50ms = 0;
        self.full_damage_count = 0;
        self.full_damage_reasons.clear();
        self.render_breakdown.reset();
        self.skipped_frame_callback_pending = 0;
        self.skipped_fps_cap = 0;
        self.skipped_surface_unconfigured = 0;
        self.skipped_no_redraw = 0;
        self.last_frame_pacing_summary_at = Some(now);
    }

    fn flush_pending_summaries(&mut self, now: Instant) -> PerfFinalSummaryReport {
        if !self.enabled {
            return PerfFinalSummaryReport::default();
        }

        let input = if self.samples_since_summary > 0 && !self.recent_latencies_ms.is_empty() {
            let summary = self.build_summary();
            log_input_summary(&summary, true);
            self.reset_input_summary(now);
            Some(summary)
        } else {
            None
        };

        let (frame_pacing, render_breakdown) =
            if self.render_frames_since_summary > 0 && !self.recent_render_ms.is_empty() {
                let summary = self.build_frame_pacing_summary();
                let render_breakdown_summary = self.build_render_breakdown_summary();
                log_frame_pacing_summary(&summary, true);
                if let Some(summary) = render_breakdown_summary.as_ref() {
                    log_render_stage_summary(summary, true);
                }
                self.reset_frame_pacing_summary(now);
                (Some(summary), render_breakdown_summary)
            } else {
                (None, None)
            };

        PerfFinalSummaryReport {
            input,
            frame_pacing,
            render_breakdown,
        }
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn perf_enabled(&self) -> bool {
        self.perf.enabled()
    }

    pub(in crate::backend::wayland) fn begin_perf_render(&mut self, now: Instant) {
        self.perf.begin_render(now);
    }

    pub(in crate::backend::wayland) fn record_perf_render_breakdown(
        &mut self,
        breakdown: PerfRenderBreakdown,
    ) {
        self.perf.record_render_breakdown(breakdown);
    }

    pub(in crate::backend::wayland) fn record_perf_render_skip(
        &mut self,
        reason: PerfRenderSkipReason,
    ) {
        self.perf.record_render_skip(reason);
    }

    pub(in crate::backend::wayland) fn record_perf_render_complete(
        &mut self,
        render_started_at: Instant,
        render_finished_at: Instant,
        vsync_enabled: bool,
        max_fps_no_vsync: u32,
        keep_rendering: bool,
    ) {
        let _ = self.perf.record_render_complete(
            render_started_at,
            render_finished_at,
            vsync_enabled,
            max_fps_no_vsync,
            keep_rendering,
        );
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
        damage: PerfFrameDamageContext<'_>,
        commit_at: Instant,
    ) {
        if !self.perf.enabled() {
            return;
        }
        let full_damage = damage_covers_surface(
            damage.damage_screen,
            damage.logical_width,
            damage.logical_height,
        );
        let mut damage_diagnostics = damage.diagnostics;
        damage_diagnostics.final_single_surface_rect =
            damage.damage_screen.len() == 1 && full_damage;
        damage_diagnostics.largest_region_area_pct_hundredths = largest_region_area_pct_hundredths(
            damage.damage_screen,
            damage.logical_width,
            damage.logical_height,
        );
        let frame = PerfFrameContext {
            render_duration: None,
            dirty_area_pct: damage_area_pct(
                damage.damage_screen,
                damage.logical_width,
                damage.logical_height,
            ),
            full_damage,
            damage_rects: damage.damage_rects,
            force_full_reason: if full_damage {
                damage.force_full_reason
            } else {
                None
            },
            damage_diagnostics,
        };
        let _ = self.perf.commit_frame(frame, commit_at);
    }

    fn active_drawing_perf_context(&self) -> Option<(Tool, usize)> {
        let DrawingState::Drawing { tool, points, .. } = &self.input_state.state else {
            return None;
        };
        Some((*tool, points.len()))
    }

    pub(in crate::backend::wayland) fn flush_perf_summaries(&mut self, now: Instant) {
        let _ = self.perf.flush_pending_summaries(now);
    }
}

fn log_input_summary(summary: &PerfSummary, final_summary: bool) {
    info!(
        "perf.input_to_paint_latency proxy=input_to_wayland_commit frames={} samples={} window_samples={} p50_ms={} p95_ms={} p99_ms={} max_ms={} dropped_input_samples={} final={}",
        summary.frames,
        summary.samples,
        summary.window_samples,
        summary.p50_ms,
        summary.p95_ms,
        summary.p99_ms,
        summary.max_ms,
        summary.dropped_input_samples,
        final_summary
    );
}

fn log_frame_pacing_summary(summary: &PerfFramePacingSummary, final_summary: bool) {
    info!(
        "perf.frame_pacing frames={} window_frames={} render_p50_ms={} render_p95_ms={} render_p99_ms={} render_max_ms={} render_over_8ms={} render_over_16ms={} render_over_33ms={} render_over_50ms={} full_damage_count={} full_damage_pct={} force_full_reason={} force_full_reasons={} skipped_frame_callback_pending={} skipped_fps_cap={} skipped_surface_unconfigured={} skipped_no_redraw={} final={}",
        summary.frames,
        summary.window_frames,
        summary.render_p50_ms,
        summary.render_p95_ms,
        summary.render_p99_ms,
        summary.render_max_ms,
        summary.render_over_8ms,
        summary.render_over_16ms,
        summary.render_over_33ms,
        summary.render_over_50ms,
        summary.full_damage_count,
        summary.full_damage_pct,
        summary.force_full_reason,
        summary.force_full_reasons,
        summary.skipped_frame_callback_pending,
        summary.skipped_fps_cap,
        summary.skipped_surface_unconfigured,
        summary.skipped_no_redraw,
        final_summary
    );
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

fn format_force_full_reason(reason: Option<FullDamageReason>) -> &'static str {
    reason.map_or("none", FullDamageReason::as_str)
}

fn format_pct(count: u64, total: u64) -> String {
    if total == 0 {
        return "0.00".to_string();
    }
    format!("{:.2}", (count as f64 / total as f64) * 100.0)
}

fn dominant_full_damage_reason(
    reasons: &BTreeMap<FullDamageReason, u64>,
) -> Option<FullDamageReason> {
    reasons
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(reason, _)| *reason)
}

fn format_full_damage_reasons(reasons: &BTreeMap<FullDamageReason, u64>) -> String {
    if reasons.is_empty() {
        return "none".to_string();
    }
    reasons
        .iter()
        .map(|(reason, count)| format!("{}:{}", reason.as_str(), count))
        .collect::<Vec<_>>()
        .join(",")
}

fn frame_budget_duration(vsync_enabled: bool, max_fps_no_vsync: u32) -> Option<Duration> {
    if vsync_enabled {
        Some(VSYNC_ASSUMED_FRAME_BUDGET)
    } else if max_fps_no_vsync == 0 {
        None
    } else {
        Some(Duration::from_micros(
            1_000_000u64 / u64::from(max_fps_no_vsync),
        ))
    }
}

fn percentile_nearest_rank(sorted_values: &[u64], percentile: u64) -> Option<u64> {
    if sorted_values.is_empty() {
        return None;
    }
    let rank = ((percentile as f64 / 100.0) * sorted_values.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted_values.len() - 1);
    sorted_values.get(index).copied()
}

#[cfg(test)]
mod tests {
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
        let _ = metrics.record_render_complete(
            base,
            base + Duration::from_millis(12),
            false,
            120,
            false,
        );

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
}
