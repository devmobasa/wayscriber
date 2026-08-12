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
#[path = "perf_modules/metrics.rs"]
mod metrics;
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
    BuffersInFlight,
}

impl fmt::Display for PerfRenderSkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameCallbackPending => f.write_str("frame_callback_pending"),
            Self::FpsCap => f.write_str("fps_cap"),
            Self::SurfaceUnconfigured => f.write_str("surface_unconfigured"),
            Self::NoRedraw => f.write_str("no_redraw"),
            Self::BuffersInFlight => f.write_str("buffers_in_flight"),
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
    skipped_buffers_in_flight: u64,
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
    skipped_buffers_in_flight: u64,
    dropped_input_samples: u64,
    last_summary_at: Option<Instant>,
    last_frame_pacing_summary_at: Option<Instant>,
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
        "perf.frame_pacing frames={} window_frames={} render_p50_ms={} render_p95_ms={} render_p99_ms={} render_max_ms={} render_over_8ms={} render_over_16ms={} render_over_33ms={} render_over_50ms={} full_damage_count={} full_damage_pct={} force_full_reason={} force_full_reasons={} skipped_frame_callback_pending={} skipped_fps_cap={} skipped_surface_unconfigured={} skipped_no_redraw={} skipped_buffers_in_flight={} final={}",
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
        summary.skipped_buffers_in_flight,
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
