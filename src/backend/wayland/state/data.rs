#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDragKind {
    Top,
}

/// Rendering scratch state awaiting extraction into `RenderRuntime`.
#[derive(Debug, Default)]
pub struct StateData {
    /// Reused pre-UI pixel snapshot for render-profile UI-only remapping.
    pub(super) render_profile_ui_baseline: Vec<u8>,
    /// Previous-frame damage bounds for transient UI effects, so partial
    /// redraws cover both the old and new footprint of each effect.
    pub(super) prev_ui_toast_damage: Option<crate::util::Rect>,
    pub(super) prev_preset_toast_damage: Option<crate::util::Rect>,
    pub(super) blocked_feedback_was_active: bool,
    pub(super) prev_text_edit_entry_damage: Option<crate::util::Rect>,
    pub(super) prev_status_hud_damage: Option<crate::util::Rect>,
    pub(super) prev_zoom_chip_damage: Option<crate::util::Rect>,
    pub(super) prev_input_hud_damage: Option<crate::util::Rect>,
    pub(super) prev_command_palette_damage: Option<crate::util::Rect>,
    pub(super) prev_color_picker_damage: Option<crate::util::Rect>,
    pub(super) prev_tool_preview_damage: Option<crate::util::Rect>,
    pub(super) prev_shape_measure_badge_damage: Option<crate::util::Rect>,
    /// Union the OCR scan overlay covered last frame, so its sweep is cleared.
    pub(super) prev_ocr_scan_damage: Option<crate::util::Rect>,
    /// Previous-frame strips for Measure Mode's crosshair, frame, and readout.
    pub(super) prev_measure_picker_damage: Vec<crate::util::Rect>,
}

impl StateData {
    pub fn new() -> Self {
        Self {
            render_profile_ui_baseline: Vec::new(),
            prev_ui_toast_damage: None,
            prev_preset_toast_damage: None,
            blocked_feedback_was_active: false,
            prev_text_edit_entry_damage: None,
            prev_status_hud_damage: None,
            prev_zoom_chip_damage: None,
            prev_input_hud_damage: None,
            prev_command_palette_damage: None,
            prev_color_picker_damage: None,
            prev_tool_preview_damage: None,
            prev_shape_measure_badge_damage: None,
            prev_ocr_scan_damage: None,
            prev_measure_picker_damage: Vec::new(),
        }
    }
}
