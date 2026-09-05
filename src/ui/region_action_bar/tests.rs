use super::layout::{ACTION_ITEM_WIDTH, BAR_HEIGHT, BAR_PADDING, ROW_GAP, SELECTION_GAP};
use super::*;
use crate::input::state::RegionSelection;
use crate::ui_text::UiTextEngine;

fn sample_bar() -> RegionActionBar {
    RegionActionBar::place(
        RegionSelection {
            start: (100.0, 100.0),
            end: (300.0, 200.0),
        },
        (800, 600),
    )
}

fn rect_inside(inner: RegionActionRect, outer: RegionActionRect) -> bool {
    inner.x + f64::EPSILON >= outer.x
        && inner.y + f64::EPSILON >= outer.y
        && inner.x + inner.width <= outer.x + outer.width + f64::EPSILON
        && inner.y + inner.height <= outer.y + outer.height + f64::EPSILON
}

fn assert_controls_stay_inside_bar(bar: &RegionActionBar) {
    let bounds = bar.bounds();
    for item in bar.items.iter().chain(bar.edit.iter()) {
        assert!(
            rect_inside(item.bounds, bounds),
            "{:?} at ({}, {}) {}x{} leaves bar {bounds:?}",
            item.action,
            item.bounds.x,
            item.bounds.y,
            item.bounds.width,
            item.bounds.height
        );
    }
    assert!(
        rect_inside(bar.toggle.bounds, bounds),
        "toggle leaves bar {bounds:?}"
    );
    for row in [&bar.items[..], &bar.edit[..]] {
        for pair in row.windows(2) {
            assert!(
                pair[0].bounds.x + pair[0].bounds.width <= pair[1].bounds.x + f64::EPSILON,
                "{:?} overlaps {:?}",
                pair[0].action,
                pair[1].action
            );
        }
    }
    assert!(
        bar.items[0].bounds.y + bar.items[0].bounds.height <= bar.edit[0].bounds.y + f64::EPSILON
    );
    assert!(bar.edit[0].bounds.y + bar.edit[0].bounds.height <= bar.toggle.bounds.y + f64::EPSILON);
}

#[test]
fn action_bar_prefers_below_then_flips_above_and_clamps_to_the_surface() {
    let centered = sample_bar();
    assert_eq!(
        centered.bounds(),
        RegionActionRect::new(35.0, 212.0, 330.0, BAR_HEIGHT)
    );

    let flipped = RegionActionBar::place(
        RegionSelection {
            start: (730.0, 560.0),
            end: (790.0, 590.0),
        },
        (800, 600),
    );
    assert_eq!(
        flipped.bounds(),
        RegionActionRect::new(462.0, 560.0 - SELECTION_GAP - BAR_HEIGHT, 330.0, BAR_HEIGHT)
    );
}

#[test]
fn action_bar_hit_returns_typed_controls_and_rejects_gaps() {
    let bar = sample_bar();
    let action_y = bar.items[0].bounds.y + bar.items[0].bounds.height / 2.0;
    let edit_y = bar.edit[0].bounds.y + bar.edit[0].bounds.height / 2.0;
    let toggle_y = bar.toggle.bounds.y + bar.toggle.bounds.height / 2.0;

    assert_eq!(bar.hit((80.0, action_y)), Some(RegionAction::Copy));
    assert_eq!(bar.hit((160.0, action_y)), Some(RegionAction::Save));
    assert_eq!(bar.hit((240.0, action_y)), Some(RegionAction::Both));
    assert_eq!(bar.hit((320.0, action_y)), Some(RegionAction::Board));
    assert_eq!(bar.hit((80.0, edit_y)), Some(RegionAction::CutBand));
    assert_eq!(bar.hit((160.0, edit_y)), Some(RegionAction::UndoCut));
    assert_eq!(bar.hit((240.0, edit_y)), Some(RegionAction::RedoCut));
    assert_eq!(bar.hit((320.0, edit_y)), Some(RegionAction::ResetCuts));
    assert_eq!(
        bar.hit((200.0, toggle_y)),
        Some(RegionAction::ToggleIncludeDrawings)
    );
    assert_eq!(bar.hit((119.0, action_y)), None, "inter-item gap");
    assert!(bar.contains((119.0, action_y)), "bar gaps stay modal-owned");
    assert_eq!(bar.hit((20.0, 20.0)), None, "outside the bar");
    assert!(!bar.contains((20.0, 20.0)));
}

#[test]
fn disabled_controls_still_consume_the_bar_but_return_no_enabled_action() {
    let bar = sample_bar();
    let availability = RegionActionAvailability {
        terminal: false,
        cut: true,
        undo: false,
        redo: false,
        reset: false,
    };
    let action_y = bar.items[0].bounds.y + bar.items[0].bounds.height / 2.0;
    assert_eq!(bar.hit((80.0, action_y)), Some(RegionAction::Copy));
    assert_eq!(bar.enabled_hit((80.0, action_y), availability), None);
    assert!(bar.contains((80.0, action_y)));
    assert_eq!(
        bar.enabled_hit(
            (
                bar.edit[0].bounds.x + bar.edit[0].bounds.width / 2.0,
                bar.edit[0].bounds.y + bar.edit[0].bounds.height / 2.0
            ),
            availability
        ),
        Some(RegionAction::CutBand)
    );
}

