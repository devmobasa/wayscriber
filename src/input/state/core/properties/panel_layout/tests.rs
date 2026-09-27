use super::super::metrics::{FOOTER_HEIGHT, PADDING_BOTTOM};
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
    let footer_top = layout.footer_top.expect("footer");
    assert_eq!(footer_top, top);
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
    let single = *state.properties_panel_layout().expect("layout");

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
