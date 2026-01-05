use super::*;

use crate::draw::ArrowLabel;

fn arrow_with_label(value: u32, font_descriptor: &FontDescriptor) -> Shape {
    Shape::Arrow {
        x1: 0,
        y1: 0,
        x2: 20,
        y2: 10,
        color: Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        thick: 2.0,
        arrow_length: 10.0,
        arrow_angle: 30.0,
        head_at_end: true,
        label: Some(ArrowLabel {
            value,
            size: 12.0,
            font_descriptor: font_descriptor.clone(),
        }),
    }
}

#[test]
fn sync_arrow_label_counter_uses_max_across_boards() {
    let mut state = create_test_input_state();
    let font_descriptor = state.font_descriptor.clone();

    state
        .canvas_set
        .active_frame_mut()
        .add_shape(arrow_with_label(2, &font_descriptor));

    state.switch_board_mode(BoardMode::Whiteboard);
    state
        .canvas_set
        .active_frame_mut()
        .add_shape(arrow_with_label(7, &font_descriptor));

    state.switch_board_mode(BoardMode::Blackboard);
    state
        .canvas_set
        .active_frame_mut()
        .add_shape(arrow_with_label(4, &font_descriptor));

    state.sync_arrow_label_counter();
    assert_eq!(state.arrow_label_counter, 8);
}
