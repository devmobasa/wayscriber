use super::layout::{
    PointerPanelLayout, covered_by_action_bar, pointer_panel_layout, selection_badge_layout,
};
use super::legend::{AREA_LEGEND_TEXT, AREA_WITH_WINDOWS_LEGEND_TEXT};
use super::*;
use crate::input::state::RegionSelection;
use crate::screen_pixels::PackedArgb32;
use crate::ui::region_action_bar::RegionActionRect;

#[test]
fn size_readout_uses_export_pixel_units_and_multiplication_sign() {
    assert_eq!(capture_size_text((0, 0)), "0 × 0");
    assert_eq!(capture_size_text((900, 620)), "900 × 620");
}

#[test]
fn legend_only_advertises_supported_window_controls() {
    assert_eq!(
        picker_legend_text(RegionCaptureWindowVisual {
            available: false,
            active: false,
            targets: &[],
            highlighted_target: None,
        }),
        "Drag to select   Shift: square   Ctrl+A: all   Esc: cancel"
    );
    assert_eq!(
        picker_legend_text(RegionCaptureWindowVisual {
            available: true,
            active: false,
            targets: &[],
            highlighted_target: None,
        }),
        "Drag to select   Shift: square   Ctrl+A: all   Space: window   Esc: cancel"
    );
    assert_eq!(
        picker_legend_text(RegionCaptureWindowVisual {
            available: true,
            active: true,
            targets: &[],
            highlighted_target: None,
        }),
        "Click: select   Super+Arrows: choose   Enter: select   Space: area   Esc: cancel"
    );
}

#[test]
fn every_selector_legend_names_the_keys_that_selector_actually_has() {
    // Recognition has no square modifier, and its select-all reads rather
    // than selects, so it must not borrow the capture wording.
    assert!(OCR_LEGEND_TEXT.contains("Ctrl+A"));
    assert!(
        !OCR_LEGEND_TEXT.contains("Shift"),
        "recognition offers no square modifier: {OCR_LEGEND_TEXT}"
    );
    for legend in [
        AREA_LEGEND_TEXT,
        AREA_WITH_WINDOWS_LEGEND_TEXT,
        OCR_LEGEND_TEXT,
    ] {
        assert!(legend.contains("Esc"), "every selector says how to leave");
    }
}

#[test]
fn the_shared_legend_paints_across_the_top_of_any_selector() {
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 400).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_legend(&UiTextEngine::default(), &ctx, (800, 400), OCR_LEGEND_TEXT);
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();
    let alpha = |x: usize, y: usize| data[y * stride + x * 4 + 3];

    assert!(alpha(400, 24) > 0, "the strip sits along the top edge");
    assert_eq!(alpha(400, 300), 0, "and nowhere else");
}

#[test]
fn pointer_panel_prefers_below_right_then_flips_and_clamps() {
    assert_eq!(
        pointer_panel_layout((20.0, 30.0), 60.0, (400, 300)),
        PointerPanelLayout {
            x: 35.0,
            y: 45.0,
            width: 76.0,
            height: 22.0,
        }
    );
    assert_eq!(
        pointer_panel_layout((390.0, 290.0), 60.0, (400, 300)),
        PointerPanelLayout {
            x: 299.0,
            y: 253.0,
            width: 76.0,
            height: 22.0,
        }
    );
    assert_eq!(
        pointer_panel_layout((2.0, 2.0), 120.0, (80, 20)),
        PointerPanelLayout {
            x: 6.0,
            y: 6.0,
            width: 68.0,
            height: 8.0,
        }
    );
}

#[test]
fn selected_area_is_cut_out_of_the_scrim() {
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_capture_picker(
        &UiTextEngine::default(),
        &ctx,
        40,
        40,
        &RegionCapturePickerVisual {
            selection: Some(RegionSelection {
                start: (10.0, 10.0),
                end: (30.0, 30.0),
            }),
            pointer: (30.0, 30.0),
            measurement: None,
            show_scrim: true,
            review: false,
            resize_handles: None,
            hovered_handle: None,
            show_legend: false,
            loupe: None,
            action_bar: None,
            hovered_action: None,
            include_drawings: false,
            cut: Default::default(),
            window: RegionCaptureWindowVisual::disabled(),
        },
        |_x, _y| None,
    );
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();
    let alpha = |x: usize, y: usize| data[y * stride + x * 4 + 3];
    assert!(alpha(2, 2) > 0, "outside the selection must be scrimmed");
    assert_eq!(alpha(20, 20), 0, "the selected pixels must remain clear");
}

