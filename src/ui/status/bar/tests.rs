use super::*;
use crate::config::{
    BoardsConfig, KeybindingsConfig, PresenterModeConfig, StatusBarItem, StatusBarStyle,
};
use crate::draw::{Color, FontDescriptor, Shape};
use crate::input::{ClickHighlightSettings, EraserMode};

/// Worst-case prefix: selection info plus a long output label on a
/// narrow screen.
const LONG_PREFIX: &str = "12 items: 1920×1080px · Output: DP-3 Dell UltraSharp U2723QE… · \
     And an implausibly long tail of extra selection detail text that must wrap";
/// Realistic cluster width (board + page + dot + tool + hints + help).
const CLUSTER_WIDTH: f64 = 400.0;
const CLUSTER_LINE_HEIGHT: f64 = 21.0;
const DOT_DIAMETER: f64 = 12.0;

fn make_state() -> InputState {
    let keybindings = KeybindingsConfig::default();
    let action_map = keybindings
        .build_action_map()
        .expect("default keybindings map");

    InputState::from_seed(crate::input::InputStateSeed {
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thickness: 4.0,
        eraser_size: 4.0,
        eraser_mode: EraserMode::Brush,
        marker_opacity: 0.32,
        fill_enabled: false,
        font_size: 32.0,
        font_descriptor: FontDescriptor::default(),
        text_background_enabled: false,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        arrow_head_at_end: false,
        show_status_bar: true,
        boards_config: BoardsConfig::default(),
        action_map: action_map,
        max_shapes_per_frame: usize::MAX,
        click_highlight_settings: ClickHighlightSettings::disabled(),
        undo_all_delay_ms: 0,
        redo_all_delay_ms: 0,
        custom_section_enabled: true,
        custom_undo_delay_ms: 0,
        custom_redo_delay_ms: 0,
        custom_undo_steps: 5,
        custom_redo_steps: 5,
        presenter_mode_config: PresenterModeConfig::default(),
    })
}

fn measure(
    style: &StatusBarStyle,
    prefix: &str,
    cluster_width: f64,
    screen_width: u32,
) -> StatusBarMeasurement {
    measure_status_bar(
        style,
        prefix,
        cluster_width,
        CLUSTER_LINE_HEIGHT,
        DOT_DIAMETER,
        screen_width,
    )
    .expect("measurement")
}

const LONG_BOARD_NAME: &str = "An Implausibly Long Board Name That Keeps Going And Going";

/// Worst-case HUD content: an implausibly long board name, the longest
/// tool label ("Freeform Polygon"), and a long wrappable prefix.
fn make_worst_case_state() -> InputState {
    let mut state = make_state();
    let active = state.boards.active_index();
    state.boards.board_states_mut()[active].spec.name = LONG_BOARD_NAME.to_string();
    state.state = DrawingState::BuildingPolygon {
        points: vec![(0, 0)],
        preview: None,
        fill: false,
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 4.0,
    };
    state.show_active_output_badge = true;
    state.active_output_label = Some("DP-3 Dell UltraSharp U2723QE 3840x2160@60".to_string());
    state
}

mod layout_and_hits;
mod width_budget;
