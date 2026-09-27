use super::super::metrics::{ACTIONS_HEIGHT, FOOTER_HEIGHT, PADDING_BOTTOM};
use super::super::types::{
    PanelRect, PropertiesPanelHit, PropertiesRowControl, PropertiesRowGeometry,
};
use crate::domain::color::PALETTE_RED;
use crate::draw::{ArrowStyle, Shape, ShapeId, TextMeasurer};
use crate::input::state::InputState;
use crate::ui_text::UiTextEngine;

const SCREEN: (u32, u32) = (800, 600);

fn arrow(state: &mut InputState) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Arrow {
        x1: 40,
        y1: 60,
        x2: 200,
        y2: 90,
        color: PALETTE_RED,
        thick: 3.0,
        arrow_length: 24.0,
        arrow_angle: 35.0,
        head_at_end: true,
        style: ArrowStyle::Pointy,
        bend: 0.0,
        label: None,
    })
}

fn text(state: &mut InputState, text: &str) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 60,
        text: text.into(),
        color: PALETTE_RED,
        size: 18.0,
        font_descriptor: Default::default(),
        background_enabled: false,
        wrap_width: None,
    })
}

fn lay_out(state: &mut InputState, screen: (u32, u32)) {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    state.update_properties_panel_layout(&ctx, screen.0, screen.1);
}

fn open_panel(ids: Vec<ShapeId>, state: &mut InputState) {
    state.set_selection(ids);
    assert!(state.show_properties_panel_with(&TextMeasurer::default()));
    lay_out(state, SCREEN);
}

fn rows(state: &InputState) -> Vec<PropertiesRowGeometry> {
    let layout = state.properties_panel_layout().expect("layout");
    layout.rows(state.properties_panel().expect("panel"))
}

fn paint(state: &InputState) -> Vec<u8> {
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, SCREEN.0 as i32, SCREEN.1 as i32)
            .unwrap();
    {
        let ctx = cairo::Context::new(&surface).unwrap();
        crate::ui::render_properties_panel_with_engine(
            &UiTextEngine::default(),
            &ctx,
            state,
            SCREEN.0,
            SCREEN.1,
        );
    }
    surface.data().unwrap().to_vec()
}

fn inside(outer: PanelRect, inner: PanelRect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.right() <= outer.right() + 1e-9
        && inner.bottom() <= outer.bottom() + 1e-9
}

/// Every clickable rectangle of a row, with the hit its center should give.
fn control_hits(row: &PropertiesRowGeometry) -> Vec<(PanelRect, PropertiesPanelHit)> {
    let index = row.index;
    match &row.control {
        PropertiesRowControl::Swatches { swatches, more } => swatches
            .iter()
            .enumerate()
            .map(|(swatch, rect)| {
                (
                    *rect,
                    PropertiesPanelHit::Swatch {
                        row: index,
                        index: swatch,
                    },
                )
            })
            .chain([(*more, PropertiesPanelHit::MoreColors(index))])
            .collect(),
        PropertiesRowControl::Stepper { down, up, .. } => vec![
            (*down, PropertiesPanelHit::StepDown(index)),
            (*up, PropertiesPanelHit::StepUp(index)),
        ],
        PropertiesRowControl::Toggle { switch } => {
            vec![(*switch, PropertiesPanelHit::Toggle(index))]
        }
        PropertiesRowControl::Slider { track, .. } => {
            vec![(*track, PropertiesPanelHit::Slider(index))]
        }
        PropertiesRowControl::ArrowHead { start, end, .. } => vec![
            (
                *start,
                PropertiesPanelHit::ArrowHead {
                    row: index,
                    at_end: false,
                },
            ),
            (
                *end,
                PropertiesPanelHit::ArrowHead {
                    row: index,
                    at_end: true,
                },
            ),
        ],
        PropertiesRowControl::ArrowStyles { buttons } => buttons
            .iter()
            .map(|(style, rect)| {
                (
                    *rect,
                    PropertiesPanelHit::ArrowStyle {
                        row: index,
                        style: *style,
                    },
                )
            })
            .collect(),
    }
}

