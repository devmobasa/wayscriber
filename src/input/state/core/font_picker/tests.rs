use super::layout::FONT_PICKER_MAX_VISIBLE;
use super::*;
use crate::draw::{Color, Shape, system_font_families};
use crate::input::events::Key;
use crate::input::state::test_support::make_test_input_state;

fn installed_family() -> String {
    system_font_families()
        .first()
        .expect("at least one family")
        .clone()
}

/// Open the production picker with its process-wide catalog ready.
///
/// The Wayland runtime normally prewarms this after its first committed frame.
/// Unit tests for filtering, selection, and scrolling arrange that same ready
/// state explicitly so they cannot pass or fail according to which unrelated
/// font test happened to initialize the global cache first.
fn open_ready_font_picker(state: &mut InputState) {
    crate::draw::prewarm_system_font_catalog();
    state.open_font_picker();
    assert!(!state.font_picker_is_loading());
}

/// One text mutation that cannot fuzzy-match any installed family: it has more
/// characters than the longest candidate, so even subsequence matching fails.
fn impossible_family_query() -> String {
    let longest = system_font_families()
        .iter()
        .map(|family| family.chars().count())
        .max()
        .unwrap_or(0);
    "x".repeat(longest + 1)
}

fn text_shape(family: &str) -> Shape {
    Shape::Text {
        x: 10,
        y: 10,
        text: "hello".to_string(),
        color: Color::new(1.0, 1.0, 1.0, 1.0),
        size: 24.0,
        font_descriptor: FontDescriptor::new(
            family.to_string(),
            "normal".to_string(),
            "normal".to_string(),
        ),
        background_enabled: false,
        wrap_width: None,
    }
}

#[test]
fn first_open_can_show_loading_without_enumerating_in_input_dispatch() {
    const CHILD_ENV: &str = "WAYSCRIBER_COLD_FONT_PICKER_TEST_CHILD";
    const TEST_NAME: &str = "input::state::core::font_picker::tests::first_open_can_show_loading_without_enumerating_in_input_dispatch";

    // Font catalog storage is process-global and other font tests can warm it
    // in parallel. Run the production opener in a fresh test process so the
    // first-open condition is deterministic and a synchronous call added
    // before the loading branch cannot hide behind test ordering.
    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg(TEST_NAME)
            .arg("--exact")
            .arg("--test-threads=1")
            .env(CHILD_ENV, "1")
            .status()
            .expect("run isolated cold font-picker test");
        assert!(status.success(), "isolated cold font-picker test failed");
        return;
    }

    assert!(!crate::draw::system_font_catalog_is_ready());
    let mut state = make_test_input_state();

    state.open_font_picker();

    assert!(state.is_font_picker_open());
    assert!(state.font_picker_is_loading());
    assert!(
        !crate::draw::system_font_catalog_is_ready(),
        "opening input dispatch must only probe the cache, never enumerate it"
    );
    assert!(state.font_picker_families().is_empty());
    assert_eq!(state.font_picker_selected(), 0);
    assert_eq!(state.font_picker_scroll(), 0);
}

#[test]
fn catalog_completion_populates_a_picker_that_opened_in_loading_state() {
    let mut state = make_test_input_state();
    state.open_font_picker_with_catalog_ready(false);
    let _ = state.take_dirty_regions();

    crate::draw::prewarm_system_font_catalog();
    assert!(state.finish_font_picker_catalog_load());

    assert!(!state.font_picker_is_loading());
    assert!(!state.font_picker_load_failed());
    assert!(!state.font_picker_families().is_empty());
    assert!(
        !state.take_dirty_regions().is_empty(),
        "catalog completion must repaint the loading surface"
    );
}

#[test]
fn catalog_worker_failure_is_visible_and_reopening_allows_a_retry() {
    let mut state = make_test_input_state();
    state.open_font_picker_with_catalog_ready(false);

    assert!(state.fail_font_picker_catalog_load());
    assert!(!state.font_picker_is_loading());
    assert!(state.font_picker_load_failed());
    assert!(state.font_picker_families().is_empty());

    state.close_font_picker();
    state.open_font_picker_with_catalog_ready(false);
    assert!(state.font_picker_is_loading());
    assert!(!state.font_picker_load_failed());
}

