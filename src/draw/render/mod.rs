//! Cairo-based rendering functions for shapes.

mod background;
mod blur;
mod highlight;
mod image;
mod pressure_strokes;
mod primitives;
mod selection;
mod shapes;
mod strokes;
mod text;
mod types;

pub use background::{fill_transparent, render_board_background};
pub use blur::{BlurRectParams, render_blur_rect};
pub use highlight::render_click_highlight;
#[allow(unused_imports)]
pub use pressure_strokes::render_freehand_pressure_borrowed;
pub(crate) use pressure_strokes::render_freehand_pressure_preview_borrowed;
pub(crate) use primitives::render_polygon_preview;
pub use selection::{render_selection_halo, render_selection_handles, selection_handle_rects};
pub use shapes::render_shape;
pub(crate) use strokes::render_eraser_stroke;
pub use strokes::{render_freehand_borrowed, render_marker_stroke_borrowed};
pub use text::{render_sticky_note, render_text};
pub use types::EraserReplayContext;
