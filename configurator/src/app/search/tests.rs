use super::*;
use iced::event::Status;
use iced::keyboard::{self, Key, Location, Modifiers, key};
use std::path::PathBuf;

use crate::models::session::SessionArtifactSummary;
use crate::models::{
    KeybindingField, KeybindingsTabId, SearchQuery, SessionCatalogItem, TabId, UiTabId,
};
use wayscriber::config::toolbar_item_ids as ids;

#[test]
fn active_search_tab_click_corrects_keybindings_nested_tab() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("pdf");
    app.active_tab = TabId::Keybindings;
    app.active_keybindings_tab = KeybindingsTabId::General;

    app.align_active_tabs_for_search();

    assert_eq!(app.active_keybindings_tab, KeybindingsTabId::CaptureView);
}

#[test]
fn active_search_tab_click_corrects_ui_nested_tab() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("presenter");
    app.active_tab = TabId::Ui;
    app.active_ui_tab = UiTabId::Toolbar;

    app.align_active_tabs_for_search();

    assert_eq!(app.active_ui_tab, UiTabId::PresenterMode);
}

#[test]
fn direct_tab_title_match_exposes_concrete_tab_content() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("keybindings");

    let summary = app.search_summary();
    let tab = summary.tab(TabId::Keybindings).expect("keybindings match");

    assert!(tab.show_all());
    assert_eq!(summary.total_matches(), 1);
}

#[test]
fn alias_match_exposes_keybindings_without_empty_tab() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("shortcut");

    let summary = app.search_summary();
    let tab = summary.tab(TabId::Keybindings).expect("shortcut alias");

    assert!(tab.show_all());
}

#[test]
fn pdf_matches_capture_and_capture_view_keybindings() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("pdf");

    let summary = app.search_summary();
    let capture = summary.tab(TabId::Capture).expect("capture match");
    let keybindings = summary.tab(TabId::Keybindings).expect("keybindings match");

    assert!(capture.area_matches(SearchArea::CapturePdf));
    assert!(keybindings.keybindings_tab_visible(KeybindingsTabId::CaptureView));
}

#[test]
fn direct_nested_keybinding_title_shows_that_tabs_rows() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("capture view");

    let summary = app.search_summary();
    let keybindings = summary.tab(TabId::Keybindings).expect("keybindings match");

    assert!(keybindings.keybindings_tab_visible(KeybindingsTabId::CaptureView));
    assert!(keybindings.keybinding_tab_title_visible(KeybindingsTabId::CaptureView));
}

#[test]
fn parent_scoped_keybinding_title_shows_nested_tab() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("keybindings capture view");

    let summary = app.search_summary();
    let keybindings = summary.tab(TabId::Keybindings).expect("keybindings match");

    assert!(!keybindings.show_all());
    assert!(keybindings.keybindings_tab_visible(KeybindingsTabId::CaptureView));
    assert!(keybindings.keybinding_tab_title_visible(KeybindingsTabId::CaptureView));
}

#[test]
fn field_level_terms_do_not_force_whole_tab_visible() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("font");

    let summary = app.search_summary();
    let drawing = summary.tab(TabId::Drawing).expect("drawing match");

    assert!(!drawing.show_all());
    assert!(drawing.area_matches(SearchArea::DrawingFont));
}

#[test]
fn exact_drawing_default_labels_match_defaults_section() {
    for query in ["font size pt", "eraser size px", "enable text background"] {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new(query);

        let summary = app.search_summary();
        let drawing = summary.tab(TabId::Drawing).expect("drawing match");

        assert!(
            drawing.area_matches(SearchArea::DrawingDefaults),
            "query should show Drawing Defaults: {query}",
        );
    }
}

#[test]
fn exact_drawing_color_and_font_labels_match_their_sections() {
    let cases = [
        ("pen color", SearchArea::DrawingColor),
        ("quick colors", SearchArea::DrawingColor),
        ("quick color label", SearchArea::DrawingColor),
        ("custom or numeric weight", SearchArea::DrawingFont),
    ];

    for (query, expected_area) in cases {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new(query);

        let summary = app.search_summary();
        let drawing = summary.tab(TabId::Drawing).expect("drawing match");

        assert!(
            drawing.area_matches(expected_area),
            "query should show Drawing section: {query}",
        );
    }
}

#[test]
fn exact_performance_rendering_labels_match_rendering_section() {
    for query in ["buffer count", "enable vsync", "max fps"] {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new(query);

        let summary = app.search_summary();
        let performance = summary.tab(TabId::Performance).expect("performance match");

        assert!(
            performance.area_matches(SearchArea::PerformanceRendering),
            "query should show Performance Rendering: {query}",
        );
    }
}

