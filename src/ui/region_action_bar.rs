//! Region review actions, their layout, and painting facade.

mod controls;
mod layout;
mod model;
mod render;

pub(crate) use layout::{RegionActionBar, RegionActionRect};
pub(crate) use model::{
    RegionAction, RegionActionAvailability, RegionActionBarVisual, RegionCutStatus,
};
pub(crate) use render::render_region_action_bar;

use crate::ui_text::UiTextStyle;

const TOGGLE_FONT_SIZE: f64 = 10.5;

fn status_label_style(size: f64) -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size,
    }
}

#[cfg(test)]
mod tests;
