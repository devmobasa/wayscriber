use std::time::Instant;

use super::*;

impl WaylandState {
    pub(in crate::backend::wayland::state::toolbar) fn apply_toolbar_offsets_throttled(
        &mut self,
        snapshot: &ToolbarSnapshot,
    ) {
        let now = Instant::now();
        let Some(interval) = toolbar_drag_throttle_interval() else {
            let _ = self.apply_toolbar_offsets(snapshot);
            self.toolbar_drag.note_applied(now);
            return;
        };
        if self.toolbar_drag.preview_active() || self.toolbar_drag.should_apply(now, interval) {
            let _ = self.apply_toolbar_offsets(snapshot);
            self.toolbar_drag.note_applied(now);
        } else {
            let _ = self.clamp_toolbar_offsets(snapshot);
        }
    }

    pub(in crate::backend::wayland::state::toolbar) fn clamp_toolbar_offsets(
        &mut self,
        snapshot: &ToolbarSnapshot,
    ) -> bool {
        let width = self.surface.width() as f64;
        let height = self.surface.height() as f64;
        if width == 0.0 || height == 0.0 {
            drag_log(|| {
                format!(
                    "skip clamp: surface not configured (width={}, height={})",
                    width, height
                )
            });
            return false;
        }
        let (top_w, top_h) = top_size(snapshot);
        let top_base_x = self.inline_top_base_x();
        let top_base_y = self.inline_top_base_y();

        let before_top = self.toolbar_chrome.top_offset();
        let input = geometry::ToolbarClampInput {
            width,
            height,
            top_size: (top_w, top_h),
            top_base_x,
            top_base_y,
            top_margin_right: Self::TOP_MARGIN_RIGHT,
            top_margin_bottom: Self::TOP_MARGIN_BOTTOM,
        };
        let top_offset = self.toolbar_chrome.top_offset();
        let offsets = geometry::ToolbarOffsets {
            top_x: top_offset.0,
            top_y: top_offset.1,
        };
        let (clamped, bounds) = geometry::clamp_toolbar_offsets(offsets, input);
        self.toolbar_chrome
            .set_top_offset((clamped.top_x, clamped.top_y));
        drag_log(|| {
            format!(
                "clamp offsets: before=({:.3}, {:.3}), after=({:.3}, {:.3}), max=({:.3}, {:.3}), size=({}, {}), top_base_x={:.3}, top_base_y={:.3}",
                before_top.0,
                before_top.1,
                self.toolbar_chrome.top_offset().0,
                self.toolbar_chrome.top_offset().1,
                bounds.max_top_x,
                bounds.max_top_y,
                width,
                height,
                top_base_x,
                top_base_y
            )
        });
        true
    }

    pub(in crate::backend::wayland::state::toolbar) fn apply_toolbar_offsets(
        &mut self,
        snapshot: &ToolbarSnapshot,
    ) -> bool {
        if self.surface.width() == 0 || self.surface.height() == 0 {
            drag_log(|| {
                format!(
                    "skip apply_toolbar_offsets: surface not configured (width={}, height={})",
                    self.surface.width(),
                    self.surface.height()
                )
            });
            return false;
        }
        let _ = self.clamp_toolbar_offsets(snapshot);
        // During local-coordinate preview drags, keep the layer-shell surface parked
        // to avoid drift as the source surface moves under the pointer. Pointer-locked
        // drags use relative deltas instead, so the suppressed real surface can track
        // the preview and avoid a visible catch-up animation on release.
        if self.toolbar_drag.preview_active() && !self.pointer_lock_active() {
            drag_log(|| "skip apply_toolbar_offsets: drag preview active without pointer lock");
            return false;
        }
        if self.protocol.layer_shell().is_none() {
            return false;
        }
        let top_base_x = self.inline_top_base_x();
        let top_offset = self.toolbar_chrome.top_offset();
        let (top_margin_left, top_margin_top) = geometry::compute_layer_margins(
            top_base_x,
            Self::TOP_BASE_MARGIN_TOP,
            geometry::ToolbarOffsets {
                top_x: top_offset.0,
                top_y: top_offset.1,
            },
        );
        drag_log(|| {
            format!(
                "apply_toolbar_offsets: top_margin_left={}, top_margin_top={}, offsets=({}, {}), scale={}, top_base_x={}",
                top_margin_left,
                top_margin_top,
                top_offset.0,
                top_offset.1,
                self.surface.scale(),
                top_base_x
            )
        });
        if debug_toolbar_drag_logging_enabled() {
            let last = self.toolbar_chrome.last_applied_margins();
            debug!(
                "apply_toolbar_offsets: top_margin_left={} (last={:?}), top_margin_top={} (last={:?}), offsets=({}, {}), top_base_x={}",
                top_margin_left,
                last.map(|(_, left)| left),
                top_margin_top,
                last.map(|(top, _)| top),
                top_offset.0,
                top_offset.1,
                top_base_x
            );
        }
        let top_changed = self
            .toolbar_chrome
            .apply_margins((top_margin_top, top_margin_left));
        if !top_changed {
            return false;
        }
        self.toolbar
            .set_top_margins(top_margin_top, top_margin_left);
        if self.toolbar_drag.preview_active() && self.pointer_lock_active() {
            self.toolbar_drag.request_flush();
        }
        top_changed
    }
}