#[test]
fn highlighted_window_is_cut_out_in_window_mode() {
    let targets = [
        RegionSelection {
            start: (4.0, 4.0),
            end: (14.0, 14.0),
        },
        RegionSelection {
            start: (20.0, 20.0),
            end: (36.0, 36.0),
        },
    ];
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_capture_picker(
        &UiTextEngine::default(),
        &ctx,
        40,
        40,
        &RegionCapturePickerVisual {
            selection: None,
            pointer: (28.0, 28.0),
            measurement: None,
            show_scrim: true,
            review: false,
            resize_handles: None,
            hovered_handle: None,
            show_legend: false,
            loupe: None,
            action_bar: None,
            hovered_action: None,
            include_drawings: false,
            cut: Default::default(),
            window: RegionCaptureWindowVisual {
                available: true,
                active: true,
                targets: &targets,
                highlighted_target: Some(1),
            },
        },
        |_x, _y| None,
    );
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();
    let alpha = |x: usize, y: usize| data[y * stride + x * 4 + 3];
    assert!(alpha(1, 1) > 0, "outside windows remains scrimmed");
    assert!(alpha(9, 9) > 0, "an unhighlighted window remains scrimmed");
    assert_eq!(alpha(28, 28), 0, "highlighted window is the clear target");
}

#[test]
fn window_mode_omits_the_area_crosshair() {
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_capture_picker(
        &UiTextEngine::default(),
        &ctx,
        40,
        40,
        &RegionCapturePickerVisual {
            selection: None,
            pointer: (20.0, 20.0),
            measurement: None,
            show_scrim: false,
            review: false,
            resize_handles: None,
            hovered_handle: None,
            show_legend: false,
            loupe: None,
            action_bar: None,
            hovered_action: None,
            include_drawings: false,
            cut: Default::default(),
            window: RegionCaptureWindowVisual {
                available: true,
                active: true,
                targets: &[],
                highlighted_target: None,
            },
        },
        |_x, _y| None,
    );
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();
    assert_eq!(data[20 * stride + 20 * 4 + 3], 0);
}

#[test]
fn highlighted_window_outline_is_stronger_than_other_targets() {
    let targets = [
        RegionSelection {
            start: (4.0, 4.0),
            end: (16.0, 16.0),
        },
        RegionSelection {
            start: (24.0, 4.0),
            end: (36.0, 16.0),
        },
    ];
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 20).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_capture_picker(
        &UiTextEngine::default(),
        &ctx,
        40,
        20,
        &RegionCapturePickerVisual {
            selection: None,
            pointer: (20.0, 18.0),
            measurement: None,
            show_scrim: false,
            review: false,
            resize_handles: None,
            hovered_handle: None,
            show_legend: false,
            loupe: None,
            action_bar: None,
            hovered_action: None,
            include_drawings: false,
            cut: Default::default(),
            window: RegionCaptureWindowVisual {
                available: true,
                active: true,
                targets: &targets,
                highlighted_target: Some(1),
            },
        },
        |_x, _y| None,
    );
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();
    let edge_alpha = |x: usize| {
        (3..=6)
            .flat_map(|y| ((x - 1)..=(x + 1)).map(move |sample_x| (sample_x, y)))
            .map(|(sample_x, y)| u32::from(data[y * stride + sample_x * 4 + 3]))
            .sum::<u32>()
    };
    assert!(
        edge_alpha(24) > edge_alpha(4),
        "the highlighted candidate must be visually stronger"
    );
}

