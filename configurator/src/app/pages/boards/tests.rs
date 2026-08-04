use super::*;
use crate::models::SearchQuery;

/// Two boards, so a fingerprint covers more than one section.
fn app_with_two_boards() -> ConfiguratorApp {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    while app.draft.boards.items.len() < 2 {
        let Some(item) = app.draft.boards.items.first().cloned() else {
            break;
        };
        app.draft.boards.items.push(item);
        app.boards_collapsed.push(false);
    }
    app
}

/// The law the binding rests on: one layout per board, so the sections it
/// builds and the values it hands them are indexed by the same thing.
#[test]
fn there_is_one_layout_per_board() {
    let app = app_with_two_boards();
    let layouts = section_layouts(&app, &app.search_summary());

    assert_eq!(layouts.len(), app.draft.boards.items.len());
}

/// The caret guarantee, stated as the layout law it rests on: no keystroke
/// in a board's text may rebuild the row it lands in.
#[test]
fn typing_into_a_board_field_leaves_the_layout_alone() {
    let mut app = app_with_two_boards();
    let summary = app.search_summary();
    let before = section_layouts(&app, &summary);

    let Some(item) = app.draft.boards.items.first_mut() else {
        return;
    };
    item.name = "Half typ".to_string();
    item.id = "half-typ".to_string();
    item.background_color.set_component(0, "0.1".to_string());
    app.color_picker_hex
        .insert(ColorPickerId::BoardBackground(0), "#1A00".to_string());

    let summary = app.search_summary();
    assert_eq!(before, section_layouts(&app, &summary));
    // The values a refresh writes in place carry the edit instead.
    assert_eq!(app.draft.boards.items[0].id, "half-typ");
    assert_eq!(
        app.color_picker_hex
            .get(&ColorPickerId::BoardBackground(0))
            .map(String::as_str),
        Some("#1A00")
    );
}

#[test]
fn toggling_a_board_switch_changes_the_layout() {
    let mut app = app_with_two_boards();
    let summary = app.search_summary();
    let before = section_layouts(&app, &summary);

    let Some(item) = app.draft.boards.items.first_mut() else {
        return;
    };
    item.pinned = !item.pinned;

    let summary = app.search_summary();
    assert_ne!(before, section_layouts(&app, &summary));
}

#[test]
fn collapsing_a_board_changes_the_layout() {
    let mut app = app_with_two_boards();
    let summary = app.search_summary();
    let before = section_layouts(&app, &summary);

    let Some(collapsed) = app.boards_collapsed.first_mut() else {
        return;
    };
    *collapsed = true;

    let summary = app.search_summary();
    assert_ne!(before, section_layouts(&app, &summary));
}

/// Search visibility belongs to the layout too: a rebuild is what applies
/// it now that nothing refreshes a section in place.
#[test]
fn a_search_that_hides_a_board_changes_the_layout() {
    let mut app = app_with_two_boards();
    let Some(item) = app.draft.boards.items.first_mut() else {
        return;
    };
    item.name = "zqxwvu".to_string();
    let summary = app.search_summary();
    let before = section_layouts(&app, &summary);

    app.search_query = SearchQuery::new("zqxwvu");
    let summary = app.search_summary();
    let layouts = section_layouts(&app, &summary);

    assert_ne!(before, layouts);
    let visible: Vec<bool> = layouts.iter().map(|layout| layout.visible).collect();
    assert_eq!(visible.first(), Some(&true));
    assert!(visible.iter().skip(1).all(|visible| !visible));
}
