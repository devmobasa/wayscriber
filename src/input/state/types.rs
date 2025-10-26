use crate::input::tool::Tool;

/// Current drawing mode state machine.
#[derive(Debug)]
pub enum DrawingState {
    Idle,
    Drawing {
        tool: Tool,
        start_x: i32,
        start_y: i32,
        points: Vec<(i32, i32)>,
    },
    TextInput {
        x: i32,
        y: i32,
        buffer: String,
    },
}
