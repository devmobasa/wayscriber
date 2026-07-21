use super::super::events::HitKind;
use super::super::hit::HitRegion;
use super::*;
use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
use crate::draw::{Color, FontDescriptor};
use crate::input::{ClickHighlightSettings, EraserMode, InputState};
use crate::ui::toolbar::model::{
    ToolbarActivation, ToolbarSessionModel, ToolbarSettingsModel, ToolbarSliderSpec,
};
use crate::ui::toolbar::{
    SessionRecentSnapshot, SidePane, ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot,
};

mod collapsible;

/// Render-time hit oracle: the side palette has no static hit builder, so
/// hits come from drawing the palette at its computed size.
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

fn create_test_input_state() -> InputState {
    let keybindings = KeybindingsConfig::default();
    let action_map = keybindings.build_action_map().unwrap();
    let action_bindings = keybindings.build_action_bindings().unwrap();

    let mut state = InputState::with_defaults(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        3.0,
        12.0,
        EraserMode::Brush,
        0.32,
        false,
        32.0,
        FontDescriptor {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        },
        false,
        20.0,
        30.0,
        false,
        true,
        BoardsConfig::default(),
        action_map,
        usize::MAX,
        ClickHighlightSettings::disabled(),
        0,
        0,
        false,
        0,
        0,
        5,
        5,
        PresenterModeConfig::default(),
    );
    state.set_action_bindings(action_bindings);
    state
}

fn snapshot_from_state(state: &InputState) -> ToolbarSnapshot {
    ToolbarSnapshot::from_input_with_bindings(state, ToolbarBindingHints::default())
}

fn event_name(event: &ToolbarEvent) -> String {
    format!("{event:?}")
}

fn activation_event_name(activation: &ToolbarActivation) -> String {
    event_name(&activation.compatibility_event())
}

#[test]
fn top_size_respects_icon_mode() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    let snapshot = snapshot_from_state(&state);
    // Width includes the island gaps/padding of the four-pill band (tools,
    // presets, history, chrome): the presets island replaced the retired
    // colors group (M7-C1/C2). Height adds the contextual style pill under
    // the 58px island band (6px gap + 40px pill) while a drawing tool is
    // active.
    assert_eq!(top_size(&snapshot), (1167, 104));

    state.toolbar_use_icons = false;
    let snapshot = snapshot_from_state(&state);
    assert_eq!(top_size(&snapshot).1, 106);
}

#[test]
fn narrow_viewports_drop_presets_then_overflow_items() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    let mut snapshot = snapshot_from_state(&state);

    // Unconstrained: presets shown, the pill's eight swatches available,
    // nothing dropped into the overflow.
    let full = crate::backend::wayland::toolbar::view::top::plan_top_strip(&snapshot);
    assert!(!full.drop_presets);
    assert_eq!(full.swatch_count, 8);
    assert!(full.dropped_tools.is_empty() && full.dropped_utilities.is_empty());
    let full_width = top_size(&snapshot).0;

    // Slightly narrow: the non-essential presets island yields first, before
    // any tool or utility is dropped (M7-C2).
    snapshot.top_viewport_max = Some(full_width as f64 - 60.0);
    let degraded = crate::backend::wayland::toolbar::view::top::plan_top_strip(&snapshot);
    assert!(degraded.drop_presets);
    assert!(degraded.dropped_tools.is_empty() && degraded.dropped_utilities.is_empty());
    assert!(top_size(&snapshot).0 as f64 <= full_width as f64 - 60.0);

    // Very narrow: droppable items move into the overflow menu; the protected
    // core (Pen, Eraser, Undo/Redo, Clear) stays. Colors and presets have
    // already left the strip.
    snapshot.top_viewport_max = Some(700.0);
    let tight = crate::backend::wayland::toolbar::view::top::plan_top_strip(&snapshot);
    assert!(tight.drop_presets);
    assert!(!tight.dropped_utilities.is_empty());
    assert!(top_size(&snapshot).0 as f64 <= 700.0);
    let (w, h) = top_size(&snapshot);
    let tree =
        crate::backend::wayland::toolbar::view::top::build_top_view(&snapshot, w as f64, h as f64);
    for id in [
        "top.tool.pen",
        "top.tool.eraser",
        "top.utility.undo",
        "top.chrome.overflow",
    ] {
        assert!(
            tree.node_by_id(&id.into()).is_some(),
            "{id} must survive width pressure"
        );
    }
    // No preset slots or color chip remain in the strip under pressure.
    assert!(tree.node_by_id(&"top.preset.0".into()).is_none());
    assert!(tree.node_by_id(&"top.group.quick-colors".into()).is_none());

    // Opening the overflow reveals Clear first, then the dropped items.
    snapshot.top_overflow_open = true;
    let (w, h) = top_size(&snapshot);
    let tree =
        crate::backend::wayland::toolbar::view::top::build_top_view(&snapshot, w as f64, h as f64);
    let overflow_ids: Vec<&str> = tree
        .nodes()
        .iter()
        .map(|node| node.id.as_str())
        .filter(|id| id.starts_with("top.overflow."))
        .collect();
    assert!(
        overflow_ids.len() > 2,
        "panel + Clear + dropped items: {overflow_ids:?}"
    );
    assert_eq!(
        overflow_ids
            .iter()
            .find(|id| **id != "top.overflow.panel")
            .copied(),
        Some("top.overflow.top.utility.clear-canvas"),
        "Clear leads the overflow menu"
    );
}

