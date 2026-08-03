use super::parse_hex;

/// What a color field says about text the save gate refuses. Kept short
/// because it shows as a tooltip on a field the user is still typing in.
const INVALID_HEX_MESSAGE: &str = "Not a hex color";

/// What an emptied color field says. Longer than the rejection message
/// because a blank field has nothing in it to point at.
const EMPTY_HEX_MESSAGE: &str = "Enter a color as #RRGGBB or #RRGGBBAA";

/// The message for hex text the model's save gate refuses, `None` while the
/// field holds a color.
///
/// Empty is invalid. Every picker edits a required color, so a cleared field
/// is not a value the save can write: without this, saving would silently keep
/// the last parsed value and discard the cleared text on reload.
pub(crate) fn hex_field_error(text: &str) -> Option<&'static str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Some(EMPTY_HEX_MESSAGE);
    }
    if parse_hex(trimmed).is_some() {
        return None;
    }
    Some(INVALID_HEX_MESSAGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_field_error_accepts_what_the_save_gate_accepts() {
        assert_eq!(hex_field_error("#00FF80"), None);
        assert_eq!(hex_field_error("00ff80"), None);
        assert_eq!(hex_field_error("#00FF80FF"), None);
    }

    #[test]
    fn hex_field_error_reports_text_the_save_gate_refuses() {
        assert_eq!(hex_field_error("#00FF8"), Some(INVALID_HEX_MESSAGE));
        assert_eq!(hex_field_error("nope"), Some(INVALID_HEX_MESSAGE));
        assert_eq!(hex_field_error("#GGGGGG"), Some(INVALID_HEX_MESSAGE));
    }

    /// An emptied field is a color the save cannot write, so it is refused
    /// like any other text that is not a color, with its own actionable text.
    #[test]
    fn hex_field_error_refuses_an_emptied_field() {
        assert_eq!(hex_field_error(""), Some(EMPTY_HEX_MESSAGE));
        assert_eq!(hex_field_error("   "), Some(EMPTY_HEX_MESSAGE));
    }
}
