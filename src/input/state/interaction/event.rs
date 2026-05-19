use crate::input::MouseButton;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScreenPoint {
    x: i32,
    y: i32,
}

impl ScreenPoint {
    pub(crate) fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub(crate) fn x(self) -> i32 {
        self.x
    }

    pub(crate) fn y(self) -> i32 {
        self.y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CanvasPoint {
    x: i32,
    y: i32,
}

impl CanvasPoint {
    pub(crate) fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub(crate) fn x(self) -> i32 {
        self.x
    }

    pub(crate) fn y(self) -> i32 {
        self.y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PointerPoints {
    screen: ScreenPoint,
    canvas: CanvasPoint,
}

impl PointerPoints {
    pub(crate) fn new(screen: ScreenPoint, canvas: CanvasPoint) -> Self {
        Self { screen, canvas }
    }

    pub(crate) fn screen(self) -> ScreenPoint {
        self.screen
    }

    pub(crate) fn canvas(self) -> CanvasPoint {
        self.canvas
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PointerPress {
    button: MouseButton,
    points: PointerPoints,
}

impl PointerPress {
    pub(crate) fn new(button: MouseButton, points: PointerPoints) -> Self {
        Self { button, points }
    }

    pub(crate) fn button(self) -> MouseButton {
        self.button
    }

    pub(crate) fn points(self) -> PointerPoints {
        self.points
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PointerMotion {
    points: PointerPoints,
}

impl PointerMotion {
    pub(crate) fn new(points: PointerPoints) -> Self {
        Self { points }
    }

    pub(crate) fn points(self) -> PointerPoints {
        self.points
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PointerRelease {
    button: MouseButton,
    points: PointerPoints,
}

impl PointerRelease {
    pub(crate) fn new(button: MouseButton, points: PointerPoints) -> Self {
        Self { button, points }
    }

    pub(crate) fn button(self) -> MouseButton {
        self.button
    }

    pub(crate) fn points(self) -> PointerPoints {
        self.points
    }
}
