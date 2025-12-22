#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ToolbarOffsets {
    pub top_x: f64,
    pub top_y: f64,
    pub side_x: f64,
    pub side_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ToolbarClampBounds {
    pub min_top_x: f64,
    pub max_top_x: f64,
    pub min_top_y: f64,
    pub max_top_y: f64,
    pub min_side_x: f64,
    pub max_side_x: f64,
    pub min_side_y: f64,
    pub max_side_y: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ToolbarClampInput {
    pub width: f64,
    pub height: f64,
    pub top_size: (u32, u32),
    pub side_size: (u32, u32),
    pub top_base_x: f64,
    pub top_base_y: f64,
    pub top_margin_right: f64,
    pub top_margin_bottom: f64,
    pub side_base_margin_left: f64,
    pub side_base_margin_top: f64,
    pub side_margin_right: f64,
    pub side_margin_bottom: f64,
}

pub(super) fn compute_inline_top_base_x(
    base: f64,
    side_visible: bool,
    side_width: f64,
    side_start_y: f64,
    top_bottom_y: f64,
    inline_top_push: f64,
    allow_push: bool,
) -> f64 {
    let side_overlaps_top = side_visible && side_start_y < top_bottom_y;
    if side_overlaps_top && allow_push {
        base + side_width + inline_top_push
    } else {
        base
    }
}

pub(super) fn clamp_toolbar_offsets(
    offsets: ToolbarOffsets,
    input: ToolbarClampInput,
) -> (ToolbarOffsets, ToolbarClampBounds) {
    let top_w = input.top_size.0 as f64;
    let top_h = input.top_size.1 as f64;
    let side_w = input.side_size.0 as f64;
    let side_h = input.side_size.1 as f64;

    let mut max_top_x = (input.width - top_w - input.top_base_x - input.top_margin_right).max(0.0);
    let mut max_top_y =
        (input.height - top_h - input.top_base_y - input.top_margin_bottom).max(0.0);
    let mut max_side_y =
        (input.height - side_h - input.side_base_margin_top - input.side_margin_bottom).max(0.0);
    let mut max_side_x =
        (input.width - side_w - input.side_base_margin_left - input.side_margin_right).max(0.0);

    max_top_x = max_top_x.max(0.0);
    max_top_y = max_top_y.max(0.0);
    max_side_x = max_side_x.max(0.0);
    max_side_y = max_side_y.max(0.0);

    let min_top_x = -input.top_base_x;
    let min_top_y = -input.top_base_y;
    let min_side_x = -input.side_base_margin_left;
    let min_side_y = -input.side_base_margin_top;

    let clamped = ToolbarOffsets {
        top_x: offsets.top_x.clamp(min_top_x, max_top_x),
        top_y: offsets.top_y.clamp(min_top_y, max_top_y),
        side_x: offsets.side_x.clamp(min_side_x, max_side_x),
        side_y: offsets.side_y.clamp(min_side_y, max_side_y),
    };

    let bounds = ToolbarClampBounds {
        min_top_x,
        max_top_x,
        min_top_y,
        max_top_y,
        min_side_x,
        max_side_x,
        min_side_y,
        max_side_y,
    };

    (clamped, bounds)
}

pub(super) fn compute_layer_margins(
    top_base_x: f64,
    top_base_margin_top: f64,
    side_base_margin_left: f64,
    side_base_margin_top: f64,
    offsets: ToolbarOffsets,
) -> (i32, i32, i32, i32) {
    let top_margin_left = (top_base_x + offsets.top_x).round() as i32;
    let top_margin_top = (top_base_margin_top + offsets.top_y).round() as i32;
    let side_margin_top = (side_base_margin_top + offsets.side_y).round() as i32;
    let side_margin_left = (side_base_margin_left + offsets.side_x).round() as i32;
    (
        top_margin_left,
        top_margin_top,
        side_margin_top,
        side_margin_left,
    )
}

pub(super) fn point_in_rect(px: f64, py: f64, x: f64, y: f64, w: f64, h: f64) -> bool {
    px >= x && px <= x + w && py >= y && py <= y + h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_top_base_x_pushes_when_overlapping_and_allowed() {
        let base = 24.0;
        let side_width = 80.0;
        let top_bottom_y = 40.0;
        let side_start_y = 12.0;
        let push = 16.0;
        let result = compute_inline_top_base_x(
            base,
            true,
            side_width,
            side_start_y,
            top_bottom_y,
            push,
            true,
        );
        assert_eq!(result, 120.0);
    }

    #[test]
    fn inline_top_base_x_skips_push_when_not_allowed() {
        let result = compute_inline_top_base_x(24.0, true, 80.0, 10.0, 40.0, 16.0, false);
        assert_eq!(result, 24.0);
    }

    #[test]
    fn point_in_rect_includes_edges() {
        assert!(point_in_rect(10.0, 10.0, 10.0, 10.0, 20.0, 20.0));
        assert!(point_in_rect(30.0, 30.0, 10.0, 10.0, 20.0, 20.0));
        assert!(!point_in_rect(30.1, 30.0, 10.0, 10.0, 20.0, 20.0));
    }

    #[test]
    fn clamp_toolbar_offsets_bounds_and_values() {
        let offsets = ToolbarOffsets {
            top_x: 500.0,
            top_y: -100.0,
            side_x: -100.0,
            side_y: 500.0,
        };
        let input = ToolbarClampInput {
            width: 300.0,
            height: 200.0,
            top_size: (100, 40),
            side_size: (60, 120),
            top_base_x: 40.0,
            top_base_y: 16.0,
            top_margin_right: 12.0,
            top_margin_bottom: 0.0,
            side_base_margin_left: 24.0,
            side_base_margin_top: 24.0,
            side_margin_right: 0.0,
            side_margin_bottom: 24.0,
        };

        let (clamped, bounds) = clamp_toolbar_offsets(offsets, input);
        assert_eq!(
            bounds,
            ToolbarClampBounds {
                min_top_x: -40.0,
                max_top_x: 148.0,
                min_top_y: -16.0,
                max_top_y: 144.0,
                min_side_x: -24.0,
                max_side_x: 216.0,
                min_side_y: -24.0,
                max_side_y: 32.0,
            }
        );
        assert_eq!(
            clamped,
            ToolbarOffsets {
                top_x: 148.0,
                top_y: -16.0,
                side_x: -24.0,
                side_y: 32.0,
            }
        );
    }

    #[test]
    fn pipeline_clamp_then_margins_matches_expected() {
        let base = 24.0;
        let top_base_x = compute_inline_top_base_x(base, true, 80.0, 12.0, 48.0, 16.0, true);
        let input = ToolbarClampInput {
            width: 500.0,
            height: 220.0,
            top_size: (200, 40),
            side_size: (80, 120),
            top_base_x,
            top_base_y: 16.0,
            top_margin_right: 12.0,
            top_margin_bottom: 0.0,
            side_base_margin_left: 24.0,
            side_base_margin_top: 24.0,
            side_margin_right: 0.0,
            side_margin_bottom: 24.0,
        };
        let offsets = ToolbarOffsets {
            top_x: 400.0,
            top_y: 10.0,
            side_x: 0.0,
            side_y: 0.0,
        };
        let (clamped, _) = clamp_toolbar_offsets(offsets, input);
        let (top_left, top_top, side_top, side_left) =
            compute_layer_margins(top_base_x, 12.0, 24.0, 24.0, clamped);

        assert_eq!(top_left, 288);
        assert_eq!(top_top, 22);
        assert_eq!(side_top, 24);
        assert_eq!(side_left, 24);
    }

    #[test]
    fn pipeline_negative_offsets_reach_left_edge() {
        let input = ToolbarClampInput {
            width: 300.0,
            height: 200.0,
            top_size: (120, 40),
            side_size: (60, 120),
            top_base_x: 40.0,
            top_base_y: 16.0,
            top_margin_right: 12.0,
            top_margin_bottom: 0.0,
            side_base_margin_left: 24.0,
            side_base_margin_top: 24.0,
            side_margin_right: 0.0,
            side_margin_bottom: 24.0,
        };
        let offsets = ToolbarOffsets {
            top_x: -400.0,
            top_y: 0.0,
            side_x: 0.0,
            side_y: 0.0,
        };
        let (clamped, _) = clamp_toolbar_offsets(offsets, input);
        let (top_left, _, _, _) = compute_layer_margins(40.0, 12.0, 24.0, 24.0, clamped);
        assert_eq!(top_left, 0);
    }
}
