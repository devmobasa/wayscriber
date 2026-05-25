mod inputs;
mod panel;

pub(in crate::app::view) use inputs::{
    color_quad_picker, color_rgb255_picker, color_triplet_picker,
};
pub(in crate::app::view) use panel::picker_panel;

use crate::models::ColorPickerId;

#[derive(Clone, Copy)]
pub(in crate::app::view) struct ColorPickerUi<'a> {
    pub id: ColorPickerId,
    pub is_open: bool,
    pub show_advanced: bool,
    pub hex_value: &'a str,
}
