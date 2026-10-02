use super::ToolbarSnapshot;

#[test]
fn shared_selection_preserves_text_order_locked_notes_and_normalized_spotlights() {
    let mut state = make_test_input_state();
    state.apply_toolbar_event(crate::ui::toolbar::ToolbarEvent::SelectTool(Tool::Select));
    let mut bold = state.style.font_descriptor.clone();
    bold.weight = "bold".to_string();
    let mut normal = state.style.font_descriptor.clone();
    normal.weight = "normal".to_string();
    let color = state.style.current_color;
    let frame = state.boards.active_frame_mut();
    let note = frame.add_shape(Shape::StickyNote {
        x: 0,
        y: 0,
        text: "Locked".into(),
        background: color,
        size: 18.0,
        font_descriptor: bold.clone(),
        wrap_width: None,
    });
    frame.shapes.last_mut().unwrap().locked = true;
    let deleted = frame.add_shape(Shape::Text {
        x: 0,
        y: 0,
        text: "Deleted bold text".into(),
        color,
        size: 18.0,
        font_descriptor: bold.clone(),
        background_enabled: false,
        wrap_width: None,
    });
    frame.remove_shape_by_id(deleted).unwrap();

    let text = frame.add_shape(Shape::Text {
        x: 10,
        y: 0,
        text: "Normal".into(),
        color,
        size: 18.0,
        font_descriptor: normal,
        background_enabled: false,
        wrap_width: None,
    });
    let bold_text = frame.add_shape(Shape::Text {
        x: 20,
        y: 0,
        text: "Bold".into(),
        color,
        size: 18.0,
        font_descriptor: bold,
        background_enabled: false,
        wrap_width: None,
    });
    let spotlights: Vec<_> = [f64::NAN, 2.5, 8.0]
        .into_iter()
        .map(|magnification| {
            frame.add_shape(Shape::Spotlight {
                cx: 0,
                cy: 0,
                rx: 10,
                ry: 10,
                magnification,
            })
        })
        .collect();

    for (texts, expected_bold) in [
        (vec![note, text, bold_text], Some(false)),
        (vec![note, bold_text, text], Some(true)),
    ] {
        let mut selected = vec![deleted, u64::MAX];
        selected.extend(texts);
        selected.extend(&spotlights);
        state.set_selection(selected);
        Frame::reset_linear_id_lookup_count();
        InputState::reset_selection_resolution_count();
        let snapshot = ToolbarSnapshot::from_input(&state);
        assert_eq!(Frame::linear_id_lookup_count(), 0);
        assert_eq!(InputState::selection_resolution_count(), 1);
        assert!(snapshot.selection_has_text);
        assert_eq!(snapshot.selected_text_bold, expected_bold);
        assert_eq!(snapshot.selection_spotlight_magnification, Some(4.0));
    }

    // Missing IDs must also be skipped by the tiny-selection path.
    for (selected, has_text, bold) in [
        (vec![deleted, u64::MAX, text], true, Some(false)),
        (vec![deleted, u64::MAX], false, None),
    ] {
        state.set_selection(selected);
        Frame::reset_linear_id_lookup_count();
        InputState::reset_selection_resolution_count();
        let snapshot = ToolbarSnapshot::from_input(&state);

        assert_eq!(Frame::linear_id_lookup_count(), 0);
        assert_eq!(InputState::selection_resolution_count(), 1);
        assert_eq!(snapshot.selection_has_text, has_text);
        assert_eq!(snapshot.selected_text_bold, bold);
        assert_eq!(snapshot.selection_spotlight_magnification, None);
        assert_eq!(snapshot.selection_properties.is_empty(), !has_text);
    }
}

use crate::{
    draw::{Frame, Shape},
    input::state::SelectionPropertyKind,
    input::{InputState, Tool, state::test_support::make_test_input_state},
};

#[test]
fn complete_snapshot_resolves_selection_once_without_per_id_lookups() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Fixture {
        Rectangles,
        TextLast,
        LockedText,
    }

    for case in [Fixture::Rectangles, Fixture::TextLast, Fixture::LockedText] {
        let mut state = make_test_input_state();
        state.apply_toolbar_event(crate::ui::toolbar::ToolbarEvent::SelectTool(Tool::Select));
        let mut font = state.style.font_descriptor.clone();
        font.weight = "bold".to_string();
        let color = state.style.current_color;
        let ids: Vec<_> = (0..2_048)
            .map(|index| {
                let text =
                    case == Fixture::LockedText || (case == Fixture::TextLast && index == 2_047);
                let shape = if text {
                    Shape::Text {
                        x: index,
                        y: 0,
                        text: "Text".to_string(),
                        color,
                        size: 18.0,
                        font_descriptor: font.clone(),
                        background_enabled: false,
                        wrap_width: None,
                    }
                } else {
                    Shape::Rect {
                        x: index,
                        y: 0,
                        w: 10,
                        h: 10,
                        color,
                        thick: 3.0,
                        fill: false,
                        fill_color: None,
                    }
                };
                let frame = state.boards.active_frame_mut();
                let id = frame.add_shape(shape);
                frame.shapes.last_mut().unwrap().locked = case == Fixture::LockedText;
                id
            })
            .collect();

        for selected in [
            ids.clone(),
            vec![ids[0]],
            vec![ids[2_047]],
            vec![ids[1], ids[700], ids[2_047]],
        ] {
            let contains_text = case == Fixture::LockedText
                || (case == Fixture::TextLast && selected.contains(&ids[2_047]));
            state.set_selection(selected);
            // Fixture/setup scans do not count toward the immutable snapshot contract.
            Frame::reset_linear_id_lookup_count();
            InputState::reset_selection_resolution_count();
            let snapshot = ToolbarSnapshot::from_input(&state);

            assert_eq!(Frame::linear_id_lookup_count(), 0, "{case:?}");
            assert_eq!(InputState::selection_resolution_count(), 1, "{case:?}");
            assert_eq!(snapshot.selection_has_text, contains_text, "{case:?}");
            assert_eq!(
                snapshot.selected_text_bold,
                if case == Fixture::TextLast && contains_text {
                    Some(true)
                } else {
                    None
                },
                "{case:?}"
            );
            assert_eq!(snapshot.selection_spotlight_magnification, None);
            let properties = &snapshot.selection_properties;
            assert!(!properties.is_empty());
            if case == Fixture::LockedText {
                assert!(properties.iter().all(|entry| entry.disabled));
                assert_eq!(
                    properties
                        .iter()
                        .find(|entry| entry.kind == SelectionPropertyKind::FontSize)
                        .unwrap()
                        .value,
                    "Locked"
                );
            } else if contains_text {
                assert_eq!(
                    properties
                        .iter()
                        .find(|entry| entry.kind == SelectionPropertyKind::FontSize)
                        .unwrap()
                        .value,
                    "18pt"
                );
            } else {
                assert_eq!(
                    properties
                        .iter()
                        .find(|entry| entry.kind == SelectionPropertyKind::Thickness)
                        .unwrap()
                        .value,
                    "3.0px"
                );
            }
        }
    }
}
