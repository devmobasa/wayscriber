use super::{
    ClipboardPasteRequest, KeybindingEditRequest, OutputFocusAction, PendingBackendAction,
    PendingSelectionClipboardPublish, PendingToolbarPersistence, PresetAction, QuickColorEdit,
    TextClipboardRequest, TextPasteTarget, ZoomAction,
};
use crate::draw::Color;
use crate::input::boards::PendingBoardRuntimeUiAction;
use crate::input::state::HexPasteTarget;
use std::collections::VecDeque;

/// Backend-owned work emitted by [`super::InputState`].
///
/// Storage policy belongs to [`InputEffectOutbox`], not to producers or
/// consumers: queue-like effects stay FIFO, one-slot effects are last-wins,
/// and signal-like effects coalesce until drained.
#[derive(Debug, Clone)]
pub(crate) enum InputEffect {
    Backend(PendingBackendAction),
    SpotlightMagnifierFeedback,
    ToolbarPersistence(PendingToolbarPersistence),
    KeybindingEdit(KeybindingEditRequest),
    OutputFocus(OutputFocusAction),
    Zoom(ZoomAction),
    CopyHex(Color),
    PasteHex(HexPasteTarget),
    TextCopy(TextClipboardRequest),
    TextPaste(TextPasteTarget),
    SelectionClipboardPublish(PendingSelectionClipboardPublish),
    ClipboardPaste(ClipboardPasteRequest),
    /// Unconditional runtime acquisition phase. A true value carries a
    /// coalesced user freeze toggle into that phase.
    FrozenPass {
        user_requested: bool,
    },
    EyedropperToggle,
    /// Unconditional runtime OCR phase. Queued request and toolbar-dismissal
    /// signals merge so a same-batch toolbar toggle is consumed together.
    OcrPass {
        requested: bool,
        dismissed_by_toolbar: bool,
    },
    Preset(PresetAction),
    QuickColor(QuickColorEdit),
    BoardRuntimeUi(PendingBoardRuntimeUiAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputEffectDrain {
    /// Follow-ups handled synchronously after a keyboard/action update.
    Immediate,
    /// Follow-ups produced by a toolbar event.
    Toolbar,
    /// The central event-loop pass.
    Runtime,
    /// Config edits that must still be submitted during shutdown.
    DurableConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::input::state::core) enum InputEffectKind {
    Backend,
    SpotlightMagnifierFeedback,
    ToolbarPersistence,
    KeybindingEdit,
    OutputFocus,
    Zoom,
    CopyHex,
    PasteHex,
    TextCopy,
    TextPaste,
    SelectionClipboardPublish,
    ClipboardPaste,
    FrozenToggle,
    EyedropperToggle,
    OcrPass,
    Preset,
    QuickColor,
    BoardRuntimeUi,
}

#[derive(Debug, Default)]
pub(in crate::input::state::core) struct InputEffectOutbox {
    effects: VecDeque<InputEffect>,
}

impl InputEffectOutbox {
    pub(in crate::input::state::core) fn emit(&mut self, effect: InputEffect) {
        if let InputEffect::Backend(incoming) = &effect
            && self
                .effects
                .iter()
                .rev()
                .find_map(|queued| match queued {
                    InputEffect::Backend(action) => Some(action),
                    _ => None,
                })
                .is_some_and(|previous| previous == incoming)
        {
            return;
        }

        match policy(&effect) {
            EffectPolicy::Fifo => self.effects.push_back(effect),
            EffectPolicy::Coalesce => {
                if !self.effects.iter().any(|queued| same_slot(queued, &effect)) {
                    self.effects.push_back(effect);
                }
            }
            EffectPolicy::Merge => {
                if let Some(InputEffect::OcrPass {
                    requested: queued_request,
                    dismissed_by_toolbar: queued_dismissal,
                }) = self
                    .effects
                    .iter_mut()
                    .find(|queued| same_slot(queued, &effect))
                    && let InputEffect::OcrPass {
                        requested,
                        dismissed_by_toolbar,
                    } = effect
                {
                    *queued_request |= requested;
                    *queued_dismissal |= dismissed_by_toolbar;
                } else {
                    self.effects.push_back(effect);
                }
            }
            EffectPolicy::LastWins => {
                if let Some(queued) = self
                    .effects
                    .iter_mut()
                    .find(|queued| same_slot(queued, &effect))
                {
                    *queued = effect;
                } else {
                    self.effects.push_back(effect);
                }
            }
        }
    }

    pub(in crate::input::state::core) fn drain(
        &mut self,
        drain: InputEffectDrain,
    ) -> Vec<InputEffect> {
        let mut drained = Vec::new();
        match drain {
            InputEffectDrain::Immediate => {
                self.drain_kinds(
                    &[
                        InputEffectKind::Zoom,
                        InputEffectKind::Preset,
                        InputEffectKind::QuickColor,
                        InputEffectKind::CopyHex,
                        InputEffectKind::PasteHex,
                        InputEffectKind::TextCopy,
                        InputEffectKind::TextPaste,
                    ],
                    &mut drained,
                );
            }
            InputEffectDrain::Toolbar => {
                self.drain_kinds(
                    &[
                        InputEffectKind::Preset,
                        InputEffectKind::QuickColor,
                        InputEffectKind::CopyHex,
                        InputEffectKind::PasteHex,
                    ],
                    &mut drained,
                );
            }
            InputEffectDrain::Runtime => {
                self.drain_kind(InputEffectKind::EyedropperToggle, &mut drained);
                let (requested, dismissed_by_toolbar) =
                    match self.drain_one(InputEffectKind::OcrPass) {
                        Some(InputEffect::OcrPass {
                            requested,
                            dismissed_by_toolbar,
                        }) => (requested, dismissed_by_toolbar),
                        Some(effect) => unreachable!("OCR drain returned {effect:?}"),
                        None => (false, false),
                    };
                drained.push(InputEffect::OcrPass {
                    requested,
                    dismissed_by_toolbar,
                });
                self.drain_kinds(
                    &[
                        InputEffectKind::CopyHex,
                        InputEffectKind::PasteHex,
                        InputEffectKind::QuickColor,
                        InputEffectKind::KeybindingEdit,
                    ],
                    &mut drained,
                );

                // A native region capture reserves the shared frozen-screen
                // acquisition before the user's freeze toggle is interpreted.
                // Non-region backend work retains its later runtime position.
                self.drain_matching(
                    |effect| {
                        matches!(
                            effect,
                            InputEffect::Backend(PendingBackendAction::Screenshot(action))
                                if action.is_region_capture()
                        )
                    },
                    &mut drained,
                );
                let user_requested = self.drain_one(InputEffectKind::FrozenToggle).is_some();
                drained.push(InputEffect::FrozenPass { user_requested });
                self.drain_kinds(
                    &[
                        InputEffectKind::BoardRuntimeUi,
                        InputEffectKind::SpotlightMagnifierFeedback,
                        InputEffectKind::Backend,
                        InputEffectKind::OutputFocus,
                        InputEffectKind::Zoom,
                    ],
                    &mut drained,
                );
            }
            InputEffectDrain::DurableConfig => {
                self.drain_kinds(
                    &[
                        InputEffectKind::Preset,
                        InputEffectKind::QuickColor,
                        InputEffectKind::KeybindingEdit,
                    ],
                    &mut drained,
                );
            }
        }
        drained
    }

    pub(in crate::input::state::core) fn drain_one(
        &mut self,
        kind: InputEffectKind,
    ) -> Option<InputEffect> {
        let index = self
            .effects
            .iter()
            .position(|effect| effect.kind() == kind)?;
        self.effects.remove(index)
    }

    pub(in crate::input::state::core) fn drain_all(
        &mut self,
        kind: InputEffectKind,
    ) -> Vec<InputEffect> {
        let mut drained = Vec::new();
        self.drain_kind(kind, &mut drained);
        drained
    }

    pub(in crate::input::state::core) fn contains(&self, kind: InputEffectKind) -> bool {
        self.effects.iter().any(|effect| effect.kind() == kind)
    }

    pub(in crate::input::state::core) fn retain_kind(
        &mut self,
        kind: InputEffectKind,
        mut keep: impl FnMut(&InputEffect) -> bool,
    ) {
        self.effects
            .retain(|effect| effect.kind() != kind || keep(effect));
    }

    fn drain_kinds(&mut self, kinds: &[InputEffectKind], drained: &mut Vec<InputEffect>) {
        for &kind in kinds {
            self.drain_kind(kind, drained);
        }
    }

    fn drain_kind(&mut self, kind: InputEffectKind, drained: &mut Vec<InputEffect>) {
        self.drain_matching(|effect| effect.kind() == kind, drained);
    }

    fn drain_matching(
        &mut self,
        mut matches: impl FnMut(&InputEffect) -> bool,
        drained: &mut Vec<InputEffect>,
    ) {
        let mut retained = VecDeque::with_capacity(self.effects.len());
        while let Some(effect) = self.effects.pop_front() {
            if matches(&effect) {
                drained.push(effect);
            } else {
                retained.push_back(effect);
            }
        }
        self.effects = retained;
    }
}

impl InputEffect {
    fn kind(&self) -> InputEffectKind {
        match self {
            Self::Backend(_) => InputEffectKind::Backend,
            Self::SpotlightMagnifierFeedback => InputEffectKind::SpotlightMagnifierFeedback,
            Self::ToolbarPersistence(_) => InputEffectKind::ToolbarPersistence,
            Self::KeybindingEdit(_) => InputEffectKind::KeybindingEdit,
            Self::OutputFocus(_) => InputEffectKind::OutputFocus,
            Self::Zoom(_) => InputEffectKind::Zoom,
            Self::CopyHex(_) => InputEffectKind::CopyHex,
            Self::PasteHex(_) => InputEffectKind::PasteHex,
            Self::TextCopy(_) => InputEffectKind::TextCopy,
            Self::TextPaste(_) => InputEffectKind::TextPaste,
            Self::SelectionClipboardPublish(_) => InputEffectKind::SelectionClipboardPublish,
            Self::ClipboardPaste(_) => InputEffectKind::ClipboardPaste,
            Self::FrozenPass { .. } => InputEffectKind::FrozenToggle,
            Self::EyedropperToggle => InputEffectKind::EyedropperToggle,
            Self::OcrPass { .. } => InputEffectKind::OcrPass,
            Self::Preset(_) => InputEffectKind::Preset,
            Self::QuickColor(_) => InputEffectKind::QuickColor,
            Self::BoardRuntimeUi(_) => InputEffectKind::BoardRuntimeUi,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectPolicy {
    Fifo,
    Coalesce,
    Merge,
    LastWins,
}

fn policy(effect: &InputEffect) -> EffectPolicy {
    match effect {
        InputEffect::ToolbarPersistence(_)
        | InputEffect::SpotlightMagnifierFeedback
        | InputEffect::FrozenPass { .. }
        | InputEffect::EyedropperToggle => EffectPolicy::Coalesce,
        InputEffect::OcrPass { .. } => EffectPolicy::Merge,
        InputEffect::Backend(_)
        | InputEffect::KeybindingEdit(_)
        | InputEffect::TextCopy(_)
        | InputEffect::TextPaste(_)
        | InputEffect::BoardRuntimeUi(_) => EffectPolicy::Fifo,
        InputEffect::OutputFocus(_)
        | InputEffect::Zoom(_)
        | InputEffect::CopyHex(_)
        | InputEffect::PasteHex(_)
        | InputEffect::SelectionClipboardPublish(_)
        | InputEffect::ClipboardPaste(_)
        | InputEffect::Preset(_)
        | InputEffect::QuickColor(_) => EffectPolicy::LastWins,
    }
}

fn same_slot(left: &InputEffect, right: &InputEffect) -> bool {
    match (left, right) {
        (InputEffect::ToolbarPersistence(left), InputEffect::ToolbarPersistence(right)) => {
            std::mem::discriminant(left) == std::mem::discriminant(right)
        }
        _ => left.kind() == right.kind(),
    }
}
