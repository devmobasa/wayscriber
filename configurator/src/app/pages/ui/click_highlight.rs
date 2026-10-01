use relm4::ComponentSender;
use wayscriber::config::{
    CLICK_HIGHLIGHT_DURATION_MAX_MS, CLICK_HIGHLIGHT_DURATION_MIN_MS, CLICK_HIGHLIGHT_OUTLINE_MAX,
    CLICK_HIGHLIGHT_OUTLINE_MIN, CLICK_HIGHLIGHT_RADIUS_MAX, CLICK_HIGHLIGHT_RADIUS_MIN,
};

use crate::messages::Message;
use crate::models::{ColorPickerId, TabId, TextField, ToggleField};

use super::super::super::state::ConfiguratorApp;
use super::super::color_rows::color_row;
use super::super::{BuiltPage, PageBuilder, validate_u32_range};
use super::{quad_color, validate_f64_range};

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group("Click Highlight")
        .switch_row(
            "Enable click highlight",
            "",
            |app| app.draft.click_highlight_enabled,
            |value| Message::ToggleChanged(ToggleField::UiClickHighlightEnabled, value),
        )
        .switch_row(
            "Show ring while highlight tool is active",
            "",
            |app| app.draft.click_highlight_show_on_highlight_tool,
            |value| Message::ToggleChanged(ToggleField::UiClickHighlightShowOnHighlightTool, value),
        )
        .switch_row(
            "Link highlight color to current pen",
            "",
            |app| app.draft.click_highlight_use_pen_color,
            |value| Message::ToggleChanged(ToggleField::UiClickHighlightUsePenColor, value),
        )
        .switch_row(
            "Force on when entering light mode",
            "",
            |app| app.draft.click_highlight_force_in_light_mode,
            |value| Message::ToggleChanged(ToggleField::UiClickHighlightForceInLightMode, value),
        );

    page.group("Ring")
        .entry_row_validated(
            "Radius",
            |app| app.draft.click_highlight_radius.clone(),
            |value| Message::TextChanged(TextField::HighlightRadius, value),
            |app| {
                validate_f64_range(
                    &app.draft.click_highlight_radius,
                    CLICK_HIGHLIGHT_RADIUS_MIN,
                    CLICK_HIGHLIGHT_RADIUS_MAX,
                )
            },
        )
        .entry_row_validated(
            "Outline thickness",
            |app| app.draft.click_highlight_outline_thickness.clone(),
            |value| Message::TextChanged(TextField::HighlightOutlineThickness, value),
            |app| {
                validate_f64_range(
                    &app.draft.click_highlight_outline_thickness,
                    CLICK_HIGHLIGHT_OUTLINE_MIN,
                    CLICK_HIGHLIGHT_OUTLINE_MAX,
                )
            },
        )
        .entry_row_validated(
            "Duration (ms)",
            |app| app.draft.click_highlight_duration_ms.clone(),
            |value| Message::TextChanged(TextField::HighlightDurationMs, value),
            |app| {
                validate_u32_range(
                    &app.draft.click_highlight_duration_ms,
                    CLICK_HIGHLIGHT_DURATION_MIN_MS as u32,
                    CLICK_HIGHLIGHT_DURATION_MAX_MS as u32,
                )
            },
        );

    page.group("Colors");
    color_row(
        &mut page,
        "Fill (hex)",
        ColorPickerId::HighlightFill,
        |app| quad_color(&app.draft.click_highlight_fill_color.components),
    );
    color_row(
        &mut page,
        "Outline (hex)",
        ColorPickerId::HighlightOutline,
        |app| quad_color(&app.draft.click_highlight_outline_color.components),
    );

    page.finish()
}