#[test]
fn action_bar_rows_never_overlap_and_stay_inside_the_padded_frame() {
    let bar = sample_bar();
    let bounds = bar.bounds();
    let toggle = bar.toggle.bounds;

    for item in bar.items {
        assert!(item.bounds.y >= bounds.y + BAR_PADDING);
        assert!(
            item.bounds.y + item.bounds.height <= bar.edit[0].bounds.y - ROW_GAP + f64::EPSILON
        );
        assert!(item.bounds.x >= bounds.x + BAR_PADDING);
        assert!(item.bounds.x + item.bounds.width <= bounds.x + bounds.width - BAR_PADDING);
        assert_eq!(item.bounds.width, ACTION_ITEM_WIDTH);
    }
    for item in bar.edit {
        assert!(item.bounds.y >= bar.items[0].bounds.y + bar.items[0].bounds.height);
        assert!(item.bounds.y + item.bounds.height <= toggle.y - ROW_GAP + f64::EPSILON);
        assert_eq!(item.bounds.width, ACTION_ITEM_WIDTH);
    }
    assert!(toggle.y + toggle.height <= bounds.y + bounds.height - BAR_PADDING);
}

#[test]
fn narrow_and_short_surfaces_keep_controls_inside_the_bar() {
    let selection = RegionSelection {
        start: (10.0, 10.0),
        end: (40.0, 30.0),
    };
    for surface in [(200, 80), (80, 40), (40, 600), (800, 36)] {
        let bar = RegionActionBar::place(selection, surface);
        assert_controls_stay_inside_bar(&bar);
        let action = bar.items[0].bounds;
        if action.width > 1.0 && action.height > 1.0 {
            assert_eq!(
                bar.hit((
                    action.x + action.width / 2.0,
                    action.y + action.height / 2.0
                )),
                Some(RegionAction::Copy),
                "typed hit on {surface:?}"
            );
        }
    }
}

#[test]
fn action_bar_exposes_the_requested_labels_and_shortcuts() {
    assert_eq!(RegionAction::Copy.label(), "Copy");
    assert_eq!(RegionAction::Copy.shortcut(), "Ctrl+C");
    assert_eq!(RegionAction::Save.label(), "Save");
    assert_eq!(RegionAction::Save.shortcut(), "Ctrl+S");
    assert_eq!(RegionAction::Both.label(), "Both");
    assert_eq!(RegionAction::Both.shortcut(), "Enter");
    assert_eq!(RegionAction::Board.label(), "Board");
    assert_eq!(RegionAction::Board.shortcut(), "B");
    assert_eq!(RegionAction::CutBand.label(), "Cut");
    assert_eq!(RegionAction::CutBand.shortcut(), "X");
    assert_eq!(RegionAction::UndoCut.shortcut(), "Ctrl+Z");
    assert_eq!(RegionAction::RedoCut.shortcut(), "Ctrl+Y");
    assert_eq!(
        RegionAction::ToggleIncludeDrawings.label(),
        "Include drawings in exports"
    );
    assert_eq!(RegionAction::ToggleIncludeDrawings.shortcut(), "D");
    assert!(RegionAction::Copy.is_terminal());
    assert!(!RegionAction::CutBand.is_terminal());
    assert!(!RegionAction::ToggleIncludeDrawings.is_terminal());
}

#[test]
fn enter_is_the_only_accented_default_action() {
    assert!(RegionAction::Both.is_primary());
    for action in [
        RegionAction::Copy,
        RegionAction::Save,
        RegionAction::Board,
        RegionAction::CutBand,
        RegionAction::UndoCut,
        RegionAction::RedoCut,
        RegionAction::ResetCuts,
        RegionAction::ToggleIncludeDrawings,
    ] {
        assert!(!action.is_primary(), "{action:?} must stay neutral");
    }
}

#[test]
fn rendering_paints_the_bar_and_each_control() {
    let bar = sample_bar();
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_action_bar(
        &UiTextEngine::default(),
        &ctx,
        &bar,
        RegionActionBarVisual::simple(Some(RegionAction::Both), true),
    );
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();
    let alpha = |x: usize, y: usize| data[y * stride + x * 4 + 3];
    let action_y = (bar.items[0].bounds.y + bar.items[0].bounds.height / 2.0) as usize;
    let toggle_y = (bar.toggle.bounds.y + bar.toggle.bounds.height / 2.0) as usize;

    assert!(alpha(40, action_y) > 0, "bar surface");
    for x in [80, 160, 240, 320] {
        assert!(alpha(x, action_y) > 0, "control at x={x}");
    }
    assert!(alpha(56, toggle_y) > 0, "checked drawings checkbox");
    assert_eq!(alpha(20, 20), 0, "outside remains untouched");
}

