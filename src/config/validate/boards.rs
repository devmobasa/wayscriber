use crate::config::types::{BoardBackgroundConfig, BoardColorConfig, BoardsConfig};
use crate::input::boards::{BoundaryBoardIdSet, clamp_board_rgb};
use log::warn;

use super::Config;

impl Config {
    pub(super) fn validate_boards(&mut self) {
        let Some(boards) = self.boards.as_mut() else {
            return;
        };

        if boards.items.is_empty() {
            boards.items = BoardsConfig::default_items();
        }

        if boards.max_count == 0 {
            warn!("boards.max_count must be >= 1; falling back to 1");
            boards.max_count = 1;
        }

        let mut seen = BoundaryBoardIdSet::new();
        for (index, item) in boards.items.iter_mut().enumerate() {
            let raw_id = item.id.clone();
            let normalized = seen.normalize_unique(&raw_id, index);
            if normalized.changes.defaulted_empty {
                warn!("Board id was empty; using '{}'", normalized.value);
            } else {
                if normalized.changes.trimmed || normalized.changes.lowercased {
                    warn!("Board id '{}' normalized to '{}'", raw_id, normalized.value);
                }
                if normalized.changes.deduplicated {
                    warn!(
                        "Board id '{}' deduplicated to '{}'",
                        raw_id, normalized.value
                    );
                }
            }
            item.id = normalized.value;

            if item.name.trim().is_empty() {
                item.name = format!("Board {}", index + 1);
                warn!("Board '{}' had empty name; using '{}'", item.id, item.name);
            }

            normalize_background(&mut item.background, &item.id);
            if let Some(color) = item.default_pen_color.as_mut() {
                clamp_color(
                    color,
                    &format!("boards.items[{}].default_pen_color", item.id),
                );
            }
        }

        boards.items.retain(|item| !item.id.is_empty());

        let transparent_in_range = boards
            .items
            .iter()
            .take(boards.max_count)
            .any(|item| item.background.is_transparent());
        if !transparent_in_range {
            if let Some(index) = boards
                .items
                .iter()
                .position(|item| item.background.is_transparent())
            {
                let item = boards.items.remove(index);
                boards.items.insert(0, item);
            } else {
                warn!("No transparent board defined; adding default overlay board");
                boards.items.insert(
                    0,
                    BoardsConfig::default_items()
                        .into_iter()
                        .find(|item| item.background.is_transparent())
                        .expect("default items include transparent board"),
                );
            }
        }

        if boards.items.len() > boards.max_count {
            warn!(
                "boards.items exceeds max_count {}; truncating",
                boards.max_count
            );
            boards.items.truncate(boards.max_count);
        }

        if !boards
            .items
            .iter()
            .any(|item| item.id == boards.default_board)
        {
            let fallback = boards
                .items
                .iter()
                .find(|item| item.background.is_transparent())
                .map(|item| item.id.clone())
                .unwrap_or_else(|| boards.items[0].id.clone());
            warn!(
                "Default board '{}' not found; falling back to '{}'",
                boards.default_board, fallback
            );
            boards.default_board = fallback;
        }
    }
}

fn normalize_background(background: &mut BoardBackgroundConfig, id: &str) {
    match background {
        BoardBackgroundConfig::Transparent(value) => {
            if value.to_lowercase() != "transparent" {
                warn!(
                    "Board '{}' background '{}' invalid; using transparent",
                    id, value
                );
                *value = "transparent".to_string();
            }
        }
        BoardBackgroundConfig::Color(color) => {
            clamp_color(color, &format!("boards.items[{}].background", id));
        }
    }
}

fn clamp_color(color: &mut BoardColorConfig, label: &str) {
    let original = color.rgb();
    let (rgb, clamped) = clamp_board_rgb(original);
    if clamped {
        for (i, (before, after)) in original.iter().zip(rgb.iter()).enumerate() {
            if before != after {
                warn!(
                    "Invalid {}[{}] = {:.3}, clamping to 0.0-1.0",
                    label, i, *before
                );
            }
        }
    }
    *color = BoardColorConfig::Rgb(rgb);
}