#[test]
fn exact_static_section_labels_match_their_sections() {
    let cases = [
        ("undo all delay", TabId::History, SearchArea::HistoryMain),
        ("redo all delay", TabId::History, SearchArea::HistoryMain),
        (
            "enable custom undo redo section",
            TabId::History,
            SearchArea::HistoryCustom,
        ),
        (
            "named export profile",
            TabId::RenderProfiles,
            SearchArea::RenderProfilesGeneral,
        ),
        (
            "per-output persistence",
            TabId::Session,
            SearchArea::SessionPersistence,
        ),
        (
            "max shapes per frame",
            TabId::Session,
            SearchArea::SessionPersistence,
        ),
        (
            "auto-compress threshold kb",
            TabId::Session,
            SearchArea::SessionPersistence,
        ),
        (
            "persist transparent mode drawings",
            TabId::Session,
            SearchArea::SessionPersistence,
        ),
        (
            "max file size mb",
            TabId::Session,
            SearchArea::SessionPersistence,
        ),
        ("show board badge", TabId::Boards, SearchArea::BoardsGeneral),
        (
            "persist runtime customizations",
            TabId::Boards,
            SearchArea::BoardsGeneral,
        ),
        (
            "place arrowhead at end of line",
            TabId::Arrow,
            SearchArea::Arrow,
        ),
    ];

    for (query, expected_tab, expected_area) in cases {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new(query);

        let summary = app.search_summary();
        let tab = summary.tab(expected_tab).expect("tab match");

        assert!(
            tab.area_matches(expected_area),
            "query should show expected section: {query}",
        );
    }
}

#[cfg(feature = "tablet-input")]
#[test]
fn exact_tablet_labels_match_tablet_section() {
    for query in [
        "enable pressure-to-thickness",
        "min thickness",
        "pressure thickness scale step",
    ] {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new(query);

        let summary = app.search_summary();
        let tablet = summary.tab(TabId::Tablet).expect("tablet match");

        assert!(
            tablet.area_matches(SearchArea::Tablet),
            "query should show Tablet section: {query}",
        );
    }
}

#[test]
fn ui_nested_alias_matches_concrete_nested_tab() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("toolbar");

    let summary = app.search_summary();
    let ui = summary.tab(TabId::Ui).expect("ui match");

    assert!(!ui.show_all());
    assert!(ui.ui_tabs().contains(&UiTabId::Toolbar));
}

#[test]
fn parent_scoped_ui_queries_match_concrete_nested_tabs() {
    let cases = [
        (
            "ui toolbar",
            &[UiTabId::Toolbar, UiTabId::ToolbarVisibility][..],
        ),
        ("ui layout mode", &[UiTabId::Toolbar][..]),
        ("ui toolbar blur", &[UiTabId::ToolbarVisibility][..]),
        ("interface presenter", &[UiTabId::PresenterMode][..]),
        ("interface status bar position", &[UiTabId::StatusBar][..]),
    ];

    for (query, expected_tabs) in cases {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new(query);

        let summary = app.search_summary();
        let ui = summary.tab(TabId::Ui).expect("ui match");

        assert!(!ui.show_all(), "query should not show all UI tabs: {query}");
        assert_eq!(
            ui.ui_tabs(),
            expected_tabs,
            "query should show concrete nested UI tab: {query}",
        );
    }
}

#[test]
fn ui_nested_visible_control_labels_match_concrete_nested_tabs() {
    let cases = [
        ("layout mode", UiTabId::Toolbar),
        (ids::TOP_TOOL_BLUR.as_str(), UiTabId::ToolbarVisibility),
        (ids::SIDE_GROUP_PRESETS.as_str(), UiTabId::ToolbarVisibility),
        ("status bar position", UiTabId::StatusBar),
        ("click highlight radius", UiTabId::ClickHighlight),
    ];

    for (query, expected_tab) in cases {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new(query);

        let summary = app.search_summary();
        let ui = summary.tab(TabId::Ui).expect("ui match");

        assert!(
            !ui.show_all(),
            "query should not force all UI tabs: {query}"
        );
        assert_eq!(
            ui.ui_tabs(),
            &[expected_tab],
            "query should show concrete nested UI tab: {query}",
        );
    }
}

#[test]
fn dynamic_matches_preserve_original_indices() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.draft.boards.items[1].name = "Meeting board".to_string();
    app.search_query = SearchQuery::new("meeting");

    let summary = app.search_summary();
    let boards = summary.tab(TabId::Boards).expect("board match");

    assert_eq!(boards.board_indices(), &[1]);
}