#[test]
fn opening_lists_every_installed_family_and_closing_forgets_the_query() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let mut state = make_test_input_state();

    open_ready_font_picker(&mut state);
    assert!(state.is_font_picker_open());
    assert_eq!(
        state.font_picker_families().len(),
        system_font_families().len()
    );

    state.handle_font_picker_key_with_measurer(&route_measurer, Key::Char('x'), Some("x"));
    assert_eq!(state.font_picker_query(), "x");

    state.close_font_picker();
    assert!(!state.is_font_picker_open());
    assert!(state.font_picker_query().is_empty());
}

#[test]
fn the_picker_opens_on_the_font_already_in_use() {
    let mut state = make_test_input_state();
    let target = system_font_families()
        .get(3)
        .cloned()
        .unwrap_or_else(installed_family);
    state.set_font_descriptor(FontDescriptor::new(
        target.clone(),
        "normal".to_string(),
        "normal".to_string(),
    ));

    open_ready_font_picker(&mut state);

    let families = state.font_picker_families();
    assert_eq!(families[state.font_picker_selected()], target);
}

#[test]
fn typing_narrows_the_list_and_backspace_widens_it_again() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let mut state = make_test_input_state();
    open_ready_font_picker(&mut state);
    let all = state.font_picker_families().len();

    for ch in "zzzz".chars() {
        state.handle_font_picker_key_with_measurer(
            &route_measurer,
            Key::Char(ch),
            Some(&ch.to_string()),
        );
    }
    assert!(state.font_picker_families().len() < all);

    for _ in 0..4 {
        state.handle_font_picker_key_with_measurer(&route_measurer, Key::Backspace, None);
    }
    assert_eq!(state.font_picker_families().len(), all);
}

#[test]
fn a_query_that_matches_nothing_leaves_an_empty_list_rather_than_the_whole_one() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let mut state = make_test_input_state();
    open_ready_font_picker(&mut state);

    for ch in "qqzzxxjj".chars() {
        state.handle_font_picker_key_with_measurer(
            &route_measurer,
            Key::Char(ch),
            Some(&ch.to_string()),
        );
    }

    assert!(state.font_picker_families().is_empty());
    // Committing an empty list must close cleanly rather than index into it.
    assert!(!state.commit_font_picker());
    assert!(!state.is_font_picker_open());
}

#[test]
fn arrow_keys_clamp_at_both_ends_instead_of_wrapping() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let mut state = make_test_input_state();
    open_ready_font_picker(&mut state);
    let count = state.font_picker_families().len();

    state.set_font_picker_selection(0);
    state.handle_font_picker_key_with_measurer(&route_measurer, Key::Up, None);
    assert_eq!(
        state.font_picker_selected(),
        0,
        "wrapping to the bottom of a 269-item list is never what Up meant"
    );

    state.handle_font_picker_key_with_measurer(&route_measurer, Key::End, None);
    assert_eq!(state.font_picker_selected(), count - 1);
    state.handle_font_picker_key_with_measurer(&route_measurer, Key::Down, None);
    assert_eq!(state.font_picker_selected(), count - 1);
}

#[test]
fn the_scroll_window_follows_the_highlight_by_the_least_it_can() {
    let mut state = make_test_input_state();
    state.update_screen_dimensions(1920, 1080);
    open_ready_font_picker(&mut state);
    let window = state.font_picker_visible_rows(state.font_picker_families().len());
    assert_eq!(
        window, FONT_PICKER_MAX_VISIBLE,
        "a 1080p output has room for the full window"
    );

    state.set_font_picker_selection(0);
    assert_eq!(state.font_picker_scroll(), 0);

    state.set_font_picker_selection(window);
    assert_eq!(
        state.font_picker_scroll(),
        1,
        "stepping one past the window scrolls one row, not a page"
    );

    state.set_font_picker_selection(0);
    assert_eq!(state.font_picker_scroll(), 0);
}