#[test]
fn measure_visual_leaves_the_screen_unscrimmed() {
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_capture_picker(
        &UiTextEngine::default(),
        &ctx,
        40,
        40,
        &RegionCapturePickerVisual {
            selection: None,
            pointer: (20.0, 20.0),
            measurement: Some("20, 20"),
            show_scrim: false,
            review: false,
            resize_handles: None,
            hovered_handle: None,
            show_legend: false,
            loupe: None,
            action_bar: None,
            hovered_action: None,
            include_drawings: false,
            cut: Default::default(),
            window: RegionCaptureWindowVisual::disabled(),
        },
        |_x, _y| None,
    );
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();
    assert_eq!(data[2 * stride + 2 * 4 + 3], 0, "no measure scrim");
    assert!(data[20 * stride + 20 * 4 + 3] > 0, "measure crosshair");
}

#[test]
fn measure_damage_uses_thin_chrome_regions_instead_of_the_full_surface() {
    let damage = measure_picker_damage(
        Some(RegionSelection {
            start: (100.0, 80.0),
            end: (700.0, 500.0),
        }),
        (400.0, 300.0),
        (800, 600),
    );

    assert!(damage.len() >= 7);
    assert!(damage.iter().all(|rect| {
        rect.x >= 0
            && rect.y >= 0
            && rect.x + rect.width <= 800
            && rect.y + rect.height <= 600
            && (rect.width < 800 || rect.height < 600)
    }));
    assert!(
        damage
            .iter()
            .any(|rect| rect.width == 800 && rect.height <= 5)
    );
    assert!(
        damage
            .iter()
            .any(|rect| rect.height == 600 && rect.width <= 5)
    );
}

#[test]
fn crosshair_remains_visible_while_selecting() {
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_capture_picker(
        &UiTextEngine::default(),
        &ctx,
        40,
        40,
        &RegionCapturePickerVisual {
            selection: Some(RegionSelection {
                start: (10.0, 10.0),
                end: (30.0, 30.0),
            }),
            pointer: (20.0, 20.0),
            measurement: None,
            show_scrim: true,
            review: false,
            resize_handles: None,
            hovered_handle: None,
            show_legend: false,
            loupe: None,
            action_bar: None,
            hovered_action: None,
            include_drawings: false,
            cut: Default::default(),
            window: RegionCaptureWindowVisual::disabled(),
        },
        |_x, _y| None,
    );
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();
    assert!(
        data[20 * stride + 20 * 4 + 3] > 0,
        "crosshair must be painted inside the clear selection"
    );
}

#[test]
fn review_visual_composes_the_action_bar_after_the_scrim() {
    let selection = RegionSelection {
        start: (100.0, 100.0),
        end: (300.0, 200.0),
    };
    let bar = crate::ui::RegionActionBar::place(selection, (800, 600));
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_capture_picker(
        &UiTextEngine::default(),
        &ctx,
        800,
        600,
        &RegionCapturePickerVisual {
            selection: Some(selection),
            pointer: (200.0, 150.0),
            measurement: Some("200 × 100"),
            show_scrim: true,
            review: true,
            resize_handles: None,
            hovered_handle: None,
            show_legend: false,
            loupe: None,
            action_bar: Some(bar),
            hovered_action: Some(crate::ui::RegionAction::Both),
            include_drawings: true,
            cut: Default::default(),
            window: RegionCaptureWindowVisual::disabled(),
        },
        |_x, _y| None,
    );
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();
    assert!(data[250 * stride + 40 * 4 + 3] > 0, "action bar surface");
    assert_eq!(
        data[150 * stride + 200 * 4 + 3],
        0,
        "review drops the targeting crosshair; the selection stays clear"
    );
    assert!(
        data[80 * stride + 110 * 4 + 3] > 0,
        "the size badge parks above the selection's top-left corner"
    );
}

