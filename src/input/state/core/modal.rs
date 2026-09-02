//! One registry for the popup surfaces that exclude each other.
//!
//! Every popup used to hand-write the list of other surfaces its opener
//! closes, and the lists disagreed: the radial menu closed all of them, help
//! closed only the radial menu, and a surface opened from a pointer path (a
//! toolbar or status-chip click bypasses the keyboard router's precedence
//! chain) could land on top of another one and starve it. The registry owns
//! the rule instead: opening a surface closes every other open surface unless
//! this module says the pair deliberately coexists.

use super::DrawingState;
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
    FontPicker,
    ContextMenu,
    BoardPicker,
    PropertiesPanel,
}

impl ModalSurface {
    pub(crate) const ALL: [ModalSurface; 10] = [
        ModalSurface::Tour,
        ModalSurface::CommandPalette,
        ModalSurface::HelpOverlay,
        ModalSurface::RadialMenu,
        ModalSurface::PrecisionEntry,
        ModalSurface::ColorPicker,
        ModalSurface::FontPicker,
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
            ModalSurface::CommandPalette
                | ModalSurface::ColorPicker
                | ModalSurface::FontPicker
                | ModalSurface::PrecisionEntry
        )
    }

    /// Whether a wheel tick belongs to this surface rather than the canvas.
    ///
    /// The axis handler ends in a fall-through that adjusts stroke thickness
    /// (or text size with Shift). Every surface that covers the canvas has to
    /// stop the wheel before it gets there, or scrolling over a modal silently
    /// edits the tool behind it. That was a per-surface `if` in the handler and
    /// three surfaces had been forgotten, so the rule lives here: a surface
    /// that covers the canvas owns the wheel, whether or not it has anything to
    /// scroll.
    ///
    /// The properties panel is deliberately out. It docks beside the canvas
    /// rather than over it, and the canvas stays drawable underneath — so the
    /// wheel still means what it means everywhere else.
    fn owns_wheel(self) -> bool {
        !matches!(self, ModalSurface::PropertiesPanel)
    }
}

