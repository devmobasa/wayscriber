use crate::backend::wayland::toolbar::layout::{ToolbarLayoutSpec, top_size};
use crate::backend::wayland::toolbar::view::{WidgetKind, WidgetTree};
use crate::config::ToolbarStrokeControls;
use crate::input::Tool;
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot, model};
use crate::ui_text::{UiTextEngine, UiTextStyle};

use super::super::{build_top_view, top_extra_height, top_input_rects, top_natural_width};

fn snapshot_for(tool: Tool, style: ToolbarStrokeControls, open: bool) -> ToolbarSnapshot {
    let state = make_test_input_state();
    let mut snapshot =
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
    snapshot.active_tool = tool;
    snapshot.tool_override = None;
    snapshot.thickness_targets_marker = tool == Tool::Marker;
    snapshot.show_text_controls = false;
    snapshot.show_marker_opacity_section = false;
    snapshot.stroke_controls = style;
    snapshot.pen_feel_open = open;
    snapshot
}

fn tree_for(snapshot: &ToolbarSnapshot) -> WidgetTree {
    let engine = UiTextEngine::default();
    let (w, h) = top_size(&engine, snapshot);
    build_top_view(&engine, snapshot, w as f64, h as f64)
}

fn feel_ids(tree: &WidgetTree) -> Vec<String> {
    tree.nodes()
        .iter()
        .map(|node| node.id.as_str().to_string())
        .filter(|id| id.starts_with("top.feel."))
        .collect()
}

fn text_style(size: f64, bold: bool) -> UiTextStyle<'static> {
    UiTextStyle {
        family: crate::ui::theme::toolbar::FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: if bold {
            cairo::FontWeight::Bold
        } else {
            cairo::FontWeight::Normal
        },
        size,
    }
}

/// The chip replaces both inline controls, keeps a fixed slot, and the
/// planner (which walks the same tree) budgets it.
#[test]
fn the_chip_stands_in_for_both_controls_inside_the_planned_width() {
    let engine = UiTextEngine::default();
    let snapshot = snapshot_for(Tool::LiveShape, ToolbarStrokeControls::Panel, false);
    let tree = tree_for(&snapshot);

    let chip = tree
        .node_by_id(&"top.style.pen-feel".into())
        .expect("Pen feel chip");
    let WidgetKind::TextButton { label, style } = &chip.kind else {
        panic!("chip kind {:?}", chip.kind);
    };
    assert_eq!(label.text, "Pen feel \u{25BE}");
    assert!(!style.active, "closed panel, resting chip");
    assert_eq!(chip.rect.2, ToolbarLayoutSpec::TOP_STYLE_PEN_FEEL_W);
    let interaction = chip.interact.as_ref().expect("chip clicks");
    assert_eq!(interaction.event, ToolbarEvent::TogglePenFeelPanel(true));
    assert_eq!(
        interaction.tooltip.as_deref(),
        Some("Smoothing: Medium \u{b7} Shape detection: Forgiving \u{2014} click to adjust")
    );
    for gone in ["top.style.pen-smoothing", "top.style.shape-sensitivity"] {
        assert!(
            tree.nodes()
                .iter()
                .all(|node| !node.id.as_str().starts_with(gone)),
            "{gone} is behind the chip"
        );
    }

    // The label fits its slot with the painter's inset on both sides.
    let drawn = engine
        .measure(text_style(14.0, true), &label.text, None)
        .expect("label measures")
        .width();
    assert!(
        drawn + 12.0 <= ToolbarLayoutSpec::TOP_STYLE_PEN_FEEL_W,
        "chip label is {drawn}px wide"
    );

    let pill = tree
        .node_by_id(&"top.island.style".into())
        .expect("style pill");
    assert!(chip.rect.0 + chip.rect.2 <= pill.rect.0 + pill.rect.2);
    let (_, h) = top_size(&engine, &snapshot);
    assert!(top_natural_width(&engine, &snapshot, h as f64) >= pill.rect.0 + pill.rect.2);
    assert!(feel_ids(&tree).is_empty(), "the panel is closed");
}

#[test]
fn the_open_chip_reads_as_pressed_and_closes_the_panel() {
    let tree = tree_for(&snapshot_for(Tool::Pen, ToolbarStrokeControls::Panel, true));

    let chip = tree.node_by_id(&"top.style.pen-feel".into()).expect("chip");
    let WidgetKind::TextButton { style, .. } = &chip.kind else {
        panic!("chip kind");
    };
    assert!(style.active);
    assert_eq!(
        chip.interact.as_ref().map(|interaction| &interaction.event),
        Some(&ToolbarEvent::TogglePenFeelPanel(false))
    );
}

