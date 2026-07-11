//! Top strip rendering: build the widget tree, paint it, emit its hits.
//!
//! All geometry lives in the tree builder (`view::top`); this module only
//! connects it to the Cairo context and the legacy hit-region consumers.

use std::time::Instant;

use anyhow::Result;

use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::view;
use crate::ui::toolbar::ToolbarSnapshot;

use super::paint::paint_tree;
use super::widgets::draw_tooltip_with_delay;

pub fn render_top_strip(
    ctx: &cairo::Context,
    width: f64,
    height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
    hover_start: Option<Instant>,
) -> Result<()> {
    let tree = view::top::build_top_view(snapshot, width, height);
    paint_tree(ctx, &tree, hover);
    hits.extend(tree.to_hit_regions());
    draw_tooltip_with_delay(ctx, hits, hover, width, height, false, hover_start);
    Ok(())
}
