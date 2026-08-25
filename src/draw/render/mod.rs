//! Cairo-based rendering functions for shapes.

mod backdrop_probe;
pub use backdrop_probe::painted_luminance as painted_background_luminance;
mod background;
mod blur;
mod highlight;
mod image;
mod pressure_strokes;
mod primitives;
mod selection;
mod shapes;
mod spotlight;
mod strokes;
mod text;
mod types;

pub use background::{fill_transparent, render_board_background};
pub use blur::{BlurRectParams, render_blur_rect};
pub use highlight::render_click_highlight;
#[allow(unused_imports)]
pub use pressure_strokes::render_freehand_pressure_borrowed;
pub(crate) use pressure_strokes::render_freehand_pressure_preview_borrowed;
pub(crate) use primitives::{render_polygon_preview, with_saved_state};
pub use selection::{render_selection_halo, render_selection_handles, selection_handle_rects};
pub use shapes::render_shape;
pub use spotlight::{
    IMMUTABLE_RASTER_SOURCE_TOKEN, SpotlightMagnifierMetrics, SpotlightMagnifierOutcome,
    SpotlightMagnifierScratch, SpotlightMagnifierSource, SpotlightPass, SpotlightRegion,
    SpotlightSnapshotStrategy, render_spotlight_magnification_pass, render_spotlight_outline,
    render_spotlight_pass, spotlight_regions_for_frame,
};
pub(crate) use strokes::render_eraser_stroke;
pub use strokes::{render_freehand_borrowed, render_marker_stroke_borrowed};
pub(crate) use text::render_sticky_note_preview;
pub use text::{
    caret_line_width, caret_outline_width, render_sticky_note, render_text, sticky_note_foreground,
    text_outline_color,
};
pub use types::EraserReplayContext;