#[test]
fn the_review_size_badge_drops_inside_a_selection_flush_with_the_top_edge() {
    let above = selection_badge_layout((40.0, 120.0, 200.0, 100.0), 60.0, None, (400, 300));
    assert_eq!(
        above,
        PointerPanelLayout {
            x: 40.0,
            y: 92.0,
            width: 76.0,
            height: 22.0,
        }
    );

    let flush = selection_badge_layout((40.0, 4.0, 200.0, 100.0), 60.0, None, (400, 300));
    assert_eq!(
        flush,
        PointerPanelLayout {
            x: 40.0,
            y: 10.0,
            width: 76.0,
            height: 22.0,
        },
        "no room above: the badge drops just inside the rectangle"
    );

    let clamped = selection_badge_layout((380.0, 200.0, 20.0, 20.0), 60.0, None, (400, 300));
    assert_eq!(clamped.x, 318.0, "the badge stays on screen");
}

#[test]
fn a_bar_that_flipped_above_the_selection_pushes_the_badge_inside_it() {
    // A selection low on the screen leaves no room for the bar below it,
    // so the bar takes the space the badge would otherwise use.
    let selection = RegionSelection {
        start: (730.0, 560.0),
        end: (790.0, 590.0),
    };
    let bar = RegionActionBar::place(selection, (800, 600));
    let bounds = bar.bounds();
    assert!(
        bounds.y + bounds.height < 560.0,
        "precondition: the bar flipped above the selection"
    );

    let rect = normalized_rect(selection);
    let badge = selection_badge_layout(rect, 60.0, Some(bounds), (800, 600));
    assert!(
        !covered_by_action_bar(badge.x, badge.y, badge.width, badge.height, Some(bounds)),
        "the badge must not be painted under the bar"
    );
    assert!(
        badge.y >= 560.0,
        "it drops inside the rectangle instead of above it"
    );

    // Without the bar the same selection keeps the outside-above spot.
    let unobstructed = selection_badge_layout(rect, 60.0, None, (800, 600));
    assert!(unobstructed.y < 560.0);
}