#[test]
fn rows_stack_from_the_divider_to_the_footer() {
    let mut state = crate::input::state::test_support::make_test_input_state();
    let id = arrow(&mut state);
    open_panel(vec![id], &mut state);
    let layout = *state.properties_panel_layout().expect("layout");
    let rows = rows(&state);

    assert_eq!(rows.len(), state.properties_panel().unwrap().entries.len());
    assert!(layout.divider_y < layout.rows_top);
    let mut top = layout.rows_top;
    for row in &rows {
        assert_eq!(
            row.rect.y, top,
            "row {} starts where the last ended",
            row.index
        );
        top = row.rect.bottom();
    }
    assert_eq!(layout.actions_top, top, "the actions sit under the rows");
    let footer_top = layout.footer_top.expect("footer");
    assert_eq!(footer_top, top + ACTIONS_HEIGHT);
    assert_eq!(
        layout.origin_y + layout.height,
        (footer_top + FOOTER_HEIGHT + PADDING_BOTTOM).ceil()
    );
}

#[test]
fn every_control_is_hit_where_it_is_drawn_and_stays_in_its_row() {
    let mut state = crate::input::state::test_support::make_test_input_state();
    let arrow = arrow(&mut state);
    let note = text(&mut state, "Label");
    let rect = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 300,
        y: 300,
        w: 40,
        h: 40,
        fill: false,
        color: PALETTE_RED,
        thick: 2.0,
    });
    open_panel(vec![arrow, note, rect], &mut state);
    let layout = *state.properties_panel_layout().expect("layout");
    let panel = state.properties_panel().expect("panel");

    for row in rows(&state) {
        assert!(inside(layout.rect(), row.rect), "row {} fits", row.index);
        for (rect, expected) in control_hits(&row) {
            assert!(inside(row.rect, rect), "{expected:?} sits in its row");
            assert!(
                rect.x >= row.content_x - 1e-9 && rect.right() <= row.content_right + 1e-9,
                "{expected:?} stays in the content column"
            );
            let (cx, cy) = rect.center();
            assert_eq!(layout.hit_at(panel, cx, cy), Some(expected));
        }
    }
    let (lock_x, lock_y) = layout.lock.center();
    assert_eq!(
        layout.hit_at(panel, lock_x, lock_y),
        Some(PropertiesPanelHit::Lock)
    );
    let (title_x, title_y) = layout.title_rect().center();
    assert_eq!(
        layout.hit_at(panel, title_x, title_y),
        Some(PropertiesPanelHit::Title)
    );
    assert_eq!(layout.hit_at(panel, layout.origin_x - 5.0, title_y), None);
}

#[test]
fn a_small_screen_keeps_the_whole_panel_on_screen() {
    let mut state = crate::input::state::test_support::make_test_input_state();
    let id = arrow(&mut state);
    state.set_selection(vec![id]);
    assert!(state.show_properties_panel_with(&TextMeasurer::default()));

    lay_out(&mut state, (360, 560));

    let layout = state.properties_panel_layout().expect("layout");
    assert!(layout.origin_x >= 12.0 && layout.origin_y >= 12.0);
    assert!(layout.origin_x + layout.width <= 360.0 - 12.0 + 1e-9);
    assert!(layout.origin_y + layout.height <= 560.0 - 12.0 + 1e-9);
}

#[test]
fn hovering_a_swatch_shows_its_name_in_a_tooltip_on_screen() {
    let mut state = crate::input::state::test_support::make_test_input_state();
    let id = arrow(&mut state);
    open_panel(vec![id], &mut state);
    let (swatch, expected) = rows(&state)
        .iter()
        .flat_map(control_hits)
        .find(|(_, hit)| matches!(hit, PropertiesPanelHit::Swatch { index: 1, .. }))
        .expect("second swatch");
    let (x, y) = swatch.center();

    state.update_pointer_position(x as i32, y as i32);
    state.update_properties_panel_hover_from_pointer(x as i32, y as i32);
    lay_out(&mut state, SCREEN);

    let panel = state.properties_panel().expect("panel");
    assert_eq!(panel.hover, Some(expected));
    assert_eq!(
        panel.tooltip(expected),
        Some(panel.swatches[1].label.clone())
    );
    let tooltip = state
        .properties_panel_layout()
        .and_then(|layout| layout.tooltip)
        .expect("tooltip");
    assert!(
        tooltip.y >= swatch.bottom(),
        "the tooltip hangs below the swatch"
    );
    assert!(tooltip.x >= 0.0 && tooltip.right() <= SCREEN.0 as f64);
}

