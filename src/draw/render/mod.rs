//! Cairo-based rendering functions for shapes.

mod background;
mod highlight;
mod primitives;
mod selection;
mod shapes;
mod strokes;
mod text;
mod types;

pub use background::{fill_transparent, render_board_background};
pub use highlight::render_click_highlight;
pub use selection::render_selection_halo;
pub use shapes::render_shape;
pub(crate) use strokes::render_eraser_stroke;
pub use strokes::{render_freehand_borrowed, render_marker_stroke_borrowed};
pub use text::{render_sticky_note, render_text};
pub use types::EraserReplayContext;
