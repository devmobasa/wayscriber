mod side;
mod spec;
mod top;

#[cfg(test)]
mod tests;

use crate::ui::toolbar::ToolbarSnapshot;

pub use side::build_side_hits;
pub(super) use spec::ToolbarLayoutSpec;
pub use top::build_top_hits;

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
