mod spec;

#[cfg(test)]
mod tests;

use crate::ui::toolbar::ToolbarSnapshot;

pub(super) use spec::ToolbarLayoutSpec;

/// Compute the target logical size for the top toolbar given snapshot state.
pub fn top_size(snapshot: &ToolbarSnapshot) -> (u32, u32) {
    let base = ToolbarLayoutSpec::new(snapshot).top_size(snapshot);
    scale_size(base, snapshot.toolbar_scale)
}

/// Compute the target logical size for the side toolbar given snapshot state.
pub fn side_size(snapshot: &ToolbarSnapshot) -> (u32, u32) {
    let base = ToolbarLayoutSpec::new(snapshot).side_size(snapshot);
    scale_size(base, snapshot.toolbar_scale)
}

/// Scroll bounds for the side palette as (natural_height, viewport_height),
/// both in pre-scale spec units; max scroll = (natural - viewport).max(0).
pub fn side_scroll_bounds(snapshot: &ToolbarSnapshot) -> (f64, f64) {
    let spec = ToolbarLayoutSpec::new(snapshot);
    let natural = spec.side_natural_height(snapshot);
    let (_, viewport) = spec.side_size(snapshot);
    (natural, viewport as f64)
}

/// Scroll bounds for the open Session/Settings popover on the top strip as
/// (natural_height, viewport_height), both in pre-scale spec units; `None`
/// while neither popover is open.
pub fn top_popover_scroll_bounds(snapshot: &ToolbarSnapshot) -> Option<(f64, f64)> {
    super::view::top::top_popover_scroll_bounds(snapshot)
}

fn scale_size(size: (u32, u32), scale: f64) -> (u32, u32) {
    // Sanitize scale: handle NaN/Inf and enforce bounds
    let scale = if scale.is_finite() {
        scale.clamp(0.5, 3.0)
    } else {
        1.0
    };
    (
        (size.0 as f64 * scale).ceil() as u32,
        (size.1 as f64 * scale).ceil() as u32,
    )
}