#[test]
fn scoped_tab_section_queries_match_target_sections() {
    let cases = [
        (
            "capture pdf",
            TabId::Capture,
            SearchArea::CapturePdf,
            &[SearchArea::CaptureFiles][..],
        ),
        (
            "background mode service",
            TabId::Daemon,
            SearchArea::DaemonService,
            &[
                SearchArea::DaemonStatus,
                SearchArea::DaemonShortcut,
                SearchArea::DaemonLightControls,
            ][..],
        ),
        (
            "daemon service",
            TabId::Daemon,
            SearchArea::DaemonService,
            &[
                SearchArea::DaemonStatus,
                SearchArea::DaemonShortcut,
                SearchArea::DaemonLightControls,
            ][..],
        ),
        (
            "daemon status",
            TabId::Daemon,
            SearchArea::DaemonStatus,
            &[
                SearchArea::DaemonService,
                SearchArea::DaemonShortcut,
                SearchArea::DaemonLightControls,
            ][..],
        ),
        (
            "screenshot pdf",
            TabId::Capture,
            SearchArea::CapturePdf,
            &[SearchArea::CaptureFiles][..],
        ),
    ];

    for (query, expected_tab, expected_area, hidden_areas) in cases {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new(query);

        let summary = app.search_summary();
        let tab = summary.tab(expected_tab).expect("tab match");

        assert!(
            !tab.show_all(),
            "scoped query should not show the whole tab: {query}",
        );
        assert!(
            tab.area_matches(expected_area),
            "query should show scoped area: {query}",
        );
        for hidden_area in hidden_areas {
            assert!(
                !tab.area_matches(*hidden_area),
                "query should not show unrelated section {hidden_area:?}: {query}",
            );
        }
    }
}

#[test]
fn inactive_token_search_preserves_raw_input_text() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("/");

    let summary = app.search_summary();

    assert!(!summary.is_active());
    assert!(summary.has_raw_input());
    assert_eq!(summary.raw_query(), "/");
    assert_eq!(summary.total_matches(), 0);
}

#[test]
fn escape_clears_inactive_raw_search_text() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("/");

    let _ = app.handle_keyboard_event(
        keyboard::Event::KeyPressed {
            key: Key::Named(key::Named::Escape),
            modified_key: Key::Named(key::Named::Escape),
            physical_key: key::Physical::Code(key::Code::Escape),
            location: Location::Standard,
            modifiers: Modifiers::empty(),
            text: None,
            repeat: false,
        },
        Status::Captured,
    );

    assert_eq!(app.search_query.raw(), "");
}

#[test]
fn escape_refocus_hint_is_cleared_after_pointer_press() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    let _ = app.handle_search_changed("preset".to_string());
    assert!(app.search_input_focus_hint);

    let _ = app.handle_pointer_pressed();
    assert!(app.search_input_focus_hint);

    let _ = app.handle_search_focus_observed(false);
    assert!(!app.search_input_focus_hint);

    let _ = app.handle_keyboard_event(
        keyboard::Event::KeyPressed {
            key: Key::Named(key::Named::Escape),
            modified_key: Key::Named(key::Named::Escape),
            physical_key: key::Physical::Code(key::Code::Escape),
            location: Location::Standard,
            modifiers: Modifiers::empty(),
            text: None,
            repeat: false,
        },
        Status::Captured,
    );

    assert_eq!(app.search_query.raw(), "");
    assert!(!app.search_input_focus_hint);
}

#[test]
fn startup_config_fallback_consumes_startup_focus_pending_state() {
    let (mut app, _task) = ConfiguratorApp::new_app();

    let _ = app.handle_startup_search_focus_config_fallback();

    assert!(app.search_input_focus_hint);
    assert!(!app.startup_search_focus_pending);
}

#[test]
fn pointer_press_cancels_pending_startup_focus() {
    let (mut app, _task) = ConfiguratorApp::new_app();

    let _ = app.handle_pointer_pressed();

    assert!(!app.startup_search_focus_pending);
}

#[test]
fn startup_config_fallback_does_not_focus_after_pointer_press() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_input_focus_hint = false;

    let _ = app.handle_pointer_pressed();
    let _ = app.handle_startup_search_focus_config_fallback();

    assert!(!app.search_input_focus_hint);
    assert!(!app.startup_search_focus_pending);
}