#[test]
fn a_short_output_still_opens_with_the_font_in_use_on_screen() {
    // The picker opens centred on the current font. Centring on half the
    // twelve-row ceiling puts the highlight below a six-row panel, so the one
    // row the picker exists to show is the one row you cannot see.
    let mut state = make_test_input_state();
    state.update_screen_dimensions(600, 400);
    let families = system_font_families();
    if families.len() < 25 {
        return;
    }
    let target = families[20].clone();
    state.set_font_descriptor(FontDescriptor::new(
        target.clone(),
        "normal".to_string(),
        "normal".to_string(),
    ));

    open_ready_font_picker(&mut state);
    let window = state.font_picker_visible_rows(state.font_picker_families().len());

    assert_eq!(state.font_picker_selected(), 20);
    let scroll = state.font_picker_scroll();
    assert!(
        (scroll..scroll + window).contains(&state.font_picker_selected()),
        "the current font must be on screen: rows {scroll}..{} show {} of {window}",
        scroll + window,
        state.font_picker_selected()
    );
}

#[test]
fn a_short_output_scrolls_by_the_rows_it_actually_shows() {
    // The panel shrinks to fit the surface, so the window the scroll math uses
    // has to shrink with it. Scrolling by the twelve-row ceiling on an output
    // that draws six would leave the highlight below the panel's bottom edge —
    // on a row nobody can see.
    let mut state = make_test_input_state();
    state.update_screen_dimensions(600, 400);
    open_ready_font_picker(&mut state);
    let window = state.font_picker_visible_rows(state.font_picker_families().len());
    assert!(
        window < FONT_PICKER_MAX_VISIBLE,
        "a 400px-tall output cannot show the full window, got {window}"
    );

    state.set_font_picker_selection(0);
    assert_eq!(state.font_picker_scroll(), 0);

    state.set_font_picker_selection(window);
    assert_eq!(
        state.font_picker_scroll(),
        1,
        "the row one past the visible window must scroll into view"
    );
    assert!(
        state.font_picker_selected() < state.font_picker_scroll() + window,
        "the highlight must stay inside the rows the panel draws"
    );
}

#[test]
fn tab_switches_to_monospace_and_back() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let mut state = make_test_input_state();
    open_ready_font_picker(&mut state);
    let all = state.font_picker_families().len();

    state.handle_font_picker_key_with_measurer(&route_measurer, Key::Tab, None);
    assert_eq!(state.font_picker_filter(), FontPickerFilter::Monospace);
    assert!(state.font_picker_families().len() <= all);

    state.handle_font_picker_key_with_measurer(&route_measurer, Key::Tab, None);
    assert_eq!(state.font_picker_filter(), FontPickerFilter::All);
    assert_eq!(state.font_picker_families().len(), all);
}

#[test]
fn choosing_a_font_with_nothing_selected_sets_what_the_next_label_uses() {
    let mut state = make_test_input_state();
    open_ready_font_picker(&mut state);
    state.set_font_picker_selection(2);
    let chosen = state.font_picker_families()[2].clone();

    assert!(state.commit_font_picker());

    assert_eq!(state.style.font_descriptor.family, chosen);
    assert!(!state.is_font_picker_open());
}

#[test]
fn choosing_a_font_with_text_selected_restyles_it_and_leaves_the_tool_alone() {
    let mut state = make_test_input_state();
    let tool_font = state.style.font_descriptor.family.clone();
    let id = state
        .boards
        .active_frame_mut()
        .add_shape(text_shape("Sans"));
    state.set_selection(vec![id]);

    open_ready_font_picker(&mut state);
    assert_eq!(state.font_picker_target(), FontPickerTarget::Selection);
    state.set_font_picker_selection(2);
    let chosen = state.font_picker_families()[2].clone();
    assert!(state.commit_font_picker());

    let frame = state.boards.active_frame();
    let Some(Shape::Text {
        font_descriptor, ..
    }) = frame.shape(id).map(|drawn| &drawn.shape)
    else {
        panic!("the text shape survives");
    };
    assert_eq!(font_descriptor.family, chosen);
    assert_eq!(
        state.style.font_descriptor.family, tool_font,
        "restyling a selection must not also change what the next label uses"
    );
}

