use crate::config::KeybindingsConfig;
use crate::input::state::{
    HelpOverlayClick, HelpOverlayCursorHint, HelpOverlayPressSource, HelpOverlayReleaseOutcome,
    InputState,
};

fn make_state() -> InputState {
    let keybindings = KeybindingsConfig::default();
    let _action_map = keybindings
        .build_action_map()
        .expect("default keybindings map");

    crate::input::state::test_support::make_test_input_state()
}

#[test]
fn toggle_help_overlay_opens_and_tracks_usage() {
    let mut state = make_state();
    state.toggle_help_overlay();

    assert!(state.help_overlay.visible);
    assert!(!state.help_overlay.quick_mode);
    assert!(state.pending_onboarding_usage.used_help_overlay);
    assert_eq!(state.help_overlay.page, 0);
}

#[test]
fn opening_help_closes_radial_menu() {
    let mut state = make_state();
    state.open_radial_menu(320.0, 240.0);
    assert!(state.is_radial_menu_open());

    state.toggle_help_overlay();

    assert!(state.help_overlay.visible);
    assert!(!state.is_radial_menu_open());
}

#[test]
fn toggle_quick_help_closes_when_already_in_quick_mode() {
    let mut state = make_state();
    state.toggle_quick_help();
    assert!(state.help_overlay.visible);
    assert!(state.help_overlay.quick_mode);

    state.toggle_quick_help();
    assert!(!state.help_overlay.visible);
    assert!(!state.help_overlay.quick_mode);
}

