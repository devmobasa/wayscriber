//! One registry for the popup surfaces that exclude each other.
//!
//! Every popup used to hand-write the list of other surfaces its opener
//! closes, and the lists disagreed: the radial menu closed all of them, help
//! closed only the radial menu, and a surface opened from a pointer path (a
//! toolbar or status-chip click bypasses the keyboard router's precedence
//! chain) could land on top of another one and starve it. The registry owns
//! the rule instead: opening a surface closes every other open surface unless
//! this module says the pair deliberately coexists.

use crate::input::state::InputState;

/// Every popup surface that participates in modal mutual exclusion, in the
/// keyboard router's precedence order (earlier gets first refusal of a key).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalSurface {
    Tour,
    CommandPalette,
    HelpOverlay,
    RadialMenu,
    PrecisionEntry,
    ColorPicker,
    ContextMenu,
    BoardPicker,
    PropertiesPanel,
}

impl ModalSurface {
    pub(crate) const ALL: [ModalSurface; 9] = [
        ModalSurface::Tour,
        ModalSurface::CommandPalette,
        ModalSurface::HelpOverlay,
        ModalSurface::RadialMenu,
        ModalSurface::PrecisionEntry,
        ModalSurface::ColorPicker,
        ModalSurface::ContextMenu,
        ModalSurface::BoardPicker,
        ModalSurface::PropertiesPanel,
    ];

    /// Whether opening `self` leaves an open `other` in place. Exclusion is
    /// the default; every entry here is a deliberate pairing.
    ///
    /// The tour is deliberately *not* an exception: it consumes every key
    /// (`tour.rs` swallows the unmatched arm) and covers the overlay, so a
    /// surface opened underneath it — a toolbar click during the tour reaches
    /// the openers — would receive neither keyboard nor pointer input.
    fn keeps_open(self, other: ModalSurface) -> bool {
        match (self, other) {
            // The board picker's page rows have their own context menus, so a
            // context menu opening over the picker is part of using it. The
            // picker still closes any context menu when *it* opens.
            (ModalSurface::ContextMenu, ModalSurface::BoardPicker) => true,
            _ => false,
        }
    }

    /// Whether keys held while this surface is engaged must bypass the
    /// backend's canvas-oriented manual repeat timer. Narrower than IME
    /// ownership: help and board-picker search still use the normal routed
    /// repeat path even though they disable the canvas IME.
    fn blocks_canvas_key_repeat(self) -> bool {
        matches!(
            self,
            ModalSurface::CommandPalette | ModalSurface::ColorPicker | ModalSurface::PrecisionEntry
        )
    }
}

impl InputState {
    /// Whether the surface is open right now.
    pub(crate) fn modal_is_open(&self, surface: ModalSurface) -> bool {
        match surface {
            ModalSurface::Tour => self.tour_active,
            ModalSurface::CommandPalette => self.command_palette_open,
            ModalSurface::HelpOverlay => self.show_help,
            ModalSurface::RadialMenu => self.is_radial_menu_open(),
            ModalSurface::PrecisionEntry => self.is_precision_entry_open(),
            ModalSurface::ColorPicker => self.is_color_picker_popup_open(),
            ModalSurface::ContextMenu => self.is_context_menu_open(),
            ModalSurface::BoardPicker => self.is_board_picker_open(),
            ModalSurface::PropertiesPanel => self.is_properties_panel_open(),
        }
    }

    /// The open surface with key-routing precedence, if any.
    pub(crate) fn engaged_modal(&self) -> Option<ModalSurface> {
        ModalSurface::ALL
            .into_iter()
            .find(|surface| self.modal_is_open(*surface))
    }

    /// Closes the surface through its canonical closer, so caches, layouts,
    /// and pointer hit maps are dropped the same way a direct dismissal
    /// drops them.
    pub(crate) fn close_modal(&mut self, surface: ModalSurface) {
        match surface {
            // Through end_tour, not a bare flag clear: the tour hides pinned
            // toolbar chrome and end_tour is what restores it. The palette's
            // old shortcut cleared the flag directly and left the toolbars
            // hidden.
            ModalSurface::Tour => self.end_tour(),
            ModalSurface::CommandPalette => {
                self.command_palette_open = false;
                self.clear_command_palette_repeat();
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
            ModalSurface::HelpOverlay => self.close_help_overlay(),
            ModalSurface::RadialMenu => self.close_radial_menu(),
            ModalSurface::PrecisionEntry => {
                self.cancel_precision_entry();
            }
            ModalSurface::ColorPicker => self.close_color_picker_popup(true),
            ModalSurface::ContextMenu => self.close_context_menu(),
            ModalSurface::BoardPicker => self.close_board_picker(),
            ModalSurface::PropertiesPanel => self.close_properties_panel(),
        }
    }

    /// The exclusion step every opener runs first: closes each open surface
    /// the opening one does not deliberately coexist with.
    pub(crate) fn close_modals_for_open(&mut self, opening: ModalSurface) {
        for other in ModalSurface::ALL {
            if other != opening && !opening.keeps_open(other) && self.modal_is_open(other) {
                self.close_modal(other);
            }
        }
    }

    /// True when another interaction captures keyboard input ahead of the
    /// canvas editor. While one is active the canvas IME must stay disabled:
    /// composed text bypasses normal key routing and would otherwise leak
    /// straight into the hidden canvas buffer.
    pub fn modal_owns_text_input(&self) -> bool {
        self.engaged_modal().is_some()
            // The keybinding capture chord can be armed after the palette
            // closes, and the screen-region modals are not popup surfaces;
            // all of them still own the keyboard.
            || self.command_palette_is_engaged()
            || self.screen_modal_is_engaged()
    }

    /// Modal paths whose editing/repeat behavior must not be driven by the
    /// backend's canvas-oriented manual repeat timer.
    pub fn modal_blocks_canvas_key_repeat(&self) -> bool {
        self.command_palette_is_engaged()
            // A screen-region modal can open from the toolbar, the command
            // palette, or a mouse path while a canvas key is still held. It
            // swallows every key press it receives, so leaving the repeat timer
            // armed would keep feeding the canvas behind the selector.
            || self.screen_modal_is_engaged()
            || ModalSurface::ALL
                .into_iter()
                .any(|surface| surface.blocks_canvas_key_repeat() && self.modal_is_open(surface))
    }

    /// Whether either screen-region modal — the eyedropper or the OCR region
    /// selector — has been asked for, including while it still waits on a
    /// capture.
    ///
    /// This is the *keyboard* boundary: the key that requested the modal is the
    /// last one the canvas sees, and every press from then on is swallowed, so
    /// key repeat, IME ownership, and stylus barrel actions all stop here.
    ///
    /// It is deliberately not the pen boundary — see
    /// [`Self::screen_modal_is_active`].
    pub(crate) fn screen_modal_is_engaged(&self) -> bool {
        self.eyedropper_is_engaged() || self.ocr_is_engaged()
    }

    /// Whether a screen-region modal is on screen and owns pointer input.
    ///
    /// The *pen* boundary, and narrower than [`Self::screen_modal_is_engaged`]:
    /// while a modal is still waiting on its capture there is nothing drawn
    /// over the canvas, and every pointer, touch, and stylus path still routes
    /// to drawing — so a stroke started during that wait is a real stroke and
    /// keeps its pressure. Activation cancels it; a capture that fails or is
    /// cancelled first leaves it intact.
    pub(crate) fn screen_modal_is_active(&self) -> bool {
        self.eyedropper_is_active() || self.ocr_is_active()
    }
}
