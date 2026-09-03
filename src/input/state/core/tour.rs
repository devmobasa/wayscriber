//! Guided tour system for onboarding new users.

use crate::domain::Action;
use crate::input::events::Key;

use super::base::InputState;

/// Tour step definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TourStep {
    Welcome,
    DrawingBasics,
    ToolbarIntro,
    CommandPalette,
    ContextMenu,
    StatusBar,
    HelpOverlay,
    Presets,
    Complete,
}

/// Lifecycle and navigation state for the guided tour.
#[derive(Debug, Default)]
pub struct TourState {
    pub(in crate::input::state) active: bool,
    pub(in crate::input::state) step: usize,
}

impl TourState {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Zero-based index of the current step, regardless of whether the tour is active.
    pub fn step(&self) -> usize {
        self.step
    }

    pub(crate) fn start(&mut self) {
        self.active = true;
        self.step = 0;
    }

    pub(crate) fn end(&mut self) {
        self.active = false;
    }

    pub(crate) fn advance(&mut self) -> bool {
        if self.step + 1 >= TourStep::COUNT {
            return false;
        }
        self.step += 1;
        true
    }

    pub(crate) fn retreat(&mut self) -> bool {
        if self.step == 0 {
            return false;
        }
        self.step -= 1;
        true
    }

    pub fn current_step(&self) -> Option<TourStep> {
        if !self.active {
            return None;
        }
        TourStep::from_index(self.step)
    }
}

impl TourStep {
    /// Total number of tour steps.
    pub const COUNT: usize = 9;

    /// Get step from index.
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Welcome),
            1 => Some(Self::DrawingBasics),
            2 => Some(Self::ToolbarIntro),
            3 => Some(Self::CommandPalette),
            4 => Some(Self::ContextMenu),
            5 => Some(Self::StatusBar),
            6 => Some(Self::HelpOverlay),
            7 => Some(Self::Presets),
            8 => Some(Self::Complete),
            _ => None,
        }
    }

    /// Get step title.
    pub fn title(&self) -> &'static str {
        match self {
            Self::Welcome => "Welcome to Wayscriber",
            Self::DrawingBasics => "Drawing Basics",
            Self::ToolbarIntro => "Toolbar Access",
            Self::CommandPalette => "Command Palette",
            Self::ContextMenu => "Context Menu",
            Self::StatusBar => "Boards & Pages",
            Self::HelpOverlay => "Help & Shortcuts",
            Self::Presets => "Quick Presets",
            Self::Complete => "Tour Complete",
        }
    }

    /// Get navigation hint for the step.
    pub fn nav_hint(&self) -> &'static str {
        match self {
            Self::Complete => "Press Enter or Escape to finish",
            _ => "Space/Enter: Next  |  Backspace: Back  |  Escape: Skip",
        }
    }
}

