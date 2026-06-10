use super::{
    ToolbarLayoutSpec, build_side_hits, create_test_input_state, side_size, snapshot_from_state,
};
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::input::{Tool, ToolbarDrawerTab};
use crate::ui::toolbar::{
    SessionRecentSnapshot, ToolbarEvent, ToolbarSideSection, ToolbarSnapshot,
};

fn rendered_side_hits(snapshot: &ToolbarSnapshot) -> Vec<HitRegion> {
    let (w, h) = side_size(snapshot);
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let mut hits = Vec::new();
    crate::backend::wayland::toolbar::render_side_palette(
        &ctx, w as f64, h as f64, snapshot, &mut hits, None, None,
    )
    .unwrap();
    hits
}

fn static_side_hits(snapshot: &ToolbarSnapshot) -> Vec<HitRegion> {
    let (w, h) = side_size(snapshot);
    let mut hits = Vec::new();
    build_side_hits(w as f64, h as f64, snapshot, &mut hits);
    hits
}

fn assert_expand_hits(hits: &[HitRegion], sections: &[ToolbarSideSection]) {
    for section in sections {
        assert!(
            hits.iter().any(|hit| matches!(
                hit.event,
                ToolbarEvent::ToggleSideSectionCollapsed(hit_section, false)
                    if hit_section == *section
            )),
            "missing expand hit for {section:?}"
        );
    }
}

#[test]
fn collapsed_header_hit_excludes_body_start_boundary() {
    let mut state = create_test_input_state();
    state.toolbar_drawer_open = true;
    state.toolbar_drawer_tab = ToolbarDrawerTab::App;
    state.show_settings_section = true;
    let snapshot = snapshot_from_state(&state);
    let hits = static_side_hits(&snapshot);
    let header = hits
        .iter()
        .find(|hit| {
            matches!(
                hit.event,
                ToolbarEvent::ToggleSideSectionCollapsed(ToolbarSideSection::Settings, true)
            )
        })
        .expect("settings collapse header");
    let body_start_y = header.rect.1 + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;

    assert!(
        !header.contains(header.rect.0 + header.rect.2 / 2.0, body_start_y),
        "collapse header must not contain the first body row boundary"
    );
}

#[test]
fn side_session_collapsed_hides_body_hits_and_keeps_expand_hit() {
    let mut state = create_test_input_state();
    state.toolbar_drawer_open = true;
    state.toolbar_drawer_tab = ToolbarDrawerTab::Session;

    let mut expanded = snapshot_from_state(&state);
    expanded.active_session_path =
        Some(std::path::PathBuf::from("/tmp/current.wayscriber-session"));
    expanded.active_session_name = Some("current.wayscriber-session".to_string());
    expanded.recent_sessions = vec![SessionRecentSnapshot {
        display_name: "recent.wayscriber-session".to_string(),
        path: std::path::PathBuf::from("/tmp/recent.wayscriber-session"),
    }];

    state
        .toolbar_collapsed_side_sections
        .insert(ToolbarSideSection::Session);
    let mut collapsed = snapshot_from_state(&state);
    collapsed.active_session_path = expanded.active_session_path.clone();
    collapsed.active_session_name = expanded.active_session_name.clone();
    collapsed.recent_sessions = expanded.recent_sessions.clone();

    let expanded_size = side_size(&expanded);
    let collapsed_size = side_size(&collapsed);
    assert!(
        collapsed_size.1 < expanded_size.1,
        "collapsed Session section should reduce side toolbar height"
    );

    let (w, h) = collapsed_size;
    let mut hits = Vec::new();
    build_side_hits(w as f64, h as f64, &collapsed, &mut hits);

    assert!(hits.iter().any(|hit| matches!(
        hit.event,
        ToolbarEvent::ToggleSideSectionCollapsed(ToolbarSideSection::Session, false)
    )));
    assert!(
        !hits
            .iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::OpenSession)),
        "collapsed Session section should hide Open hit"
    );
    assert!(
        !hits
            .iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::OpenRecentSession(_))),
        "collapsed Session section should hide recent hits"
    );
}

#[test]
fn side_settings_collapsed_hides_body_hits_and_keeps_expand_hit() {
    let mut state = create_test_input_state();
    state.toolbar_drawer_open = true;
    state.toolbar_drawer_tab = ToolbarDrawerTab::App;
    state.show_settings_section = true;
    let expanded = snapshot_from_state(&state);

    state
        .toolbar_collapsed_side_sections
        .insert(ToolbarSideSection::Settings);
    let collapsed = snapshot_from_state(&state);

    let expanded_size = side_size(&expanded);
    let collapsed_size = side_size(&collapsed);
    assert!(
        collapsed_size.1 < expanded_size.1,
        "collapsed Settings section should reduce side toolbar height"
    );

    let (w, h) = collapsed_size;
    let mut hits = Vec::new();
    build_side_hits(w as f64, h as f64, &collapsed, &mut hits);

    assert!(hits.iter().any(|hit| matches!(
        hit.event,
        ToolbarEvent::ToggleSideSectionCollapsed(ToolbarSideSection::Settings, false)
    )));
    assert!(
        !hits
            .iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::ToggleContextAwareUi(_))),
        "collapsed Settings section should hide setting toggles"
    );
    assert!(
        !hits
            .iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::OpenConfigFile)),
        "collapsed Settings section should hide action buttons"
    );
}