#[test]
fn tab_key_cancels_pending_startup_focus() {
    let (mut app, _task) = ConfiguratorApp::new_app();

    let _ = app.handle_keyboard_event(
        keyboard::Event::KeyPressed {
            key: Key::Named(key::Named::Tab),
            modified_key: Key::Named(key::Named::Tab),
            physical_key: key::Physical::Code(key::Code::Tab),
            location: Location::Standard,
            modifiers: Modifiers::empty(),
            text: None,
            repeat: false,
        },
        Status::Captured,
    );

    assert!(!app.startup_search_focus_pending);
}

#[test]
fn observed_search_focus_allows_captured_home_end() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_input_focus_hint = false;

    let _ = app.handle_search_focus_observed(true);

    assert!(app.search_input_focus_hint);
    for key in [key::Named::Home, key::Named::End] {
        let event = keyboard::Event::KeyPressed {
            key: Key::Named(key),
            modified_key: Key::Named(key),
            physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
            location: Location::Standard,
            modifiers: Modifiers::empty(),
            text: None,
            repeat: false,
        };

        assert!(
            content_scroll_action_for_status(&event, Status::Captured, app.search_input_focus_hint)
                .is_some()
        );
    }
}

#[test]
fn captured_home_end_scroll_only_when_search_focus_hint_is_active() {
    for key in [key::Named::Home, key::Named::End] {
        let event = keyboard::Event::KeyPressed {
            key: Key::Named(key),
            modified_key: Key::Named(key),
            physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
            location: Location::Standard,
            modifiers: Modifiers::empty(),
            text: None,
            repeat: false,
        };

        assert_eq!(
            content_scroll_action_for_status(&event, Status::Captured, false),
            None
        );
        assert!(content_scroll_action_for_status(&event, Status::Captured, true).is_some());
        assert!(content_scroll_action_for_status(&event, Status::Ignored, false).is_some());
    }
}

#[test]
fn board_item_static_labels_match_board_rows() {
    for query in ["display name", "board id", "override default pen color"] {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new(query);

        let summary = app.search_summary();
        let boards = summary.tab(TabId::Boards).expect("board match");
        let expected = (0..app.draft.boards.items.len()).collect::<Vec<_>>();

        assert_eq!(
            boards.board_indices(),
            expected.as_slice(),
            "query should show board rows by static row label: {query}",
        );
    }
}

#[test]
fn render_profile_matches_preserve_original_indices() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    let profile = app.draft.render_profiles.new_profile();
    app.draft.render_profiles.profiles.push(profile);
    app.draft.render_profiles.profiles[0].name = "Night colors".to_string();
    app.search_query = SearchQuery::new("night");

    let summary = app.search_summary();
    let render = summary
        .tab(TabId::RenderProfiles)
        .expect("render profile match");

    assert_eq!(render.render_profile_indices(), &[0]);
}

#[test]
fn render_profile_label_match_keeps_profile_controls_visible() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    let profile = app.draft.render_profiles.new_profile();
    app.draft.render_profiles.profiles.push(profile);
    app.search_query = SearchQuery::new("render profile 1");

    let summary = app.search_summary();
    let render = summary
        .tab(TabId::RenderProfiles)
        .expect("render profile match");

    assert_eq!(render.render_profile_indices(), &[0]);
    assert!(render.render_profile_controls_visible(0));
    assert!(render.render_profile_mapping_indices().is_empty());
}

#[test]
fn session_catalog_action_match_keeps_catalog_items_visible() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.session_catalog.replace_items(vec![catalog_item("s-1")]);
    app.search_query = SearchQuery::new("rename");

    let summary = app.search_summary();
    let session = summary.tab(TabId::Session).expect("session match");

    assert!(session.area_matches(SearchArea::SessionCatalog));
    assert!(session.session_item_visible("s-1"));
}

#[test]
fn preset_slot_search_indexes_visible_slot_body_labels() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.draft.presets.slot_mut(1).expect("slot 1").enabled = true;
    app.search_query = SearchQuery::new("arrow head at end");

    let summary = app.search_summary();
    let presets = summary.tab(TabId::Presets).expect("preset match");

    assert_eq!(presets.preset_slots(), &[1]);
}

#[test]
fn render_profile_mapping_match_preserves_mapping_identity() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    let mut profile = app.draft.render_profiles.new_profile();
    profile.mappings[0].from = "#123456".to_string();
    app.draft.render_profiles.profiles.push(profile);
    app.search_query = SearchQuery::new("#123456");

    let summary = app.search_summary();
    let render = summary
        .tab(TabId::RenderProfiles)
        .expect("render profile match");

    assert_eq!(render.render_profile_indices(), &[0]);
    assert!(!render.render_profile_controls_visible(0));
    assert_eq!(render.render_profile_mapping_indices(), &[(0, 0)]);
}

