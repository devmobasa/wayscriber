use super::*;
use crate::draw::FontDescriptor;
use crate::input::InputState;
use crate::input::state::TopMenuState;
use crate::ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot};

fn create_test_input_state() -> InputState {
    crate::input::state::test_support::TestInputStateBuilder::default()
        .thickness(3.0)
        .eraser_size(12.0)
        .font_descriptor(FontDescriptor {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        })
        .custom_section_enabled(false)
        .build()
}

fn snapshot_from_state(state: &InputState) -> ToolbarSnapshot {
    ToolbarSnapshot::from_input_with_bindings(state, ToolbarBindingHints::default())
}

#[test]
fn top_size_respects_icon_mode() {
    let mut state = create_test_input_state();
    state.set_toolbar_use_icons(true);
    let snapshot = snapshot_from_state(&state);
    // Width includes the island gaps/padding of the four-pill band (tools,
    // presets, history, chrome): the presets island replaced the retired
    // colors group (M7-C1/C2), and the chrome island carries the layout
    // cycle and About alongside pin and minimize. Height adds the contextual
    // style pill under the 58px island band (6px gap + 40px pill) while a
    // drawing tool is active.
    assert_eq!(
        top_size(&crate::ui_text::UiTextEngine::default(), &snapshot),
        (1227, 104)
    );

    state.set_toolbar_use_icons(false);
    let snapshot = snapshot_from_state(&state);
    assert_eq!(
        top_size(&crate::ui_text::UiTextEngine::default(), &snapshot).1,
        106
    );
}

#[test]
fn narrow_viewports_drop_presets_then_overflow_items() {
    let mut state = create_test_input_state();
    state.set_toolbar_use_icons(true);
    let mut snapshot = snapshot_from_state(&state);

    // Unconstrained: presets shown, the pill's eight swatches available,
    // nothing dropped into the overflow.
    let full = crate::backend::wayland::toolbar::view::top::plan_top_strip(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
    );
    assert!(!full.drop_presets);
    assert_eq!(full.swatch_count, 8);
    assert!(full.dropped_tools.is_empty() && full.dropped_utilities.is_empty());
    let full_width = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot).0;

    // Slightly narrow: the non-essential presets island yields first, before
    // any tool or utility is dropped (M7-C2).
    snapshot.top_viewport_max = Some(full_width as f64 - 60.0);
    let degraded = crate::backend::wayland::toolbar::view::top::plan_top_strip(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
    );
    assert!(degraded.drop_presets);
    assert!(degraded.dropped_tools.is_empty() && degraded.dropped_utilities.is_empty());
    assert!(
        top_size(&crate::ui_text::UiTextEngine::default(), &snapshot).0 as f64
            <= full_width as f64 - 60.0
    );

    // Very narrow: droppable items move into the overflow menu; the protected
    // core (Pen, Eraser, Undo/Redo, Clear) stays. Colors and presets have
    // already left the strip.
    snapshot.top_viewport_max = Some(700.0);
    let tight = crate::backend::wayland::toolbar::view::top::plan_top_strip(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
    );
    assert!(tight.drop_presets);
    assert!(!tight.dropped_utilities.is_empty());
    assert!(top_size(&crate::ui_text::UiTextEngine::default(), &snapshot).0 as f64 <= 700.0);
    let (w, h) = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    let tree = crate::backend::wayland::toolbar::view::top::build_top_view(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
        w as f64,
        h as f64,
    );
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
    let (w, h) = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    let tree = crate::backend::wayland::toolbar::view::top::build_top_view(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
        w as f64,
        h as f64,
    );
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
    state.set_toolbar_use_icons(true);
    let mut items = state.toolbar_items().clone();
    items.set_hidden(crate::config::toolbar_item_ids::TOP_UTILITY_HIGHLIGHT, true);
    items.set_hidden(crate::config::toolbar_item_ids::TOP_CHROME_OVERFLOW, true);
    state.test_set_toolbar_items(items);
    let mut snapshot = snapshot_from_state(&state);
    snapshot.top_viewport_max = Some(700.0);

    let plan = crate::backend::wayland::toolbar::view::top::plan_top_strip(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
    );
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
    let (w, h) = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    let tree = crate::backend::wayland::toolbar::view::top::build_top_view(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
        w as f64,
        h as f64,
    );
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
        state.set_toolbar_use_icons(use_icons);
        let mut snapshot = snapshot_from_state(&state);
        snapshot.top_viewport_max = Some(480.0);
        let (width, _) = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
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
    state.set_toolbar_use_icons(false);
    let mut snapshot = snapshot_from_state(&state);
    for budget in [376, 320, 300] {
        snapshot.top_viewport_max = Some(budget as f64);
        assert!(
            top_size(&crate::ui_text::UiTextEngine::default(), &snapshot).0 <= budget,
            "planned width {} exceeds {budget}",
            top_size(&crate::ui_text::UiTextEngine::default(), &snapshot).0
        );
    }
}

