use super::WaylandState;
use crate::{
    config::Action,
    input::{
        InputState, Key,
        state::{InputEffect, InputEffectDrain},
    },
};

/// The input HUD's live state, before and after an input update.
///
/// Persistence is not decided here: an action that changes a durable chrome
/// preference queues its own entry, carrying the pre-change value, because
/// only the action knows whether the change was the user's own. A mode
/// transition moves the same fields without it being a choice, and a toggle
/// pressed while focus mode is active breaks focus in the same breath -- both
/// of which a before/after diff would read wrongly.
///
/// The reader thread is different: it follows the HUD's live state whatever
/// moved it, so that one stays a diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InputHudSnapshot {
    enabled: bool,
}

impl InputHudSnapshot {
    fn from_input_state(input_state: &InputState) -> Self {
        Self {
            enabled: input_state.input_hud_enabled(),
        }
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn apply_input_key(&mut self, key: Key) {
        self.apply_input_update(|input_state| input_state.on_key_press(key));
    }

    pub(in crate::backend::wayland) fn apply_input_key_repeat(&mut self, key: Key) {
        self.apply_input_update(|input_state| input_state.on_key_repeat(key));
    }

    pub(in crate::backend::wayland) fn dispatch_input_action(&mut self, action: Action) {
        self.apply_input_update(|input_state| input_state.handle_action(action));
    }

    fn apply_input_update(&mut self, update: impl FnOnce(&mut InputState)) {
        #[cfg(feature = "tablet-input")]
        let prev_thickness = self.input_state.style.current_thickness;
        let hud_before = InputHudSnapshot::from_input_state(&self.input_state);

        update(&mut self.input_state);
        self.input_state.needs_redraw = true;
        self.sync_overlay_interactivity();

        if hud_before != InputHudSnapshot::from_input_state(&self.input_state) {
            // A mode transition can flip the HUD without it being a
            // preference, but the reader thread must follow the live state
            // either way.
            self.sync_input_monitor();
        }

        #[cfg(feature = "tablet-input")]
        self.sync_stylus_thickness_after_input_update(prev_thickness);

        self.drain_input_action_followups();
    }

    fn drain_input_action_followups(&mut self) {
        for effect in self
            .input_state
            .drain_input_effects(InputEffectDrain::Immediate)
        {
            match effect {
                InputEffect::Zoom(action) => self.handle_zoom_action(action),
                InputEffect::Preset(action) => self.handle_preset_action(action),
                InputEffect::QuickColor(edit) => self.handle_quick_color_edit(edit),
                InputEffect::CopyHex(color) => self.handle_copy_hex_color(color),
                InputEffect::PasteHex(target) => self.handle_paste_hex_color(target),
                InputEffect::TextCopy(request) => self.handle_copy_text(request),
                InputEffect::TextPaste(target) => self.handle_paste_text(target),
                effect @ (InputEffect::Backend(_)
                | InputEffect::SpotlightMagnifierFeedback
                | InputEffect::ToolbarPersistence(_)
                | InputEffect::KeybindingEdit(_)
                | InputEffect::OutputFocus(_)
                | InputEffect::SelectionClipboardPublish(_)
                | InputEffect::ClipboardPaste(_)
                | InputEffect::FrozenPass { .. }
                | InputEffect::EyedropperToggle
                | InputEffect::OcrPass { .. }
                | InputEffect::BoardRuntimeUi(_)) => {
                    unreachable!("immediate drain returned {effect:?}")
                }
            }
        }
        self.drain_clipboard_requests();
    }

    #[cfg(feature = "tablet-input")]
    fn sync_stylus_thickness_after_input_update(&mut self, prev: f64) {
        if !self.sync_stylus_thickness_cache(prev) {
            return;
        }

        if self.tablet.tip_down {
            self.record_stylus_peak(self.input_state.style.current_thickness);
        } else {
            self.tablet.peak_thickness = None;
        }
    }
}