#[test]
fn exact_pdf_field_labels_match_pdf_section() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("show pdf page labels");

    let summary = app.search_summary();
    let capture = summary.tab(TabId::Capture).expect("capture match");

    assert!(capture.area_matches(SearchArea::CapturePdf));
}

#[test]
fn exact_pdf_background_field_label_matches_pdf_section() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("transparent page background");

    let summary = app.search_summary();
    let capture = summary.tab(TabId::Capture).expect("capture match");

    assert!(capture.area_matches(SearchArea::CapturePdf));
}

#[test]
fn exact_general_ui_field_labels_match_general_ui_section() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("focus loss");

    let summary = app.search_summary();
    let ui = summary.tab(TabId::Ui).expect("ui match");

    assert!(ui.area_matches(SearchArea::UiGeneral));
}

#[test]
fn exact_capture_file_labels_match_capture_file_section() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("enable capture shortcuts");

    let summary = app.search_summary();
    let capture = summary.tab(TabId::Capture).expect("capture match");

    assert!(capture.area_matches(SearchArea::CaptureFiles));
}

#[test]
fn exact_capture_filename_labels_match_capture_sections() {
    let cases = [
        (
            "capture filename template",
            SearchArea::CaptureFiles,
            SearchArea::CapturePdf,
        ),
        (
            "pdf filename template",
            SearchArea::CapturePdf,
            SearchArea::CaptureFiles,
        ),
        (
            "capture pdf filename template",
            SearchArea::CapturePdf,
            SearchArea::CaptureFiles,
        ),
        (
            "capture show pdf page labels",
            SearchArea::CapturePdf,
            SearchArea::CaptureFiles,
        ),
    ];

    for (query, expected_area, hidden_area) in cases {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new(query);

        let summary = app.search_summary();
        let capture = summary.tab(TabId::Capture).expect("capture match");

        assert!(
            capture.area_matches(expected_area),
            "query should show matching capture area: {query}",
        );
        assert!(
            !capture.area_matches(hidden_area),
            "query should hide unrelated capture area: {query}",
        );
    }
}

#[test]
fn disabled_preset_slots_do_not_match_hidden_body_controls() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.draft.presets.slot_mut(1).expect("slot 1").enabled = false;
    app.draft.presets.slot_mut(2).expect("slot 2").enabled = true;
    app.search_query = SearchQuery::new("arrow head at end");

    let summary = app.search_summary();
    let presets = summary.tab(TabId::Presets).expect("preset match");

    assert!(!presets.preset_slots().contains(&1));
    assert!(presets.preset_slots().contains(&2));
}

#[test]
fn daemon_area_terms_match_individual_sections() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("light");

    let summary = app.search_summary();
    let daemon = summary.tab(TabId::Daemon).expect("daemon match");

    assert!(daemon.area_matches(SearchArea::DaemonLightControls));
    assert!(!daemon.area_matches(SearchArea::DaemonShortcut));
}

#[test]
fn exact_keybinding_input_label_matches_keybinding_rows() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("shortcut list");

    let summary = app.search_summary();
    let keybindings = summary.tab(TabId::Keybindings).expect("keybindings match");

    assert!(!keybindings.show_all());
    assert!(!keybindings.keybinding_tabs().is_empty());
}

#[test]
fn eyedropper_search_exposes_screen_color_keybinding() {
    let (mut app, _task) = ConfiguratorApp::new_app();
    app.search_query = SearchQuery::new("eyedropper");

    let summary = app.search_summary();
    let keybindings = summary.tab(TabId::Keybindings).expect("keybindings match");

    assert!(keybindings.keybinding_field_visible(KeybindingField::PickScreenColor));
    assert!(
        keybindings
            .keybinding_tabs()
            .contains(&KeybindingsTabId::Drawing)
    );
}

fn catalog_item(id: &str) -> SessionCatalogItem {
    SessionCatalogItem {
        id: id.to_string(),
        display_name: "Lecture".to_string(),
        path: PathBuf::from("/tmp/lecture.wayscriber-session"),
        path_label: "/tmp/lecture.wayscriber-session".to_string(),
        canonical_path_label: None,
        created_label: "now".to_string(),
        last_opened_label: "Never".to_string(),
        last_saved_label: "Never".to_string(),
        artifacts: SessionArtifactSummary {
            primary_exists: false,
            backup_exists: false,
            recovery_exists: false,
            clear_marker_exists: false,
            lock_exists: false,
            non_lock_size_bytes: 0,
        },
    }
}