#[test]
fn reordered_overflow_items_keep_visual_order() {
    let mut state = create_test_input_state();
    state.set_toolbar_use_icons(true);
    let mut items = state.toolbar_items().clone();
    items.set_hidden(
        crate::config::toolbar_item_ids::TOP_UTILITY_SCREENSHOT,
        false,
    );
    items.move_item_to_index(
        crate::config::ToolbarItemOrderGroup::TopControls,
        crate::config::toolbar_item_ids::TOP_UTILITY_HIGHLIGHT,
        0,
    );
    items.move_item_to_index(
        crate::config::ToolbarItemOrderGroup::TopControls,
        crate::config::toolbar_item_ids::TOP_UTILITY_SCREENSHOT,
        1,
    );
    state.test_set_toolbar_items(items);
    let mut snapshot = snapshot_from_state(&state);
    snapshot.top_viewport_max = Some(560.0);

    let plan = crate::backend::wayland::toolbar::view::top::plan_top_strip(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
    );
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
    state.set_toolbar_use_icons(true);
    state.set_tool_override(Some(crate::input::Tool::RegularPolygon));
    state.test_set_toolbar_menu_state(
        TopMenuState::ShapePicker,
        state.toolbar_top_popover_scroll(),
    );
    let snapshot = snapshot_from_state(&state);
    assert!(snapshot.shape_picker_open);

    let (w, h) = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    assert!(h > 58, "open popover grows the surface: {h}");
    let tree = crate::backend::wayland::toolbar::view::top::build_top_view(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
        w as f64,
        h as f64,
    );

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
    state.test_set_toolbar_menu_state(TopMenuState::Closed, state.toolbar_top_popover_scroll());
    let snapshot = snapshot_from_state(&state);
    let (w, h) = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    assert_eq!(h, 104);
    let tree = crate::backend::wayland::toolbar::view::top::build_top_view(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
        w as f64,
        h as f64,
    );
    assert!(tree.node_by_id(&"top.utility.fill".into()).is_none());
}