/// The layout choice above only helps if the composed frame agrees: the
/// bar is painted after the badge, so a badge under it would simply
/// disappear.
#[test]
fn the_flipped_above_review_bar_never_paints_over_the_size_badge() {
    let selection = RegionSelection {
        start: (730.0, 560.0),
        end: (790.0, 590.0),
    };
    let bar = RegionActionBar::place(selection, (800, 600));
    let render = |measurement: Option<&str>| {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_capture_picker(
            &UiTextEngine::default(),
            &ctx,
            800,
            600,
            &RegionCapturePickerVisual {
                selection: Some(selection),
                pointer: (760.0, 575.0),
                measurement,
                show_scrim: true,
                review: true,
                resize_handles: None,
                hovered_handle: None,
                show_legend: false,
                loupe: None,
                action_bar: Some(bar),
                hovered_action: None,
                include_drawings: false,
                cut: Default::default(),
                window: RegionCaptureWindowVisual::disabled(),
            },
            |_x, _y| None,
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap().to_vec();
        (data, stride)
    };

    let (with_badge, stride) = render(Some("60 × 30"));
    let (without_badge, _) = render(None);
    let bounds = bar.bounds();
    let mut visible_badge_pixels = 0usize;
    for y in 0..600 {
        for x in 0..800 {
            let offset = y * stride + x * 4;
            if with_badge[offset..offset + 4] == without_badge[offset..offset + 4] {
                continue;
            }
            let inside_bar = (bounds.x..bounds.x + bounds.width).contains(&(x as f64))
                && (bounds.y..bounds.y + bounds.height).contains(&(y as f64));
            if !inside_bar {
                visible_badge_pixels += 1;
            }
        }
    }
    // The badge box is 76 x 22; essentially all of it must survive.
    assert!(
        visible_badge_pixels > 1_000,
        "only {visible_badge_pixels} badge pixels escaped the action bar"
    );
}

#[test]
fn a_badge_blocked_on_the_left_slides_to_the_selections_right_edge() {
    // A bar clamped over the top-left of a large selection: above and
    // inside-left are both covered, inside-right is clear.
    let bar = RegionActionRect::new(0.0, 0.0, 300.0, 90.0);
    let badge = selection_badge_layout((20.0, 40.0, 600.0, 400.0), 60.0, Some(bar), (800, 600));
    assert!(!covered_by_action_bar(
        badge.x,
        badge.y,
        badge.width,
        badge.height,
        Some(bar)
    ));
    assert_eq!(badge.x, 544.0, "right-aligned inside the selection");
}

#[test]
fn a_badge_with_no_clear_placement_falls_back_inside_the_selection() {
    // A full-width bar over the whole rectangle leaves nothing clear; the
    // badge still lands on the selection rather than somewhere arbitrary.
    let bar = RegionActionRect::new(0.0, 0.0, 800.0, 600.0);
    let rect = (20.0, 40.0, 600.0, 400.0);
    assert_eq!(
        selection_badge_layout(rect, 60.0, Some(bar), (800, 600)),
        PointerPanelLayout {
            x: 20.0,
            y: 46.0,
            width: 76.0,
            height: 22.0,
        },
        "inside the rectangle's top-left corner is the fallback"
    );
}

#[test]
fn review_paints_grips_where_targeting_paints_corner_arms() {
    let selection = RegionSelection {
        start: (60.0, 60.0),
        end: (240.0, 200.0),
    };
    let render = |review: bool| {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 300, 260).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_capture_picker(
            &UiTextEngine::default(),
            &ctx,
            300,
            260,
            &RegionCapturePickerVisual {
                selection: Some(selection),
                // Park the pointer in a corner: targeting still paints a
                // crosshair, and its two lines must not cross the pixels
                // this test samples.
                pointer: (8.0, 252.0),
                measurement: None,
                // No scrim, so only chrome puts ink on the surface and
                // the two frames can be compared pixel for pixel.
                show_scrim: false,
                review,
                resize_handles: review.then(|| crate::ui::RegionResizeHandles::place(selection)),
                hovered_handle: None,
                show_legend: false,
                loupe: None,
                action_bar: None,
                hovered_action: None,
                include_drawings: false,
                cut: Default::default(),
                window: RegionCaptureWindowVisual::disabled(),
            },
            |_x, _y| None,
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        (surface.data().unwrap().to_vec(), stride)
    };

    let (reviewing, stride) = render(true);
    let (targeting, _) = render(false);
    let alpha = |data: &[u8], x: usize, y: usize| data[y * stride + x * 4 + 3];

    // An edge midpoint carries a grip only in Review, and the chip reaches
    // above the frame line the two modes share.
    assert!(alpha(&reviewing, 150, 57) > 0, "top edge grip");
    assert_eq!(alpha(&targeting, 150, 57), 0, "targeting has no edge grip");
    assert!(alpha(&reviewing, 60, 60) > 0, "top-left corner grip");

    // The corner arms run inward along both edges from each corner. Review
    // drops them so they cannot fight the corner grip for the same pixels;
    // this row is past the grip but still inside the arm's reach.
    assert!(
        alpha(&targeting, 59, 75) > 0,
        "targeting draws a corner arm below the top-left corner"
    );
    assert_eq!(
        alpha(&reviewing, 59, 75),
        0,
        "review drops the arm; the grip covers the corner instead"
    );
}

#[test]
fn capture_loupe_reuses_the_pixel_loupe_renderer_when_enabled() {
    let visual = RegionCaptureLoupeVisual::when_enabled(true, (20.0, 30.0), (50.0, 50.0))
        .expect("enabled immutable option");
    assert!(RegionCaptureLoupeVisual::when_enabled(false, (20.0, 30.0), (50.0, 50.0),).is_none());

    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 200).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_capture_loupe(&ctx, (200, 200), visual, |_x, _y| {
        Some(crate::draw::Color::new(1.0, 0.0, 0.0, 1.0))
    });
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();
    assert!(data[98 * stride + 88 * 4 + 3] > 0, "loupe center pixel");
    assert_eq!(data[5 * stride + 5 * 4 + 3], 0, "outside untouched");
}

