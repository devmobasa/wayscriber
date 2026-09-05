mod actions;
mod core;
mod from_config;
mod highlight;
mod input_hud;
pub(crate) mod interaction;
mod mouse;
mod render;
mod spotlight;
mod text_resources;
pub(crate) use core::{
    DrawingStyle, HistoryLimits, IdleHandle, SpotlightMagnificationTrack, TopMenuState,
};
pub(crate) use core::{InputEffect, InputEffectDrain};
pub(in crate::input::state) use spotlight::SpotlightWheelGesture;
pub(crate) use spotlight::{SpotlightFrameRegions, SpotlightWheelClaim, SpotlightWheelOutcome};
pub(crate) use text_resources::InputTextResources;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use actions::key_press::bindings::key_to_action_label_for_test;
pub(crate) use core::board_picker::BoardPickerEditMode;
pub(crate) use core::board_picker::BoardPickerFocus;
pub(crate) use core::board_picker::{
    PAGE_DELETE_ICON_MARGIN, PAGE_DELETE_ICON_SIZE, PAGE_NAME_HEIGHT, PAGE_NAME_PADDING,
};
#[cfg(test)]
pub(crate) use core::build_text_input_preview;
pub use core::color_picker_popup::{HEX_INPUT_MAX_CHARS, color_to_hex, parse_hex_color};
pub(crate) use core::utility::ocr_scan::{OcrScanOutcome, result_opacity};
#[allow(unused_imports)]
pub use core::{
    BLOCKED_ACTION_DURATION_MS, BoardPickerCursorHint, BoardPickerLayout,
    COLOR_PICKER_POPUP_HEIGHT, COLOR_PICKER_POPUP_WIDTH, COLOR_PICKER_PREVIEW_SIZE,
    COLOR_PICKER_RECENT_SWATCH_COUNT, COLOR_PICKER_RECENT_SWATCH_SIZE, COMMAND_PALETTE_MAX_VISIBLE,
    ColorPickerCursorHint, ColorPickerPopupLayout, ColorPickerPopupState, CommandPaletteCursorHint,
    CommandPaletteListRow, CommandPaletteState, CompassDir, CompositorCapabilities,
    ContextMenuCursorHint, ContextMenuEntry, ContextMenuKind, ContextMenuState, DesktopEnvironment,
    DrawingState, EyedropperCaptureSource, EyedropperUiState, FontPickerFilter, FontPickerLayout,
    FontPickerResults, FontPickerRow, FontPickerTarget, HelpOverlayClick, HelpOverlayCursorHint,
    HelpOverlayReleaseOutcome, ImePreedit, InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS,
    OutputFocusAction, PRESET_FEEDBACK_DURATION_MS, PRESET_TOAST_DURATION_MS, PickerDrag,
    PrecisionEntryState, PresetAction, PresetFeedbackKind, PressureThicknessEditMode,
    PressureThicknessEntryMode, QuickColorEdit, RADIAL_COMPASS_SLICES, RADIAL_PAINT_DELAY,
    RADIAL_TOOL_SEGMENT_COUNT, RadialMenuLayout, RadialMenuState, RadialParent, RadialRingSwatch,
    RadialSegmentId, RadialSlice, RadialSliceKind, RegionInputSource, RegionPurposeTag,
    RegionSelectUiState, RegionSelection, SIZE_RING_ARC_SPAN, SIZE_RING_ARC_START,
    ScreenCaptureSource, SelectionAxis, SelectionHandle, SelectionPolicy, SelectionPropertyEntry,
    SelectionPropertyKind, ShellMode, TextInputMode, Toast, ToastPriority, ToastPushOutcome,
    TourStep, UI_TOAST_DURATION_MS, UiToastKind, UiVisibility, ZoomAction, color_picker_rgb_to_hsv,
    compass_slice, font_picker_layout, font_picker_rows, size_ring_angle_for_value,
    size_ring_value_for_angle, slice_parent, sub_ring_child_count, sub_ring_children,
};
#[allow(unused_imports)]
pub(crate) use core::{
    BoardPasteTarget, ClipboardFingerprint, ClipboardPasteRequest, HelpOverlayPressSource,
    HelperLaunchRequest, HexPasteTarget, KeybindingEditOperation, KeybindingEditRequest,
    LocalSelectionContext, PasteAnchor, PendingBackendAction, PendingOnboardingUsage,
    PendingSelectionClipboardPublish, PendingToolbarPersistence, TextClipboardRequest,
    TextCutTarget, TextPasteEdit, TextPasteTarget, ToastCommand, ToastPress,
    WayscriberClipboardSelection,
};
pub(crate) use core::{
    COMMAND_PALETTE_INPUT_HEIGHT, COMMAND_PALETTE_ITEM_HEIGHT, COMMAND_PALETTE_LIST_GAP,
    COMMAND_PALETTE_PADDING, COMMAND_PALETTE_QUERY_PLACEHOLDER, COMMAND_PALETTE_ROW_ACTION_COUNT,
    COMMAND_PALETTE_ROW_ACTION_GAP, COMMAND_PALETTE_ROW_ACTION_SIZE, COMMAND_PALETTE_ROW_ICON_GAP,
    COMMAND_PALETTE_ROW_ICON_SIZE, COMMAND_PALETTE_TOP_RATIO, action_meta_token_score, fuzzy_score,
    query_tokens,
};
pub use highlight::ClickHighlightSettings;
#[allow(unused_imports)]
pub use input_hud::{
    InputHudActiveSource, InputHudEntry, InputHudEntryKind, InputHudSettings, InputHudState,
    input_hud_key_label, input_hud_mouse_label, input_hud_scroll_label, is_bare_modifier,
};

