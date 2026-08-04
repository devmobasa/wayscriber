use wayscriber::config::enums::ColorSpec;
use wayscriber::draw::Color;
use wayscriber::util::{ConfigHexColorError, name_to_color, parse_config_hex_color};

use super::super::error::FormError;
use super::named::{ColorMode, NamedColorOption};

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
                rgb: color_to_rgb_strings(spec.to_color()),
                selected_named: NamedColorOption::from_str(name),
            },
            ColorSpec::Rgb([r, g, b]) => Self {
                mode: ColorMode::Rgb,
                name: String::new(),
                rgb: [r.to_string(), g.to_string(), b.to_string()],
                selected_named: NamedColorOption::Custom,
            },
            // The configurator's RGB fields carry no alpha channel, so a
            // translucent color is shown as its hex form rather than being
            // silently flattened to opaque by the three RGB boxes.
            ColorSpec::Rgba(_) => Self {
                mode: ColorMode::Named,
                name: wayscriber::input::state::color_to_hex(spec.to_color()),
                rgb: color_to_rgb_strings(spec.to_color()),
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
                let value = self.named_value();

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

    pub fn to_known_color_spec_with_field(&self, field: &str) -> Result<ColorSpec, FormError> {
        match self.mode {
            ColorMode::Named => {
                let value = self.named_value();
                if value.is_empty() {
                    return Err(FormError::new(
                        field.to_string(),
                        "Please enter a color name or #RRGGBB / #RRGGBBAA hex color.",
                    ));
                }

                match parse_config_hex_color(&value) {
                    Ok(_) => return Ok(ColorSpec::Name(value)),
                    Err(ConfigHexColorError::MissingHash) => {}
                    Err(_) => {
                        return Err(FormError::new(
                            field.to_string(),
                            "Expected #RRGGBB or #RRGGBBAA hex color.",
                        ));
                    }
                }

                if name_to_color(&value).is_some() {
                    Ok(ColorSpec::Name(value))
                } else {
                    Err(FormError::new(
                        field.to_string(),
                        "Expected a known color name or #RRGGBB / #RRGGBBAA hex color.",
                    ))
                }
            }
            ColorMode::Rgb => self.to_color_spec_with_field(field),
        }
    }

    pub fn update_named_from_current(&mut self) {
        self.selected_named = NamedColorOption::from_str(&self.name);
    }

    pub fn sync_rgb_from_preview(&mut self) -> bool {
        let Some(color) = self.preview_color() else {
            return false;
        };
        self.rgb = color_to_rgb_strings(color);
        true
    }

    pub fn selected_named_is_custom(&self) -> bool {
        self.selected_named == NamedColorOption::Custom
    }

    pub fn preview_color(&self) -> Option<Color> {
        match self.mode {
            ColorMode::Named => {
                let name = self.named_value();

                if name.is_empty() {
                    return None;
                }

                match parse_config_hex_color(&name) {
                    Ok(color) => Some(color),
                    Err(ConfigHexColorError::MissingHash) => name_to_color(&name),
                    Err(_) => None,
                }
            }
            ColorMode::Rgb => {
                let mut components = [0.0f64; 3];
                for (index, value) in self.rgb.iter().enumerate() {
                    let trimmed = value.trim();
                    if trimmed.is_empty() {
                        return None;
                    }
                    let parsed = trimmed.parse::<f64>().ok()?;
                    if !(0.0..=255.0).contains(&parsed) {
                        return None;
                    }
                    components[index] = parsed / 255.0;
                }
                Some(Color {
                    r: components[0],
                    g: components[1],
                    b: components[2],
                    a: 1.0,
                })
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

    fn named_value(&self) -> String {
        if self.selected_named_is_custom() {
            self.name.trim().to_string()
        } else {
            self.selected_named.as_value().to_string()
        }
    }
}

/// The three RGB boxes for a color, as the byte strings the fields hold.
fn color_to_rgb_strings(color: Color) -> [String; 3] {
    [
        ((color.r.clamp(0.0, 1.0) * 255.0).round() as u8).to_string(),
        ((color.g.clamp(0.0, 1.0) * 255.0).round() as u8).to_string(),
        ((color.b.clamp(0.0, 1.0) * 255.0).round() as u8).to_string(),
    ]
}