#[test]
fn overflow_contains_only_visible_items_and_is_structural() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    state
        .toolbar_items
        .set_hidden(crate::config::toolbar_item_ids::TOP_UTILITY_HIGHLIGHT, true);
    state
        .toolbar_items
        .set_hidden(crate::config::toolbar_item_ids::TOP_CHROME_OVERFLOW, true);
    state.resolved_toolbar_items = state.toolbar_items.resolved();
    let mut snapshot = snapshot_from_state(&state);
    snapshot.top_viewport_max = Some(700.0);

    let plan = crate::backend::wayland::toolbar::view::top::plan_top_strip(&snapshot);
    assert!(
        !plan.dropped_tools.is_empty() || !plan.dropped_utilities.is_empty(),
        "the 700px budget must force items into the overflow: {plan:?}"
    );
    assert!(
        !plan
            .dropped_utilities
            .contains(&crate::ui::toolbar::model::TopUtilityButton::Screenshot)
    );
    assert!(
        !plan
            .dropped_utilities
            .contains(&crate::ui::toolbar::model::TopUtilityButton::Highlight)
    );

    snapshot.top_overflow_open = true;
    let (w, h) = top_size(&snapshot);
    let tree =
        crate::backend::wayland::toolbar::view::top::build_top_view(&snapshot, w as f64, h as f64);
    assert!(tree.node_by_id(&"top.chrome.overflow".into()).is_some());
    assert!(
        tree.node_by_id(&"top.overflow.top.utility.screenshot".into())
            .is_none()
    );
    assert!(
        tree.node_by_id(&"top.overflow.top.utility.highlight".into())
            .is_none()
    );
}

#[test]
fn top_strip_fits_480_pixels_in_icon_and_text_modes() {
    for use_icons in [true, false] {
        let mut state = create_test_input_state();
        state.toolbar_use_icons = use_icons;
        let mut snapshot = snapshot_from_state(&state);
        snapshot.top_viewport_max = Some(480.0);
        let (width, _) = top_size(&snapshot);
        assert!(
            width <= 480,
            "{} mode planned width {width} exceeds 480",
            if use_icons { "icon" } else { "text" }
        );
    }
}

#[test]
fn compact_top_strip_respects_budget_without_the_old_floor() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = false;
    let mut snapshot = snapshot_from_state(&state);
    for budget in [376, 320, 300] {
        snapshot.top_viewport_max = Some(budget as f64);
        assert!(
            top_size(&snapshot).0 <= budget,
            "planned width {} exceeds {budget}",
            top_size(&snapshot).0
        );
    }
}