#[cfg(test)]
pub(crate) mod test_support {
    use super::core::{CanvasIndex, InputStateSeed, Keymap};
    use crate::config::{Action, BoardsConfig, KeybindingsConfig, PresenterModeConfig, Shortcut};
    use crate::draw::FontDescriptor;
    use crate::input::{ClickHighlightSettings, InputState};
    use std::collections::HashMap;

    pub(crate) struct TestInputStateBuilder {
        seed: InputStateSeed,
    }

    impl Default for TestInputStateBuilder {
        fn default() -> Self {
            Self::with_keybindings(KeybindingsConfig::default())
        }
    }

    impl TestInputStateBuilder {
        pub(crate) fn with_keybindings(keybindings: KeybindingsConfig) -> Self {
            let keymap = Keymap::from_config(
                &keybindings,
                &crate::config::MouseDragToolsConfig::default(),
            );
            let mut style = super::DrawingStyle::from((
                &crate::config::DrawingConfig::default(),
                &crate::config::ArrowConfig::default(),
                &crate::config::SpotlightConfig::default(),
            ));
            style.current_color = crate::draw::RED;
            style.current_thickness = 4.0;
            style.eraser_size = 4.0;
            style.marker_opacity = 0.32;
            style.current_font_size = 32.0;
            style.font_descriptor = FontDescriptor::default();
            style.arrow_length = 20.0;
            style.arrow_angle = 30.0;
            style.arrow_head_at_end = false;
            style.tool_settings = crate::input::PerToolDrawingSettings::new(
                style.current_color,
                style.current_thickness,
            );
            style.tool_settings.step_marker.thickness =
                super::core::utility::default_step_marker_size(style.current_font_size);
            let history_limits = super::HistoryLimits::from(&crate::config::HistoryConfig {
                undo_all_delay_ms: 0,
                redo_all_delay_ms: 0,
                custom_section_enabled: true,
                custom_undo_delay_ms: 0,
                custom_redo_delay_ms: 0,
                custom_undo_steps: 5,
                custom_redo_steps: 5,
            });

            Self {
                seed: InputStateSeed {
                    style,
                    ui_visibility: super::UiVisibility::from(&crate::config::UiConfig::default()),
                    boards_config: BoardsConfig::default(),
                    keymap,
                    canvas_index: CanvasIndex::from_config(6.0, usize::MAX),
                    click_highlight_settings: ClickHighlightSettings::disabled(),
                    history_limits,
                    presenter_mode_config: PresenterModeConfig::default(),
                },
            }
        }

        pub(crate) fn action_map(mut self, action_map: HashMap<Shortcut, Action>) -> Self {
            self.seed.keymap.set_action_map(action_map);
            self
        }

        pub(crate) fn action_bindings(
            mut self,
            action_bindings: HashMap<Action, Vec<Shortcut>>,
        ) -> Self {
            self.seed.keymap.set_action_bindings(action_bindings);
            self
        }

        pub(crate) fn thickness(mut self, thickness: f64) -> Self {
            self.seed.style.current_thickness = thickness;
            for tool in crate::input::tool::ToolSettingsSlot::ALL {
                self.seed.style.tool_settings.get_slot_mut(tool).thickness = thickness;
            }
            self
        }

        pub(crate) fn eraser_size(mut self, eraser_size: f64) -> Self {
            self.seed.style.eraser_size = eraser_size;
            self
        }

        pub(crate) fn font_descriptor(mut self, font_descriptor: FontDescriptor) -> Self {
            self.seed.style.font_descriptor = font_descriptor;
            self
        }

        pub(crate) fn text_background_enabled(mut self, enabled: bool) -> Self {
            self.seed.style.text_background_enabled = enabled;
            self
        }

        pub(crate) fn click_highlight_settings(mut self, settings: ClickHighlightSettings) -> Self {
            self.seed.click_highlight_settings = settings;
            self
        }

        pub(crate) fn custom_section_enabled(mut self, enabled: bool) -> Self {
            self.seed.history_limits.set_custom_section_enabled(enabled);
            self
        }

        pub(crate) fn build(self) -> InputState {
            InputState::from_seed(self.seed)
        }
    }

    pub(crate) fn make_test_input_state() -> InputState {
        TestInputStateBuilder::default().build()
    }

    // This helper is for tests that only need a stable InputState plus optional
    // action-binding label overrides. It intentionally keeps the default
    // dispatch/action map and swaps only the formatted bindings.
    pub(crate) fn make_test_input_state_with_action_bindings(
        action_bindings: HashMap<Action, Vec<Shortcut>>,
    ) -> InputState {
        TestInputStateBuilder::default()
            .action_bindings(action_bindings)
            .build()
    }

    #[test]
    fn test_input_state_builder_applies_named_overrides() {
        let state = TestInputStateBuilder::default()
            .thickness(3.0)
            .eraser_size(12.0)
            .text_background_enabled(true)
            .custom_section_enabled(false)
            .build();

        assert_eq!(state.style.current_thickness, 3.0);
        assert_eq!(state.style.eraser_size, 12.0);
        assert!(state.style.text_background_enabled);
        assert!(!state.history_limits.custom_section_enabled());
    }
}
