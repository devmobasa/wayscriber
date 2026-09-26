use crate::backend::wayland::toolbar::layout::top_size;
use crate::backend::wayland::toolbar::view::{WidgetKind, WidgetTree};
use crate::draw::ArrowStyle;
use crate::input::Tool;
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot, model};
use crate::ui_text::{UiTextEngine, UiTextStyle};

use super::super::{build_top_view, top_extra_height, top_input_rects, top_natural_width};
use super::PREVIEW_GAP;

fn snapshot_for(style: ArrowStyle, open: bool) -> ToolbarSnapshot {
    let state = make_test_input_state();
    let mut snapshot =
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
    snapshot.active_tool = Tool::Arrow;
    snapshot.tool_override = None;
    snapshot.show_text_controls = false;
    snapshot.show_marker_opacity_section = false;
    snapshot.arrow_style = style;
    snapshot.arrow_style_menu_open = open;
    snapshot
}

fn tree_for(snapshot: &ToolbarSnapshot) -> WidgetTree {
    let engine = UiTextEngine::default();
    let (w, h) = top_size(&engine, snapshot);
    build_top_view(&engine, snapshot, w as f64, h as f64)
}

fn menu_ids(tree: &WidgetTree) -> Vec<String> {
    tree.nodes()
        .iter()
        .map(|node| node.id.as_str().to_string())
        .filter(|id| id.starts_with("top.arrow-style."))
        .collect()
}

#[test]
fn the_chip_draws_and_names_the_style_and_opens_the_menu() {
    let tree = tree_for(&snapshot_for(ArrowStyle::Pointy, false));

    let chip = tree
        .node_by_id(&"top.style.arrow-style".into())
        .expect("arrow style chip");
    let WidgetKind::TextButton { style, .. } = &chip.kind else {
        panic!("chip kind {:?}", chip.kind);
    };
    assert!(!style.active, "closed menu, resting chip");
    assert_eq!(chip.rect.2, model::ARROW_STYLE_CHIP_W);
    let interaction = chip.interact.as_ref().expect("chip clicks");
    assert_eq!(interaction.event, ToolbarEvent::ToggleArrowStyleMenu(true));
    assert_eq!(
        interaction.tooltip.as_deref(),
        Some("Arrow style: Pointy \u{2014} click to choose")
    );

    let glyph = tree
        .node_by_id(&"top.style.arrow-style.glyph".into())
        .expect("chip glyph");
    assert_eq!(
        glyph.kind,
        WidgetKind::ArrowStylePreview {
            style: ArrowStyle::Pointy
        }
    );
    assert!(glyph.interact.is_none(), "the chip takes the click");
    let label = tree
        .node_by_id(&"top.style.arrow-style.label".into())
        .expect("chip label");
    assert!(matches!(
        &label.kind,
        WidgetKind::Label(label) if label.text == "Pointy \u{25BE}"
    ));
    assert!(menu_ids(&tree).is_empty(), "the menu is closed");
}

/// The longest name still fits beside the glyph, with the painter's inset
/// before the chip's edge.
#[test]
fn every_chip_label_fits_its_slot() {
    let engine = UiTextEngine::default();
    let style = UiTextStyle {
        family: crate::ui::theme::toolbar::FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: 14.0,
    };
    let label_room = model::ARROW_STYLE_CHIP_W
        - model::ARROW_STYLE_MENU_INSET * 2.0
        - model::ARROW_STYLE_CHIP_GLYPH_W
        - PREVIEW_GAP / 2.0;

    for arrow in ArrowStyle::ALL {
        let text = model::arrow_style_chip_label(arrow);
        let drawn = engine
            .measure(style, &text, None)
            .expect("label measures")
            .width();
        assert!(drawn <= label_room, "{text} is {drawn}px in {label_room}px");
    }
}

#[test]
fn the_open_menu_lists_every_style_and_marks_the_current_one() {
    let tree = tree_for(&snapshot_for(ArrowStyle::Curved, true));

    let chip = tree.node_by_id(&"top.style.arrow-style".into()).unwrap();
    assert_eq!(
        chip.interact.as_ref().map(|interaction| &interaction.event),
        Some(&ToolbarEvent::ToggleArrowStyleMenu(false))
    );
    assert_eq!(
        chip.interact.as_ref().unwrap().tooltip,
        None,
        "the open menu is the explanation"
    );
    assert!(
        tree.node_by_id(&"top.arrow-style.panel".into()).is_some(),
        "menu panel"
    );
    for entry in model::arrow_style_menu_entries(ArrowStyle::Curved) {
        let id = entry.id();
        let row = tree
            .node_by_id(&id.clone().into())
            .unwrap_or_else(|| panic!("{id}"));
        let WidgetKind::TextButton { style, .. } = &row.kind else {
            panic!("{id} is a button");
        };
        assert_eq!(style.active, entry.style == ArrowStyle::Curved, "{id}");
        let interaction = row.interact.as_ref().expect("row clicks");
        assert_eq!(interaction.event, ToolbarEvent::SetArrowStyle(entry.style));
        assert_eq!(interaction.tooltip, Some(entry.tooltip()));
        assert_eq!(
            tree.node_by_id(&format!("{id}.preview").into())
                .map(|node| &node.kind),
            Some(&WidgetKind::ArrowStylePreview { style: entry.style })
        );
    }
}

/// Whether `outer` covers `inner` on both axes.
fn contains(outer: (f64, f64, f64, f64), inner: (f64, f64, f64, f64)) -> bool {
    outer.0 <= inner.0
        && outer.1 <= inner.1
        && outer.0 + outer.2 >= inner.0 + inner.2
        && outer.1 + outer.3 >= inner.1 + inner.3
}

/// The menu grows the surface and its input region, never the strip's
/// width, and hangs inside the surface.
#[test]
fn the_open_menu_grows_height_and_input_but_not_width() {
    let engine = UiTextEngine::default();
    let closed = snapshot_for(ArrowStyle::Standard, false);
    let open = snapshot_for(ArrowStyle::Standard, true);

    assert_eq!(
        top_natural_width(&engine, &open, 58.0),
        top_natural_width(&engine, &closed, 58.0)
    );
    assert!(top_extra_height(&engine, &open) > top_extra_height(&engine, &closed));

    let (w, h) = top_size(&engine, &open);
    let tree = build_top_view(&engine, &open, w as f64, h as f64);
    let panel = tree
        .node_by_id(&"top.arrow-style.panel".into())
        .expect("panel")
        .rect;
    assert!(panel.1 + panel.3 <= h as f64, "the menu fits the surface");
    let rects = top_input_rects(&engine, &open, w as f64, h as f64).expect("rects");
    assert!(
        rects.iter().any(|rect| contains(*rect, panel)),
        "the whole menu accepts input: {panel:?} in {rects:?}"
    );
}
