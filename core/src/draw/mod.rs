//! Lightweight drawing data types shared with the configurator.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Color {
    #[allow(dead_code)]
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }
}

pub const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
pub const GREEN: Color = Color {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};
pub const BLUE: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};
pub const YELLOW: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};
pub const ORANGE: Color = Color {
    r: 1.0,
    g: 0.5,
    b: 0.0,
    a: 1.0,
};
pub const PINK: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};
pub const WHITE: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
pub const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
pub const TRANSPARENT: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

pub mod color {
    pub use super::{BLACK, BLUE, Color, GREEN, ORANGE, PINK, RED, TRANSPARENT, WHITE, YELLOW};
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FontDescriptor {
    pub family: String,
    pub weight: String,
    pub style: String,
}

impl FontDescriptor {
    pub fn new(family: String, weight: String, style: String) -> Self {
        Self {
            family,
            weight,
            style,
        }
    }

    pub fn to_pango_string(&self, size: f64) -> String {
        let mut parts = vec![self.family.clone()];
        if self.style.to_lowercase() != "normal" {
            parts.push(capitalize_first(&self.style));
        }
        if self.weight.to_lowercase() != "normal" {
            parts.push(capitalize_first(&self.weight));
        }
        parts.push(format!("{}", size.round() as i32));
        parts.join(" ")
    }
}

impl Default for FontDescriptor {
    fn default() -> Self {
        Self {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        }
    }
}

pub mod font {
    pub use super::FontDescriptor;
}

fn capitalize_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub mod shape {
    use super::{Color, FontDescriptor};
    use serde::{Deserialize, Serialize};

    pub const REGULAR_POLYGON_MIN_SIDES: u8 = 3;
    pub const REGULAR_POLYGON_MAX_SIDES: u8 = 12;
    pub const REGULAR_POLYGON_DEFAULT_SIDES: u8 = 5;

    #[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
    #[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "kebab-case", tag = "type")]
    pub enum PolygonKind {
        Triangle,
        Parallelogram,
        Rhombus,
        Regular { sides: u8 },
        Freeform,
    }

    impl PolygonKind {
        pub fn label(self) -> &'static str {
            match self {
                Self::Triangle => "Triangle",
                Self::Parallelogram => "Parallelogram",
                Self::Rhombus => "Rhombus",
                Self::Regular { .. } => "Regular Polygon",
                Self::Freeform => "Freeform Polygon",
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum PolygonTemplate {
        Triangle,
        Parallelogram,
        Rhombus,
        Regular,
    }

    impl PolygonTemplate {
        pub(crate) fn kind(self, regular_sides: u8) -> PolygonKind {
            match self {
                Self::Triangle => PolygonKind::Triangle,
                Self::Parallelogram => PolygonKind::Parallelogram,
                Self::Rhombus => PolygonKind::Rhombus,
                Self::Regular => PolygonKind::Regular {
                    sides: clamp_regular_sides(regular_sides),
                },
            }
        }
    }

    pub fn clamp_regular_sides(sides: u8) -> u8 {
        sides.clamp(REGULAR_POLYGON_MIN_SIDES, REGULAR_POLYGON_MAX_SIDES)
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct EmbeddedImage {
        pub mime_type: String,
        pub width: u32,
        pub height: u32,
        pub bytes: Vec<u8>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct EraserBrush {
        pub size: f64,
        pub kind: EraserKind,
    }

    #[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
    #[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum EraserKind {
        Circle,
        Rect,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct ArrowLabel {
        pub value: u32,
        pub size: f64,
        pub font_descriptor: FontDescriptor,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct StepMarkerLabel {
        pub value: u32,
        pub size: f64,
        pub font_descriptor: FontDescriptor,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[allow(clippy::large_enum_variant)]
    pub enum Shape {
        Freehand {
            points: Vec<(i32, i32)>,
            color: Color,
            thick: f64,
        },
        FreehandPressure {
            points: Vec<(i32, i32, f32)>,
            color: Color,
        },
        Line {
            x1: i32,
            y1: i32,
            x2: i32,
            y2: i32,
            color: Color,
            thick: f64,
        },
        Rect {
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            fill: bool,
            color: Color,
            thick: f64,
        },
        Ellipse {
            cx: i32,
            cy: i32,
            rx: i32,
            ry: i32,
            fill: bool,
            color: Color,
            thick: f64,
        },
        Polygon {
            kind: PolygonKind,
            points: Vec<(i32, i32)>,
            fill: bool,
            color: Color,
            thick: f64,
        },
        Arrow {
            x1: i32,
            y1: i32,
            x2: i32,
            y2: i32,
            color: Color,
            thick: f64,
            arrow_length: f64,
            arrow_angle: f64,
            #[serde(default = "default_arrow_head_at_end")]
            head_at_end: bool,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            label: Option<ArrowLabel>,
        },
        BlurRect {
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            strength: f64,
        },
        StepMarker {
            x: i32,
            y: i32,
            color: Color,
            label: StepMarkerLabel,
        },
        Text {
            x: i32,
            y: i32,
            text: String,
            color: Color,
            size: f64,
            font_descriptor: FontDescriptor,
            background_enabled: bool,
            #[serde(default)]
            wrap_width: Option<i32>,
        },
        StickyNote {
            x: i32,
            y: i32,
            text: String,
            background: Color,
            size: f64,
            font_descriptor: FontDescriptor,
            #[serde(default)]
            wrap_width: Option<i32>,
        },
        MarkerStroke {
            points: Vec<(i32, i32)>,
            color: Color,
            thick: f64,
        },
        EraserStroke {
            points: Vec<(i32, i32)>,
            brush: EraserBrush,
        },
        Image {
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            data: EmbeddedImage,
        },
    }

    fn default_arrow_head_at_end() -> bool {
        true
    }
}

pub use shape::{
    ArrowLabel, EmbeddedImage, EraserBrush, EraserKind, PolygonKind, REGULAR_POLYGON_DEFAULT_SIDES,
    REGULAR_POLYGON_MAX_SIDES, REGULAR_POLYGON_MIN_SIDES, Shape, StepMarkerLabel,
    clamp_regular_sides,
};

#[cfg(test)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub shapes: Vec<Shape>,
    #[serde(default)]
    pub page_name: Option<String>,
    #[serde(default)]
    pub view_offset: (i32, i32),
}

#[cfg(test)]
impl Frame {
    pub fn new() -> Self {
        Self {
            shapes: Vec::new(),
            page_name: None,
            view_offset: (0, 0),
        }
    }

    pub fn add_shape(&mut self, shape: Shape) -> usize {
        self.shapes.push(shape);
        self.shapes.len()
    }

    pub fn has_persistable_data(&self) -> bool {
        !self.shapes.is_empty() || self.page_name.is_some() || self.view_offset != (0, 0)
    }
}

#[cfg(test)]
impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}