#[test]
fn chosen_fonts_come_back_to_the_top_of_an_unfiltered_list() {
    let mut state = make_test_input_state();
    open_ready_font_picker(&mut state);
    state.set_font_picker_selection(4);
    let chosen = state.font_picker_families()[4].clone();
    state.commit_font_picker();

    open_ready_font_picker(&mut state);

    assert_eq!(state.font_picker_recents().first(), Some(&chosen));
    assert_eq!(
        state.font_picker_families().first(),
        Some(&chosen),
        "a font you just used should be within reach next time"
    );
}

#[test]
fn recents_keep_the_most_recent_first_without_repeats() {
    let mut state = make_test_input_state();
    let families = system_font_families();
    let (first, second) = (families[0].clone(), families[1].clone());

    for family in [&first, &second, &first] {
        open_ready_font_picker(&mut state);
        let index = state
            .font_picker_families()
            .iter()
            .position(|name| name == family)
            .expect("family is listed");
        state.set_font_picker_selection(index);
        state.commit_font_picker();
    }

    assert_eq!(state.font_picker_recents(), [first, second]);
}

#[test]
fn escape_closes_without_changing_anything() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let mut state = make_test_input_state();
    let before = state.style.font_descriptor.family.clone();
    open_ready_font_picker(&mut state);
    state.set_font_picker_selection(3);

    state.handle_font_picker_key_with_measurer(&route_measurer, Key::Escape, None);

    assert!(!state.is_font_picker_open());
    assert_eq!(state.style.font_descriptor.family, before);
    assert!(state.font_picker_recents().is_empty());
}

#[test]
fn stray_keys_are_swallowed_rather_than_reaching_the_canvas_behind_the_modal() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let mut state = make_test_input_state();
    open_ready_font_picker(&mut state);

    assert!(state.handle_font_picker_key_with_measurer(&route_measurer, Key::Delete, None));
    assert!(state.handle_font_picker_key_with_measurer(&route_measurer, Key::Ctrl, None));
    assert!(state.is_font_picker_open());
}

#[test]
fn a_closed_picker_consumes_nothing() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let mut state = make_test_input_state();

    assert!(!state.handle_font_picker_key_with_measurer(&route_measurer, Key::Escape, None));
    assert!(!state.font_picker_hover(10.0, 10.0));
    assert!(!state.font_picker_press(10.0, 10.0));
}

/// A picker open on a surface big enough for the full twelve-row window.
fn open_picker() -> InputState {
    let mut state = make_test_input_state();
    state.update_screen_dimensions(1920, 1080);
    open_ready_font_picker(&mut state);
    state.set_font_picker_selection(0);
    state
}

#[test]
fn a_wheel_tick_moves_the_window_three_rows() {
    let mut state = open_picker();
    if state.font_picker_families().len() < 40 {
        return;
    }

    state.font_picker_wheel_scroll(1);
    assert_eq!(state.font_picker_scroll(), 3);
    state.font_picker_wheel_scroll(1);
    assert_eq!(state.font_picker_scroll(), 6);
    state.font_picker_wheel_scroll(-1);
    assert_eq!(state.font_picker_scroll(), 3);
}

#[test]
fn the_wheel_stops_at_both_ends_of_the_list() {
    let mut state = open_picker();
    let count = state.font_picker_families().len();
    let window = state.font_picker_visible_rows(count);
    if count < 40 {
        return;
    }

    for _ in 0..count {
        state.font_picker_wheel_scroll(1);
    }
    assert_eq!(
        state.font_picker_scroll(),
        count - window,
        "the last page is the end; there is nothing past it to show"
    );

    for _ in 0..count {
        state.font_picker_wheel_scroll(-1);
    }
    assert_eq!(state.font_picker_scroll(), 0);
}

