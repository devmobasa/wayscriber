mod arrow;
mod color;
mod fill;
mod preset;
mod spotlight;
mod stroke;
mod text;

pub(crate) use color::RecolorOpacity;

use crate::draw::Shape;
use crate::input::state::core::base::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};
use crate::input::state::core::properties::summary::shape_thickness;
use crate::input::state::core::properties::types::SelectionPropertyKind;

/// Whether a slider for `kind` has anything to set on `shape`.
pub(in crate::input::state::core::properties) fn level_applies(
    kind: SelectionPropertyKind,
    shape: &Shape,
) -> bool {
    match kind {
        SelectionPropertyKind::Thickness => shape_thickness(shape).is_some(),
        SelectionPropertyKind::Opacity => color::has_color(shape),
        _ => false,
    }
}

/// Sets `kind` to `value` on one shape, as a slider drag previews it:
/// whether the shape changed.
pub(in crate::input::state::core::properties) fn set_level(
    kind: SelectionPropertyKind,
    shape: &mut Shape,
    value: f64,
) -> bool {
    match kind {
        SelectionPropertyKind::Thickness => stroke::set_thickness(
            shape,
            value.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS),
        )
        .unwrap_or(false),
        SelectionPropertyKind::Opacity => color::set_opacity_to(shape, value).unwrap_or(false),
        _ => false,
    }
}
