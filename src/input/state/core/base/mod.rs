mod input_effect_outbox;
mod state;
mod toast_queue;
mod types;

pub(crate) use input_effect_outbox::{InputEffect, InputEffectDrain};
pub(in crate::input::state::core) use input_effect_outbox::{InputEffectKind, InputEffectOutbox};
pub use state::InputState;
pub(crate) use state::{FocusModeRestore, LightModeRestore, PresenterRestore};
pub use toast_queue::{Toast, ToastPriority, ToastPushOutcome, ToastQueue};
pub use types::{
    BLOCKED_ACTION_DURATION_MS, BOARD_DELETE_CONFIRM_MS, BOARD_UNDO_EXPIRE_MS,
    CompositorCapabilities, DesktopEnvironment, DrawingState, MAX_STROKE_THICKNESS,
    MIN_STROKE_THICKNESS, OutputFocusAction, PAGE_DELETE_CONFIRM_MS, PAGE_UNDO_EXPIRE_MS,
    PRESET_FEEDBACK_DURATION_MS, PRESET_TOAST_DURATION_MS, PresetAction, PresetFeedbackKind,
    PressureThicknessEditMode, PressureThicknessEntryMode, QuickColorEdit, SelectionAxis,
    SelectionHandle, ShellMode, TEXT_EDIT_ENTRY_DURATION_MS, TextInputMode, UI_TOAST_DURATION_MS,
    UiToastKind, ZoomAction,
};
pub(crate) use types::{
    BlockedActionFeedback, BoardPickerClickState, ClipboardFingerprint, ClipboardPasteRequest,
    DelayedHistory, HistoryMode, PasteAnchor, PendingBackendAction, PendingBoardDelete,
    PendingClipboardFallback, PendingOnboardingUsage, PendingPageDelete,
    PendingSelectionClipboardPublish, PendingToolbarPersistence, PolygonClickState,
    PresetFeedbackState, SelectionPublishState, TextBlockDrag, TextClickState,
    TextClipboardRequest, TextCutTarget, TextEditEntryFeedback, TextPasteEdit, TextPasteTarget,
    ToastCommand, ToastPress, WayscriberClipboardSelection,
};
pub(crate) use types::{KeybindingEditOperation, KeybindingEditRequest};