#[test]
fn highlight_ring_row_grows_the_bar_only_while_active() {
    let mut state = create_test_input_state();
    state.set_toolbar_use_icons(true);
    let snapshot = snapshot_from_state(&state);
    // Band (58) plus the contextual style pill (6 + 40) — no ring lane yet.
    assert_eq!(
        top_size(&crate::ui_text::UiTextEngine::default(), &snapshot).1,
        104
    );

    state.set_highlight_tool(true);
    let snapshot = snapshot_from_state(&state);
    assert!(snapshot.highlight_tool_active);
    let (w, h) = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    // The highlight tool has no style properties, so the pill yields and
    // only the ring lane grows the 58px band.
    assert!(h > 58, "ring row grows the bar: {h}");
    let tree = crate::backend::wayland::toolbar::view::top::build_top_view(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
        w as f64,
        h as f64,
    );
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
    state.set_toolbar_use_icons(true);
    state.set_highlight_tool(true);
    state.test_set_toolbar_menu_state(
        TopMenuState::ShapePicker,
        state.toolbar_top_popover_scroll(),
    );
    let snapshot = snapshot_from_state(&state);
    let (w, h) = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    let tree = crate::backend::wayland::toolbar::view::top::build_top_view(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
        w as f64,
        h as f64,
    );
    let ring = tree
        .node_by_id(&"top.utility.highlight-ring".into())
        .expect("ring row");
    let shapes = tree
        .node_by_id(&"top.shapes.panel".into())
        .expect("shapes panel");
    assert!(shapes.rect.1 >= ring.rect.1 + ring.rect.3);

    state.test_set_toolbar_menu_state(
        TopMenuState::TopOverflow,
        state.toolbar_top_popover_scroll(),
    );
    let mut items = state.toolbar_items().clone();
    items.set_hidden(
        crate::config::toolbar_item_ids::TOP_UTILITY_SCREENSHOT,
        false,
    );
    state.test_set_toolbar_items(items);
    let mut snapshot = snapshot_from_state(&state);
    snapshot.top_viewport_max = (480..=1120).rev().find_map(|budget| {
        snapshot.top_viewport_max = Some(budget as f64);
        let plan = crate::backend::wayland::toolbar::view::top::plan_top_strip(
            &crate::ui_text::UiTextEngine::default(),
            &snapshot,
        );
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
    let (w, h) = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    let tree = crate::backend::wayland::toolbar::view::top::build_top_view(
        &crate::ui_text::UiTextEngine::default(),
        &snapshot,
        w as f64,
        h as f64,
    );
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
fn top_size_scales_with_toolbar_scale() {
    let mut state = create_test_input_state();
    state.set_toolbar_use_icons(true);
    state.test_set_toolbar_appearance(state.toolbar_use_icons(), 1.0);
    let snapshot = snapshot_from_state(&state);
    let base_size = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);

    // Scale 1.5x should increase size proportionally
    state.test_set_toolbar_appearance(state.toolbar_use_icons(), 1.5);
    let snapshot = snapshot_from_state(&state);
    let scaled_size = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
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
    state.test_set_toolbar_appearance(state.toolbar_use_icons(), 0.75);
    let snapshot = snapshot_from_state(&state);
    let small_size = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
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
    state.set_toolbar_use_icons(true);
    state.test_set_toolbar_appearance(state.toolbar_use_icons(), 1.0);
    let snapshot = snapshot_from_state(&state);
    let base_size = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);

    // NaN should fall back to 1.0
    state.test_set_toolbar_appearance(state.toolbar_use_icons(), f64::NAN);
    let snapshot = snapshot_from_state(&state);
    let nan_size = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    assert_eq!(nan_size, base_size, "NaN scale should fall back to 1.0");

    // Infinity should fall back to 1.0
    state.test_set_toolbar_appearance(state.toolbar_use_icons(), f64::INFINITY);
    let snapshot = snapshot_from_state(&state);
    let inf_size = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    assert_eq!(
        inf_size, base_size,
        "Infinity scale should fall back to 1.0"
    );

    // Negative infinity should fall back to 1.0
    state.test_set_toolbar_appearance(state.toolbar_use_icons(), f64::NEG_INFINITY);
    let snapshot = snapshot_from_state(&state);
    let neg_inf_size = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    assert_eq!(
        neg_inf_size, base_size,
        "Neg infinity scale should fall back to 1.0"
    );
}

#[test]
fn scale_size_clamps_extreme_values() {
    let mut state = create_test_input_state();
    state.set_toolbar_use_icons(true);

    // Test upper bound clamping (max 3.0)
    state.test_set_toolbar_appearance(state.toolbar_use_icons(), 10.0);
    let snapshot = snapshot_from_state(&state);
    let huge_size = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);

    state.test_set_toolbar_appearance(state.toolbar_use_icons(), 3.0);
    let snapshot = snapshot_from_state(&state);
    let max_size = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    assert_eq!(huge_size, max_size, "Scale > 3.0 should clamp to 3.0");

    // Test lower bound clamping (min 0.5)
    state.test_set_toolbar_appearance(state.toolbar_use_icons(), 0.1);
    let snapshot = snapshot_from_state(&state);
    let tiny_size = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);

    state.test_set_toolbar_appearance(state.toolbar_use_icons(), 0.5);
    let snapshot = snapshot_from_state(&state);
    let min_size = top_size(&crate::ui_text::UiTextEngine::default(), &snapshot);
    assert_eq!(tiny_size, min_size, "Scale < 0.5 should clamp to 0.5");
}
