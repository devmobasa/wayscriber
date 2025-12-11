//! Zoom state and command handling. Rendering and capture layers consume this state.

use crate::zoom::{RectF, clamp_factor, crop_rect_logical, logical_rect_to_frame_px};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ZoomMode {
    Live,
    Frozen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomCommand {
    Toggle,
    ZoomIn,
    ZoomOut,
    Reset,
    RequestCurrentMonitor,
    ToggleInputCapture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomCommandResult {
    NoChange,
    StateChanged,
    RequestCurrentMonitor,
}

/// Derived view info for rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomView {
    pub crop_logical: RectF,
    pub crop_frame: RectF,
    pub scale_factor: f64,
}

#[derive(Debug, Clone)]
pub struct ZoomState {
    /// Whether zoom is currently active.
    pub active: bool,
    /// Current mode (live stream vs frozen frame).
    pub mode: ZoomMode,
    /// Current applied zoom factor.
    pub factor: f32,
    /// Target factor for smoothing; equals factor when idle.
    pub target_factor: f32,
    /// Center in logical coordinates for the active output.
    pub center_logical: (f64, f64),
    /// Last pointer position in logical coordinates.
    pub last_pointer_logical: (f64, f64),
    /// Minimum allowed factor.
    pub min_factor: f32,
    /// Maximum allowed factor.
    pub max_factor: f32,
    /// Step applied for +/-.
    pub step: f32,
    /// Whether zoom should capture input (vs passthrough).
    pub capture_input: bool,
}

impl ZoomState {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn configure(&mut self, min_factor: f32, max_factor: f32, step: f32, capture_input: bool) {
        self.min_factor = min_factor;
        self.max_factor = max_factor;
        self.step = step;
        self.capture_input = capture_input;
        self.factor = clamp_factor(self.factor, self.min_factor, self.max_factor);
        self.target_factor = clamp_factor(self.target_factor, self.min_factor, self.max_factor);
    }

    pub fn update_pointer(&mut self, pointer_logical: (f64, f64)) {
        self.last_pointer_logical = pointer_logical;
        if !self.active {
            self.center_logical = pointer_logical;
        }
    }

    pub fn apply_command(
        &mut self,
        cmd: ZoomCommand,
        pointer_logical: (f64, f64),
        viewport_logical: (f64, f64),
    ) -> ZoomCommandResult {
        let mut state_changed = false;
        match cmd {
            ZoomCommand::Toggle => {
                if self.active {
                    self.active = false;
                    self.factor = 1.0;
                    self.target_factor = 1.0;
                    state_changed = true;
                } else {
                    self.active = true;
                    self.mode = ZoomMode::Frozen;
                    self.center_logical = pointer_logical;
                    self.last_pointer_logical = pointer_logical;
                    state_changed = true;
                }
            }
            ZoomCommand::ZoomIn => {
                if !self.active {
                    self.active = true;
                    self.mode = ZoomMode::Frozen;
                    self.center_logical = pointer_logical;
                }
                state_changed |= self.bump_factor(self.step);
            }
            ZoomCommand::ZoomOut => {
                if !self.active {
                    self.active = true;
                    self.mode = ZoomMode::Frozen;
                    self.center_logical = pointer_logical;
                }
                state_changed |= self.bump_factor(-self.step);
            }
            ZoomCommand::Reset => {
                self.active = true;
                self.mode = ZoomMode::Frozen;
                self.center_logical = pointer_logical;
                self.last_pointer_logical = pointer_logical;
                state_changed |= self.set_factor(1.0);
            }
            ZoomCommand::RequestCurrentMonitor => {
                // For now handled upstream; treat as hint to backend.
                return ZoomCommandResult::RequestCurrentMonitor;
            }
            ZoomCommand::ToggleInputCapture => {
                self.capture_input = !self.capture_input;
                state_changed = true;
            }
        }

        // Ensure the center remains clamped within the viewport.
        if state_changed {
            self.center_logical = clamp_center(self.center_logical, viewport_logical);
        }

        if state_changed {
            ZoomCommandResult::StateChanged
        } else {
            ZoomCommandResult::NoChange
        }
    }

    fn bump_factor(&mut self, delta: f32) -> bool {
        let prev = self.target_factor;
        let next = clamp_factor(self.target_factor + delta, self.min_factor, self.max_factor);
        self.target_factor = next;
        self.factor = next;
        (next - prev).abs() > f32::EPSILON
    }

    fn set_factor(&mut self, value: f32) -> bool {
        let prev = self.target_factor;
        let next = clamp_factor(value, self.min_factor, self.max_factor);
        self.target_factor = next;
        self.factor = next;
        (next - prev).abs() > f32::EPSILON
    }

    pub fn is_active(&self) -> bool {
        self.active && self.factor > 1.0 - f32::EPSILON
    }

    #[allow(dead_code)]
    pub fn crop_rect_logical(&self, viewport_logical: (f64, f64)) -> RectF {
        crop_rect_logical(
            self.center_logical,
            self.factor,
            viewport_logical,
            self.min_factor,
            self.max_factor,
        )
    }

    #[allow(dead_code)]
    pub fn crop_rect_frame(
        &self,
        viewport_logical: (f64, f64),
        output_origin_logical: (f64, f64),
        scale: f64,
    ) -> RectF {
        let logical_rect = crop_rect_logical(
            self.center_logical,
            self.factor,
            viewport_logical,
            self.min_factor,
            self.max_factor,
        );
        logical_rect_to_frame_px(logical_rect, output_origin_logical, scale)
    }

    #[allow(dead_code)]
    pub fn scale_factor(&self, viewport_logical: (f64, f64)) -> f64 {
        let rect = crop_rect_logical(
            self.center_logical,
            self.factor,
            viewport_logical,
            self.min_factor,
            self.max_factor,
        );
        (viewport_logical.0 / rect.width).max(1.0)
    }

    pub fn view_for_viewport(
        &self,
        viewport_logical: (f64, f64),
        output_origin_logical: (f64, f64),
        scale: f64,
        rotation_degrees: i32,
    ) -> Option<ZoomView> {
        if !self.is_active() {
            return None;
        }
        if rotation_degrees.rem_euclid(360) != 0 {
            // Rotation not yet supported; guard to avoid misaligned sampling.
            return None;
        }
        let crop_logical = crop_rect_logical(
            self.center_logical,
            self.factor,
            viewport_logical,
            self.min_factor,
            self.max_factor,
        );
        let crop_frame = logical_rect_to_frame_px(crop_logical, output_origin_logical, scale);
        let scale_factor = (viewport_logical.0 / crop_logical.width).max(1.0);
        Some(ZoomView {
            crop_logical,
            crop_frame,
            scale_factor,
        })
    }
}

impl Default for ZoomState {
    fn default() -> Self {
        Self {
            active: false,
            mode: ZoomMode::Frozen,
            factor: 1.0,
            target_factor: 1.0,
            center_logical: (0.0, 0.0),
            last_pointer_logical: (0.0, 0.0),
            min_factor: 1.0,
            max_factor: 4.0,
            step: 0.2,
            capture_input: false,
        }
    }
}

fn clamp_center(center: (f64, f64), viewport_logical: (f64, f64)) -> (f64, f64) {
    let (mut x, mut y) = center;
    let (w, h) = viewport_logical;
    if w.is_normal() {
        x = x.clamp(0.0, w);
    }
    if h.is_normal() {
        y = y.clamp(0.0, h);
    }
    (x, y)
}
