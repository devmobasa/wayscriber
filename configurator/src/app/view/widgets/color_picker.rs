mod canvas;
mod inputs;
mod panel;

pub(in crate::app::view) use inputs::{color_quad_picker, color_triplet_picker};

use crate::models::ColorPickerId;

#[derive(Clone, Copy)]
pub(in crate::app::view) struct ColorPickerUi<'a> {
    pub id: ColorPickerId,
    pub is_open: bool,
    pub show_advanced: bool,
    pub hex_value: &'a str,
}
