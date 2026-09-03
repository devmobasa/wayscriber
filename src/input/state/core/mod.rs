mod base;
mod board;
pub(crate) mod board_picker;
mod captured_image;
pub(crate) mod color_picker_popup;
mod command_palette;
mod dirty;
mod eyedropper;
mod font_cycle;
pub(crate) mod font_picker;
mod help_overlay;
mod highlight_controls;
mod history;
mod history_limits;
mod ime;
mod index;
mod input_hud_controls;
pub(crate) mod key_repeat;
mod keymap;
mod menus;
pub(crate) mod modal;
mod presets;
mod properties;
pub(crate) mod radial_menu;
mod region_select;
mod search;
mod selection;
mod selection_actions;
pub(crate) use history_limits::HistoryLimits;
pub(in crate::input::state) use keymap::Keymap;
pub(crate) use presets::PresetSlots;
pub(crate) use selection_actions::{IdleHandle, SpotlightMagnificationTrack};
pub(in crate::input::state) use view::ViewState;
mod session;
mod session_preflight;
mod session_preflight_exact;
mod status_hud;
mod style;
mod text_editing;
mod text_font;
mod tool_controls;
mod top_menu;
mod tour;
pub(crate) mod utility;
mod view;
mod zoom_chip;

#[cfg(test)]
mod top_menu_tests;

pub(crate) use top_menu::TopMenuState;

pub(in crate::input::state) use base::InputStateSeed;
pub use base::{
    BLOCKED_ACTION_DURATION_MS, CompositorCapabilities, DesktopEnvironment, DrawingState,
    InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS, OutputFocusAction,
    PRESET_FEEDBACK_DURATION_MS, PRESET_TOAST_DURATION_MS, PresetAction, PresetFeedbackKind,
    PressureThicknessEditMode, PressureThicknessEntryMode, QuickColorEdit, SelectionAxis,
    SelectionHandle, ShellMode, TextInputMode, Toast, ToastPriority, ToastPushOutcome, ToastQueue,
    UI_TOAST_DURATION_MS, UiToastKind, UiVisibility, ZoomAction,
};
pub(crate) use base::{
    BoardPickerClickState, TextClipboardRequest, TextCutTarget, TextPasteEdit, TextPasteTarget,
    ToastCommand, ToastPress,
};
pub(crate) use base::{
    ClipboardFingerprint, ClipboardPasteRequest, HelperLaunchRequest, PasteAnchor,
    PendingBackendAction, PendingOnboardingUsage, PendingSelectionClipboardPublish,
    PendingToolbarPersistence, WayscriberClipboardSelection,
};
pub(crate) use base::{InputEffect, InputEffectDrain};
pub(crate) use base::{KeybindingEditOperation, KeybindingEditRequest};
pub use board_picker::{BoardPickerCursorHint, BoardPickerLayout, BoardPickerPanel};
pub(crate) use captured_image::BoardPasteTarget;
pub(crate) use color_picker_popup::HexPasteTarget;
pub use color_picker_popup::PickerDrag;
pub use color_picker_popup::{
    ColorPickerCursorHint, ColorPickerPopupLayout, ColorPickerPopupPanel, ColorPickerPopupState,
    POPUP_HEIGHT as COLOR_PICKER_POPUP_HEIGHT, POPUP_WIDTH as COLOR_PICKER_POPUP_WIDTH,
    PREVIEW_SIZE as COLOR_PICKER_PREVIEW_SIZE,
    RECENT_SWATCH_COUNT as COLOR_PICKER_RECENT_SWATCH_COUNT,
    RECENT_SWATCH_SIZE as COLOR_PICKER_RECENT_SWATCH_SIZE, rgb_to_hsv as color_picker_rgb_to_hsv,
};
pub(crate) use command_palette::{
    COMMAND_PALETTE_INPUT_HEIGHT, COMMAND_PALETTE_ITEM_HEIGHT, COMMAND_PALETTE_LIST_GAP,
    COMMAND_PALETTE_PADDING, COMMAND_PALETTE_QUERY_PLACEHOLDER, COMMAND_PALETTE_ROW_ACTION_COUNT,
    COMMAND_PALETTE_ROW_ACTION_GAP, COMMAND_PALETTE_ROW_ACTION_SIZE, COMMAND_PALETTE_ROW_ICON_GAP,
    COMMAND_PALETTE_ROW_ICON_SIZE, COMMAND_PALETTE_TOP_RATIO, action_meta_token_score,
    query_tokens,
};
pub use command_palette::{
    COMMAND_PALETTE_MAX_VISIBLE, CommandPaletteCursorHint, CommandPaletteListRow,
    CommandPaletteState,
};
pub use eyedropper::{EyedropperCaptureSource, EyedropperUiState};
#[allow(unused_imports)]
pub use font_picker::{
    FontPickerFilter, FontPickerLayout, FontPickerResults, FontPickerRow, FontPickerTarget,
    font_picker_layout, font_picker_rows,
};
pub use help_overlay::HelpOverlayState;
pub use ime::ImePreedit;
#[cfg(test)]
pub(crate) use ime::build_text_input_preview;
pub use menus::{
    ContextMenuCursorHint, ContextMenuEntry, ContextMenuKind, ContextMenuState, MenuCommand,
};
pub use properties::{SelectionPropertyEntry, SelectionPropertyKind};
pub use radial_menu::{
    COMPASS_SLICES as RADIAL_COMPASS_SLICES, CompassDir, RADIAL_PAINT_DELAY, RadialMenuLayout,
    RadialMenuPanel, RadialMenuState, RadialParent, RadialRingSwatch, RadialSegmentId, RadialSlice,
    RadialSliceKind, SIZE_RING_ARC_SPAN, SIZE_RING_ARC_START,
    TOOL_SEGMENT_COUNT as RADIAL_TOOL_SEGMENT_COUNT, compass_slice, size_ring_angle_for_value,
    size_ring_value_for_angle, slice_parent, sub_ring_child_count, sub_ring_children,
};
pub use region_select::{
    RegionInputSource, RegionPurposeTag, RegionSelectUiState, RegionSelection, ScreenCaptureSource,
    SelectionPolicy,
};
pub(crate) use search::fuzzy_score;
pub(crate) use selection::LocalSelectionContext;
pub(crate) use style::DrawingStyle;
pub(crate) use text_editing::TextEditing;
pub use tool_controls::PrecisionEntryState;
pub use tour::{TourState, TourStep};
pub(crate) use utility::HelpOverlayPressSource;
pub(crate) use utility::SequenceMatch;
pub(crate) use utility::default_step_marker_size;
pub use utility::{HelpOverlayClick, HelpOverlayCursorHint, HelpOverlayReleaseOutcome};