#[test]
fn common_side_sections_collapsed_keep_only_headers() {
    let mut state = create_test_input_state();
    state.toolbar_drawer_open = true;
    state.toolbar_drawer_tab = ToolbarDrawerTab::App;
    state.show_step_section = true;

    let sections = [
        ToolbarSideSection::Colors,
        ToolbarSideSection::Presets,
        ToolbarSideSection::Thickness,
        ToolbarSideSection::TextSize,
        ToolbarSideSection::Font,
        ToolbarSideSection::Actions,
        ToolbarSideSection::StepUndo,
        ToolbarSideSection::Settings,
    ];
    state
        .toolbar_collapsed_side_sections
        .extend(sections.iter().copied());

    let mut collapsed = snapshot_from_state(&state);
    collapsed.active_session_path =
        Some(std::path::PathBuf::from("/tmp/current.wayscriber-session"));
    collapsed.active_session_name = Some("current.wayscriber-session".to_string());
    collapsed.recent_sessions = vec![SessionRecentSnapshot {
        display_name: "recent.wayscriber-session".to_string(),
        path: std::path::PathBuf::from("/tmp/recent.wayscriber-session"),
    }];

    state.toolbar_collapsed_side_sections.clear();
    let mut expanded = snapshot_from_state(&state);
    expanded.active_session_path = collapsed.active_session_path.clone();
    expanded.active_session_name = collapsed.active_session_name.clone();
    expanded.recent_sessions = collapsed.recent_sessions.clone();
    assert!(side_size(&collapsed).1 < side_size(&expanded).1);

    for hits in [static_side_hits(&collapsed), rendered_side_hits(&collapsed)] {
        assert_expand_hits(&hits, &sections);
        assert!(
            !hits
                .iter()
                .any(|hit| matches!(hit.kind, HitKind::PickColor { .. }))
        );
        assert!(!hits.iter().any(|hit| matches!(
            hit.event,
            ToolbarEvent::SetThickness(_)
                | ToolbarEvent::SetFontSize(_)
                | ToolbarEvent::SetFont(_)
                | ToolbarEvent::OpenSession
                | ToolbarEvent::OpenRecentSession(_)
                | ToolbarEvent::ToggleContextAwareUi(_)
                | ToolbarEvent::ToggleCustomSection(_)
        )));
    }
}

#[test]
fn drawer_view_sections_collapsed_keep_only_headers() {
    let mut state = create_test_input_state();
    state.toolbar_drawer_open = true;
    state.toolbar_drawer_tab = ToolbarDrawerTab::View;
    let sections = [ToolbarSideSection::Boards, ToolbarSideSection::Pages];
    state
        .toolbar_collapsed_side_sections
        .extend(sections.iter().copied());
    let collapsed = snapshot_from_state(&state);

    for hits in [static_side_hits(&collapsed), rendered_side_hits(&collapsed)] {
        assert_expand_hits(&hits, &sections);
        assert!(
            !hits
                .iter()
                .any(|hit| matches!(hit.event, ToolbarEvent::PageNew | ToolbarEvent::BoardNew))
        );
    }
}

#[test]
fn tool_specific_side_sections_collapsed_keep_only_headers() {
    let mut state = create_test_input_state();
    state
        .toolbar_collapsed_side_sections
        .insert(ToolbarSideSection::EraserMode);
    state.set_tool_override(Some(Tool::Eraser));
    let collapsed = snapshot_from_state(&state);
    for hits in [static_side_hits(&collapsed), rendered_side_hits(&collapsed)] {
        assert_expand_hits(&hits, &[ToolbarSideSection::EraserMode]);
        assert!(
            !hits
                .iter()
                .any(|hit| matches!(hit.event, ToolbarEvent::SetEraserMode(_)))
        );
    }

    let mut state = create_test_input_state();
    state
        .toolbar_collapsed_side_sections
        .insert(ToolbarSideSection::PolygonSides);
    state.set_tool_override(Some(Tool::RegularPolygon));
    let collapsed = snapshot_from_state(&state);
    for hits in [static_side_hits(&collapsed), rendered_side_hits(&collapsed)] {
        assert_expand_hits(&hits, &[ToolbarSideSection::PolygonSides]);
        assert!(
            !hits
                .iter()
                .any(|hit| matches!(hit.event, ToolbarEvent::NudgePolygonSides(_)))
        );
    }

    let mut state = create_test_input_state();
    state
        .toolbar_collapsed_side_sections
        .insert(ToolbarSideSection::ArrowLabels);
    state.set_tool_override(Some(Tool::Arrow));
    let collapsed = snapshot_from_state(&state);
    for hits in [static_side_hits(&collapsed), rendered_side_hits(&collapsed)] {
        assert_expand_hits(&hits, &[ToolbarSideSection::ArrowLabels]);
        assert!(
            !hits
                .iter()
                .any(|hit| matches!(hit.event, ToolbarEvent::ToggleArrowLabels(_)))
        );
    }

    let mut state = create_test_input_state();
    state
        .toolbar_collapsed_side_sections
        .insert(ToolbarSideSection::MarkerOpacity);
    state.set_tool_override(Some(Tool::Marker));
    let collapsed = snapshot_from_state(&state);
    for hits in [static_side_hits(&collapsed), rendered_side_hits(&collapsed)] {
        assert_expand_hits(&hits, &[ToolbarSideSection::MarkerOpacity]);
        assert!(
            !hits
                .iter()
                .any(|hit| matches!(hit.event, ToolbarEvent::NudgeMarkerOpacity(_)))
        );
    }

    let mut state = create_test_input_state();
    state
        .toolbar_collapsed_side_sections
        .insert(ToolbarSideSection::StepMarkers);
    state.set_tool_override(Some(Tool::StepMarker));
    let collapsed = snapshot_from_state(&state);
    for hits in [static_side_hits(&collapsed), rendered_side_hits(&collapsed)] {
        assert_expand_hits(&hits, &[ToolbarSideSection::StepMarkers]);
        assert!(
            !hits
                .iter()
                .any(|hit| matches!(hit.event, ToolbarEvent::ResetStepMarkerCounter))
        );
    }
}
