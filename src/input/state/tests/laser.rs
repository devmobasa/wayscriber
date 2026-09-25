use std::time::{Duration, Instant};

use super::*;
use crate::config::{LaserConfig, PresenterToolBehavior};
use crate::input::tool::ProvisionalToolStroke;
use crate::session::{SessionOptions, snapshot_from_input};

const STROKE: [(i32, i32); 4] = [(40, 40), (80, 52), (120, 60), (160, 64)];

/// The whole default lifetime of finished ink, plus a margin.
const PAST_LIFETIME: Duration = Duration::from_millis(1200 + 500 + 50);

fn laser_state() -> InputState {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::Laser)));
    state
}

fn draw(state: &mut InputState, path: &[(i32, i32)]) {
    let (first, rest) = path.split_first().expect("path has points");
    state.on_mouse_press(MouseButton::Left, first.0, first.1);
    for &(x, y) in rest {
        state.on_mouse_motion(x, y);
    }
    let last = path.last().expect("path has points");
    state.on_mouse_release(MouseButton::Left, last.0, last.1);
}

fn run_action(state: &mut InputState, action: Action) {
    let measurer = crate::draw::TextMeasurer::default();
    let ui_engine = crate::ui_text::UiTextEngine::default();
    state.handle_action_with_resources(
        crate::input::state::InputTextResources {
            measurer: &measurer,
            ui_engine: &ui_engine,
        },
        action,
    );
}

#[test]
fn the_select_laser_action_and_its_l_key_pick_the_laser() {
    assert_eq!(Tool::Laser.action(), Some(Action::SelectLaserTool));
    assert_eq!(
        Tool::from_select_action(Action::SelectLaserTool),
        Some(Tool::Laser)
    );

    let mut state = create_test_input_state();
    run_action(&mut state, Action::SelectLaserTool);
    assert_eq!(state.tool_override(), Some(Tool::Laser));

    let mut state = create_test_input_state();
    state.on_key_press(Key::Char('l'));
    state.on_key_release(Key::Char('l'));
    assert_eq!(state.active_tool(), Tool::Laser);
}

#[test]
fn a_laser_stroke_never_reaches_the_frame_history_or_session() {
    let mut state = laser_state();
    state.clear_session_dirty();

    draw(&mut state, &STROKE);

    let frame = state.boards.active_frame();
    assert!(frame.shapes.is_empty(), "laser ink is not a shape");
    assert_eq!(frame.undo_stack_len(), 0, "nothing to undo");
    assert!(!state.is_session_dirty(), "nothing to save");
    assert!(state.laser.has_ink(), "the stroke became fading ink");
    assert!(matches!(state.state, DrawingState::Idle));
}

#[test]
fn session_snapshots_and_exports_see_no_laser_ink() {
    let mut state = laser_state();
    draw(&mut state, &STROKE);
    let mut options = SessionOptions::new(std::path::PathBuf::from("/tmp"), "laser");
    options.persist_transparent = true;

    let snapshot = snapshot_from_input(&state, &options).expect("tool state is persisted");

    assert!(
        snapshot
            .boards
            .iter()
            .flat_map(|board| board.pages.pages.iter())
            .all(|page| page.shapes.is_empty()),
        "no page in the snapshot carries laser ink"
    );
    // Canvas and PDF export, capture review, and hit-testing all read the
    // frame, which the stroke never entered.
    assert!(state.boards.active_frame().shapes.is_empty());
}

#[test]
fn laser_ink_holds_then_fades_and_is_removed() {
    let mut state = laser_state();
    draw(&mut state, &STROKE);
    let released = Instant::now();

    assert!(!state.advance_laser_ink_for(released, true));
    assert!(state.laser.has_ink());
    assert!(state.laser_ink_wake_after_for(released, true).is_some());

    assert!(!state.advance_laser_ink_for(released + PAST_LIFETIME, true));
    assert!(!state.laser.has_ink());
    assert_eq!(
        state.laser_ink_wake_after_for(released + PAST_LIFETIME, true),
        None,
        "no ink, no wakeups"
    );
}

#[test]
fn a_second_stroke_keeps_the_first_on_screen_until_both_fade() {
    let mut state = laser_state();
    draw(&mut state, &STROKE);
    draw(&mut state, &[(40, 200), (120, 210)]);
    let released = Instant::now();

    assert_eq!(state.laser.stroke_count(), 2);
    assert!(!state.advance_laser_ink_for(released, true));
    assert!(!state.advance_laser_ink_for(released + PAST_LIFETIME, true));
    assert_eq!(state.laser.stroke_count(), 0);
}

#[test]
fn the_live_laser_preview_uses_the_laser_style_and_glow_damage() {
    let mut state = create_test_input_state();
    let config = LaserConfig {
        color: [0.1, 0.9, 0.2, 1.0],
        width: 10.0,
        ..LaserConfig::default()
    };
    state.init_laser_from_config(&config);
    assert!(state.set_tool_override(Some(Tool::Laser)));

    state.on_mouse_press(MouseButton::Left, 100, 100);
    let _ = state.take_dirty_regions();
    state.on_mouse_motion(140, 100);

    match state.provisional_tool_stroke(140, 100) {
        ProvisionalToolStroke::Laser { points, style } => {
            assert_eq!(points.last(), Some(&(140, 100)));
            assert_eq!(style.width, 10.0);
            assert_eq!(style.color.g, 0.9);
        }
        _ => panic!("the laser previews as laser ink"),
    }
    let regions = state.take_dirty_regions();
    let halo_edge = 100 + (state.laser_style().glow_width() / 2.0) as i32 - 1;
    assert!(
        regions.iter().any(|rect| rect.contains(120, halo_edge)),
        "the glow's outer edge is damaged, got {regions:?}"
    );
}

