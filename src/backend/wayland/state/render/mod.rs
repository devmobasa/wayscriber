use super::*;

macro_rules! record_stage {
    ($breakdown:expr, $field:ident, $body:expr) => {{
        record_render_stage(
            $breakdown.is_some(),
            $breakdown.as_mut(),
            |breakdown, duration| {
                breakdown.stages.$field = breakdown.stages.$field.saturating_add(duration);
            },
            || $body,
        )
    }};
}

mod canvas;
mod measure_badge;
mod paint;
mod plan;
mod prepare;
mod profile;
mod runtime;
mod submit;
mod tool_preview;
mod ui;
mod ui_effect_damage;

use plan::{FrameGeometry, FrameVisibility, plan_frame};
pub(in crate::backend::wayland) use runtime::RenderRuntime;
use runtime::{UiEffect, UiEffectFlags};

/// What a render pass actually did.
///
/// `BuffersInFlight` is not a frame: nothing was painted or committed, so the
/// caller must keep the redraw pending and leave the frame counters alone.
/// Discarding this silently would let a caller treat an uncommitted frame as
/// on-screen, so it is `#[must_use]`.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::backend::wayland) enum RenderOutcome {
    Committed { keep_rendering: bool },
    BuffersInFlight,
}

fn record_render_stage<T>(
    enabled: bool,
    breakdown: Option<&mut PerfRenderBreakdown>,
    record: impl FnOnce(&mut PerfRenderBreakdown, Duration),
    body: impl FnOnce() -> T,
) -> T {
    let started = enabled.then(Instant::now);
    let result = body();
    if let (Some(breakdown), Some(started)) = (breakdown, started) {
        record(breakdown, Instant::now().saturating_duration_since(started));
    }
    result
}

impl WaylandState {
    pub(in crate::backend::wayland) fn render(
        &mut self,
        qh: &QueueHandle<Self>,
    ) -> Result<RenderOutcome> {
        debug!("=== RENDER START ===");
        // Suppression and surface geometry precede acquisition; animation time,
        // layout, damage history and profile selection follow it.
        let visibility = FrameVisibility::new(
            self.suppression.reason(),
            self.input_state.board_is_transparent(),
        );
        let geometry = FrameGeometry::new(
            self.surface.width(),
            self.surface.height(),
            self.surface.scale(),
        );
        let buffer_count = self.config.performance.buffer_count as usize;
        let mut breakdown = self.perf_enabled().then(|| PerfRenderBreakdown {
            surface_px: u64::from(geometry.physical_width)
                .saturating_mul(u64::from(geometry.physical_height)),
            ..PerfRenderBreakdown::default()
        });
        let acquired = record_stage!(breakdown, buffer_acquire, {
            self.surface.acquire_buffer(
                self.protocol.shm(),
                buffer_count,
                geometry.physical_width as i32,
                geometry.physical_height as i32,
                geometry.stride,
            )
        })?;
        let outcome = render_acquired_frame(acquired, |acquired| {
            let prepared = self.prepare_frame(geometry, visibility, &acquired, &mut breakdown);
            let plan = plan_frame(prepared);
            self.paint_frame(&plan, &acquired, &mut breakdown)?;
            self.submit_frame(qh, acquired, &plan, &mut breakdown)?;
            Ok(plan.keep_rendering)
        })?;
        if outcome == RenderOutcome::BuffersInFlight {
            debug!("All {buffer_count} buffers in flight - deferring this frame");
            self.record_perf_render_skip(PerfRenderSkipReason::BuffersInFlight);
        }
        Ok(outcome)
    }
}

/// The only entrance to preparation and painting. A busy slot leaves animation,
/// input damage and effect history untouched so the next real frame can drain them.
fn render_acquired_frame<B>(
    acquired: Option<B>,
    render: impl FnOnce(B) -> Result<bool>,
) -> Result<RenderOutcome> {
    let Some(buffer) = acquired else {
        return Ok(RenderOutcome::BuffersInFlight);
    };
    render(buffer).map(|keep_rendering| RenderOutcome::Committed { keep_rendering })
}

#[cfg(test)]
mod tests;