#[test]
fn reordered_overflow_items_keep_visual_order() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    state.toolbar_items.set_hidden(
        crate::config::toolbar_item_ids::TOP_UTILITY_SCREENSHOT,
        false,
    );
    state.toolbar_items.move_item_to_index(
        crate::config::ToolbarItemOrderGroup::TopControls,
        crate::config::toolbar_item_ids::TOP_UTILITY_HIGHLIGHT,
        0,
    );
    state.toolbar_items.move_item_to_index(
        crate::config::ToolbarItemOrderGroup::TopControls,
        crate::config::toolbar_item_ids::TOP_UTILITY_SCREENSHOT,
        1,
    );
    state.resolved_toolbar_items = state.toolbar_items.resolved();
    let mut snapshot = snapshot_from_state(&state);
    snapshot.top_viewport_max = Some(560.0);

    let plan = crate::backend::wayland::toolbar::view::top::plan_top_strip(&snapshot);
    let highlight = plan
        .dropped_utilities
        .iter()
        .position(|item| *item == crate::ui::toolbar::model::TopUtilityButton::Highlight)
        .expect("highlight dropped");
    let screenshot = plan
        .dropped_utilities
        .iter()
        .position(|item| *item == crate::ui::toolbar::model::TopUtilityButton::Screenshot)
        .expect("screenshot dropped");
    assert!(highlight < screenshot);
}

#[test]
fn shapes_popover_hosts_the_relocated_tool_options() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    state.set_tool_override(Some(crate::input::Tool::RegularPolygon));
    state.toolbar_shapes_expanded = true;
    let snapshot = snapshot_from_state(&state);
    assert!(snapshot.shape_picker_open);

    let (w, h) = top_size(&snapshot);
    assert!(h > 58, "open popover grows the surface: {h}");
    let tree =
        crate::backend::wayland::toolbar::view::top::build_top_view(&snapshot, w as f64, h as f64);

    // The grid renders inside popover chrome with a caret.
    let panel = tree
        .node_by_id(&"top.shapes.panel".into())
        .expect("shapes popover panel");
    assert!(matches!(
        panel.kind,
        crate::backend::wayland::toolbar::view::WidgetKind::Popover { .. }
    ));

    // The old mini-checkbox lane's controls live inside the popover now.
    let fill = tree
        .node_by_id(&"top.utility.fill".into())
        .expect("fill option row");
    let inside = |rect: (f64, f64, f64, f64)| {
        rect.0 >= panel.rect.0
            && rect.1 >= panel.rect.1
            && rect.0 + rect.2 <= panel.rect.0 + panel.rect.2 + 0.5
            && rect.1 + rect.3 <= panel.rect.1 + panel.rect.3 + 0.5
    };
    assert!(inside(fill.rect), "fill row sits inside the popover");
    assert!(matches!(
        fill.interact.as_ref().unwrap().event,
        ToolbarEvent::ToggleFill(_)
    ));
    let minus = tree
        .node_by_id(&"top.options.sides-minus".into())
        .expect("sides minus");
    assert!(inside(minus.rect));
    assert!(matches!(
        minus.interact.as_ref().unwrap().event,
        ToolbarEvent::NudgePolygonSides(-1)
    ));

    // With the popover closed the bar keeps only the island band plus the
    // contextual style pill — the permanently reserved mini-checkbox lane
    // is gone. The pill carries its own Fill toggle for shape tools.
    state.toolbar_shapes_expanded = false;
    let snapshot = snapshot_from_state(&state);
    let (w, h) = top_size(&snapshot);
    assert_eq!(h, 104);
    let tree =
        crate::backend::wayland::toolbar::view::top::build_top_view(&snapshot, w as f64, h as f64);
    assert!(tree.node_by_id(&"top.utility.fill".into()).is_none());
}

#[test]
fn highlight_ring_row_grows_the_bar_only_while_active() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    let snapshot = snapshot_from_state(&state);
    // Band (58) plus the contextual style pill (6 + 40) — no ring lane yet.
    assert_eq!(top_size(&snapshot).1, 104);

    state.set_highlight_tool(true);
    let snapshot = snapshot_from_state(&state);
    assert!(snapshot.highlight_tool_active);
    let (w, h) = top_size(&snapshot);
    // The highlight tool has no style properties, so the pill yields and
    // only the ring lane grows the 58px band.
    assert!(h > 58, "ring row grows the bar: {h}");
    let tree =
        crate::backend::wayland::toolbar::view::top::build_top_view(&snapshot, w as f64, h as f64);
    assert!(
        tree.node_by_id(&"top.island.style".into()).is_none(),
        "no style pill while the highlight tool is active"
    );
    let ring = tree
        .node_by_id(&"top.utility.highlight-ring".into())
        .expect("ring checkbox");
    assert!(matches!(
        ring.interact.as_ref().unwrap().event,
        ToolbarEvent::ToggleHighlightToolRing(_)
    ));
}

