//! Pins the modal registry's exclusion matrix.
//!
//! Before the registry, each opener hand-wrote the surfaces it closes and the
//! lists disagreed — a surface opened from a pointer path could land on top
//! of another and starve it in the key-routing chain. These tests pin the
//! rule (open closes everything else) and its one deliberate exception.

use super::helpers::create_test_input_state;
use crate::input::state::core::modal::ModalSurface;

/// The bug that motivated the registry: help opened from a toolbar or
/// status-chip click bypassed the keyboard router and left the color picker
/// popup open underneath, where help silently starved it of keys.
#[test]
fn opening_help_closes_the_color_picker_popup() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    assert!(state.is_color_picker_popup_open());

    state.toggle_help_overlay();

    assert!(state.show_help);
    assert!(!state.is_color_picker_popup_open());
}

#[test]
fn opening_help_closes_the_board_picker() {
    let mut state = create_test_input_state();
    state.open_board_picker();
    assert!(state.is_board_picker_open());

    state.toggle_help_overlay();

    assert!(state.show_help);
    assert!(!state.is_board_picker_open());
}

/// The board picker's page rows have their own context menus; opening one
/// must not dismiss the picker underneath it.
#[test]
fn a_context_menu_keeps_the_board_picker_open() {
    let mut state = create_test_input_state();
    state.open_board_picker();
    assert!(state.is_board_picker_open());

    state.open_page_context_menu((10, 10), 0, 0);

    assert!(state.is_context_menu_open());
    assert!(
        state.is_board_picker_open(),
        "page context menus operate on the picker's rows"
    );
}

/// The tour consumes every key and covers the overlay, so a surface opened
/// underneath it would get no input at all. A toolbar click during the tour
/// reaches these openers, so every one of them has to end the tour.
#[test]
fn every_opener_ends_the_tour() {
    for (name, open) in [
        (
            "help",
            (|state: &mut crate::input::InputState| state.toggle_help_overlay())
                as fn(&mut crate::input::InputState),
        ),
        ("board picker", |state| state.open_board_picker()),
        ("palette", |state| state.toggle_command_palette()),
        ("color picker", |state| state.open_color_picker_popup()),
        ("radial", |state| state.toggle_radial_menu(100.0, 100.0)),
    ] {
        let mut state = create_test_input_state();
        state.start_tour();
        assert!(state.tour_active);

        open(&mut state);

        assert!(!state.tour_active, "opening the {name} must end the tour");
    }
}

/// The tour hides pinned toolbar chrome and `end_tour` is what restores it,
/// so an opener that ends the tour must route through it rather than clearing
/// the flag — the palette's old shortcut left the toolbars hidden.
#[test]
fn an_opener_that_ends_the_tour_restores_pinned_chrome() {
    let mut state = create_test_input_state();
    state.toolbar_top_pinned = true;
    state.toolbar_side_pinned = true;
    state.start_tour();
    state.toolbar_top_visible = false;
    state.toolbar_side_visible = false;
    state.toolbar_visible = false;

    state.toggle_command_palette();

    assert!(!state.tour_active);
    assert!(
        state.toolbar_visible,
        "ending the tour must restore pinned toolbar chrome"
    );
}

/// The registry invariant: after opening any surface, no other surface it
/// excludes is still open. Exercised pairwise over every surface a bare
/// fixture can open.
#[test]
fn openers_leave_no_excluded_surface_behind() {
    type Opener = (
        &'static str,
        ModalSurface,
        fn(&mut crate::input::InputState),
    );

    let openers: &[Opener] = &[
        ("palette", ModalSurface::CommandPalette, |state| {
            state.toggle_command_palette()
        }),
        ("help", ModalSurface::HelpOverlay, |state| {
            state.toggle_help_overlay()
        }),
        ("radial", ModalSurface::RadialMenu, |state| {
            state.toggle_radial_menu(100.0, 100.0)
        }),
        ("color picker", ModalSurface::ColorPicker, |state| {
            state.open_color_picker_popup()
        }),
        ("context menu", ModalSurface::ContextMenu, |state| {
            state.open_page_context_menu((10, 10), 0, 0)
        }),
        ("board picker", ModalSurface::BoardPicker, |state| {
            state.open_board_picker()
        }),
    ];

    for (first_name, first_surface, open_first) in openers {
        for (second_name, second_surface, open_second) in openers {
            if first_surface == second_surface {
                continue;
            }
            let mut state = create_test_input_state();
            open_first(&mut state);
            assert!(
                state.modal_is_open(*first_surface),
                "fixture could not open {first_name}"
            );
            open_second(&mut state);
            assert!(
                state.modal_is_open(*second_surface),
                "opening {second_name} over {first_name} failed"
            );

            let context_over_picker = *second_surface == ModalSurface::ContextMenu
                && *first_surface == ModalSurface::BoardPicker;
            assert_eq!(
                state.modal_is_open(*first_surface),
                context_over_picker,
                "opening {second_name} over {first_name}: expected the pair to be {}",
                if context_over_picker {
                    "coexisting"
                } else {
                    "exclusive"
                }
            );
        }
    }
}