#[test]
fn help_overlay_cursor_hint_maps_real_layout_regions() {
    let mut state = make_state();
    // A closed overlay never reports a hint, whatever the hit map holds.
    assert_eq!(state.help_overlay_cursor_hint_at(150, 215), None);

    state.toggle_help_overlay();
    crate::ui::install_help_hit_map_for_test(
        (100.0, 100.0, 200.0, 300.0),
        Some((110.0, 130.0, 180.0, 24.0)),
        &[(120.0, 200.0, 160.0, 30.0, crate::config::Action::ToggleHelp)],
    );

    assert_eq!(
        state.help_overlay_cursor_hint_at(150, 215),
        Some(HelpOverlayCursorHint::Pointer)
    );
    assert_eq!(
        state.help_overlay_cursor_hint_at(150, 140),
        Some(HelpOverlayCursorHint::Text)
    );
    assert_eq!(
        state.help_overlay_cursor_hint_at(150, 280),
        Some(HelpOverlayCursorHint::Default)
    );
    assert_eq!(state.help_overlay_cursor_hint_at(10, 10), None);

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn help_overlay_click_runs_rows_and_dismisses_outside() {
    let mut state = make_state();
    state.toggle_help_overlay();
    crate::ui::install_help_hit_map_for_test(
        (100.0, 100.0, 200.0, 300.0),
        Some((110.0, 130.0, 180.0, 24.0)),
        &[(
            120.0,
            200.0,
            160.0,
            30.0,
            crate::config::Action::ToggleStatusBar,
        )],
    );

    assert_eq!(
        state.help_overlay_click_at(150, 215),
        HelpOverlayClick::Run(crate::config::Action::ToggleStatusBar)
    );
    assert_eq!(
        state.help_overlay_click_at(150, 140),
        HelpOverlayClick::Inside
    );
    assert_eq!(
        state.help_overlay_click_at(150, 280),
        HelpOverlayClick::Inside
    );
    assert_eq!(
        state.help_overlay_click_at(10, 10),
        HelpOverlayClick::Outside
    );

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn close_help_overlay_resets_state_and_clears_hit_map() {
    let mut state = make_state();
    state.toggle_help_overlay();
    state.help_overlay.scroll = 42.0;
    crate::ui::install_help_hit_map_for_test(
        (100.0, 100.0, 200.0, 300.0),
        None,
        &[(120.0, 200.0, 160.0, 30.0, crate::config::Action::ToggleHelp)],
    );

    state.close_help_overlay();

    assert!(!state.help_overlay.visible);
    assert_eq!(state.help_overlay.scroll, 0.0);
    // Closing dropped the stale hit map, so a later click resolves outside.
    assert_eq!(crate::ui::help_overlay_region_at(150.0, 215.0), None);
    assert_eq!(
        state.help_overlay_click_at(150, 215),
        HelpOverlayClick::Outside
    );
}

/// Install a hit map with a single clickable row at (120..280, 200..230)
/// inside the box (100..300, 100..400) and a search well at (110..290,
/// 130..154). The overlay is opened first so the install survives the
/// open-time defensive clear, mirroring a real render pass populating the
/// map while help is visible.
fn state_with_help_row(action: crate::config::Action) -> InputState {
    let mut state = make_state();
    state.toggle_help_overlay();
    crate::ui::install_help_hit_map_for_test(
        (100.0, 100.0, 200.0, 300.0),
        Some((110.0, 130.0, 180.0, 24.0)),
        &[(120.0, 200.0, 160.0, 30.0, action)],
    );
    state
}

#[test]
fn help_release_runs_row_only_when_press_and_release_share_the_row() {
    let mut state = state_with_help_row(crate::config::Action::ClearCanvas);

    // Press and release both on the row -> the row's action runs.
    state.note_help_overlay_press(HelpOverlayPressSource::Pointer(1), 150, 215);
    assert_eq!(
        state.resolve_help_overlay_release(HelpOverlayPressSource::Pointer(1), 150, 215),
        Some(HelpOverlayReleaseOutcome::Run(
            crate::config::Action::ClearCanvas
        ))
    );
    // The recorded press was consumed.
    assert!(state.help_overlay.pending_presses.is_empty());

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn help_press_on_chrome_then_drag_onto_row_does_not_run() {
    // The destructive-action hazard: a press that starts on bare chrome and
    // is dragged onto a clickable row (here the ClearCanvas row) must never
    // execute it.
    let mut state = state_with_help_row(crate::config::Action::ClearCanvas);

    // (150, 280) is inside the box but below the row and search well: chrome.
    state.note_help_overlay_press(HelpOverlayPressSource::Pointer(1), 150, 280);
    assert_eq!(
        state.help_overlay.pending_presses,
        vec![(HelpOverlayPressSource::Pointer(1), HelpOverlayClick::Inside)]
    );
    assert_eq!(
        state.resolve_help_overlay_release(HelpOverlayPressSource::Pointer(1), 150, 215),
        Some(HelpOverlayReleaseOutcome::None)
    );

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn help_press_outside_then_release_on_row_does_not_run() {
    let mut state = state_with_help_row(crate::config::Action::ClearCanvas);

    state.note_help_overlay_press(HelpOverlayPressSource::Pointer(1), 10, 10);
    assert_eq!(
        state.help_overlay.pending_presses,
        vec![(
            HelpOverlayPressSource::Pointer(1),
            HelpOverlayClick::Outside
        )]
    );
    assert_eq!(
        state.resolve_help_overlay_release(HelpOverlayPressSource::Pointer(1), 150, 215),
        Some(HelpOverlayReleaseOutcome::None)
    );

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn help_press_and_release_outside_dismisses() {
    let mut state = state_with_help_row(crate::config::Action::ClearCanvas);

    state.note_help_overlay_press(HelpOverlayPressSource::Pointer(1), 10, 10);
    assert_eq!(
        state.resolve_help_overlay_release(HelpOverlayPressSource::Pointer(1), 20, 20),
        Some(HelpOverlayReleaseOutcome::Dismiss)
    );

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn help_press_on_row_then_release_on_chrome_does_not_run() {
    let mut state = state_with_help_row(crate::config::Action::ClearCanvas);

    state.note_help_overlay_press(HelpOverlayPressSource::Pointer(1), 150, 215);
    assert_eq!(
        state.resolve_help_overlay_release(HelpOverlayPressSource::Pointer(1), 150, 280),
        Some(HelpOverlayReleaseOutcome::None)
    );

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn help_release_without_a_recorded_press_is_inert() {
    let mut state = state_with_help_row(crate::config::Action::ClearCanvas);

    // No note_help_overlay_press call: a release cannot fabricate intent.
    assert!(state.help_overlay.pending_presses.is_empty());
    assert_eq!(
        state.resolve_help_overlay_release(HelpOverlayPressSource::Touch, 150, 215),
        None
    );

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn help_release_requires_the_modality_that_owned_the_press() {
    let mut state = state_with_help_row(crate::config::Action::ClearCanvas);
    state.note_help_overlay_press(HelpOverlayPressSource::Touch, 150, 215);

    assert_eq!(
        state.resolve_help_overlay_release(HelpOverlayPressSource::Pointer(1), 150, 215),
        None,
        "a pointer release must fall through when touch owns the help press"
    );
    assert!(
        !state.help_overlay.pending_presses.is_empty(),
        "another modality must not consume the touch press"
    );
    assert_eq!(
        state.resolve_help_overlay_release(HelpOverlayPressSource::Touch, 150, 215),
        Some(HelpOverlayReleaseOutcome::Run(
            crate::config::Action::ClearCanvas
        ))
    );

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn help_pointer_ownership_is_tracked_per_button() {
    let mut state = state_with_help_row(crate::config::Action::ClearCanvas);
    let left = HelpOverlayPressSource::Pointer(1);
    let middle = HelpOverlayPressSource::Pointer(2);
    state.note_help_overlay_press(left, 150, 215);
    state.note_help_overlay_press(middle, 150, 215);

    assert!(
        state.clear_help_overlay_press_for(middle),
        "a middle press made while help is open owns its release"
    );
    assert!(
        !state.clear_help_overlay_press_for(middle),
        "a middle release whose press preceded help must fall through"
    );
    assert_eq!(
        state.resolve_help_overlay_release(left, 150, 215),
        Some(HelpOverlayReleaseOutcome::Run(
            crate::config::Action::ClearCanvas
        )),
        "middle ownership must not consume the left help click"
    );

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn closing_help_keeps_its_press_release_owned_without_running_the_stale_target() {
    let mut state = state_with_help_row(crate::config::Action::ClearCanvas);
    let pointer = HelpOverlayPressSource::Pointer(1);
    state.note_help_overlay_press(pointer, 150, 215);

    state.close_help_overlay();

    assert!(state.help_overlay.pending_presses.is_empty());
    assert_eq!(state.help_overlay.consume_only_presses, vec![pointer]);

    assert_eq!(
        state.resolve_help_overlay_release(pointer, 150, 215),
        Some(HelpOverlayReleaseOutcome::None),
        "a press swallowed by help must still consume its physical release after help closes"
    );
    assert!(state.help_overlay.pending_presses.is_empty());
    assert!(state.help_overlay.consume_only_presses.is_empty());

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn reopening_help_cannot_retarget_a_press_from_the_previous_layout() {
    let mut state = state_with_help_row(crate::config::Action::ClearCanvas);
    let pointer = HelpOverlayPressSource::Pointer(1);
    state.note_help_overlay_press(pointer, 150, 215);

    state.close_help_overlay();
    state.toggle_help_overlay();
    crate::ui::install_help_hit_map_for_test(
        (100.0, 100.0, 200.0, 300.0),
        None,
        &[(
            120.0,
            200.0,
            160.0,
            30.0,
            crate::config::Action::ClearCanvas,
        )],
    );

    assert_eq!(
        state.resolve_help_overlay_release(pointer, 150, 215),
        Some(HelpOverlayReleaseOutcome::None),
        "an old press may only be consumed, never resolved against a reopened layout"
    );

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn a_new_help_press_supersedes_consume_only_ownership_for_its_source() {
    let mut state = state_with_help_row(crate::config::Action::ClearCanvas);
    let pointer = HelpOverlayPressSource::Pointer(1);
    state.note_help_overlay_press(pointer, 150, 215);
    state.close_help_overlay();
    state.toggle_help_overlay();
    crate::ui::install_help_hit_map_for_test(
        (100.0, 100.0, 200.0, 300.0),
        None,
        &[(
            120.0,
            200.0,
            160.0,
            30.0,
            crate::config::Action::ClearCanvas,
        )],
    );

    state.note_help_overlay_press(pointer, 150, 215);

    assert_eq!(
        state.resolve_help_overlay_release(pointer, 150, 215),
        Some(HelpOverlayReleaseOutcome::Run(
            crate::config::Action::ClearCanvas
        ))
    );

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn opening_help_drops_stale_hit_map_geometry() {
    let mut state = make_state();
    // Simulate geometry left over from a previous open.
    crate::ui::install_help_hit_map_for_test(
        (100.0, 100.0, 200.0, 300.0),
        None,
        &[(120.0, 200.0, 160.0, 30.0, crate::config::Action::ToggleHelp)],
    );

    // Opening must drop it so a click can never act on the previous layout
    // before the first fresh render repopulates the map.
    state.toggle_help_overlay();

    assert!(state.help_overlay.visible);
    assert_eq!(crate::ui::help_overlay_region_at(150.0, 215.0), None);
    assert!(state.help_overlay.pending_presses.is_empty());

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn starting_the_tour_routes_help_close_through_the_canonical_closer() {
    let mut state = make_state();
    state.toggle_help_overlay();
    crate::ui::install_help_hit_map_for_test(
        (100.0, 100.0, 200.0, 300.0),
        None,
        &[(120.0, 200.0, 160.0, 30.0, crate::config::Action::ToggleHelp)],
    );

    state.start_tour();

    assert!(!state.help_overlay.visible);
    // Routing through close_help_overlay dropped the cached hit map, so a
    // click after help reopens can never act on this stale layout.
    assert_eq!(crate::ui::help_overlay_region_at(150.0, 215.0), None);

    crate::ui::clear_help_overlay_hit_map();
}

#[test]
fn opening_the_command_palette_routes_help_close_through_the_canonical_closer() {
    let mut state = make_state();
    state.toggle_help_overlay();
    crate::ui::install_help_hit_map_for_test(
        (100.0, 100.0, 200.0, 300.0),
        None,
        &[(120.0, 200.0, 160.0, 30.0, crate::config::Action::ToggleHelp)],
    );

    state.toggle_command_palette();

    assert!(!state.help_overlay.visible);
    assert!(state.command_palette.open);
    assert_eq!(crate::ui::help_overlay_region_at(150.0, 215.0), None);

    crate::ui::clear_help_overlay_hit_map();
}