#[test]
fn accepted_cut_preview_paints_before_the_scrim_hole() {
    let pixels = PackedArgb32::new(
        2,
        1,
        8,
        [0x33, 0x22, 0x11, 0xFF, 0xCC, 0xBB, 0xAA, 0xFF].to_vec(),
    )
    .unwrap();
    let created = unsafe {
        cairo::ImageSurface::create_for_data_unsafe(
            pixels.data().as_ptr() as *mut u8,
            cairo::Format::ARgb32,
            2,
            1,
            8,
        )
    };
    assert!(
        created.is_ok(),
        "preview pixels must be a valid Cairo source: {created:?}"
    );
    drop(created);
    // Large enough that the 4–20px corner arms cannot cover the samples,
    // and far enough from the 1px frame. 2×1 source scales 16× onto this
    // 32×16 display: (18, 18) is inside the first source pixel, (34, 18)
    // inside the second.
    let display = RegionSelection {
        start: (10.0, 10.0),
        end: (42.0, 26.0),
    };
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 60, 40).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    render_region_capture_picker(
        &UiTextEngine::default(),
        &ctx,
        60,
        40,
        &RegionCapturePickerVisual {
            selection: Some(display),
            pointer: (4.0, 4.0),
            measurement: None,
            show_scrim: true,
            review: true,
            resize_handles: None,
            hovered_handle: None,
            show_legend: false,
            loupe: None,
            action_bar: None,
            hovered_action: None,
            include_drawings: false,
            cut: RegionCaptureCutVisual {
                preview: Some(RegionCutPreviewVisual {
                    pixels: &pixels,
                    display,
                }),
                ..Default::default()
            },
            window: RegionCaptureWindowVisual::disabled(),
        },
        |_x, _y| None,
    );
    drop(ctx);
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();
    let alpha = |x: usize, y: usize| data[y * stride + x * 4 + 3];
    assert!(alpha(18, 18) > 0, "preview occupies the displayed output");
    assert!(alpha(2, 2) > 0, "vacated source is dimmed by the scrim");
    assert_eq!(
        &data[18 * stride + 18 * 4..18 * stride + 18 * 4 + 4],
        &[0x33, 0x22, 0x11, 0xFF],
        "first source pixel fills the left half of the displayed output"
    );
    assert_eq!(
        &data[18 * stride + 34 * 4..18 * stride + 34 * 4 + 4],
        &[0xCC, 0xBB, 0xAA, 0xFF],
        "second source pixel fills the right half of the displayed output"
    );
}
#[test]
fn retained_text_owner_matches_fresh_picker_pixels_across_modes_and_density() {
    let engine = UiTextEngine::default();
    let selection = RegionSelection {
        start: (90.0, 70.0),
        end: (480.0, 220.0),
    };
    for density in [1, 2, 1] {
        for (review, show_scrim) in [(false, true), (true, true), (false, false)] {
            let paint = |engine: &UiTextEngine| {
                let mut surface = cairo::ImageSurface::create(
                    cairo::Format::ARgb32,
                    640 * density,
                    480 * density,
                )
                .unwrap();
                {
                    let ctx = cairo::Context::new(&surface).unwrap();
                    ctx.scale(f64::from(density), f64::from(density));
                    render_region_capture_picker(
                        engine,
                        &ctx,
                        640,
                        480,
                        &RegionCapturePickerVisual {
                            selection: Some(selection),
                            pointer: (480.0, 220.0),
                            measurement: Some("390 × 150"),
                            show_scrim,
                            review,
                            resize_handles: None,
                            hovered_handle: None,
                            show_legend: false,
                            loupe: None,
                            action_bar: review
                                .then(|| RegionActionBar::place(selection, (640, 480))),
                            hovered_action: None,
                            include_drawings: true,
                            cut: Default::default(),
                            window: RegionCaptureWindowVisual::disabled(),
                        },
                        |_, _| None,
                    );
                    render_region_legend(engine, &ctx, (640, 480), OCR_LEGEND_TEXT);
                }
                surface.data().unwrap().to_vec()
            };
            let actual = paint(&engine);
            assert!(actual.iter().any(|&byte| byte != 0));
            assert!(
                actual == paint(&UiTextEngine::default()),
                "retained picker pixels differ"
            );
        }
    }
}
