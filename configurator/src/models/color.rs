use iced::Color;
use wayscriber::config::enums::ColorSpec;
use wayscriber::util::name_to_color;

use super::error::FormError;
use super::util::{format_float, parse_f64};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Named,
    Rgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedColorOption {
    Red,
    Green,
    Blue,
    Yellow,
    Orange,
    Pink,
    White,
    Black,
    Custom,
}

impl NamedColorOption {
    pub fn list() -> Vec<Self> {
        vec![
            Self::Red,
            Self::Green,
            Self::Blue,
            Self::Yellow,
            Self::Orange,
            Self::Pink,
            Self::White,
            Self::Black,
            Self::Custom,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Yellow => "Yellow",
            Self::Orange => "Orange",
            Self::Pink => "Pink",
            Self::White => "White",
            Self::Black => "Black",
            Self::Custom => "Custom",
        }
    }

    pub fn as_value(&self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Yellow => "yellow",
            Self::Orange => "orange",
            Self::Pink => "pink",
            Self::White => "white",
            Self::Black => "black",
            Self::Custom => "",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "red" => Self::Red,
            "green" => Self::Green,
            "blue" => Self::Blue,
            "yellow" => Self::Yellow,
            "orange" => Self::Orange,
            "pink" => Self::Pink,
            "white" => Self::White,
            "black" => Self::Black,
            _ => Self::Custom,
        }
    }
}

impl std::fmt::Display for NamedColorOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorInput {
    pub mode: ColorMode,
    pub name: String,
    pub rgb: [String; 3],
    pub selected_named: NamedColorOption,
}

impl ColorInput {
    pub fn from_color(spec: &ColorSpec) -> Self {
        match spec {
            ColorSpec::Name(name) => Self {
                mode: ColorMode::Named,
                name: name.clone(),
                rgb: ["255".into(), "0".into(), "0".into()],
                selected_named: NamedColorOption::from_str(name),
            },
            ColorSpec::Rgb([r, g, b]) => Self {
                mode: ColorMode::Rgb,
                name: String::new(),
                rgb: [r.to_string(), g.to_string(), b.to_string()],
                selected_named: NamedColorOption::Custom,
            },
        }
    }

    pub fn to_color_spec(&self) -> Result<ColorSpec, FormError> {
        self.to_color_spec_with_field("drawing.default_color")
    }

    pub fn to_color_spec_with_field(&self, field: &str) -> Result<ColorSpec, FormError> {
        match self.mode {
            ColorMode::Named => {
                let value = if self.selected_named_is_custom() {
                    self.name.trim().to_string()
                } else {
                    self.selected_named.as_value().to_string()
                };

                if value.trim().is_empty() {
                    Err(FormError::new(
                        field.to_string(),
                        "Please enter a color name.",
                    ))
                } else {
                    Ok(ColorSpec::Name(value))
                }
            }
            ColorMode::Rgb => {
                let mut rgb = [0u8; 3];
                for (index, component) in self.rgb.iter().enumerate() {
                    let field = format!("{field}[{index}]");
                    let parsed = component.trim().parse::<i64>().map_err(|_| {
                        FormError::new(&field, "Expected integer between 0 and 255")
                    })?;
                    if !(0..=255).contains(&parsed) {
                        return Err(FormError::new(&field, "Value must be between 0 and 255"));
                    }
                    rgb[index] = parsed as u8;
                }
                Ok(ColorSpec::Rgb(rgb))
            }
        }
    }

    pub fn update_named_from_current(&mut self) {
        self.selected_named = NamedColorOption::from_str(&self.name);
    }

    pub fn selected_named_is_custom(&self) -> bool {
        self.selected_named == NamedColorOption::Custom
    }

    pub fn preview_color(&self) -> Option<Color> {
        match self.mode {
            ColorMode::Named => {
                let name = if self.selected_named_is_custom() {
                    self.name.trim().to_string()
                } else {
                    self.selected_named.as_value().to_string()
                };

                if name.is_empty() {
                    return None;
                }

                name_to_color(&name).map(|color| {
                    Color::from_rgba(
                        color.r as f32,
                        color.g as f32,
                        color.b as f32,
                        color.a as f32,
                    )
                })
            }
            ColorMode::Rgb => {
                let mut components = [0.0f32; 3];
                for (index, value) in self.rgb.iter().enumerate() {
                    let trimmed = value.trim();
                    if trimmed.is_empty() {
                        return None;
                    }
                    let parsed = trimmed.parse::<f32>().ok()?;
                    if !(0.0..=255.0).contains(&parsed) {
                        return None;
                    }
                    components[index] = parsed / 255.0;
                }
                Some(Color::from_rgba(
                    components[0],
                    components[1],
                    components[2],
                    1.0,
                ))
            }
        }
    }