#[test]
fn releasing_a_laser_stroke_repaints_its_path_not_the_preview_box() {
    let mut state = laser_state();
    let diagonal: Vec<_> = (0..60).map(|i| (i * 10, i * 10)).collect();

    state.on_mouse_press(MouseButton::Left, 0, 0);
    for &(x, y) in &diagonal[1..] {
        state.on_mouse_motion(x, y);
    }
    let _ = state.take_dirty_regions();
    state.on_mouse_release(MouseButton::Left, 590, 590);
    let regions = state.take_dirty_regions();

    assert!(!regions.is_empty());
    assert!(
        !regions.iter().any(|rect| rect.contains(500, 50)),
        "an off-path corner of the stroke's box stays untouched, got {regions:?}"
    );
}

#[test]
fn clearing_the_canvas_also_clears_laser_ink() {
    let mut state = laser_state();
    draw(&mut state, &STROKE);
    assert!(state.laser.has_ink());

    run_action(&mut state, Action::ClearCanvas);

    assert!(!state.laser.has_ink());
}

#[test]
fn from_config_applies_the_laser_section() {
    let mut config = crate::config::Config::default();
    config.laser.width = 12.0;
    config.laser.color = [0.0, 0.5, 1.0, 1.0];

    let state = InputState::from_config(&config);

    assert_eq!(state.laser_style().width, 12.0);
    assert_eq!(state.laser_style().color.b, 1.0);
}

#[test]
fn locked_presenter_mode_still_allows_the_laser() {
    let mut state = create_test_input_state();
    state.presenter_mode_config_mut_for_test().tool_behavior =
        PresenterToolBehavior::ForceHighlightLocked;
    let measurer = crate::draw::TextMeasurer::default();
    let ui_engine = crate::ui_text::UiTextEngine::default();
    state.toggle_presenter_mode_with_resources(crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    });
    assert_eq!(state.tool_override(), Some(Tool::Highlight));

    assert!(!state.set_tool_override(Some(Tool::Pen)));
    assert!(state.set_tool_override(Some(Tool::Laser)));
    draw(&mut state, &STROKE);

    assert!(state.laser.has_ink(), "the laser draws while presenting");
    assert!(state.boards.active_frame().shapes.is_empty());
    assert!(state.set_tool_override(Some(Tool::Highlight)));
}

#[test]
fn regular_and_advanced_strips_show_the_laser_with_the_pens_and_simple_does_not() {
    use crate::ui::toolbar::{ToolbarSnapshot, model};

    use crate::config::ToolbarLayoutMode;

    let snapshot = ToolbarSnapshot::from_input(&create_test_input_state());

    for mode in [ToolbarLayoutMode::Regular, ToolbarLayoutMode::Advanced] {
        let strip: Vec<_> = model::visible_top_tool_buttons(mode, &snapshot).collect();
        let marker = strip
            .iter()
            .position(|&tool| tool == Tool::Marker)
            .expect("marker");
        assert_eq!(strip.get(marker + 1), Some(&Tool::Laser), "{mode:?}");
    }
    assert_eq!(
        model::top_tool_group(Tool::Laser),
        model::TopToolGroup::Pens
    );
    assert!(
        !model::visible_top_tool_buttons(ToolbarLayoutMode::Simple, &snapshot)
            .any(|tool| tool == Tool::Laser)
    );
    assert_eq!(
        model::toolbar_item_id_for_tool(Tool::Laser),
        crate::config::toolbar_item_ids::TOP_TOOL_LASER
    );
}

#[test]
fn the_command_palette_finds_the_laser_by_its_presenter_synonyms() {
    let mut state = create_test_input_state();

    for query in ["laser", "pointer", "presenter", "fading ink"] {
        state.command_palette.set_query(query);
        assert!(
            state
                .filtered_commands()
                .iter()
                .any(|entry| entry.action == Action::SelectLaserTool),
            "query {query:?} should list the laser"
        );
    }
}

#[test]
fn the_status_bar_reports_the_lasers_own_width_and_color() {
    let mut state = create_test_input_state();
    let config = LaserConfig {
        color: [0.1, 0.9, 0.2, 1.0],
        width: 10.0,
        ..LaserConfig::default()
    };
    state.init_laser_from_config(&config);
    state.style.current_thickness = 3.0;

    assert!(state.set_tool_override(Some(Tool::Laser)));

    assert!((state.status_size_for_tool(Tool::Laser) - 10.0).abs() < f64::EPSILON);
    let color = state.status_color_for_tool(Tool::Laser);
    assert!((color.g - 0.9).abs() < f64::EPSILON);
    assert!(
        (state.status_size_for_tool(Tool::Pen) - state.thickness_for_tool(Tool::Pen)).abs()
            < f64::EPSILON
    );
}
