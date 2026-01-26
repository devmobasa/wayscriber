//! Guided tour system for onboarding new users.

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
    HelpOverlay,
    Presets,
    Complete,
}

impl TourStep {
    /// Total number of tour steps.
    pub const COUNT: usize = 8;

    /// Get step from index.
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Welcome),
            1 => Some(Self::DrawingBasics),
            2 => Some(Self::ToolbarIntro),
            3 => Some(Self::CommandPalette),
            4 => Some(Self::ContextMenu),
            5 => Some(Self::HelpOverlay),
            6 => Some(Self::Presets),
            7 => Some(Self::Complete),
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
            Self::HelpOverlay => "Help & Shortcuts",
            Self::Presets => "Quick Presets",
            Self::Complete => "Tour Complete",
        }
    }

    /// Get step description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Welcome => {
                "Wayscriber is a screen annotation tool.\nDraw anywhere on your screen to highlight, explain, or present."
            }
            Self::DrawingBasics => {
                "Click and drag to draw with the pen tool.\nUse R/G/B/Y keys to change colors.\nScroll wheel or +/- to adjust thickness."
            }
            Self::ToolbarIntro => {
                "Press F2 to toggle the toolbar.\nThe toolbar provides quick access to all tools and settings."
            }
            Self::CommandPalette => {
                "Press Ctrl+K to open the command palette.\nQuickly search and run any action by typing.\nAccess all features without memorizing shortcuts."
            }
            Self::ContextMenu => {
                "Right-click anywhere for quick actions.\nAccess boards, pages, and common commands.\nShape-specific options when clicking on shapes."
            }
            Self::HelpOverlay => {
                "Press F1 to see all keyboard shortcuts.\nType to search for specific commands."
            }
            Self::Presets => {
                "Keys 1-5 apply saved tool presets.\nShift+1-5 saves current tool settings.\nCtrl+1-5 clears a preset slot."
            }
            Self::Complete => {
                "You're ready to annotate!\nPress F1 anytime to review shortcuts.\nEnjoy using Wayscriber!"
            }
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
    /// Start the guided tour.
    pub fn start_tour(&mut self) {
        self.tour_active = true;
        self.tour_step = 0;
        // Close other overlays
        if self.show_help {
            self.show_help = false;
        }
        if self.command_palette_open {
            self.command_palette_open = false;
        }
        self.close_context_menu();
        self.close_properties_panel();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// End the tour (skip or complete).
    pub fn end_tour(&mut self) {
        self.tour_active = false;
        if !self.presenter_mode || !self.presenter_mode_config.hide_toolbars {
            let top_visible = self.toolbar_top_pinned;
            let side_visible = self.toolbar_side_pinned;
            if !self.toolbar_visible() && (top_visible || side_visible) {
                self.toolbar_top_visible = top_visible;
                self.toolbar_side_visible = side_visible;
                self.toolbar_visible = top_visible || side_visible;
            }
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Advance to the next tour step.
    pub fn tour_next(&mut self) {
        if self.tour_step + 1 < TourStep::COUNT {
            self.tour_step += 1;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        } else {
            self.end_tour();
        }
    }

    /// Go back to the previous tour step.
    pub fn tour_prev(&mut self) {
        if self.tour_step > 0 {
            self.tour_step -= 1;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Get the current tour step.
    pub fn current_tour_step(&self) -> Option<TourStep> {
        if self.tour_active {
            TourStep::from_index(self.tour_step)
        } else {
            None
        }
    }

    /// Handle a key press while the tour is active.
    /// Returns true if the key was handled.
    pub(crate) fn handle_tour_key(&mut self, key: Key) -> bool {
        if !self.tour_active {
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
