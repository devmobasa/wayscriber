use std::time::Instant;

use crate::backend::wayland::surface_geometry::SurfaceGeometry;
use crate::util::Rect;

use super::super::{FullDamageReason, OverlaySuppression, PerfDamageDiagnostics};
use super::profile::FrameProfile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FrameGeometry {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) scale: i32,
    pub(super) preferred_scale: Option<u32>,
    pub(super) wire_scale: i32,
    pub(super) physical_width: u32,
    pub(super) physical_height: u32,
    pub(super) stride: i32,
    pub(super) byte_len: usize,
}

impl FrameGeometry {
    pub(super) fn from_surface(geometry: SurfaceGeometry) -> Self {
        let SurfaceGeometry {
            logical_width: width,
            logical_height: height,
            preferred_scale,
            buffer_scale: wire_scale,
            pixel_width: physical_width,
            pixel_height: physical_height,
            stride,
            byte_len,
            ..
        } = geometry;
        Self {
            width,
            height,
            scale: if preferred_scale.is_some() {
                1
            } else {
                wire_scale
            },
            preferred_scale,
            wire_scale,
            physical_width,
            physical_height,
            stride,
            byte_len,
        }
    }

    #[cfg(test)]
    pub(super) fn new(width: u32, height: u32, scale: i32) -> Self {
        Self::from_surface(SurfaceGeometry::new(width, height, scale, None, 3).unwrap())
    }

    pub(super) fn scale_x(self) -> f64 {
        self.physical_width as f64 / self.width as f64
    }

    pub(super) fn scale_y(self) -> f64 {
        self.physical_height as f64 / self.height as f64
    }

    pub(super) fn damage(self, rect: Rect) -> Option<Rect> {
        SurfaceGeometry::map_damage(
            rect,
            (self.width, self.height),
            (self.physical_width, self.physical_height),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FrameVisibility {
    pub(super) render_canvas: bool,
    pub(super) render_transients: bool,
    pub(super) render_ui: bool,
}

impl FrameVisibility {
    pub(super) fn new(suppression: OverlaySuppression, board_is_transparent: bool) -> Self {
        let suppression = suppression.effective_for_board(board_is_transparent);
        Self {
            render_canvas: suppression.renders_canvas(),
            render_transients: suppression.renders_canvas_transients(),
            render_ui: suppression.renders_ui(),
        }
    }
}

pub(super) struct CanvasPolicyInputs {
    pub(super) capture_picker_active: bool,
    pub(super) include_drawings: bool,
    pub(super) transform_active: bool,
    pub(super) origin: (f64, f64),
    pub(super) zoom_scale: Option<f64>,
    pub(super) text_halo_enabled: bool,
    pub(super) layer_cache_usable: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CanvasFrame {
    pub(super) draw_committed: bool,
    pub(super) render_transients: bool,
    pub(super) transform_active: bool,
    pub(super) origin: (f64, f64),
    pub(super) zoom_scale: Option<f64>,
    pub(super) text_halo_enabled: bool,
    pub(super) layer_cache_eligible: bool,
}

/// Values sampled by preparation after successful buffer acquisition.
/// Slot history and empty-damage fallback remain with the mutable preparer.
pub(super) struct PreparedFrame {
    pub(super) geometry: FrameGeometry,
    pub(super) visibility: FrameVisibility,
    pub(super) canvas: CanvasPolicyInputs,
    pub(super) damage_screen: Vec<Rect>,
    pub(super) full_damage_reason: Option<FullDamageReason>,
    pub(super) damage_diagnostics: PerfDamageDiagnostics,
    pub(super) profile: FrameProfile,
    pub(super) now: Instant,
    pub(super) keep_rendering: bool,
}

pub(super) struct FrameDamage {
    pub(super) screen: Vec<Rect>,
    pub(super) world: Vec<Rect>,
    pub(super) buffer: Vec<Rect>,
    pub(super) full_reason: Option<FullDamageReason>,
    pub(super) diagnostics: PerfDamageDiagnostics,
}

pub(super) struct FramePlan {
    pub(super) geometry: FrameGeometry,
    pub(super) canvas: CanvasFrame,
    pub(super) render_canvas: bool,
    pub(super) render_ui: bool,
    pub(super) damage: FrameDamage,
    pub(super) profile: FrameProfile,
    pub(super) now: Instant,
    pub(super) keep_rendering: bool,
}

/// Derives paint and submission values without advancing runtime state.
pub(super) fn plan_frame(prepared: PreparedFrame) -> FramePlan {
    let inputs = prepared.canvas;
    let canvas = CanvasFrame {
        draw_committed: !inputs.capture_picker_active || inputs.include_drawings,
        render_transients: prepared.visibility.render_transients && !inputs.capture_picker_active,
        transform_active: inputs.transform_active,
        origin: inputs.origin,
        zoom_scale: inputs.zoom_scale,
        text_halo_enabled: inputs.text_halo_enabled,
        layer_cache_eligible: inputs.layer_cache_usable && !inputs.capture_picker_active,
    };
    let world = if canvas.transform_active {
        let zoom = canvas.zoom_scale.unwrap_or(1.0).max(f64::MIN_POSITIVE);
        let view_width = (f64::from(prepared.geometry.width) / zoom).ceil() as i32;
        let view_height = (f64::from(prepared.geometry.height) / zoom).ceil() as i32;
        Rect::new(
            canvas.origin.0.floor() as i32,
            canvas.origin.1.floor() as i32,
            view_width,
            view_height,
        )
        .map(|rect| vec![rect])
        .unwrap_or_default()
    } else {
        prepared.damage_screen.clone()
    };
    let buffer = prepared
        .damage_screen
        .iter()
        .filter_map(|region| prepared.geometry.damage(*region))
        .collect();
    FramePlan {
        geometry: prepared.geometry,
        canvas,
        render_canvas: prepared.visibility.render_canvas,
        render_ui: prepared.visibility.render_ui,
        damage: FrameDamage {
            screen: prepared.damage_screen,
            world,
            buffer,
            full_reason: prepared.full_damage_reason,
            diagnostics: prepared.damage_diagnostics,
        },
        profile: prepared.profile,
        now: prepared.now,
        keep_rendering: prepared.keep_rendering,
    }
}

#[cfg(test)]
mod tests;
