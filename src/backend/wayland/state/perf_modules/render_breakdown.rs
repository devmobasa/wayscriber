use std::time::Duration;

use log::info;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::backend::wayland) enum PerfRenderProfileKind {
    #[default]
    None,
    Canvas,
    Ui,
    CanvasAndUi,
}

impl PerfRenderProfileKind {
    pub(in crate::backend::wayland) fn from_flags(canvas: bool, ui: bool) -> Self {
        match (canvas, ui) {
            (false, false) => Self::None,
            (true, false) => Self::Canvas,
            (false, true) => Self::Ui,
            (true, true) => Self::CanvasAndUi,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Canvas => "canvas",
            Self::Ui => "ui",
            Self::CanvasAndUi => "canvas_and_ui",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::backend::wayland) struct PerfRenderStageDurations {
    pub(in crate::backend::wayland) advance_animations: Duration,
    pub(in crate::backend::wayland) dirty_collect: Duration,
    pub(in crate::backend::wayland) buffer_acquire: Duration,
    pub(in crate::backend::wayland) cairo_surface: Duration,
    pub(in crate::backend::wayland) clear_clip: Duration,
    pub(in crate::backend::wayland) background: Duration,
    pub(in crate::backend::wayland) completed_shapes: Duration,
    pub(in crate::backend::wayland) provisional: Duration,
    pub(in crate::backend::wayland) ui: Duration,
    pub(in crate::backend::wayland) render_profile: Duration,
    pub(in crate::backend::wayland) damage_commit: Duration,
    pub(in crate::backend::wayland) toolbar: Duration,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::backend::wayland) struct PerfRenderBreakdown {
    pub(in crate::backend::wayland) stages: PerfRenderStageDurations,
    pub(in crate::backend::wayland) surface_px: u64,
    pub(in crate::backend::wayland) shapes_total: usize,
    pub(in crate::backend::wayland) shapes_tested: usize,
    pub(in crate::backend::wayland) shapes_rendered: usize,
    pub(in crate::backend::wayland) provisional_points: usize,
    pub(in crate::backend::wayland) render_profile: PerfRenderProfileKind,
    pub(in crate::backend::wayland) canvas_layer_cache_hit: bool,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PerfRenderBreakdownSummary {
    pub(super) frames: u64,
    pub(super) samples: u64,
    pub(super) dominant_stage: String,
    pub(super) dominant_stage_avg: Duration,
    pub(super) stage_avg: PerfRenderStageDurations,
    pub(super) surface_px_max: u64,
    pub(super) shapes_total_max: usize,
    pub(super) shapes_tested_avg: u64,
    pub(super) shapes_rendered_avg: u64,
    pub(super) shape_cull_pct: String,
    pub(super) provisional_points_max: usize,
    pub(super) render_profile_frames: u64,
    pub(super) canvas_layer_cache_hits: u64,
}

#[derive(Debug, Default)]
pub(super) struct PerfRenderBreakdownAccumulator {
    samples: u64,
    stage_totals: PerfRenderStageDurations,
    surface_px_max: u64,
    shapes_total_max: usize,
    shapes_tested_total: u64,
    shapes_rendered_total: u64,
    provisional_points_max: usize,
    render_profile_frames: u64,
    canvas_layer_cache_hits: u64,
}

impl PerfRenderBreakdownAccumulator {
    pub(super) fn record(&mut self, breakdown: &PerfRenderBreakdown) {
        self.samples += 1;
        add_stage_totals(&mut self.stage_totals, &breakdown.stages);
        self.surface_px_max = self.surface_px_max.max(breakdown.surface_px);
        self.shapes_total_max = self.shapes_total_max.max(breakdown.shapes_total);
        self.shapes_tested_total = self
            .shapes_tested_total
            .saturating_add(breakdown.shapes_tested as u64);
        self.shapes_rendered_total = self
            .shapes_rendered_total
            .saturating_add(breakdown.shapes_rendered as u64);
        self.provisional_points_max = self
            .provisional_points_max
            .max(breakdown.provisional_points);
        if breakdown.render_profile != PerfRenderProfileKind::None {
            self.render_profile_frames += 1;
        }
        if breakdown.canvas_layer_cache_hit {
            self.canvas_layer_cache_hits += 1;
        }
    }

    pub(super) fn build_summary(&self, frames: u64) -> Option<PerfRenderBreakdownSummary> {
        if self.samples == 0 {
            return None;
        }

        let stage_avg = average_stage_durations(&self.stage_totals, self.samples);
        let (dominant_stage, dominant_stage_avg) = dominant_render_stage(&stage_avg);

        Some(PerfRenderBreakdownSummary {
            frames,
            samples: self.samples,
            dominant_stage: dominant_stage.to_string(),
            dominant_stage_avg,
            stage_avg,
            surface_px_max: self.surface_px_max,
            shapes_total_max: self.shapes_total_max,
            shapes_tested_avg: average_count(self.shapes_tested_total, self.samples),
            shapes_rendered_avg: average_count(self.shapes_rendered_total, self.samples),
            shape_cull_pct: shape_cull_pct_from_counts(
                self.shapes_tested_total,
                self.shapes_rendered_total,
            ),
            provisional_points_max: self.provisional_points_max,
            render_profile_frames: self.render_profile_frames,
            canvas_layer_cache_hits: self.canvas_layer_cache_hits,
        })
    }

    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
}

pub(super) fn log_render_stage_frame(frame: u64, render_ms: u64, breakdown: &PerfRenderBreakdown) {
    let dominant = dominant_render_stage(&breakdown.stages);
    info!(
        "perf.render_stage frame={} render_ms={} dominant_stage={} dominant_stage_ms={} advance_animations_ms={} dirty_collect_ms={} buffer_acquire_ms={} cairo_surface_ms={} clear_clip_ms={} background_ms={} completed_shapes_ms={} provisional_ms={} ui_ms={} render_profile_ms={} damage_commit_ms={} toolbar_ms={} surface_px={} shapes_total={} shapes_tested={} shapes_rendered={} shape_cull_pct={} provisional_points={} render_profile_active={} canvas_layer_cache_hit={}",
        frame,
        render_ms,
        dominant.0,
        format_duration_ms(dominant.1),
        format_duration_ms(breakdown.stages.advance_animations),
        format_duration_ms(breakdown.stages.dirty_collect),
        format_duration_ms(breakdown.stages.buffer_acquire),
        format_duration_ms(breakdown.stages.cairo_surface),
        format_duration_ms(breakdown.stages.clear_clip),
        format_duration_ms(breakdown.stages.background),
        format_duration_ms(breakdown.stages.completed_shapes),
        format_duration_ms(breakdown.stages.provisional),
        format_duration_ms(breakdown.stages.ui),
        format_duration_ms(breakdown.stages.render_profile),
        format_duration_ms(breakdown.stages.damage_commit),
        format_duration_ms(breakdown.stages.toolbar),
        breakdown.surface_px,
        breakdown.shapes_total,
        breakdown.shapes_tested,
        breakdown.shapes_rendered,
        shape_cull_pct_from_counts(
            breakdown.shapes_tested as u64,
            breakdown.shapes_rendered as u64
        ),
        breakdown.provisional_points,
        breakdown.render_profile.as_str(),
        breakdown.canvas_layer_cache_hit
    );
}

pub(super) fn log_render_stage_summary(summary: &PerfRenderBreakdownSummary, final_summary: bool) {
    info!(
        "perf.render_stage_summary frames={} samples={} dominant_stage={} dominant_stage_avg_ms={} advance_animations_avg_ms={} dirty_collect_avg_ms={} buffer_acquire_avg_ms={} cairo_surface_avg_ms={} clear_clip_avg_ms={} background_avg_ms={} completed_shapes_avg_ms={} provisional_avg_ms={} ui_avg_ms={} render_profile_avg_ms={} damage_commit_avg_ms={} toolbar_avg_ms={} surface_px_max={} shapes_total_max={} shapes_tested_avg={} shapes_rendered_avg={} shape_cull_pct={} provisional_points_max={} render_profile_frames={} canvas_layer_cache_hits={} final={}",
        summary.frames,
        summary.samples,
        summary.dominant_stage,
        format_duration_ms(summary.dominant_stage_avg),
        format_duration_ms(summary.stage_avg.advance_animations),
        format_duration_ms(summary.stage_avg.dirty_collect),
        format_duration_ms(summary.stage_avg.buffer_acquire),
        format_duration_ms(summary.stage_avg.cairo_surface),
        format_duration_ms(summary.stage_avg.clear_clip),
        format_duration_ms(summary.stage_avg.background),
        format_duration_ms(summary.stage_avg.completed_shapes),
        format_duration_ms(summary.stage_avg.provisional),
        format_duration_ms(summary.stage_avg.ui),
        format_duration_ms(summary.stage_avg.render_profile),
        format_duration_ms(summary.stage_avg.damage_commit),
        format_duration_ms(summary.stage_avg.toolbar),
        summary.surface_px_max,
        summary.shapes_total_max,
        summary.shapes_tested_avg,
        summary.shapes_rendered_avg,
        summary.shape_cull_pct,
        summary.provisional_points_max,
        summary.render_profile_frames,
        summary.canvas_layer_cache_hits,
        final_summary
    );
}

fn add_stage_totals(total: &mut PerfRenderStageDurations, frame: &PerfRenderStageDurations) {
    total.advance_animations = total
        .advance_animations
        .saturating_add(frame.advance_animations);
    total.dirty_collect = total.dirty_collect.saturating_add(frame.dirty_collect);
    total.buffer_acquire = total.buffer_acquire.saturating_add(frame.buffer_acquire);
    total.cairo_surface = total.cairo_surface.saturating_add(frame.cairo_surface);
    total.clear_clip = total.clear_clip.saturating_add(frame.clear_clip);
    total.background = total.background.saturating_add(frame.background);
    total.completed_shapes = total
        .completed_shapes
        .saturating_add(frame.completed_shapes);
    total.provisional = total.provisional.saturating_add(frame.provisional);
    total.ui = total.ui.saturating_add(frame.ui);
    total.render_profile = total.render_profile.saturating_add(frame.render_profile);
    total.damage_commit = total.damage_commit.saturating_add(frame.damage_commit);
    total.toolbar = total.toolbar.saturating_add(frame.toolbar);
}

fn average_stage_durations(
    total: &PerfRenderStageDurations,
    samples: u64,
) -> PerfRenderStageDurations {
    PerfRenderStageDurations {
        advance_animations: average_duration(total.advance_animations, samples),
        dirty_collect: average_duration(total.dirty_collect, samples),
        buffer_acquire: average_duration(total.buffer_acquire, samples),
        cairo_surface: average_duration(total.cairo_surface, samples),
        clear_clip: average_duration(total.clear_clip, samples),
        background: average_duration(total.background, samples),
        completed_shapes: average_duration(total.completed_shapes, samples),
        provisional: average_duration(total.provisional, samples),
        ui: average_duration(total.ui, samples),
        render_profile: average_duration(total.render_profile, samples),
        damage_commit: average_duration(total.damage_commit, samples),
        toolbar: average_duration(total.toolbar, samples),
    }
}

fn average_duration(total: Duration, samples: u64) -> Duration {
    let nanos = total
        .as_nanos()
        .checked_div(u128::from(samples))
        .unwrap_or(0);
    Duration::from_nanos(nanos.min(u128::from(u64::MAX)) as u64)
}

fn average_count(total: u64, samples: u64) -> u64 {
    total.checked_div(samples).unwrap_or(0)
}

fn dominant_render_stage(stages: &PerfRenderStageDurations) -> (&'static str, Duration) {
    [
        ("advance_animations", stages.advance_animations),
        ("dirty_collect", stages.dirty_collect),
        ("buffer_acquire", stages.buffer_acquire),
        ("cairo_surface", stages.cairo_surface),
        ("clear_clip", stages.clear_clip),
        ("background", stages.background),
        ("completed_shapes", stages.completed_shapes),
        ("provisional", stages.provisional),
        ("ui", stages.ui),
        ("render_profile", stages.render_profile),
        ("damage_commit", stages.damage_commit),
        ("toolbar", stages.toolbar),
    ]
    .into_iter()
    .max_by_key(|(_, duration)| *duration)
    .unwrap_or(("none", Duration::ZERO))
}

fn shape_cull_pct_from_counts(tested: u64, rendered: u64) -> String {
    if tested == 0 {
        "n/a".to_string()
    } else {
        let culled = tested.saturating_sub(rendered);
        format_pct(culled, tested)
    }
}

fn format_pct(count: u64, total: u64) -> String {
    if total == 0 {
        return "0.00".to_string();
    }
    format!("{:.2}", (count as f64 / total as f64) * 100.0)
}

fn format_duration_ms(duration: Duration) -> String {
    format!("{:.2}", duration.as_secs_f64() * 1000.0)
}