#[test]
fn deferred_refresh_updates_the_header_before_layout_and_paint() {
    let mut state = crate::input::state::test_support::make_test_input_state();
    let id = text(&mut state, "你好 initial");
    open_panel(vec![id], &mut state);
    let before = state.properties_panel().unwrap().subtitle.clone();
    let painted = paint(&state);
    assert!(painted.iter().any(|&byte| byte != 0), "the panel paints");
    assert_eq!(painted, paint(&state), "painting is deterministic");

    {
        let frame = state.boards.active_frame_mut();
        let shape = frame.shape_mut(id).unwrap();
        if let Shape::Text { text, .. } = &mut shape.shape {
            *text = "A substantially wider changed text label".into();
        }
        shape.invalidate_bounds();
    }
    state.properties.mark_needs_refresh();
    lay_out(&mut state, SCREEN);

    assert!(!state.properties.needs_refresh());
    assert_ne!(
        state.properties_panel().unwrap().subtitle,
        before,
        "the size in the subtitle follows the edit"
    );

    state.selection_interaction.set(Vec::new());
    state.properties.mark_needs_refresh();
    lay_out(&mut state, SCREEN);
    assert!(state.properties_panel().is_none());
    assert!(state.properties_panel_layout().is_none());
}

fn spotlight(state: &mut InputState) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Spotlight {
        cx: 400,
        cy: 300,
        rx: 40,
        ry: 30,
        magnification: 2.0,
    })
}

/// Every row and control of the open panel is on a screen `height` tall and
/// answers the pointer where it is drawn.
fn assert_all_reachable(state: &InputState, height: f64) {
    let layout = *state.properties_panel_layout().expect("layout");
    let panel = state.properties_panel().expect("panel");
    assert!(layout.origin_y >= 12.0 - 1e-9);
    assert!(
        layout.origin_y + layout.height <= height - 12.0 + 1e-9,
        "panel bottom {} passes the screen ({height})",
        layout.origin_y + layout.height
    );
    for row in layout.rows(panel) {
        assert!(inside(layout.rect(), row.rect), "row {} fits", row.index);
        for (rect, expected) in control_hits(&row) {
            let (cx, cy) = rect.center();
            assert_eq!(layout.hit_at(panel, cx, cy), Some(expected));
        }
    }
}

#[test]
fn a_slightly_short_screen_drops_the_keyboard_hints_first() {
    let mut state = crate::input::state::test_support::make_test_input_state();
    let id = arrow(&mut state);
    open_panel(vec![id], &mut state);
    let full = state.properties_panel_layout().expect("layout").height;

    let height = full + 24.0 - FOOTER_HEIGHT / 2.0;
    lay_out(&mut state, (SCREEN.0, height as u32));

    let layout = *state.properties_panel_layout().expect("layout");
    assert_eq!(layout.footer_top, None, "the hints go");
    assert_eq!(layout.column_budget, f64::INFINITY, "one column still fits");
    assert_all_reachable(&state, height.floor());
}

#[test]
fn a_selection_taller_than_the_screen_flows_into_columns() {
    let mut state = crate::input::state::test_support::make_test_input_state();
    let rect = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 300,
        y: 200,
        w: 40,
        h: 40,
        fill: false,
        color: PALETTE_RED,
        thick: 2.0,
    });
    let ids = vec![
        rect,
        arrow(&mut state),
        text(&mut state, "Note"),
        spotlight(&mut state),
    ];
    open_panel(ids, &mut state);
    lay_out(&mut state, (SCREEN.0, 2000));
    let single = *state.properties_panel_layout().expect("layout");
    assert_eq!(
        single.column_budget,
        f64::INFINITY,
        "one column on a tall screen"
    );

    lay_out(&mut state, (SCREEN.0, 480));

    let layout = *state.properties_panel_layout().expect("layout");
    let rows = rows(&state);
    let columns: std::collections::BTreeSet<u64> =
        rows.iter().map(|row| row.content_x.to_bits()).collect();
    assert!(columns.len() > 1, "the rows spread over columns");
    assert!(layout.width > single.width);
    assert!(layout.origin_x + layout.width <= SCREEN.0 as f64 - 12.0 + 1e-9);
    assert_all_reachable(&state, 480.0);
    let heights: Vec<f64> = columns
        .iter()
        .map(|x| {
            rows.iter()
                .filter(|row| row.content_x.to_bits() == *x)
                .map(|row| row.rect.height)
                .sum()
        })
        .collect();
    let (short, tall) = heights
        .iter()
        .fold((f64::MAX, 0.0_f64), |(lo, hi), h| (lo.min(*h), hi.max(*h)));
    assert!(
        tall - short < 100.0,
        "the columns come out about even: {heights:?}"
    );
}

