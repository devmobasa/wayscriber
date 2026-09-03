use crate::util::Rect;

use super::super::super::base::InputState;
use super::super::super::base::SelectionHandle;

#[derive(Debug, Clone, Copy)]
pub(super) struct ResizeHandleProbe {
    pub(super) handle: SelectionHandle,
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) radius: i32,
}

const MIN_RESIZE_SIZE: f64 = 10.0;

impl InputState {
    pub(super) fn point_near(&self, x: i32, y: i32, cx: i32, cy: i32, radius: i32) -> bool {
        (x - cx).abs() <= radius && (y - cy).abs() <= radius
    }

    pub(super) fn selection_handle_probes(
        bounds: &Rect,
        corner_radius: i32,
        edge_radius: i32,
    ) -> [ResizeHandleProbe; 8] {
        let right = bounds.x + bounds.width;
        let bottom = bounds.y + bounds.height;
        let mid_x = bounds.x + bounds.width / 2;
        let mid_y = bounds.y + bounds.height / 2;
        [
            Self::build_handle_probe(SelectionHandle::TopLeft, bounds.x, bounds.y, corner_radius),
            Self::build_handle_probe(SelectionHandle::TopRight, right, bounds.y, corner_radius),
            Self::build_handle_probe(SelectionHandle::BottomLeft, bounds.x, bottom, corner_radius),
            Self::build_handle_probe(SelectionHandle::BottomRight, right, bottom, corner_radius),
            Self::build_handle_probe(SelectionHandle::Top, mid_x, bounds.y, edge_radius),
            Self::build_handle_probe(SelectionHandle::Bottom, mid_x, bottom, edge_radius),
            Self::build_handle_probe(SelectionHandle::Left, bounds.x, mid_y, edge_radius),
            Self::build_handle_probe(SelectionHandle::Right, right, mid_y, edge_radius),
        ]
    }

    fn build_handle_probe(
        handle: SelectionHandle,
        x: i32,
        y: i32,
        radius: i32,
    ) -> ResizeHandleProbe {
        ResizeHandleProbe {
            handle,
            x,
            y,
            radius,
        }
    }

    pub(super) fn clamp_resize_size(value: f64) -> f64 {
        value.max(MIN_RESIZE_SIZE)
    }

    pub(super) fn compute_scale_factors(
        handle: SelectionHandle,
        bounds: &Rect,
        dx: i32,
        dy: i32,
    ) -> (f64, f64, f64, f64) {
        let w = bounds.width as f64;
        let h = bounds.height as f64;
        let (new_w, new_h) = Self::compute_resized_dimensions(handle, w, h, dx as f64, dy as f64);
        let (anchor_x, anchor_y) = Self::anchor_for_handle(handle, bounds);
        (new_w / w, new_h / h, anchor_x, anchor_y)
    }

    fn compute_resized_dimensions(
        handle: SelectionHandle,
        width: f64,
        height: f64,
        dx: f64,
        dy: f64,
    ) -> (f64, f64) {
        let new_w = Self::resize_width_for_handle(handle, width, dx);
        let new_h = Self::resize_height_for_handle(handle, height, dy);
        (new_w, new_h)
    }

    fn resize_width_for_handle(handle: SelectionHandle, width: f64, dx: f64) -> f64 {
        match handle {
            SelectionHandle::TopLeft | SelectionHandle::BottomLeft => {
                Self::clamp_resize_size(width - dx)
            }
            SelectionHandle::TopRight | SelectionHandle::BottomRight => {
                Self::clamp_resize_size(width + dx)
            }
            SelectionHandle::Top | SelectionHandle::Bottom => width,
            SelectionHandle::Left => Self::clamp_resize_size(width - dx),
            SelectionHandle::Right => Self::clamp_resize_size(width + dx),
        }
    }

    fn resize_height_for_handle(handle: SelectionHandle, height: f64, dy: f64) -> f64 {
        match handle {
            SelectionHandle::TopLeft | SelectionHandle::TopRight => {
                Self::clamp_resize_size(height - dy)
            }
            SelectionHandle::BottomLeft | SelectionHandle::BottomRight => {
                Self::clamp_resize_size(height + dy)
            }
            SelectionHandle::Left | SelectionHandle::Right => height,
            SelectionHandle::Top => Self::clamp_resize_size(height - dy),
            SelectionHandle::Bottom => Self::clamp_resize_size(height + dy),
        }
    }

    fn anchor_for_handle(handle: SelectionHandle, bounds: &Rect) -> (f64, f64) {
        let x = bounds.x as f64;
        let y = bounds.y as f64;
        let w = bounds.width as f64;
        let h = bounds.height as f64;
        match handle {
            SelectionHandle::TopLeft => (x + w, y + h),
            SelectionHandle::TopRight => (x, y + h),
            SelectionHandle::BottomLeft => (x + w, y),
            SelectionHandle::BottomRight => (x, y),
            SelectionHandle::Top => (x + w / 2.0, y + h),
            SelectionHandle::Bottom => (x + w / 2.0, y),
            SelectionHandle::Left => (x + w, y + h / 2.0),
            SelectionHandle::Right => (x, y + h / 2.0),
        }
    }
}
