//! Event-loop glue for GIF playback: visibility-aware deadlines and the
//! per-render advance call.
//!
//! GIF clocks deliberately do not join the `ui_animation` tick: that clock
//! fires at `ui_animation_fps` regardless of frame delays, and a wake without
//! resulting damage escalates into a full-surface repaint. Deadlines here come
//! from the entries themselves, and the wake predicate
//! ([`WaylandState::gif_frames_due`]) matches the advance pass exactly.

use super::WaylandState;
use crate::util::Rect;
use std::time::{Duration, Instant};

impl WaylandState {
    /// World-space rect currently visible on the overlay, mirroring the
    /// `damage_world` mapping in the render pass: identity when no canvas
    /// transform is active, the panned/zoomed view rect otherwise.
    fn gif_visible_world_rect(&self) -> Option<Rect> {
        let width = self.surface.width();
        let height = self.surface.height();
        if self.canvas_transform_active() {
            let scale = if self.zoom.active {
                self.zoom.scale.max(f64::MIN_POSITIVE)
            } else {
                1.0
            };
            let view_width = ((width as f64) / scale).ceil() as i32;
            let view_height = ((height as f64) / scale).ceil() as i32;
            let (view_x, view_y) = self.canvas_view_origin();
            Rect::new(
                view_x.floor() as i32,
                view_y.floor() as i32,
                view_width,
                view_height,
            )
        } else {
            Rect::new(
                0,
                0,
                width.min(i32::MAX as u32) as i32,
                height.min(i32::MAX as u32) as i32,
            )
        }
    }

    /// Advances due GIF frames; called from the render pass's
    /// `advance_animations` stage. Damage lands in the input-state dirty
    /// tracker, which the same render pass drains right afterwards.
    pub(in crate::backend::wayland) fn advance_gif_animations(&mut self, now: Instant) -> bool {
        let view = self.gif_visible_world_rect();
        let interval_floor = self.ui_animation_interval;
        self.input_state
            .advance_gif_animations(now, view, interval_floor)
    }

    /// Event-loop deadline for the next GIF frame among visible, playing
    /// entries. `None` when nothing is scheduled (or a redraw is already
    /// pending for an overdue frame).
    pub(in crate::backend::wayland) fn gif_frame_timeout(&self, now: Instant) -> Option<Duration> {
        let view = self.gif_visible_world_rect();
        self.input_state.gif_frame_timeout(now, view)
    }

    /// Wake-check twin of the advance pass: true when a visible, playing GIF
    /// frame is due.
    pub(in crate::backend::wayland) fn gif_frames_due(&self, now: Instant) -> bool {
        let view = self.gif_visible_world_rect();
        self.input_state.gif_frames_due(now, view)
    }
}