#[test]
fn highlight_ring_and_top_popovers_use_separate_lanes() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    state.set_highlight_tool(true);
    state.toolbar_shapes_expanded = true;
    let snapshot = snapshot_from_state(&state);
    let (w, h) = top_size(&snapshot);
    let tree =
        crate::backend::wayland::toolbar::view::top::build_top_view(&snapshot, w as f64, h as f64);
    let ring = tree
        .node_by_id(&"top.utility.highlight-ring".into())
        .expect("ring row");
    let shapes = tree
        .node_by_id(&"top.shapes.panel".into())
        .expect("shapes panel");
    assert!(shapes.rect.1 >= ring.rect.1 + ring.rect.3);

    state.toolbar_shapes_expanded = false;
    state.toolbar_top_overflow_open = true;
    state.toolbar_items.set_hidden(
        crate::config::toolbar_item_ids::TOP_UTILITY_SCREENSHOT,
        false,
    );
    state.resolved_toolbar_items = state.toolbar_items.resolved();
    let mut snapshot = snapshot_from_state(&state);
    snapshot.top_viewport_max = (480..=1120).rev().find_map(|budget| {
        snapshot.top_viewport_max = Some(budget as f64);
        let plan = crate::backend::wayland::toolbar::view::top::plan_top_strip(&snapshot);
        let has_dropped_items =
            !plan.dropped_tools.is_empty() || !plan.dropped_utilities.is_empty();
        (has_dropped_items
            && !plan
                .dropped_utilities
                .contains(&crate::ui::toolbar::model::TopUtilityButton::Highlight))
        .then_some(budget as f64)
    });
    assert!(
        snapshot.top_viewport_max.is_some(),
        "overflow budget retaining highlight"
    );
    let (w, h) = top_size(&snapshot);
    let tree =
        crate::backend::wayland::toolbar::view::top::build_top_view(&snapshot, w as f64, h as f64);
    let ring = tree
        .node_by_id(&"top.utility.highlight-ring".into())
        .expect("ring row");
    let overflow = tree
        .node_by_id(&"top.overflow.panel".into())
        .expect("overflow panel");
    assert!(
        overflow.rect.1 >= ring.rect.1 + ring.rect.3,
        "ring={:?}, overflow={:?}, surface=({w}, {h}), budget={:?}",
        ring.rect,
        overflow.rect,
        snapshot.top_viewport_max
    );
}

#[test]
fn scrolled_side_hits_are_clipped_at_both_viewport_edges() {
    let mut state = create_test_input_state();
    state.toolbar_side_pane = SidePane::Settings;
    state.toolbar_side_scroll[SidePane::Settings.index()] = 17.3;
    let mut snapshot = snapshot_from_state(&state);
    snapshot.side_viewport_max = Some(240.0);
    snapshot.side_scroll = 17.3;
    let (_, height) = side_size(&snapshot);
    let content_top = ToolbarLayoutSpec::new(&snapshot).side_content_start_y();
    let hits = rendered_side_hits(&snapshot);
    let content_hits = &hits[8..];

    assert!(!content_hits.is_empty());
    assert!(content_hits.iter().all(|hit| {
        hit.rect.1 >= content_top - f64::EPSILON
            && hit.rect.1 + hit.rect.3 <= height as f64 + f64::EPSILON
    }));
    assert!(content_hits.iter().any(|hit| {
        (hit.rect.1 - content_top).abs() < 0.01
            || (hit.rect.1 + hit.rect.3 - height as f64).abs() < 0.01
    }));
}