#[test]
fn scrolling_carries_the_highlight_only_when_it_would_be_left_behind() {
    let mut state = open_picker();
    let window = state.font_picker_visible_rows(state.font_picker_families().len());
    if state.font_picker_families().len() < 40 {
        return;
    }

    // Highlight a row further down the window; one tick still leaves it visible.
    state.set_font_picker_selection(5);
    state.font_picker_wheel_scroll(1);
    assert_eq!(
        state.font_picker_selected(),
        5,
        "a row still on screen keeps the highlight, so Enter applies what is lit"
    );

    // Keep going until the window has moved past it.
    state.font_picker_wheel_scroll(1);
    state.font_picker_wheel_scroll(1);
    let scroll = state.font_picker_scroll();
    assert!(state.font_picker_selected() >= scroll);
    assert!(state.font_picker_selected() < scroll + window);
}

#[test]
fn a_held_arrow_repeats_after_a_delay_and_stops_on_release() {
    let route_measurer = crate::draw::TextMeasurer::default();
    use std::time::{Duration, Instant};

    let mut state = open_picker();
    if state.font_picker_families().len() < 40 {
        return;
    }
    let now = Instant::now();

    state.handle_font_picker_key_with_measurer(&route_measurer, Key::Down, None);
    assert_eq!(
        state.font_picker_selected(),
        1,
        "the press itself moves one"
    );
    assert!(
        state.font_picker_repeat_timeout(now).is_some(),
        "holding a navigation key has to wake the loop, or it never moves again"
    );
    assert!(
        !state.tick_font_picker_repeat(now),
        "nothing repeats before the initial delay"
    );

    assert!(state.tick_font_picker_repeat(now + Duration::from_millis(300)));
    assert_eq!(state.font_picker_selected(), 2);

    state.on_key_release(Key::Down);
    assert_eq!(state.font_picker_repeat_timeout(now), None);
    assert!(!state.tick_font_picker_repeat(now + Duration::from_secs(5)));
    assert_eq!(state.font_picker_selected(), 2);
}

#[test]
fn a_long_hold_repeats_faster_than_a_short_one() {
    let route_measurer = crate::draw::TextMeasurer::default();
    use std::time::{Duration, Instant};

    // The list runs to hundreds of families. At the command palette's flat rate
    // crossing it takes about fifteen seconds, which is long enough that people
    // give up and reach for the mouse.
    let mut state = open_picker();
    if state.font_picker_families().len() < 60 {
        return;
    }
    let start = Instant::now();
    state.handle_font_picker_key_with_measurer(&route_measurer, Key::Down, None);

    let steps_in = |state: &mut InputState, from: Duration, window: Duration| {
        let deadline = start + from + window;
        let mut at = start + from;
        let mut steps = 0;
        while at <= deadline {
            if state.tick_font_picker_repeat(at) {
                steps += 1;
            }
            at += Duration::from_millis(5);
        }
        steps
    };

    let early = steps_in(
        &mut state,
        Duration::from_millis(300),
        Duration::from_millis(500),
    );
    let late = steps_in(
        &mut state,
        Duration::from_millis(2000),
        Duration::from_millis(500),
    );

    assert!(
        late > early,
        "the same half-second of holding must travel further later: {early} then {late}"
    );
}

#[test]
fn moving_the_highlight_repaints_the_panel_rather_than_the_screen() {
    let route_measurer = crate::draw::TextMeasurer::default();
    // A held arrow ticks up to fifty times a second. Marking the whole surface
    // each time is the entire canvas re-rendered per row.
    let mut state = open_picker();
    let _ = state.take_dirty_regions();

    state.handle_font_picker_key_with_measurer(&route_measurer, Key::Down, None);
    let regions = state.take_dirty_regions();

    assert!(!regions.is_empty(), "the move has to repaint something");
    assert!(
        regions
            .iter()
            .all(|rect| rect.width < 1920 || rect.height < 1080),
        "a row move must not repaint the whole surface, got {regions:?}"
    );
}