fn four_kinds(state: &mut InputState) -> Vec<ShapeId> {
    let rect = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 300,
        y: 200,
        w: 40,
        h: 40,
        fill: false,
        color: PALETTE_RED,
        thick: 2.0,
    });
    vec![rect, arrow(state), text(state, "Note"), spotlight(state)]
}

/// Every control of every row wholly inside the scroll viewport answers the
/// pointer where it is drawn, and those rows sit on screen.
fn assert_visible_rows_reachable(state: &InputState, screen: (f64, f64)) -> Vec<usize> {
    let layout = *state.properties_panel_layout().expect("layout");
    let panel = state.properties_panel().expect("panel");
    let viewport = layout.rows_viewport();
    let mut visible = Vec::new();
    for row in layout.rows(panel) {
        if !inside(viewport, row.rect) {
            continue;
        }
        visible.push(row.index);
        assert!(row.rect.right() <= screen.0 - 12.0 + 1e-9);
        assert!(row.rect.bottom() <= screen.1 - 12.0 + 1e-9);
        for (rect, expected) in control_hits(&row) {
            let (cx, cy) = rect.center();
            assert_eq!(layout.hit_at(panel, cx, cy), Some(expected));
        }
    }
    visible
}

#[test]
fn when_columns_would_leave_the_screen_the_rows_scroll_instead() {
    let measurer = TextMeasurer::default();
    let screen = (480, 360);
    let mut state = crate::input::state::test_support::make_test_input_state();
    let ids = four_kinds(&mut state);
    open_panel(ids, &mut state);

    lay_out(&mut state, screen);

    let layout = *state.properties_panel_layout().expect("layout");
    let scroll = layout.scroll.expect("the rows scroll");
    assert_eq!(scroll.offset, 0.0);
    assert!(layout.origin_x >= 12.0 - 1e-9);
    assert!(
        layout.origin_x + layout.width <= 480.0 - 12.0 + 1e-9,
        "no column runs off the right"
    );
    assert!(layout.origin_y + layout.height <= 360.0 - 12.0 + 1e-9);
    assert!(layout.lock.right() <= 480.0 - 12.0 + 1e-9);
    let first = assert_visible_rows_reachable(&state, (480.0, 360.0));
    assert!(first.contains(&0));

    // A row clipped out of the viewport takes no clicks; what is drawn
    // there (the actions, or nothing) does.
    let hidden = rows(&state)
        .into_iter()
        .find(|row| row.rect.y >= layout.rows_viewport().bottom())
        .expect("a row below the viewport");
    let (hx, hy) = hidden.rect.center();
    let hit = layout.hit_at(state.properties_panel().unwrap(), hx, hy);
    assert!(
        hit.and_then(PropertiesPanelHit::row).is_none(),
        "{hit:?} reached a clipped row"
    );

    // The wheel scrolls rather than stepping a row, down to the last row.
    let (x, y) = layout.rows_viewport().center();
    for _ in 0..40 {
        assert!(state.properties_panel_wheel_with(&measurer, x as i32, y as i32, 1));
    }
    lay_out(&mut state, screen);
    let scroll = state
        .properties_panel_layout()
        .unwrap()
        .scroll
        .expect("still scrolling");
    assert_eq!(scroll.offset, scroll.max_offset);
    let last = state.properties_panel().unwrap().entries.len() - 1;
    assert!(assert_visible_rows_reachable(&state, (480.0, 360.0)).contains(&last));
}

#[test]
fn keyboard_focus_scrolls_its_row_into_view() {
    let screen = (480, 360);
    let mut state = crate::input::state::test_support::make_test_input_state();
    let ids = four_kinds(&mut state);
    open_panel(ids, &mut state);
    lay_out(&mut state, screen);
    let last = state.properties_panel().unwrap().entries.len() - 1;

    state.set_properties_panel_focus(Some(last));
    lay_out(&mut state, screen);

    let layout = *state.properties_panel_layout().expect("layout");
    let row = rows(&state).into_iter().nth(last).expect("last row");
    assert!(inside(layout.rows_viewport(), row.rect));

    state.set_properties_panel_focus(Some(0));
    lay_out(&mut state, screen);
    let row = rows(&state).into_iter().next().expect("first row");
    assert!(inside(
        state.properties_panel_layout().unwrap().rows_viewport(),
        row.rect
    ));
}