#[test]
fn minimized_side_palette_is_a_single_restore_tab() {
    let mut state = create_test_input_state();
    state.toolbar_side_minimized = true;
    let snapshot = snapshot_from_state(&state);

    assert_eq!(side_size(&snapshot), (24, 64));

    let hits = rendered_side_hits(&snapshot);
    assert_eq!(hits.len(), 1, "one restore hit only: {hits:?}");
    assert!(matches!(
        hits[0].event,
        ToolbarEvent::SetSideMinimized(false)
    ));
}

#[test]
fn side_color_picker_offers_sat_val_area_and_hue_bar() {
    let count_swatches = |hits: &[HitRegion]| {
        hits.iter()
            .filter(|hit| matches!(hit.event, ToolbarEvent::SetQuickColor { .. }))
            .count()
    };

    let mut state = create_test_input_state();
    state.show_more_colors = false;
    let snapshot = snapshot_from_state(&state);
    let compact_hits = rendered_side_hits(&snapshot);
    let sv = compact_hits
        .iter()
        .find(|hit| matches!(hit.kind, HitKind::PickSatVal { .. }))
        .expect("sat/val area hit");
    let hue = compact_hits
        .iter()
        .find(|hit| matches!(hit.kind, HitKind::PickHue { .. }))
        .expect("hue bar hit");
    assert_eq!(sv.rect.3, ToolbarLayoutSpec::SIDE_COLOR_SV_HEIGHT);
    assert_eq!(hue.rect.3, ToolbarLayoutSpec::SIDE_COLOR_HUE_HEIGHT);
    assert!(
        hue.rect.1 > sv.rect.1 + sv.rect.3,
        "hue bar sits below the sat/val area"
    );

    state.show_more_colors = true;
    let snapshot = snapshot_from_state(&state);
    let expanded_hits = rendered_side_hits(&snapshot);
    assert!(
        count_swatches(&expanded_hits) > count_swatches(&compact_hits),
        "extended palette should add swatch hits"
    );
}

#[test]
fn canvas_pane_right_aligns_destructive_buttons() {
    let mut state = create_test_input_state();
    state.toolbar_side_pane = SidePane::Canvas;
    state.toolbar_use_icons = true;
    let snapshot = snapshot_from_state(&state);
    let hits = rendered_side_hits(&snapshot);

    let right_edge = ToolbarLayoutSpec::SIDE_START_X
        + (ToolbarLayoutSpec::SIDE_WIDTH as f64 - ToolbarLayoutSpec::SIDE_CONTENT_PADDING_X);
    for (event_name, matcher) in [
        (
            "ClearCanvas",
            &(|hit: &HitRegion| matches!(hit.event, ToolbarEvent::ClearCanvas { .. }))
                as &dyn Fn(&HitRegion) -> bool,
        ),
        ("BoardDelete", &|hit: &HitRegion| {
            matches!(hit.event, ToolbarEvent::BoardDelete)
        }),
        ("PageDelete", &|hit: &HitRegion| {
            matches!(hit.event, ToolbarEvent::PageDelete)
        }),
    ] {
        let hit = hits
            .iter()
            .find(|hit| matcher(hit))
            .unwrap_or_else(|| panic!("{event_name} hit"));
        assert!(
            (hit.rect.0 + hit.rect.2 - right_edge).abs() < 1.0,
            "{event_name} should be right-aligned"
        );
    }

    // The guard gap: the button before the delete does not abut it.
    let duplicate = hits
        .iter()
        .find(|hit| matches!(hit.event, ToolbarEvent::PageDuplicate))
        .expect("page duplicate hit");
    let delete = hits
        .iter()
        .find(|hit| matches!(hit.event, ToolbarEvent::PageDelete))
        .expect("page delete hit");
    assert!(
        duplicate.rect.0 + duplicate.rect.2 + ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP
            < delete.rect.0,
        "destructive delete should sit apart from its neighbors"
    );
}

