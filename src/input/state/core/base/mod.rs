mod input_effect_outbox;
mod state;
mod toast_queue;
mod types;
mod ui_visibility;

pub(crate) use input_effect_outbox::{InputEffect, InputEffectDrain};
pub(in crate::input::state::core) use input_effect_outbox::{InputEffectKind, InputEffectOutbox};
pub use state::InputState;
pub(in crate::input::state) use state::InputStateSeed;
pub(crate) use state::{FocusModeRestore, LightModeRestore, PresenterRestore};
pub use toast_queue::{Toast, ToastPriority, ToastPushOutcome, ToastQueue};
pub use types::{
    BLOCKED_ACTION_DURATION_MS, BOARD_DELETE_CONFIRM_MS, BOARD_UNDO_EXPIRE_MS,
    CompositorCapabilities, DesktopEnvironment, DrawingState, MAX_STROKE_THICKNESS,
    MIN_STROKE_THICKNESS, OutputFocusAction, PAGE_DELETE_CONFIRM_MS, PAGE_UNDO_EXPIRE_MS,
    PRESET_FEEDBACK_DURATION_MS, PRESET_TOAST_DURATION_MS, PresetAction, PresetFeedbackKind,
    PressureThicknessEditMode, PressureThicknessEntryMode, QuickColorEdit, SelectionAxis,
    SelectionHandle, ShellMode, TextInputMode, UI_TOAST_DURATION_MS, UiToastKind, ZoomAction,
};
pub(crate) use types::{
    BlockedActionFeedback, BoardPickerClickState, ClipboardFingerprint, ClipboardPasteRequest,
    HelperLaunchRequest, PasteAnchor, PendingBackendAction, PendingBoardDelete,
    PendingOnboardingUsage, PendingPageDelete, PendingSelectionClipboardPublish,
    PendingToolbarPersistence, PresetFeedbackState, TextClipboardRequest, TextCutTarget,
    TextPasteEdit, TextPasteTarget, ToastCommand, ToastPress, WayscriberClipboardSelection,
};
pub(crate) use types::{KeybindingEditOperation, KeybindingEditRequest};
pub use ui_visibility::UiVisibility;