impl InputState {
    /// Whether the surface is open right now.
    pub(crate) fn modal_is_open(&self, surface: ModalSurface) -> bool {
        match surface {
            ModalSurface::Tour => self.tour_active,
            ModalSurface::CommandPalette => self.command_palette.open,
            ModalSurface::HelpOverlay => self.show_help,
            ModalSurface::RadialMenu => self.is_radial_menu_open(),
            ModalSurface::PrecisionEntry => self.is_precision_entry_open(),
            ModalSurface::ColorPicker => self.is_color_picker_popup_open(),
            ModalSurface::FontPicker => self.is_font_picker_open(),
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

    /// True when a popup, in-progress text editor, or pending screen-region
    /// modal should receive pointer input before auxiliary-button shortcuts.
    pub(crate) fn modal_owns_pointer_shortcuts(&self) -> bool {
        self.engaged_modal().is_some()
            || matches!(self.state, DrawingState::TextInput { .. })
            || self.screen_modal_is_engaged()
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
                self.command_palette.open = false;
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
            ModalSurface::FontPicker => self.close_font_picker(),
            ModalSurface::ContextMenu => self.close_context_menu(),
            ModalSurface::BoardPicker => self.close_board_picker(),
            ModalSurface::PropertiesPanel => self.close_properties_panel(),
        }
    }

    /// The exclusion step every opener runs first: closes each open surface
    /// the opening one does not deliberately coexist with.
    pub(crate) fn close_modals_for_open(&mut self, opening: ModalSurface) {
        self.clear_pending_sequence();
        for other in ModalSurface::ALL {
            if other != opening && !opening.keeps_open(other) && self.modal_is_open(other) {
                self.close_modal(other);
            }
        }
    }

    /// Close everything a screen-region modal must not compete with, and
    /// cancel any unfinished gesture. Shared by the eyedropper and OCR: both
    /// take over pointer input entirely while they are up.
    ///
    /// Every registered surface, rather than a list kept by hand. The hand list
    /// had drifted: the font picker and the precise-entry popup were both
    /// missing, so a selector opened over one of them hid it and left it to
    /// reappear when the selector closed. Going through the registry also means
    /// each surface is dismissed by its own closer — the tour used to be a bare
    /// flag clear here, which left the toolbar chrome it hides still hidden.
    pub(crate) fn prepare_for_screen_modal(&mut self) {
        self.cancel_active_interaction();
        for surface in ModalSurface::ALL {
            if self.modal_is_open(surface) {
                self.close_modal(surface);
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

    /// Whether an open surface claims the wheel, so an axis frame must not
    /// fall through to the canvas tool behind it.
    ///
    /// Surfaces with something to scroll handle their own frames before this is
    /// consulted; this is what swallows the rest.
    pub fn modal_owns_wheel(&self) -> bool {
        // The eyedropper and the region selectors are not registry surfaces but
        // cover the screen just as completely. `is_active` rather than
        // `is_engaged`, matching the press/motion/release boundary: while a
        // capture is still pending nothing is drawn over the canvas and the
        // pointer still belongs to it.
        self.screen_modal_is_active()
            || ModalSurface::ALL
                .into_iter()
                .any(|surface| surface.owns_wheel() && self.modal_is_open(surface))
    }

    /// Whether either screen-region modal — the eyedropper or the generalized
    /// OCR/capture/measure region selector — has been asked for, including
    /// while a capture-backed purpose still waits on its screen image.
    ///
    /// This is the *keyboard* boundary: the key that requested the modal is the
    /// last one the canvas sees, and every press from then on is swallowed, so
    /// key repeat, IME ownership, and stylus barrel actions all stop here.
    ///
    /// It is deliberately not the pen boundary — see
    /// [`Self::screen_modal_is_active`].
    pub(crate) fn screen_modal_is_engaged(&self) -> bool {
        self.eyedropper_is_engaged() || self.region_is_engaged()
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
        self.eyedropper_is_active() || self.region_is_active()
    }
}

#[cfg(test)]
mod wheel_tests {
    use super::ModalSurface;
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn a_surface_covering_the_canvas_claims_the_wheel() {
        // Without this the axis handler falls through to the tool behind the
        // panel, and scrolling over a modal quietly changes the pen.
        let mut state = make_test_input_state();
        assert!(!state.modal_owns_wheel(), "nothing is open");

        state.open_font_picker();
        assert!(state.modal_owns_wheel(), "font picker");
        state.close_font_picker();

        state.open_color_picker_popup();
        assert!(state.modal_owns_wheel(), "colour picker");
        state.close_color_picker_popup(false);

        state.open_precision_entry(crate::ui::toolbar::PrecisionEntryTarget::Thickness);
        assert!(state.modal_owns_wheel(), "precision entry");
    }

    #[test]
    fn a_screen_selector_claims_the_wheel_the_way_it_claims_the_pointer() {
        // The eyedropper and the region selectors are not registry surfaces but
        // cover the screen just as completely. Press, motion, and release all
        // stop at them; the wheel used to carry on to zoom, Spotlight, and
        // stroke thickness behind them.
        let mut state = make_test_input_state();
        state.activate_eyedropper(None);

        assert!(state.eyedropper_is_active());
        assert!(state.modal_owns_wheel());
    }

    #[test]
    fn a_screen_selector_closes_every_registered_surface_it_covers() {
        // Not a list kept by hand: one that drifts leaves a surface hidden
        // under the selector, to reappear when it closes.
        let mut state = make_test_input_state();
        state.open_font_picker();
        assert!(state.is_font_picker_open());

        state.prepare_for_screen_modal();

        assert!(!state.is_font_picker_open());
        assert!(
            ModalSurface::ALL
                .into_iter()
                .all(|surface| !state.modal_is_open(surface)),
            "a screen selector leaves nothing open behind it"
        );
    }

    #[test]
    fn the_properties_panel_leaves_the_wheel_to_the_canvas() {
        // It docks beside the canvas rather than over it, and the canvas stays
        // drawable underneath, so the wheel still means what it means elsewhere.
        let mut state = make_test_input_state();
        let id = state
            .boards
            .active_frame_mut()
            .add_shape(crate::draw::Shape::Rect {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
                fill: false,
                color: crate::draw::Color::new(1.0, 1.0, 1.0, 1.0),
                thick: 2.0,
            });
        state.set_selection(vec![id]);
        assert!(state.show_properties_panel());

        assert!(state.is_properties_panel_open());
        assert!(!state.modal_owns_wheel());
    }
}