#[test]
fn preset_slots_save_on_empty_click_and_clear_on_hover_badge() {
    let state = create_test_input_state();
    let mut snapshot = snapshot_from_state(&state);
    snapshot.presets[0] = Some(crate::ui::toolbar::PresetSlotSnapshot {
        name: None,
        tool: crate::input::Tool::Pen,
        color: Color {
            r: 1.0,
            g: 0.5,
            b: 0.0,
            a: 1.0,
        },
        size: 4.0,
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        font_size: None,
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        show_status_bar: None,
    });

    // Without hover: filled slot applies, empty slots save, and the old
    // per-slot micro action row is gone.
    let hits = rendered_side_hits(&snapshot);
    assert!(
        hits.iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::ApplyPreset(1)))
    );
    assert!(
        hits.iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::SavePreset(2)))
    );
    assert!(
        !hits
            .iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::SavePreset(1))),
        "filled slot should not save on click"
    );
    assert!(
        !hits
            .iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::ClearPreset(_))),
        "clear affordance only appears on hover"
    );

    // Hovering the filled slot reveals the clear badge, and its hit takes
    // precedence over the apply hit underneath.
    let apply_hit = hits
        .iter()
        .find(|hit| matches!(hit.event, ToolbarEvent::ApplyPreset(1)))
        .expect("apply hit");
    let hover = (
        apply_hit.rect.0 + apply_hit.rect.2 / 2.0,
        apply_hit.rect.1 + apply_hit.rect.3 / 2.0,
    );
    let (w, h) = side_size(&snapshot);
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let mut hovered_hits = Vec::new();
    crate::backend::wayland::toolbar::render_side_palette(
        &ctx,
        w as f64,
        h as f64,
        &snapshot,
        &mut hovered_hits,
        Some(hover),
        None,
    )
    .unwrap();
    let clear_index = hovered_hits
        .iter()
        .position(|hit| matches!(hit.event, ToolbarEvent::ClearPreset(1)))
        .expect("clear badge hit while hovered");
    let apply_index = hovered_hits
        .iter()
        .position(|hit| matches!(hit.event, ToolbarEvent::ApplyPreset(1)))
        .expect("apply hit while hovered");
    assert!(
        clear_index < apply_index,
        "clear badge must win first-match hit testing over apply"
    );
}

#[test]
fn side_header_leads_with_drag_handle_and_offers_all_panes() {
    let state = create_test_input_state();
    let snapshot = snapshot_from_state(&state);
    let hits = rendered_side_hits(&snapshot);

    assert!(matches!(hits[0].kind, HitKind::DragMoveSide));
    assert!(
        hits.iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::ToggleBoardPicker))
    );
    for pane in SidePane::ALL {
        assert!(
            hits.iter()
                .any(|hit| hit.event == ToolbarEvent::SetSidePane(pane)),
            "missing pane nav hit for {pane:?}"
        );
    }
}

#[test]
fn side_size_caps_height_at_viewport_max_and_reports_scroll_bounds() {
    let state = create_test_input_state();
    let mut snapshot = snapshot_from_state(&state);

    snapshot.side_viewport_max = None;
    let (_, natural_height) = side_size(&snapshot);
    let (natural, viewport) = side_scroll_bounds(&snapshot);
    assert_eq!(viewport, natural_height as f64);
    assert!(natural <= viewport);

    let cap = (natural_height as f64 / 2.0).floor();
    snapshot.side_viewport_max = Some(cap);
    let (_, capped_height) = side_size(&snapshot);
    assert_eq!(capped_height as f64, cap.ceil());
    let (natural, viewport) = side_scroll_bounds(&snapshot);
    assert!(
        natural - viewport > 0.0,
        "capped pane should report positive max scroll"
    );

    // The scrollbar hit carries the same max scroll for drag handling.
    let hits = rendered_side_hits(&snapshot);
    assert!(hits.iter().any(|hit| matches!(
        hit.kind,
        HitKind::DragScrollSide { max_scroll } if (max_scroll - (natural - viewport)).abs() < 1.0
    )));
}

#[test]
fn side_settings_pane_hits_include_model_controls() {
    let mut state = create_test_input_state();
    state.toolbar_side_pane = SidePane::Settings;
    state.show_settings_section = true;
    state.toolbar_layout_mode = crate::config::ToolbarLayoutMode::Regular;
    let snapshot = snapshot_from_state(&state);
    let model = ToolbarSettingsModel::from_snapshot(&snapshot).expect("settings model");
    let expected: Vec<_> = model
        .toggles()
        .iter()
        .map(|toggle| activation_event_name(&toggle.activation))
        .chain(
            model
                .buttons()
                .iter()
                .map(|button| event_name(&button.event)),
        )
        .collect();

    let hits = rendered_side_hits(&snapshot);
    let hit_events: Vec<_> = hits.iter().map(|hit| event_name(&hit.event)).collect();

    for expected_event in &expected {
        assert!(
            hit_events.contains(expected_event),
            "missing settings hit {expected_event}"
        );
    }
    assert!(hit_events.contains(&"ToggleContextAwareUi(false)".to_string()));
}

