mod paint;
mod top_strip;
mod widgets;

use std::time::Duration;

pub use top_strip::render_top_strip;

/// Delay before showing toolbar tooltips.
pub(in crate::backend::wayland) const TOOLTIP_DELAY: Duration = Duration::from_millis(250);
