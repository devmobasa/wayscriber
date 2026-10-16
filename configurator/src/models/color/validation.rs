use wayscriber::i18n::tr;

use super::parse_hex;

/// What a color field says about text the save gate refuses. Kept short
/// because it shows as a tooltip on a field the user is still typing in.
fn invalid_hex_message() -> &'static str {
    tr("Not a hex color")
}

/// What an emptied color field says. Longer than the rejection message
/// because a blank field has nothing in it to point at.
fn empty_hex_message() -> &'static str {
    tr("Enter a color as #RRGGBB or #RRGGBBAA")
}

/// The message for hex text the model's save gate refuses, `None` while the
/// field holds a color.
///
/// Empty is invalid. Every picker edits a required color, so a cleared field
/// is not a value the save can write: without this, saving would silently keep
/// the last parsed value and discard the cleared text on reload.
pub(crate) fn hex_field_error(text: &str) -> Option<&'static str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Some(empty_hex_message());
    }
    if parse_hex(trimmed).is_some() {
        return None;
    }
    Some(invalid_hex_message())
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
        assert_eq!(hex_field_error("#00FF8"), Some(invalid_hex_message()));
        assert_eq!(hex_field_error("nope"), Some(invalid_hex_message()));
        assert_eq!(hex_field_error("#GGGGGG"), Some(invalid_hex_message()));
    }

    /// An emptied field is a color the save cannot write, so it is refused
    /// like any other text that is not a color, with its own actionable text.
    #[test]
    fn hex_field_error_refuses_an_emptied_field() {
        assert_eq!(hex_field_error(""), Some(empty_hex_message()));
        assert_eq!(hex_field_error("   "), Some(empty_hex_message()));
    }
}
