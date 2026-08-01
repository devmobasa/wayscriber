use super::*;

impl PerfMetrics {
    pub(in crate::backend::wayland::state) fn from_env() -> Self {
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

    pub(super) fn record_render_breakdown(&mut self, breakdown: PerfRenderBreakdown) {
        if !self.enabled {
            return;
        }
        self.last_render_breakdown = Some(breakdown);
    }

    pub(super) fn record_render_skip(&mut self, reason: PerfRenderSkipReason) {
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

    pub(super) fn record_render_complete(
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

    pub(super) fn commit_frame(
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

    pub(super) fn flush_pending_summaries(&mut self, now: Instant) -> PerfFinalSummaryReport {
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

#[cfg(test)]
#[path = "metrics/tests.rs"]
mod tests;