#[test]
fn wide_settings_toggles_span_the_full_content_width() {
    let mut state = create_test_input_state();
    state.toolbar_side_pane = SidePane::Settings;
    state.toolbar_layout_mode = crate::config::ToolbarLayoutMode::Regular;
    state.refresh_section_visibility();
    let snapshot = snapshot_from_state(&state);
    let hits = rendered_side_hits(&snapshot);

    let full_width =
        ToolbarLayoutSpec::SIDE_WIDTH as f64 - ToolbarLayoutSpec::SIDE_CONTENT_PADDING_X;
    let step = hits
        .iter()
        .find(|hit| matches!(hit.event, ToolbarEvent::ToggleStepSection(_)))
        .expect("multi-step toggle hit");
    assert!(
        (step.rect.2 - full_width).abs() < 1.0,
        "long label takes a full row: {}",
        step.rect.2
    );
    let advanced = hits
        .iter()
        .find(|hit| matches!(hit.event, ToolbarEvent::ToggleActionsAdvanced(_)))
        .expect("advanced actions toggle hit");
    assert!((advanced.rect.2 - full_width).abs() < 1.0);

    // Narrow toggles still pair up in half-width cells.
    let narrow = hits
        .iter()
        .find(|hit| matches!(hit.event, ToolbarEvent::ToggleContextAwareUi(_)))
        .expect("context toggle hit");
    assert!(narrow.rect.2 < full_width / 2.0 + 1.0);
}

#[test]
fn side_session_pane_hits_include_model_controls_and_recents() {
    let mut state = create_test_input_state();
    state.toolbar_side_pane = SidePane::Session;
    let mut snapshot = snapshot_from_state(&state);
    snapshot.active_session_path =
        Some(std::path::PathBuf::from("/tmp/current.wayscriber-session"));
    snapshot.active_session_name = Some("current.wayscriber-session".to_string());
    snapshot.recent_sessions = vec![SessionRecentSnapshot {
        display_name: "recent.wayscriber-session".to_string(),
        path: std::path::PathBuf::from("/tmp/recent.wayscriber-session"),
    }];
    let model = ToolbarSessionModel::from_snapshot(&snapshot).expect("session model");

    let hits = rendered_side_hits(&snapshot);
    let hit_events: Vec<_> = hits.iter().map(|hit| event_name(&hit.event)).collect();

    for button in &model.buttons {
        assert!(
            hit_events.contains(&event_name(&button.event)),
            "missing session button hit {:?}",
            button.event
        );
    }
    assert!(hit_events.iter().any(|event| {
        event.contains("OpenRecentSession") && event.contains("recent.wayscriber-session")
    }));
}

#[test]
fn side_session_overwrite_confirmation_hits_replace_action_buttons() {
    let mut state = create_test_input_state();
    state.toolbar_side_pane = SidePane::Session;
    let mut snapshot = snapshot_from_state(&state);
    let target = std::path::PathBuf::from("/tmp/existing.wayscriber-session");
    snapshot.active_session_path =
        Some(std::path::PathBuf::from("/tmp/current.wayscriber-session"));
    snapshot.active_session_name = Some("current.wayscriber-session".to_string());
    snapshot.pending_save_as_overwrite_path = Some(target.clone());

    let rendered_hits = rendered_side_hits(&snapshot);
    assert_session_overwrite_confirmation_hits(&rendered_hits, &target);
}

fn assert_session_overwrite_confirmation_hits(
    hits: &[crate::backend::wayland::toolbar::hit::HitRegion],
    target: &std::path::Path,
) {
    assert!(hits.iter().any(|hit| matches!(
        &hit.event,
        ToolbarEvent::SaveSessionAsConfirm(path) if path == target
    )));
    assert!(
        hits.iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::SaveSessionAsCancel))
    );
    assert!(
        !hits
            .iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::SaveSessionAs)),
        "pending overwrite prompt should replace the Save As action grid"
    );
}

