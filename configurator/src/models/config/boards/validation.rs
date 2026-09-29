use std::collections::HashMap;

use wayscriber::config::BoardBackgroundConfig;

use crate::models::color::ColorTripletInput;
use crate::models::error::FormError;

use super::{BoardBackgroundOption, BoardsDraft};

pub(super) fn default_pen_fallback(background: &BoardBackgroundConfig) -> [f64; 3] {
    match background {
        BoardBackgroundConfig::Transparent(_) => [0.0, 0.0, 0.0],
        BoardBackgroundConfig::Color(color) => {
            let rgb = color.rgb();
            let avg = (rgb[0] + rgb[1] + rgb[2]) / 3.0;
            if avg >= 0.5 {
                [0.0, 0.0, 0.0]
            } else {
                [1.0, 1.0, 1.0]
            }
        }
    }
}

pub(super) fn parse_triplet(
    input: &ColorTripletInput,
    field_prefix: &str,
    errors: &mut Vec<FormError>,
) -> Option<[f64; 3]> {
    match input.to_array(field_prefix) {
        Ok(values) => Some(values),
        Err(err) => {
            errors.push(err);
            None
        }
    }
}

pub(super) fn parse_usize<F>(
    value: &str,
    field: &'static str,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(usize),
{
    match value.trim().parse::<usize>() {
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}

/// Refuses the board-list shapes core would silently reshape on load.
///
/// `validate_boards` lowercases and deduplicates ids, re-inserts a missing
/// transparent board, and drops boards past `max_count`. Any of those turns
/// a save into a correction core refuses to write, so each is reported here
/// against the field the user has to change.
pub(super) fn check_board_list(
    draft: &BoardsDraft,
    max_count: Option<usize>,
    errors: &mut Vec<FormError>,
) {
    if max_count == Some(0) {
        errors.push(FormError::new("boards.max_count", "Expected at least 1"));
    }

    let mut first_index_by_id: HashMap<String, usize> = HashMap::new();
    for (index, id) in draft.effective_ids().into_iter().enumerate() {
        let field = format!("boards.items[{index}].id");
        let lowercase = id.to_lowercase();
        if lowercase != id {
            errors.push(FormError::new(
                field,
                format!("Board ids are lowercase; use \"{lowercase}\""),
            ));
            continue;
        }

        if let Some(first) = first_index_by_id.get(&id) {
            errors.push(FormError::new(
                field,
                format!("Board {} already uses the id \"{id}\"", first + 1),
            ));
            continue;
        }
        first_index_by_id.insert(id, index);
    }

    let has_transparent = draft
        .items
        .iter()
        .any(|item| item.background_kind == BoardBackgroundOption::Transparent);
    if !has_transparent {
        errors.push(FormError::new(
            "boards.items",
            "Keep one board with a Transparent background; the overlay draws on it",
        ));
    }

    if let Some(max_count) = max_count.filter(|max| *max > 0)
        && draft.items.len() > max_count
    {
        errors.push(FormError::new(
            "boards.items",
            format!(
                "{} boards exceed Max boards ({max_count}); remove a board or raise Max boards",
                draft.items.len()
            ),
        ));
    }
}