/// Pen and Marker get the smoothing section; Shape Pen adds detection. Each
/// section names its level, lays its bars across the column, and hints at
/// what the level does; only smoothing carries the preview.
#[test]
fn the_open_panel_shows_the_sections_the_tool_uses() {
    let pen = tree_for(&snapshot_for(Tool::Pen, ToolbarStrokeControls::Panel, true));
    let smoothing_only: Vec<String> = [
        "top.feel.panel",
        "top.feel.title",
        "top.feel.smoothing.label",
        "top.feel.smoothing.value",
        "top.feel.smoothing.level-1",
        "top.feel.smoothing.level-2",
        "top.feel.smoothing.level-3",
        "top.feel.smoothing.level-4",
        "top.feel.smoothing.level-5",
        "top.feel.smoothing.level-6",
        "top.feel.smoothing.preview",
        "top.feel.smoothing.hint",
    ]
    .map(str::to_string)
    .to_vec();
    assert_eq!(feel_ids(&pen), smoothing_only);

    let shape_pen = snapshot_for(Tool::LiveShape, ToolbarStrokeControls::Panel, true);
    let tree = tree_for(&shape_pen);
    let mut both = smoothing_only.clone();
    both.extend(
        [
            "top.feel.detection.label",
            "top.feel.detection.value",
            "top.feel.detection.level-1",
            "top.feel.detection.level-2",
            "top.feel.detection.level-3",
            "top.feel.detection.level-4",
            "top.feel.detection.hint",
        ]
        .map(str::to_string),
    );
    assert_eq!(feel_ids(&tree), both);

    let label = |id: &str| match &tree.node_by_id(&id.to_string().into()).expect(id).kind {
        WidgetKind::Label(label) => label.clone(),
        other => panic!("{id} kind {other:?}"),
    };
    assert_eq!(label("top.feel.title").text, "Pen feel");
    assert_eq!(label("top.feel.smoothing.label").text, "Smoothing");
    assert_eq!(label("top.feel.smoothing.value").text, "Medium");
    assert_eq!(
        label("top.feel.smoothing.hint").text,
        "Steadies a shaky hand"
    );
    assert_eq!(label("top.feel.detection.label").text, "Shape detection");
    assert_eq!(label("top.feel.detection.value").text, "Forgiving");
    assert_eq!(
        label("top.feel.detection.hint").text,
        "Rough strokes become shapes"
    );
    assert!(label("top.feel.smoothing.label").caption);
    assert!(label("top.feel.smoothing.value").bold);

    // The level name sits flush right in the column, after its name.
    let name = tree
        .node_by_id(&"top.feel.smoothing.label".into())
        .expect("label");
    let value = tree
        .node_by_id(&"top.feel.smoothing.value".into())
        .expect("value");
    assert!((value.rect.0 + value.rect.2 - (name.rect.0 + name.rect.2)).abs() < 1e-9);
    assert!(value.rect.0 > name.rect.0 + 80.0, "{:?}", value.rect);
}

/// The panel's bars are the inline meter's model under panel ids: the same
/// click events, so a click and a wheel step (routed by event) behave alike.
#[test]
fn panel_bars_carry_the_meter_events_and_the_preview_follows_the_level() {
    let mut snapshot = snapshot_for(Tool::LiveShape, ToolbarStrokeControls::Panel, true);
    snapshot.pen_smoothing = 5;
    snapshot.shape_recognition_sensitivity = 1;
    let tree = tree_for(&snapshot);

    for section in model::pen_feel_sections(&snapshot) {
        let mut left: Option<f64> = None;
        for segment in &section.meter.segments {
            let node = tree
                .node_by_id(&segment.id.clone().into())
                .unwrap_or_else(|| panic!("{}", segment.id));
            let WidgetKind::MeterBar { filled, enabled } = node.kind else {
                panic!("{} kind {:?}", segment.id, node.kind);
            };
            assert_eq!(filled, segment.filled, "{}", segment.id);
            assert!(enabled);
            let interaction = node.interact.as_ref().expect("bar clicks");
            assert_eq!(interaction.event, segment.event, "{}", segment.id);
            assert_eq!(
                model::StrokeSetting::for_wheel(&interaction.event, &snapshot),
                Some(section.setting),
                "a wheel over {} steps its setting",
                segment.id
            );
            // The topmost hit at the bar's center is the bar itself.
            let center = (
                node.rect.0 + node.rect.2 / 2.0,
                node.rect.1 + node.rect.3 / 2.0,
            );
            assert_eq!(
                tree.hit(center.0, center.1).map(|hit| hit.id.as_str()),
                Some(segment.id.as_str())
            );
            if let Some(left) = left {
                assert!((node.rect.0 - left).abs() < 1e-9, "{} abuts", segment.id);
            }
            left = Some(node.rect.0 + node.rect.2);
        }
        let row_w = left.expect("bars") - section_left(&tree, section.setting);
        assert!((row_w - model::PEN_FEEL_CONTENT_W).abs() < 1e-9);
    }

    let preview = tree
        .node_by_id(&"top.feel.smoothing.preview".into())
        .expect("preview");
    assert_eq!(preview.kind, WidgetKind::SmoothingPreview { level: 5 });
    assert_eq!(
        (preview.rect.2, preview.rect.3),
        (model::PEN_FEEL_CONTENT_W, model::PEN_FEEL_PREVIEW_H)
    );
    assert!(preview.interact.is_none(), "the preview is a display");
}

