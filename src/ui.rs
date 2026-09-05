pub mod toolbar;

pub mod anim;
mod arrow_bend_handle;
mod board_picker;
mod color_picker_popup;
mod command_palette;
pub mod constants;
mod context_menu;
mod eyedropper_loupe;
mod font_picker;
mod help_overlay;
mod input_hud;
mod measure_badge;
mod ocr_scan;
mod onboarding_card;
mod precision_entry;
mod primitives;
mod properties_panel;
mod radial_menu;
mod render_context;
pub(crate) use render_context::{UiRenderCaches, UiRenderCtx};
mod region_action_bar;
mod region_capture_picker;
mod region_resize_handles;
mod spotlight_control;
mod status;
mod text_highlight;
pub mod theme;
mod toasts;
mod tour;

pub(crate) use arrow_bend_handle::render_arrow_bend_handle;
pub use board_picker::render_board_picker;
pub(crate) use board_picker::render_board_picker_with_halo;
pub use color_picker_popup::{color_picker_popup_visual_geometry, render_color_picker_popup};
pub(crate) use color_picker_popup::{
    color_picker_popup_visual_geometry_with_engine, render_color_picker_popup_with_engine,
};
pub(crate) use command_palette::{
    CommandPaletteView, command_palette_visual_geometry_with_engine, paint_command_palette,
};
pub use command_palette::{command_palette_visual_geometry, render_command_palette};
pub use context_menu::render_context_menu;
pub(crate) use context_menu::render_context_menu_with_engine;
pub(crate) use eyedropper_loupe::{compute_eyedropper_loupe_layout, render_eyedropper_loupe};
pub use font_picker::render_font_picker;
#[allow(unused_imports)]
pub use help_overlay::HelpOverlayBindings;
pub(crate) use help_overlay::{HelpContentCache, render_help_overlay_result_with_content};
#[allow(unused_imports)]
pub use help_overlay::{
    HelpHitMap, HelpOverlayRegion, HelpRenderResult, render_help_overlay,
    render_help_overlay_result,
};
pub use input_hud::{input_hud_geometry, render_input_hud};
pub(crate) use input_hud::{input_hud_geometry_with_engine, render_input_hud_with_engine};
pub(crate) use measure_badge::{
    ShapeMeasureBadge, measure_shape_badge, shape_measure_badge_text_style,
};
pub(crate) use ocr_scan::{
    ocr_scan_geometry, render_ocr_scan_result, render_ocr_scan_still, render_ocr_scan_sweep,
};
pub(crate) use onboarding_card::render_onboarding_card_with_engine;
pub use onboarding_card::{OnboardingCard, OnboardingChecklistItem, render_onboarding_card};
pub use precision_entry::render_precision_entry_popup;
pub(crate) use precision_entry::render_precision_entry_popup_with_engine;
/// Shared measured-text trimming, also used by the standalone about dialog.
pub(crate) use primitives::ellipsize_to_fit_with_engine;
pub(crate) use primitives::{checkerboard_behind, draw_pill};
pub use properties_panel::render_properties_panel;
pub(crate) use properties_panel::render_properties_panel_with_engine;
pub use radial_menu::render_radial_menu;
pub(crate) use radial_menu::render_radial_menu_with_context;
pub(crate) use region_action_bar::{
    RegionAction, RegionActionAvailability, RegionActionBar, RegionCutStatus,
};
pub(crate) use region_capture_picker::{
    OCR_LEGEND_TEXT, RegionCaptureCutVisual, RegionCaptureLoupeVisual, RegionCapturePickerVisual,
    RegionCaptureWindowVisual, RegionCutDragVisual, RegionCutPreviewVisual, capture_size_text,
    measure_picker_damage, render_region_capture_picker, render_region_legend,
};
pub(crate) use region_resize_handles::RegionResizeHandles;
pub(crate) use spotlight_control::render_spotlight_magnification_control;
pub use status::{
    StatusHudLayout, StatusHudSegmentKind, ZoomChipButtonKind, ZoomChipLayout, ZoomChipPress,
    compute_status_hud_layout, compute_zoom_chip_layout, render_editing_badge, render_frozen_badge,
    render_page_badge, render_pan_badge, render_status_bar, render_status_bar_with_theme,
    render_zoom_badge, render_zoom_chip, render_zoom_chip_with_theme, status_hud_geometry,
    zoom_chip_geometry,
};
pub(crate) use status::{
    compute_status_hud_layout_with_resources, compute_zoom_chip_layout_with_engine,
    render_editing_badge_with_engine, render_frozen_badge_with_engine,
    render_page_badge_with_engine, render_pan_badge_with_engine, render_status_bar_with_resources,
    render_zoom_badge_with_engine, render_zoom_chip_with_resources,
};
pub use toasts::{
    blocked_feedback_rects, preset_toast_geometry, render_blocked_feedback, render_preset_toast,
    render_ui_toast, ui_toast_geometry,
};
pub(crate) use toasts::{
    preset_toast_geometry_with_engine, render_preset_toast_with_engine,
    render_ui_toast_with_engine, ui_toast_geometry_with_engine,
};
pub use tour::render_tour;
pub(crate) use tour::render_tour_with_engine;

#[cfg(test)]
#[path = "ui/tests/theme_compatibility.rs"]
mod theme_compatibility;
