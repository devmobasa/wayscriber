use super::Config;

impl Config {
    pub(super) fn validate_fonts(&mut self) {
        // Validate font weight is reasonable
        let valid_weight = matches!(
            self.drawing.font_weight.to_lowercase().as_str(),
            "normal" | "bold" | "light" | "ultralight" | "heavy" | "ultrabold"
        ) || self
            .drawing
            .font_weight
            .parse::<u32>()
            .is_ok_and(|w| (100..=900).contains(&w));

        if !valid_weight {
            log::warn!(
                "Invalid font_weight '{}', falling back to 'bold'",
                self.drawing.font_weight
            );
            self.drawing.font_weight = "bold".to_string();
        }

        // Validate font style
        if !matches!(
            self.drawing.font_style.to_lowercase().as_str(),
            "normal" | "italic" | "oblique"
        ) {
            log::warn!(
                "Invalid font_style '{}', falling back to 'normal'",
                self.drawing.font_style
            );
            self.drawing.font_style = "normal".to_string();
        }
    }
}