#[test]
fn the_first_query_that_shrinks_the_list_repaints_the_panel_it_is_leaving() {
    let route_measurer = crate::draw::TextMeasurer::default();
    // Opening on the installed catalog draws a tall panel; the first query can
    // cut it to no rows. Partial repaints clip to their damage, so unless the
    // taller panel is damaged too its lower half stays on screen underneath.
    let mut state = make_test_input_state();
    state.update_screen_dimensions(1920, 1080);
    open_ready_font_picker(&mut state);
    let tall = state
        .font_picker_panel_bounds()
        .expect("an open picker has a panel");
    let _ = state.take_dirty_regions();

    // A query no family can match shrinks the panel to its smallest in one
    // mutation. Any earlier mutation would record the tall panel as a side
    // effect and hide the defect this test protects.
    let query = impossible_family_query();
    state.handle_font_picker_key_with_measurer(&route_measurer, Key::Char('x'), Some(&query));
    assert!(state.font_picker_families().is_empty());
    let short = state
        .font_picker_panel_bounds()
        .expect("an open picker has a panel");
    assert!(
        short.height < tall.height,
        "this fixture needs the panel to actually shrink: {tall:?} then {short:?}"
    );

    let regions = state.take_dirty_regions();
    let bottom_y = tall.y + tall.height - 2;
    let mid_x = tall.x + tall.width / 2;
    assert!(
        regions.iter().any(|rect| rect.contains(mid_x, bottom_y)),
        "the tall panel's bottom edge must be repainted, got {regions:?}"
    );
}

#[test]
fn the_first_narrowing_query_after_resize_repaints_the_resized_panel() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let mut state = make_test_input_state();
    state.update_screen_dimensions(1920, 1080);
    open_ready_font_picker(&mut state);
    let opening = state
        .font_picker_panel_bounds()
        .expect("an open picker has a panel");

    // The backend fully repaints a configured resize. Move the still-tall
    // panel far enough that damage at its old position cannot cover it.
    state.update_screen_dimensions(800, 900);
    let resized = state
        .font_picker_panel_bounds()
        .expect("the resized picker has a panel");
    let bottom_y = resized.y + resized.height - 2;
    let mid_x = resized.x + resized.width / 2;
    assert_ne!(opening, resized, "the fixture must move the panel");
    assert!(
        !opening.contains(mid_x, bottom_y),
        "the old panel must not accidentally cover the probe point"
    );
    let _ = state.take_dirty_regions();

    let query = impossible_family_query();
    state.handle_font_picker_key_with_measurer(&route_measurer, Key::Char('x'), Some(&query));
    assert!(state.font_picker_families().is_empty());

    let regions = state.take_dirty_regions();
    assert!(
        regions.iter().any(|rect| rect.contains(mid_x, bottom_y)),
        "the resized tall panel's bottom edge must be repainted, got {regions:?}"
    );
}

#[test]
fn reopening_the_picker_does_not_leave_the_old_key_repeating() {
    let route_measurer = crate::draw::TextMeasurer::default();
    use std::time::{Duration, Instant};

    let mut state = make_test_input_state();
    state.update_screen_dimensions(1920, 1080);
    open_ready_font_picker(&mut state);
    state.handle_font_picker_key_with_measurer(&route_measurer, Key::Down, None);
    assert!(state.font_picker_repeat_timeout(Instant::now()).is_some());

    open_ready_font_picker(&mut state);

    assert_eq!(
        state.font_picker_repeat_timeout(Instant::now()),
        None,
        "a fresh picker must not inherit a key the last one was repeating"
    );
    let selected = state.font_picker_selected();
    assert!(!state.tick_font_picker_repeat(Instant::now() + Duration::from_secs(2)));
    assert_eq!(state.font_picker_selected(), selected);
}