    pub fn summary(&self) -> String {
        match self.mode {
            ColorMode::Named => {
                if self.selected_named_is_custom() {
                    let trimmed = self.name.trim();
                    if trimmed.is_empty() {
                        "Custom name".to_string()
                    } else {
                        trimmed.to_string()
                    }
                } else {
                    self.selected_named.label().to_string()
                }
            }
            ColorMode::Rgb => format!(
                "{}, {}, {}",
                self.rgb[0].trim(),
                self.rgb[1].trim(),
                self.rgb[2].trim()
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorTripletInput {
    pub components: [String; 3],
}

impl ColorTripletInput {
    pub fn from(values: [f64; 3]) -> Self {
        Self {
            components: values.map(format_float),
        }
    }

    pub fn set_component(&mut self, index: usize, value: String) {
        if let Some(slot) = self.components.get_mut(index) {
            *slot = value;
        }
    }

    pub fn to_array(&self, field: &'static str) -> Result<[f64; 3], FormError> {
        let mut out = [0.0f64; 3];
        for (index, value) in self.components.iter().enumerate() {
            let parsed = parse_f64(value.trim())
                .map_err(|err| FormError::new(format!("{field}[{index}]"), err))?;
            out[index] = parsed;
        }
        Ok(out)
    }

    pub fn summary(&self) -> String {
        [
            self.components[0].trim(),
            self.components[1].trim(),
            self.components[2].trim(),
        ]
        .join(", ")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorQuadInput {
    pub components: [String; 4],
}

impl ColorQuadInput {
    pub fn from(values: [f64; 4]) -> Self {
        Self {
            components: values.map(format_float),
        }
    }

    pub fn set_component(&mut self, index: usize, value: String) {
        if let Some(slot) = self.components.get_mut(index) {
            *slot = value;
        }
    }

    pub fn to_array(&self, field: &'static str) -> Result<[f64; 4], FormError> {
        let mut out = [0.0f64; 4];
        for (index, value) in self.components.iter().enumerate() {
            let parsed = parse_f64(value.trim())
                .map_err(|err| FormError::new(format!("{field}[{index}]"), err))?;
            out[index] = parsed;
        }
        Ok(out)
    }

    pub fn summary(&self) -> String {
        [
            self.components[0].trim(),
            self.components[1].trim(),
            self.components[2].trim(),
            self.components[3].trim(),
        ]
        .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wayscriber::config::enums::ColorSpec;

    #[test]
    fn color_input_named_round_trip_and_preview() {
        let spec = ColorSpec::Name("red".to_string());
        let input = ColorInput::from_color(&spec);

        assert_eq!(input.mode, ColorMode::Named);
        assert_eq!(input.selected_named, NamedColorOption::Red);
        assert_eq!(input.summary(), "Red");

        let preview = input.preview_color().expect("preview should resolve");
        assert!((preview.r - 1.0).abs() < f32::EPSILON);
        assert!((preview.g - 0.0).abs() < f32::EPSILON);
        assert!((preview.b - 0.0).abs() < f32::EPSILON);

        let round_trip = input.to_color_spec().expect("to_color_spec should succeed");
        match round_trip {
            ColorSpec::Name(name) => assert_eq!(name, "red"),
            _ => panic!("expected named color"),
        }
    }

    #[test]
    fn color_input_custom_name_requires_value() {
        let input = ColorInput {
            mode: ColorMode::Named,
            name: "   ".to_string(),
            rgb: ["0".to_string(), "0".to_string(), "0".to_string()],
            selected_named: NamedColorOption::Custom,
        };

        let err = input.to_color_spec().expect_err("expected error");
        assert_eq!(err.field, "drawing.default_color");
    }

    #[test]
    fn color_input_rgb_rejects_out_of_range_component() {
        let input = ColorInput {
            mode: ColorMode::Rgb,
            name: String::new(),
            rgb: ["255".to_string(), "0".to_string(), "300".to_string()],
            selected_named: NamedColorOption::Custom,
        };

        let err = input.to_color_spec().expect_err("expected error");
        assert_eq!(err.field, "drawing.default_color[2]");
        assert!(err.message.contains("between 0 and 255"));
    }

    #[test]
    fn color_input_rgb_rejects_negative_component() {
        let input = ColorInput {
            mode: ColorMode::Rgb,
            name: String::new(),
            rgb: ["-1".to_string(), "10".to_string(), "20".to_string()],
            selected_named: NamedColorOption::Custom,
        };

        let err = input.to_color_spec().expect_err("expected error");
        assert_eq!(err.field, "drawing.default_color[0]");
        assert!(err.message.contains("between 0 and 255"));
    }

    #[test]
    fn color_input_rgb_rejects_non_integer_component() {
        let input = ColorInput {
            mode: ColorMode::Rgb,
            name: String::new(),
            rgb: ["12.5".to_string(), "10".to_string(), "20".to_string()],
            selected_named: NamedColorOption::Custom,
        };

        let err = input.to_color_spec().expect_err("expected error");
        assert_eq!(err.field, "drawing.default_color[0]");
        assert!(err.message.contains("Expected integer"));
    }

    #[test]
    fn color_input_preview_rgb_rejects_out_of_range() {
        let input = ColorInput {
            mode: ColorMode::Rgb,
            name: String::new(),
            rgb: ["256".to_string(), "0".to_string(), "0".to_string()],
            selected_named: NamedColorOption::Custom,
        };

        assert!(
            input.preview_color().is_none(),
            "preview should be None for out-of-range component"
        );
    }

    #[test]
    fn color_triplet_input_reports_invalid_component() {
        let input = ColorTripletInput {
            components: ["0.1".to_string(), "oops".to_string(), "0.3".to_string()],
        };

        let err = input
            .to_array("board.whiteboard_color")
            .expect_err("expected error");
        assert_eq!(err.field, "board.whiteboard_color[1]");
    }

    #[test]
    fn color_quad_input_summary_trims_components() {
        let input = ColorQuadInput {
            components: [
                " 0.1 ".to_string(),
                "0.2".to_string(),
                " 0.3".to_string(),
                "0.4 ".to_string(),
            ],
        };

        assert_eq!(input.summary(), "0.1, 0.2, 0.3, 0.4");
    }
}
