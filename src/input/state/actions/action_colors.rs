use crate::config::QuickColorPalette;
use crate::domain::Action;
use crate::draw::Color;
use crate::input::Tool;

use super::super::{HexPasteTarget, InputState};

/// Cap on the session-only recent-color list (`InputState::recent_colors`).
pub(crate) const RECENT_COLORS_CAP: usize = 6;

impl InputState {
    pub fn set_quick_colors(&mut self, quick_colors: QuickColorPalette) {
        self.quick_colors = quick_colors;
    }

    pub(in crate::input::state) fn handle_color_action(&mut self, action: Action) -> bool {
        if action == Action::PickScreenColor {
            self.request_eyedropper_toggle();
            return true;
        }
        let Some(color) = self.quick_colors.color_for_action(action) else {
            return false;
        };
        let _ = self.apply_color_from_ui(color);
        true
    }

    pub(crate) fn apply_color_from_ui(&mut self, color: Color) -> bool {
        self.note_recent_color(color);
        // First-run teaching signal: any color application (quick-color key,
        // radial swatch, picker, eyedropper) counts as "changed a color".
        self.pending_onboarding_usage.used_color_change = true;
        let mut changed = self.set_color(color);
        if self.active_tool() == Tool::Select && !self.selected_shape_ids().is_empty() {
            let selection_changed = self.apply_selection_color_value(color);
            changed = selection_changed || changed;
        }
        changed
    }

    /// Record a UI-applied color in the session-only recents list:
    /// most-recent-first, deduped, capped (never persisted).
    fn note_recent_color(&mut self, color: Color) {
        self.recent_colors.retain(|recent| *recent != color);
        self.recent_colors.insert(0, color);
        self.recent_colors.truncate(RECENT_COLORS_CAP);
    }

    /// Request a hex-color copy to the clipboard. The color is captured now so
    /// a later popup or tool transition cannot retarget the request.
    pub fn request_copy_hex(&mut self) {
        let color = self
            .color_picker_popup_current_color()
            .unwrap_or_else(|| self.color_for_tool(self.active_tool()));
        self.pending_copy_hex = Some(color);
    }

    /// Request a hex-color paste from the clipboard. Popup requests retain the
    /// current popup generation so a later popup cannot inherit them.
    pub fn request_paste_hex(&mut self) {
        self.pending_paste_hex = Some(
            self.color_picker_popup_generation()
                .map_or(HexPasteTarget::ActiveTool, |generation| {
                    HexPasteTarget::ColorPickerPopup { generation }
                }),
        );
    }

    /// Take and clear whether a copy-hex request is pending.
    pub fn take_pending_copy_hex(&mut self) -> bool {
        self.take_pending_copy_hex_request().is_some()
    }

    pub(crate) fn take_pending_copy_hex_request(&mut self) -> Option<Color> {
        self.pending_copy_hex.take()
    }

    /// Take and clear whether a paste-hex request is pending.
    pub fn take_pending_paste_hex(&mut self) -> bool {
        self.take_pending_paste_hex_request().is_some()
    }

    pub(crate) fn take_pending_paste_hex_request(&mut self) -> Option<HexPasteTarget> {
        self.pending_paste_hex.take()
    }

    pub(crate) fn hex_paste_target_is_current(&self, target: HexPasteTarget) -> bool {
        match target {
            HexPasteTarget::ActiveTool => true,
            HexPasteTarget::ColorPickerPopup { generation } => {
                self.color_picker_popup_generation_is_current(generation)
            }
        }
    }
}
