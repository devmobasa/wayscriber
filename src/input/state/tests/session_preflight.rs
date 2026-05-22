use super::*;
use crate::draw::{EmbeddedImage, EraserBrush, ShapeId};
use crate::input::BOARD_ID_BLACKBOARD;
use crate::session::{self, CompressionMode, SessionOptions};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
enum PointShapeKind {
    Freehand,
    Marker,
    Eraser,
    Pressure,
}

fn board_index(state: &InputState, id: &str) -> usize {
    state
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == id)
        .expect("board index")
}

#[test]
fn page_duplicate_blocks_mixed_image_and_point_strokes_when_real_save_exceeds_limit() {
    for kind in [
        PointShapeKind::Freehand,
        PointShapeKind::Marker,
        PointShapeKind::Eraser,
        PointShapeKind::Pressure,
    ] {
        let points = 10_000;
        let mut options = duplicate_preflight_options_base();
        options.max_file_size_bytes =
            projected_point_page_duplicate_written_size(kind, points, &options).saturating_sub(1)
                as u64;

        let mut state = create_test_input_state();
        let board = board_index(&state, BOARD_ID_BLACKBOARD);
        state.switch_board(BOARD_ID_BLACKBOARD);
        add_active_image_shape(&mut state);
        add_active_point_shape(&mut state, kind, points);
        state.set_session_preflight_options(Some(options));

        state.page_duplicate();

        assert_eq!(
            state.boards.board_states()[board].pages.page_count(),
            1,
            "{kind:?}"
        );
        assert!(
            state
                .ui_toast
                .as_ref()
                .is_some_and(|toast| toast.message.contains("Page duplicate blocked")),
            "{kind:?}"
        );
    }
}

fn add_active_image_shape(state: &mut InputState) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Image {
        x: 10,
        y: 20,
        w: 120,
        h: 90,
        data: EmbeddedImage {
            mime_type: "image/png".to_string(),
            width: 240,
            height: 180,
            bytes: Vec::new(),
        },
    })
}

fn add_active_point_shape(state: &mut InputState, kind: PointShapeKind, points: usize) -> ShapeId {
    let color = state.current_color;
    let thick = state.current_thickness;
    match kind {
        PointShapeKind::Freehand => state.boards.active_frame_mut().add_shape(Shape::Freehand {
            points: point_path(points),
            color,
            thick,
        }),
        PointShapeKind::Marker => state
            .boards
            .active_frame_mut()
            .add_shape(Shape::MarkerStroke {
                points: point_path(points),
                color,
                thick,
            }),
        PointShapeKind::Eraser => state
            .boards
            .active_frame_mut()
            .add_shape(Shape::EraserStroke {
                points: point_path(points),
                brush: EraserBrush {
                    size: thick,
                    kind: EraserKind::Circle,
                },
            }),
        PointShapeKind::Pressure => {
            state
                .boards
                .active_frame_mut()
                .add_shape(Shape::FreehandPressure {
                    points: pressure_point_path(points),
                    color,
                })
        }
    }
}

fn duplicate_preflight_options_base() -> SessionOptions {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "point-preflight");
    options.persist_transparent = true;
    options.persist_whiteboard = true;
    options.persist_blackboard = true;
    options.persist_history = false;
    options.restore_tool_state = false;
    options.compression = CompressionMode::Off;
    options.max_file_size_bytes = u64::MAX;
    options
}

fn point_path(count: usize) -> Vec<(i32, i32)> {
    (0..count)
        .map(|index| {
            let x = i32::try_from(index).unwrap_or(i32::MAX);
            (x, x.saturating_mul(13).rem_euclid(997))
        })
        .collect()
}

fn pressure_point_path(count: usize) -> Vec<(i32, i32, f32)> {
    point_path(count)
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| (x, y, 1.0 + (index % 7) as f32 * 0.25))
        .collect()
}

fn projected_point_page_duplicate_written_size(
    kind: PointShapeKind,
    points: usize,
    options: &SessionOptions,
) -> usize {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_BLACKBOARD);
    add_active_image_shape(&mut state);
    add_active_point_shape(&mut state, kind, points);
    state.page_duplicate();
    let snapshot = session::snapshot_from_input(&state, options).expect("snapshot present");
    session::estimate_snapshot_save(&snapshot, options)
        .expect("estimate save")
        .visible_without_history
        .written_size
}