#[test]
fn scrolling_after_a_click_moves_hover_off_the_control_that_scrolled_away() {
    let measurer = TextMeasurer::default();
    let screen = (480, 360);
    let mut state = crate::input::state::test_support::make_test_input_state();
    let ids = four_kinds(&mut state);
    open_panel(ids, &mut state);
    lay_out(&mut state, screen);
    let swatch = PropertiesPanelHit::Swatch { row: 0, index: 1 };
    let rect = state
        .properties_panel_layout()
        .unwrap()
        .hit_rect(state.properties_panel().unwrap(), swatch)
        .expect("swatch");
    let (x, y) = (rect.center().0 as i32, rect.center().1 as i32);

    // A click remembers the row quietly, and the pointer rests on the swatch.
    state.update_pointer_position(x, y);
    assert!(state.press_properties_panel_at_with(&measurer, x, y));
    state.release_properties_panel_at_with(&measurer, x, y);
    lay_out(&mut state, screen);
    state.update_properties_panel_hover_from_pointer(x, y);
    lay_out(&mut state, screen);
    assert_eq!(state.properties_panel().unwrap().keyboard_focus, Some(0));
    assert_eq!(state.properties_panel().unwrap().hover, Some(swatch));
    assert!(state.properties_panel_layout().unwrap().tooltip.is_some());

    for _ in 0..3 {
        assert!(state.properties_panel_wheel_with(&measurer, x, y, 1));
    }
    lay_out(&mut state, screen);

    let panel = state.properties_panel().unwrap();
    let layout = state.properties_panel_layout().unwrap();
    assert_ne!(panel.hover, Some(swatch), "the swatch scrolled away");
    assert_eq!(
        panel.hover,
        state.properties_panel_active_hit_at(x, y),
        "hover follows whatever now sits under the pointer"
    );
    if panel.hover.and_then(|hit| panel.tooltip(hit)).is_none() {
        assert!(layout.tooltip.is_none(), "no tooltip left behind");
    }
}

#[test]
fn keyboard_scrolling_drops_the_hover_of_a_swatch_it_scrolls_away() {
    let measurer = TextMeasurer::default();
    let screen = (480, 360);
    let mut state = crate::input::state::test_support::make_test_input_state();
    let ids = four_kinds(&mut state);
    open_panel(ids, &mut state);
    lay_out(&mut state, screen);
    let swatch = PropertiesPanelHit::Swatch { row: 0, index: 1 };
    let rect = state
        .properties_panel_layout()
        .unwrap()
        .hit_rect(state.properties_panel().unwrap(), swatch)
        .expect("swatch");
    let (x, y) = (rect.center().0 as i32, rect.center().1 as i32);
    state.update_pointer_position(x, y);
    state.update_properties_panel_hover_from_pointer(x, y);
    lay_out(&mut state, screen);
    assert_eq!(state.properties_panel().unwrap().hover, Some(swatch));

    let length = state
        .properties_panel()
        .unwrap()
        .entries
        .iter()
        .position(|entry| entry.label == "Arrow length")
        .expect("arrow length row");
    state.set_properties_panel_focus(Some(0));
    while state.properties_panel().unwrap().keyboard_focus != Some(length) {
        assert!(
            state.handle_properties_panel_key_with_measurer(&measurer, crate::input::Key::Down)
        );
        lay_out(&mut state, screen);
    }

    let panel = state.properties_panel().unwrap();
    let layout = state.properties_panel_layout().unwrap();
    assert!(layout.scroll.expect("scrolls").offset > 0.0);
    assert_ne!(panel.hover, Some(swatch), "the swatch scrolled out of view");
    assert_eq!(panel.hover, state.properties_panel_active_hit_at(x, y));
    if panel.hover.and_then(|hit| panel.tooltip(hit)).is_none() {
        assert!(layout.tooltip.is_none(), "no tooltip left over the header");
    }
}
