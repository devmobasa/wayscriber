use super::*;
use crate::backend::wayland::toolbar::view::WidgetTree;
use crate::backend::wayland::toolbar::view::node::{
    ButtonStyle, LabelSpec, ShortcutBadgePlacement, WidgetKind, WidgetNode,
};
use crate::ui::toolbar::ToolbarBindingHints;

fn pixels(density: i32, paint: impl FnOnce(&cairo::Context)) -> Vec<u8> {
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, 1400 * density, 800 * density).unwrap();
    surface.set_device_scale(density as f64, density as f64);
    let ctx = cairo::Context::new(&surface).unwrap();
    paint(&ctx);
    drop(ctx);
    surface.data().unwrap().to_vec()
}

#[test]
fn retained_widget_text_matches_fresh_targets() {
    let engine = UiTextEngine::default();
    let mut tree = WidgetTree::new((400.0, 500.0));
    let label = || {
        LabelSpec::new(
            "Toolbar 測試 label with enough words to truncate",
            14.0,
            true,
        )
    };
    let kinds = [
        WidgetKind::TextButton {
            label: label(),
            style: ButtonStyle::plain(),
        },
        WidgetKind::TextButton {
            label: label(),
            style: ButtonStyle::disabled(),
        },
        WidgetKind::Label(label().wrapped()),
        WidgetKind::Label(LabelSpec::new("Plain label", 13.0, false)),
        WidgetKind::Checkbox {
            checked: true,
            label: label(),
        },
        WidgetKind::MiniCheckbox {
            checked: true,
            label: LabelSpec::new("Mini 測試", 12.0, false),
        },
        WidgetKind::SegmentedControl {
            left: LabelSpec::new("Left", 12.0, false),
            right: LabelSpec::new("Right", 12.0, false),
            active_right: true,
        },
        WidgetKind::PresetSlot {
            glyph: None,
            color: (1.0, 0.0, 0.0, 1.0),
            label: "2".into(),
            active: false,
        },
    ];
    for (index, kind) in kinds.into_iter().enumerate() {
        tree.push(
            WidgetNode::decor(
                format!("fixture.{index}"),
                (20.0, 20.0 + index as f64 * 60.0, 160.0, 55.0),
                kind,
            )
            .with_shortcut_badge(
                Some("K"),
                if index % 2 == 0 {
                    ShortcutBadgePlacement::Corner
                } else {
                    ShortcutBadgePlacement::Below
                },
            ),
        );
    }
    for density in [1, 2, 1] {
        let actual = pixels(density, |ctx| {
            paint_tree(&engine, ctx, &tree, Some((80.0, 390.0)))
        });
        let expected = pixels(density, |ctx| {
            paint_tree(&UiTextEngine::default(), ctx, &tree, Some((80.0, 390.0)))
        });
        assert!(actual == expected, "widget text at density {density}");
        assert!(actual.iter().any(|byte| *byte != 0));
    }
}

#[test]
fn retained_top_strip_preserves_fade_hits_and_delayed_tooltips_across_targets() {
    let engine = UiTextEngine::default();
    let mut input = crate::input::state::test_support::make_test_input_state();
    input.set_toolbar_use_icons(false);
    let mut snapshot =
        ToolbarSnapshot::from_input_with_bindings(&input, ToolbarBindingHints::default());
    snapshot.settings_popover_open = true;
    for fade in [1.0, 0.45] {
        snapshot.top_fade = fade;
        let (width, height) = crate::backend::wayland::toolbar::top_size(&engine, &snapshot);
        let tree = view::top::build_top_view(&engine, &snapshot, width as f64, height as f64);
        let planned_hits = tree.to_hit_regions();
        let hovered = planned_hits
            .iter()
            .find(|hit| hit.tooltip.is_some())
            .unwrap();
        let hover = Some((
            hovered.rect.0 + hovered.rect.2 / 2.0,
            hovered.rect.1 + hovered.rect.3 / 2.0,
        ));
        for density in [1, 2, 1] {
            let mut hits = Vec::new();
            let start = Some(
                Instant::now() - super::super::TOOLTIP_DELAY - std::time::Duration::from_secs(1),
            );
            let actual = pixels(density, |ctx| {
                render_top_strip(
                    &engine,
                    ctx,
                    width as f64,
                    height as f64,
                    &snapshot,
                    &mut hits,
                    hover,
                    start,
                )
                .unwrap()
            });
            let mut fresh_hits = Vec::new();
            let expected = pixels(density, |ctx| {
                render_top_strip(
                    &UiTextEngine::default(),
                    ctx,
                    width as f64,
                    height as f64,
                    &snapshot,
                    &mut fresh_hits,
                    hover,
                    start,
                )
                .unwrap()
            });
            assert!(actual == expected, "strip density {density}, fade {fade}");
            assert_eq!(hits.len(), planned_hits.len());
            assert_eq!(hits.len(), fresh_hits.len());
            for ((actual, fresh), planned) in hits.iter().zip(&fresh_hits).zip(&planned_hits) {
                assert_eq!(actual.rect, planned.rect);
                assert_eq!(actual.rect, fresh.rect);
                assert_eq!(actual.event, planned.event);
                assert_eq!(actual.kind, planned.kind);
                assert_eq!(actual.focus_id, planned.focus_id);
                assert_eq!(actual.tooltip, planned.tooltip);
            }
            let tooltip = pixels(density, |ctx| {
                draw_tooltip_with_delay(
                    &engine,
                    ctx,
                    &hits,
                    hover,
                    width as f64,
                    height as f64,
                    false,
                    start,
                )
            });
            assert!(tooltip.iter().any(|byte| *byte != 0));
            // A future start deterministically exercises the not-yet-ready path.
            let hidden = pixels(density, |ctx| {
                draw_tooltip_with_delay(
                    &engine,
                    ctx,
                    &hits,
                    hover,
                    width as f64,
                    height as f64,
                    false,
                    Some(Instant::now() + std::time::Duration::from_secs(60)),
                )
            });
            assert!(hidden.iter().all(|byte| *byte == 0));
        }
    }
}