fn section_left(tree: &WidgetTree, setting: model::StrokeSetting) -> f64 {
    tree.node_by_id(&model::pen_feel_id(setting, "label").into())
        .expect("section label")
        .rect
        .0
}

/// The open panel grows the surface and its input region, but never the
/// strip's width; it hangs below the pill with its caret on the chip.
#[test]
fn the_open_panel_grows_height_and_input_but_not_width() {
    let engine = UiTextEngine::default();
    for tool in [Tool::Pen, Tool::LiveShape] {
        let closed = snapshot_for(tool, ToolbarStrokeControls::Panel, false);
        let open = snapshot_for(tool, ToolbarStrokeControls::Panel, true);

        assert_eq!(
            top_natural_width(&engine, &open, 58.0),
            top_natural_width(&engine, &closed, 58.0),
            "{tool:?}"
        );
        assert!(top_extra_height(&engine, &open) > top_extra_height(&engine, &closed));

        let (w, h) = top_size(&engine, &open);
        let tree = build_top_view(&engine, &open, w as f64, h as f64);
        let panel = tree
            .node_by_id(&"top.feel.panel".into())
            .expect("panel")
            .rect;
        let WidgetKind::Popover { caret_x, caret_up } = tree
            .node_by_id(&"top.feel.panel".into())
            .expect("panel")
            .kind
        else {
            panic!("panel kind");
        };
        let pill = tree
            .node_by_id(&"top.island.style".into())
            .expect("pill")
            .rect;
        let chip = tree
            .node_by_id(&"top.style.pen-feel".into())
            .expect("chip")
            .rect;
        assert!(caret_up, "{tool:?} opens below");
        assert!(panel.1 >= pill.1 + pill.3, "{tool:?} clears the pill");
        assert!(panel.1 + panel.3 <= h as f64, "{tool:?} fits the surface");
        assert!(
            caret_x >= chip.0 && caret_x <= chip.0 + chip.2,
            "{tool:?} caret"
        );
        assert_eq!(
            (panel.2, panel.3),
            model::pen_feel_panel_size(&open),
            "{tool:?}"
        );

        let rects = top_input_rects(&engine, &open, w as f64, h as f64).expect("rects");
        assert!(
            rects.iter().any(|rect| rect.0 <= panel.0
                && rect.0 + rect.2 >= panel.0 + panel.2
                && rect.1 <= panel.1
                && rect.1 + rect.3 >= panel.1 + panel.3),
            "{tool:?} panel accepts input: {rects:?}"
        );
    }
}

/// A stale open flag without a chip to hang from (meter or stepper style, or
/// a tool without stroke-feel settings) draws and reserves nothing.
#[test]
fn the_panel_needs_its_chip() {
    let engine = UiTextEngine::default();
    for snapshot in [
        snapshot_for(Tool::Pen, ToolbarStrokeControls::Meter, true),
        snapshot_for(Tool::Pen, ToolbarStrokeControls::Stepper, true),
        snapshot_for(Tool::Rect, ToolbarStrokeControls::Panel, true),
    ] {
        let mut closed = snapshot.clone();
        closed.pen_feel_open = false;

        assert!(feel_ids(&tree_for(&snapshot)).is_empty());
        assert_eq!(
            top_extra_height(&engine, &snapshot),
            top_extra_height(&engine, &closed)
        );
    }
}

/// Every level name, section name, and hint fits the content column at the
/// sizes the panel draws them.
#[test]
fn panel_text_fits_its_column() {
    let engine = UiTextEngine::default();
    let width = |text: &str, size: f64, bold: bool| {
        engine
            .measure(text_style(size, bold), text, None)
            .expect("text measures")
            .width()
    };
    let caption = crate::ui::theme::toolbar::FONT_SIZE_TOOLTIP;
    let hint = crate::ui::theme::toolbar::FONT_SIZE_SMALL;

    for (setting, levels) in [
        (model::StrokeSetting::Smoothing, 0..=6),
        (model::StrokeSetting::ShapeDetection, 0..=4),
    ] {
        for level in levels {
            let header = width(setting.name(), caption, false)
                + 12.0
                + width(setting.level_name(level), caption, true);
            assert!(
                header <= model::PEN_FEEL_CONTENT_W,
                "{setting:?} {level} header is {header}px"
            );
            let line = width(setting.level_hint(level), hint, false);
            assert!(
                line <= model::PEN_FEEL_CONTENT_W,
                "{setting:?} {level} hint is {line}px"
            );
        }
    }
}
