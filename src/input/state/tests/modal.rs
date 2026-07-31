//! Pins the modal registry's exclusion matrix.
//!
//! Before the registry, each opener hand-wrote the surfaces it closes and the
//! lists disagreed — a surface opened from a pointer path could land on top
//! of another and starve it in the key-routing chain. These tests pin the
//! rule (open closes everything else) and its two deliberate exceptions.

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

/// The tour guides the user into opening other surfaces, so openers must not
/// end it — except the palette, which has always dismissed it.
#[test]
fn the_tour_survives_every_opener_except_the_palette() {
    let mut state = create_test_input_state();
    state.start_tour();
    assert!(state.tour_active);

    state.toggle_help_overlay();
    assert!(state.tour_active, "help must not end the tour");
    state.close_help_overlay();

    state.open_board_picker();
    assert!(state.tour_active, "the board picker must not end the tour");
    state.close_board_picker();

    state.toggle_command_palette();
    assert!(!state.tour_active, "the palette dismisses the tour");
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
