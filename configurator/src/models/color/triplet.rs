use super::super::error::FormError;
use super::super::util::{format_float, parse_f64};

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

    pub fn to_array(&self, field: &str) -> Result<[f64; 3], FormError> {
        let mut out = [0.0f64; 3];
        for (index, value) in self.components.iter().enumerate() {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(FormError::new(
                    format!("{field}[{index}]"),
                    "Value is required",
                ));
            }
            let parsed = parse_f64(trimmed)
                .map_err(|err| FormError::new(format!("{field}[{index}]"), err))?;
            out[index] = parsed;
        }
        Ok(out)
    }
}