#[test]
fn the_drawings_checkbox_carries_the_state_instead_of_a_full_width_slab() {
    let bar = sample_bar();
    let toggle_y = (bar.toggle.bounds.y + bar.toggle.bounds.height / 2.0) as usize;
    let row_alpha = |checked: bool| {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_action_bar(
            &UiTextEngine::default(),
            &ctx,
            &bar,
            RegionActionBarVisual::simple(None, checked),
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        (
            u32::from(data[toggle_y * stride + 300 * 4 + 3]),
            u32::from(data[toggle_y * stride + 56 * 4 + 3]),
        )
    };

    let (off_row, off_box) = row_alpha(false);
    let (on_row, on_box) = row_alpha(true);
    assert_eq!(off_row, on_row, "the row background must not change");
    assert!(on_box > 0 && off_box > 0, "the box is drawn either way");
}

#[test]
fn updating_and_failed_preview_states_paint_status_text() {
    let bar = sample_bar();
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_action_bar(
        &UiTextEngine::default(),
        &ctx,
        &bar,
        RegionActionBarVisual {
            hovered: None,
            include_drawings: false,
            availability: RegionActionAvailability {
                terminal: false,
                cut: true,
                undo: true,
                redo: false,
                reset: true,
            },
            cut_armed: true,
            status: Some(RegionCutStatus::Updating),
        },
    );
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();
    let status = bar.status_bounds().unwrap();
    let status_y = (status.y + status.height / 2.0) as usize;
    let alpha = data[status_y * stride + 200 * 4 + 3];
    assert!(alpha > 0, "status caption is visible");
}

fn paint_bar(
    width: i32,
    height: i32,
    bar: RegionActionBar,
    status: Option<RegionCutStatus>,
) -> (usize, Vec<u8>) {
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_action_bar(
        &UiTextEngine::default(),
        &ctx,
        &bar,
        RegionActionBarVisual {
            hovered: None,
            include_drawings: false,
            availability: RegionActionAvailability {
                terminal: false,
                cut: true,
                undo: true,
                redo: false,
                reset: true,
            },
            cut_armed: false,
            status,
        },
    );
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let pixels = surface.data().unwrap().to_vec();
    (stride, pixels)
}

#[test]
fn short_surface_status_paint_stays_inside_its_row() {
    let selection = RegionSelection {
        start: (10.0, 10.0),
        end: (40.0, 30.0),
    };
    for surface in [(800, 36), (800, 100), (200, 80)] {
        let bar = RegionActionBar::place(selection, surface);
        let width = i32::try_from(surface.0).unwrap();
        let height = i32::try_from(surface.1).unwrap();
        let (stride, without) = paint_bar(width, height, bar, None);
        let (_, with_status) = paint_bar(width, height, bar, Some(RegionCutStatus::Failed));
        let row = bar.status_bounds();
        for y in 0..surface.1 as usize {
            for x in 0..surface.0 as usize {
                let offset = y * stride + x * 4;
                if without[offset..offset + 4] == with_status[offset..offset + 4] {
                    continue;
                }
                let Some(row) = row else {
                    panic!("status painted with no status row on {surface:?}");
                };
                assert!(
                    row.contains((x as f64 + 0.5, y as f64 + 0.5)),
                    "status paint at ({x}, {y}) left the {row:?} row on {surface:?}"
                );
                assert!(
                    bar.bounds.contains((x as f64 + 0.5, y as f64 + 0.5)),
                    "status paint at ({x}, {y}) left the bar on {surface:?}"
                );
            }
        }
    }
}
#[test]
fn retained_text_owner_matches_fresh_bar_pixels_across_status_and_density() {
    let engine = UiTextEngine::default();
    for density in [1, 2, 1] {
        for status in [
            None,
            Some(RegionCutStatus::Updating),
            Some(RegionCutStatus::Failed),
        ] {
            let paint = |engine: &UiTextEngine| {
                let mut surface = cairo::ImageSurface::create(
                    cairo::Format::ARgb32,
                    800 * density,
                    600 * density,
                )
                .unwrap();
                {
                    let ctx = cairo::Context::new(&surface).unwrap();
                    ctx.scale(f64::from(density), f64::from(density));
                    render_region_action_bar(
                        engine,
                        &ctx,
                        &sample_bar(),
                        RegionActionBarVisual {
                            hovered: Some(RegionAction::CutBand),
                            include_drawings: status.is_none(),
                            availability: RegionActionAvailability {
                                terminal: status.is_none(),
                                cut: true,
                                undo: true,
                                redo: false,
                                reset: true,
                            },
                            cut_armed: true,
                            status,
                        },
                    );
                }
                surface.data().unwrap().to_vec()
            };
            let actual = paint(&engine);
            assert!(actual.iter().any(|&byte| byte != 0));
            assert!(
                actual == paint(&UiTextEngine::default()),
                "retained action bar pixels differ"
            );
        }
    }
}
