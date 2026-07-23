pub mod toolbar;

pub mod anim;
mod board_picker;
mod color_picker_popup;
mod command_palette;
pub mod constants;
mod context_menu;
mod help_overlay;
mod onboarding_card;
mod precision_entry;
mod primitives;
mod properties_panel;
mod radial_menu;
mod status;
mod text_highlight;
pub mod theme;
mod toasts;
mod tour;

pub use board_picker::render_board_picker;
pub use color_picker_popup::{color_picker_popup_visual_geometry, render_color_picker_popup};
pub use command_palette::{command_palette_visual_geometry, render_command_palette};
pub use context_menu::render_context_menu;
#[allow(unused_imports)]
pub use help_overlay::HelpOverlayBindings;
#[cfg(test)]
pub use help_overlay::install_help_hit_map_for_test;
#[allow(unused_imports)]
pub use help_overlay::{
    HelpOverlayRegion, clear_help_overlay_hit_map, help_overlay_region_at,
    invalidate_help_overlay_cache, render_help_overlay,
};
pub use onboarding_card::{OnboardingCard, OnboardingChecklistItem, render_onboarding_card};
pub use precision_entry::render_precision_entry_popup;
pub use properties_panel::render_properties_panel;
pub use radial_menu::render_radial_menu;
pub use status::{
    StatusHudLayout, StatusHudSegmentKind, ZoomChipButtonKind, ZoomChipLayout, ZoomChipPress,
    compute_status_hud_layout, compute_zoom_chip_layout, render_editing_badge, render_frozen_badge,
    render_page_badge, render_pan_badge, render_status_bar, render_zoom_badge, render_zoom_chip,
    status_hud_geometry, zoom_chip_geometry,
};
pub use toasts::{
    blocked_feedback_rects, preset_toast_geometry, render_blocked_feedback, render_preset_toast,
    render_ui_toast, ui_toast_geometry,
};
pub use tour::render_tour;