#[test]
fn font_size_nudge_hits_use_slider_spec_step() {
    let mut state = create_test_input_state();
    state.show_text_controls = true;
    state.current_font_size = 32.0;
    let snapshot = snapshot_from_state(&state);
    let (w, h) = side_size(&snapshot);

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let mut hits = Vec::new();
    crate::backend::wayland::toolbar::render_side_palette(
        &ctx, w as f64, h as f64, &snapshot, &mut hits, None, None,
    )
    .unwrap();

    let step = ToolbarSliderSpec::FONT_SIZE.step.expect("font size step");
    assert!(hits.iter().any(|hit| matches!(
        hit.event,
        ToolbarEvent::NudgeFontSize(value) if (value + step).abs() < f64::EPSILON
    )));
    assert!(hits.iter().any(|hit| matches!(
        hit.event,
        ToolbarEvent::NudgeFontSize(value) if (value - step).abs() < f64::EPSILON
    )));
}

#[test]
fn top_size_scales_with_toolbar_scale() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    state.toolbar_scale = 1.0;
    let snapshot = snapshot_from_state(&state);
    let base_size = top_size(&snapshot);

    // Scale 1.5x should increase size proportionally
    state.toolbar_scale = 1.5;
    let snapshot = snapshot_from_state(&state);
    let scaled_size = top_size(&snapshot);
    assert_eq!(
        scaled_size.0,
        (base_size.0 as f64 * 1.5).ceil() as u32,
        "Width should scale by 1.5x"
    );
    assert_eq!(
        scaled_size.1,
        (base_size.1 as f64 * 1.5).ceil() as u32,
        "Height should scale by 1.5x"
    );

    // Scale 0.75x should decrease size
    state.toolbar_scale = 0.75;
    let snapshot = snapshot_from_state(&state);
    let small_size = top_size(&snapshot);
    assert!(
        small_size.0 < base_size.0,
        "Scaled down width should be smaller"
    );
    assert!(
        small_size.1 < base_size.1,
        "Scaled down height should be smaller"
    );
}

#[test]
fn scale_size_handles_non_finite_values() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    state.toolbar_scale = 1.0;
    let snapshot = snapshot_from_state(&state);
    let base_size = top_size(&snapshot);

    // NaN should fall back to 1.0
    state.toolbar_scale = f64::NAN;
    let snapshot = snapshot_from_state(&state);
    let nan_size = top_size(&snapshot);
    assert_eq!(nan_size, base_size, "NaN scale should fall back to 1.0");

    // Infinity should fall back to 1.0
    state.toolbar_scale = f64::INFINITY;
    let snapshot = snapshot_from_state(&state);
    let inf_size = top_size(&snapshot);
    assert_eq!(
        inf_size, base_size,
        "Infinity scale should fall back to 1.0"
    );

    // Negative infinity should fall back to 1.0
    state.toolbar_scale = f64::NEG_INFINITY;
    let snapshot = snapshot_from_state(&state);
    let neg_inf_size = top_size(&snapshot);
    assert_eq!(
        neg_inf_size, base_size,
        "Neg infinity scale should fall back to 1.0"
    );
}

#[test]
fn scale_size_clamps_extreme_values() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;

    // Test upper bound clamping (max 3.0)
    state.toolbar_scale = 10.0;
    let snapshot = snapshot_from_state(&state);
    let huge_size = top_size(&snapshot);

    state.toolbar_scale = 3.0;
    let snapshot = snapshot_from_state(&state);
    let max_size = top_size(&snapshot);
    assert_eq!(huge_size, max_size, "Scale > 3.0 should clamp to 3.0");

    // Test lower bound clamping (min 0.5)
    state.toolbar_scale = 0.1;
    let snapshot = snapshot_from_state(&state);
    let tiny_size = top_size(&snapshot);

    state.toolbar_scale = 0.5;
    let snapshot = snapshot_from_state(&state);
    let min_size = top_size(&snapshot);
    assert_eq!(tiny_size, min_size, "Scale < 0.5 should clamp to 0.5");
}