impl InputState {
    /// Build a tour step's description dynamically for the current bindings.
    /// Every key mention is resolved through [`Self::shortcut_for_action`] (or
    /// the toolbar rebind-modifier helper), so no key string is ever hardcoded
    /// and the copy tracks the user's actual configuration.
    pub fn tour_step_description(&self, step: TourStep) -> String {
        match step {
            TourStep::Welcome => "Wayscriber is a screen annotation tool.\n\
                 Draw anywhere on your screen to highlight, explain, or present."
                .to_string(),
            TourStep::DrawingBasics => {
                let mut lines = vec!["Click and drag to draw with the pen tool.".to_string()];
                match self.tour_join_shortcuts(&[
                    Action::SetColorRed,
                    Action::SetColorGreen,
                    Action::SetColorBlue,
                    Action::SetColorYellow,
                ]) {
                    Some(colors) => lines.push(format!("Press {colors} to change colors.")),
                    None => lines.push("Use the color keys to change colors.".to_string()),
                }
                match self
                    .tour_join_shortcuts(&[Action::IncreaseThickness, Action::DecreaseThickness])
                {
                    Some(thick) => {
                        lines.push(format!("Scroll wheel or {thick} adjusts thickness."))
                    }
                    None => lines.push("The scroll wheel adjusts thickness.".to_string()),
                }
                match self.shortcut_for_action(Action::ToggleRadialMenu) {
                    Some(radial) => lines.push(format!(
                        "{radial} opens the radial menu for quick tool/color changes."
                    )),
                    None => {
                        lines.push("The radial menu offers quick tool/color changes.".to_string())
                    }
                }
                lines.join("\n")
            }
            TourStep::ToolbarIntro => {
                let mut lines = Vec::new();
                let toggle = self.shortcut_for_action(Action::ToggleToolbar);
                let cycle = self.shortcut_for_action(Action::CycleToolbarDisplay);
                match (toggle, cycle) {
                    (Some(toggle), Some(cycle)) => lines.push(format!(
                        "Press {toggle} to toggle the toolbar; {cycle} cycles it \
                         full \u{2192} micro \u{2192} hidden."
                    )),
                    (Some(toggle), None) => {
                        lines.push(format!("Press {toggle} to toggle the toolbar."))
                    }
                    (None, Some(cycle)) => lines.push(format!(
                        "Press {cycle} to cycle the toolbar full \u{2192} micro \u{2192} hidden."
                    )),
                    (None, None) => lines.push("Toggle the toolbar from its actions.".to_string()),
                }
                lines.push(
                    "The toolbar provides quick access to all tools and settings.".to_string(),
                );
                if let Some(click) = self.toolbar_rebind_modifier.click_label() {
                    lines.push(format!(
                        "By default, {click} a bindable control to change its shortcut."
                    ));
                }
                lines.join("\n")
            }
            TourStep::CommandPalette => {
                let mut lines = Vec::new();
                match self.shortcut_for_action(Action::ToggleCommandPalette) {
                    Some(key) => lines.push(format!("Press {key} to open the command palette.")),
                    None => lines.push("Open the command palette to run any action.".to_string()),
                }
                lines.push("Quickly search and run any action by typing.".to_string());
                lines.push("Use the row controls to edit, unbind, or reset shortcuts.".to_string());
                lines.push(
                    "Press Ctrl+Shift+E on a row to change its shortcut in the configurator."
                        .to_string(),
                );
                lines.join("\n")
            }
            TourStep::ContextMenu => "Right-click anywhere for quick actions.\n\
                 Access boards, pages, and common commands.\n\
                 Shape-specific options when clicking on shapes."
                .to_string(),
            TourStep::StatusBar => {
                let board = self.ui_visibility.show_status_board_badge && self.boards.show_badge();
                let page = self.ui_visibility.show_status_page_badge;
                let entry = match (board, page) {
                    (true, true) => Some("Board or Page"),
                    (true, false) => Some("Board"),
                    (false, true) => Some("Page"),
                    (false, false) => None,
                };
                let mut lines = match entry {
                    Some(entry) => vec![format!(
                        "Click the {entry} segment in the status bar to open the board picker."
                    )],
                    None => vec![
                        "Board/Page status-bar segments are hidden in your configuration."
                            .to_string(),
                    ],
                };
                match (entry, self.shortcut_for_action(Action::BoardPicker)) {
                    (Some(_), Some(key)) => lines.push(format!(
                        "Switch between boards and pages there, or press {key}."
                    )),
                    (Some(_), None) => {
                        lines.push("Switch between boards and pages there.".to_string())
                    }
                    (None, Some(key)) => {
                        lines.push(format!("Press {key} to open the board picker."))
                    }
                    (None, None) => {
                        lines.push("Open the board picker from an action menu.".to_string())
                    }
                }
                lines.join("\n")
            }
            TourStep::HelpOverlay => match self.shortcut_for_action(Action::ToggleHelp) {
                Some(key) => format!(
                    "Press {key} to see all keyboard shortcuts.\n\
                     Type to search for specific commands."
                ),
                None => "Open the help overlay to see all keyboard shortcuts.\n\
                     Type to search for specific commands."
                    .to_string(),
            },
            TourStep::Presets => {
                let mut lines = Vec::new();
                match self.tour_shortcut_range(Action::ApplyPreset1, Action::ApplyPreset5) {
                    Some(apply) => lines.push(format!("{apply} apply saved tool presets.")),
                    None => lines.push("Preset keys apply saved tool presets.".to_string()),
                }
                if let Some(save) =
                    self.tour_shortcut_range(Action::SavePreset1, Action::SavePreset5)
                {
                    lines.push(format!("{save} saves current tool settings."));
                }
                if let Some(clear) =
                    self.tour_shortcut_range(Action::ClearPreset1, Action::ClearPreset5)
                {
                    lines.push(format!("{clear} clears a preset slot."));
                }
                lines.join("\n")
            }
            TourStep::Complete => match self.shortcut_for_action(Action::ToggleHelp) {
                Some(key) => format!(
                    "You're ready to annotate!\n\
                     Press {key} anytime to review shortcuts.\n\
                     Enjoy using Wayscriber!"
                ),
                None => "You're ready to annotate!\nEnjoy using Wayscriber!".to_string(),
            },
        }
    }

