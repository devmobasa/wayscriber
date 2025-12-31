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
    ToolbarLayoutSpec::new(snapshot).top_size(snapshot)
}

/// Compute the target logical size for the side toolbar given snapshot state.
pub fn side_size(snapshot: &ToolbarSnapshot) -> (u32, u32) {
    ToolbarLayoutSpec::new(snapshot).side_size(snapshot)
}