    /// Resolve each action's shortcut and join the bound ones with `"/"`.
    /// `None` when none resolve (all unbound).
    fn tour_join_shortcuts(&self, actions: &[Action]) -> Option<String> {
        let labels: Vec<String> = actions
            .iter()
            .filter_map(|action| self.shortcut_for_action(*action))
            .collect();
        (!labels.is_empty()).then(|| labels.join("/"))
    }

    /// Render a preset range like `"1-5"` from the first and last slot's
    /// resolved shortcuts. Falls back to whichever end is bound; `None` when
    /// neither is.
    fn tour_shortcut_range(&self, first: Action, last: Action) -> Option<String> {
        match (
            self.shortcut_for_action(first),
            self.shortcut_for_action(last),
        ) {
            (Some(a), Some(b)) => Some(format!("{a}-{b}")),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    /// Start the guided tour.
    pub fn start_tour(&mut self) {
        if self.focus_mode_active() {
            // The tour restores pinned chrome when it ends, so it must begin
            // from Focus Mode's real baseline rather than nesting underneath
            // that transient snapshot owner.
            self.toggle_focus_mode();
        }
        self.close_modals_for_open(crate::input::state::core::modal::ModalSurface::Tour);
        self.tour.start();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Replay the guided tour from the help overlay's "Replay tour" footer.
    /// A dedicated seam (distinct from [`Self::start_tour`]) so replay always
    /// starts the overlay regardless of the persisted `tour_shown` flag — and
    /// so a future replay-specific behavior has a single call site to hang on.
    pub fn start_tour_replay(&mut self) {
        self.start_tour();
    }

    /// End the tour (skip or complete).
    pub fn end_tour(&mut self) {
        self.tour.end();
        if !self.presenter_mode || !self.presenter_mode_config.hide_toolbars {
            let top_visible = self.toolbar_top_pinned;
            if !self.toolbar_visible() && top_visible {
                self.toolbar_top_visible = true;
                self.toolbar_visible = true;
            }
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Advance to the next tour step.
    pub fn tour_next(&mut self) {
        if !self.tour.advance() {
            self.end_tour();
            return;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Go back to the previous tour step.
    pub fn tour_prev(&mut self) {
        if self.tour.retreat() {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Get the current tour step.
    pub fn current_tour_step(&self) -> Option<TourStep> {
        self.tour.current_step()
    }

    /// Handle a key press while the tour is active.
    /// Returns true if the key was handled.
    pub(crate) fn handle_tour_key(&mut self, key: Key) -> bool {
        if !self.tour.is_active() {
            return false;
        }

        match key {
            Key::Escape => {
                self.end_tour();
                true
            }
            Key::Return | Key::Space => {
                self.tour_next();
                true
            }
            Key::Backspace => {
                self.tour_prev();
                true
            }
            _ => true, // Consume all other keys while tour is active
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TourState, TourStep};

    #[test]
    fn tour_navigation_respects_both_ends() {
        let mut tour = TourState::default();
        tour.start();
        assert_eq!(tour.current_step(), Some(TourStep::Welcome));
        assert!(!tour.retreat());
        for _ in 1..TourStep::COUNT {
            assert!(tour.advance());
        }
        assert_eq!(tour.current_step(), Some(TourStep::Complete));
        assert!(!tour.advance());
        tour.end();
        assert_eq!(tour.current_step(), None);
    }
    use crate::config::ToolbarRebindModifier;
    use crate::config::keybindings::Action;
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn tour_introduces_both_shortcut_editing_paths() {
        let mut state = make_test_input_state();

        // The ToolbarIntro step routes the shortcut-rebind chord through the
        // modifier helper — never a hardcoded key string. Default is Ctrl+Shift.
        state.toolbar_rebind_modifier = ToolbarRebindModifier::CtrlShift;
        let toolbar = state.tour_step_description(TourStep::ToolbarIntro);
        assert!(
            toolbar.contains(
                ToolbarRebindModifier::CtrlShift
                    .click_label()
                    .expect("chord")
            ),
            "toolbar copy: {toolbar:?}"
        );

        // Changing the modifier changes the copy, proving it is generated, not
        // hardcoded.
        state.toolbar_rebind_modifier = ToolbarRebindModifier::CtrlAlt;
        let toolbar = state.tour_step_description(TourStep::ToolbarIntro);
        assert!(
            toolbar.contains("Ctrl+Alt+click"),
            "toolbar copy: {toolbar:?}"
        );
        assert!(!toolbar.contains("Ctrl+Shift+click"), "copy: {toolbar:?}");

        // The F9/F2 facts are resolved through shortcut_for_action, not literals
        // (F9 toggles the toolbar; F2 cycles its display — kept correct here).
        let toggle = state
            .shortcut_for_action(Action::ToggleToolbar)
            .expect("toolbar toggle bound");
        let cycle = state
            .shortcut_for_action(Action::CycleToolbarDisplay)
            .expect("toolbar cycle bound");
        assert!(toolbar.contains(&toggle), "toolbar copy: {toolbar:?}");
        assert!(toolbar.contains(&cycle), "toolbar copy: {toolbar:?}");

        // The command palette step teaches the row shortcut controls and names
        // the configurator route beside them.
        let palette = state.tour_step_description(TourStep::CommandPalette);
        assert!(palette.contains("edit"), "palette copy: {palette:?}");
        assert!(palette.contains("unbind"), "palette copy: {palette:?}");
        assert!(palette.contains("reset"), "palette copy: {palette:?}");
        assert!(
            palette.contains("Ctrl+Shift+E"),
            "palette copy: {palette:?}"
        );
        assert!(
            palette.contains("configurator"),
            "palette copy: {palette:?}"
        );
    }

    #[test]
    fn starting_tour_exits_focus_mode_before_tour_owns_chrome() {
        let mut state = make_test_input_state();
        state.handle_action(Action::ToggleFocusMode);
        assert!(state.focus_mode_active());
        assert!(!state.ui_visibility.show_status_bar);

        state.start_tour_replay();

        assert!(state.tour.active);
        assert!(
            !state.focus_mode_active(),
            "the tour and Focus Mode must not own chrome simultaneously"
        );
        state.end_tour();
        assert!(
            state.ui_visibility.show_status_bar,
            "pre-Focus chrome must be restored"
        );
        assert!(
            state.toolbar_visible(),
            "pinned toolbar must remain visible"
        );
    }

    #[test]
    fn tour_copy_tracks_rebound_shortcuts() {
        use crate::config::{KeybindingsConfig, Shortcut};

        let mut bindings = KeybindingsConfig::default()
            .build_action_bindings()
            .expect("default bindings");
        bindings.insert(
            Action::ToggleCommandPalette,
            vec![Shortcut::parse("Ctrl+Shift+P").expect("binding")],
        );
        let mut state = make_test_input_state();
        state.set_action_bindings(bindings);

        let palette = state.tour_step_description(TourStep::CommandPalette);
        assert!(
            palette.contains("Ctrl+Shift+P"),
            "palette copy did not follow the rebind: {palette:?}"
        );
    }

    #[test]
    fn status_bar_step_teaches_board_picker_and_tracks_binding() {
        use crate::config::{KeybindingsConfig, Shortcut};

        // The M9 board-picker beat always names the on-screen entry point (the
        // status bar) position-neutrally and resolves the board-picker key through
        // `shortcut_for_action` — never a hardcoded literal.
        let mut bindings = KeybindingsConfig::default()
            .build_action_bindings()
            .expect("default bindings");
        bindings.insert(
            Action::BoardPicker,
            vec![Shortcut::parse("Ctrl+Shift+B").expect("binding")],
        );
        let mut state = make_test_input_state();
        state.set_action_bindings(bindings);

        let copy = state.tour_step_description(TourStep::StatusBar);
        assert!(copy.contains("status bar"), "status bar copy: {copy:?}");
        assert!(
            copy.contains("Board or Page segment"),
            "tour must name the actual clickable segments: {copy:?}"
        );
        assert!(
            copy.contains("Ctrl+Shift+B"),
            "board-picker copy did not follow the rebind: {copy:?}"
        );

        // Unbinding the board picker drops the key mention without hardcoding.
        let mut bindings = KeybindingsConfig::default()
            .build_action_bindings()
            .expect("default bindings");
        bindings.insert(Action::BoardPicker, Vec::new());
        state.set_action_bindings(bindings);
        let copy = state.tour_step_description(TourStep::StatusBar);
        assert!(copy.contains("status bar"), "status bar copy: {copy:?}");
        assert!(
            !copy.contains("press"),
            "unbound board picker must not name a key: {copy:?}"
        );

        // If both configurable picker segments are hidden, never imply that
        // clicking an arbitrary part of the status bar opens the picker.
        state.ui_visibility.show_status_board_badge = false;
        state.ui_visibility.show_status_page_badge = false;
        let copy = state.tour_step_description(TourStep::StatusBar);
        assert!(copy.contains("segments are hidden"), "copy: {copy:?}");
        assert!(!copy.contains("Click the status bar"), "copy: {copy:?}");
    }
}
