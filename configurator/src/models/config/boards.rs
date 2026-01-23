use std::collections::HashSet;

use wayscriber::config::{
    BoardBackgroundConfig, BoardColorConfig, BoardItemConfig, BoardsConfig, Config,
};

use super::super::color::ColorTripletInput;
use super::super::error::FormError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardBackgroundOption {
    Transparent,
    Color,
}

impl BoardBackgroundOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Transparent, Self::Color]
    }

    pub fn label(&self) -> &'static str {
        match self {
            BoardBackgroundOption::Transparent => "Transparent",
            BoardBackgroundOption::Color => "Solid color",
        }
    }
}

impl std::fmt::Display for BoardBackgroundOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardItemTextField {
    Id,
    Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardItemToggleField {
    AutoAdjustPen,
    Persist,
    Pinned,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OptionalTripletInput {
    pub enabled: bool,
    pub color: ColorTripletInput,
}

impl OptionalTripletInput {
    fn from_option(value: Option<&BoardColorConfig>, fallback: [f64; 3]) -> Self {
        match value {
            Some(color) => Self {
                enabled: true,
                color: ColorTripletInput::from(color.rgb()),
            },
            None => Self {
                enabled: false,
                color: ColorTripletInput::from(fallback),
            },
        }
    }

    fn to_option(
        &self,
        field_prefix: &str,
        errors: &mut Vec<FormError>,
    ) -> Option<BoardColorConfig> {
        if !self.enabled {
            return None;
        }
        parse_triplet(&self.color, field_prefix, errors).map(BoardColorConfig::Rgb)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoardItemDraft {
    pub id: String,
    pub name: String,
    pub background_kind: BoardBackgroundOption,
    pub background_color: ColorTripletInput,
    pub default_pen_color: OptionalTripletInput,
    pub auto_adjust_pen: bool,
    pub persist: bool,
    pub pinned: bool,
}

impl BoardItemDraft {
    fn from_config(item: &BoardItemConfig) -> Self {
        let (background_kind, background_color) = match &item.background {
            BoardBackgroundConfig::Transparent(_) => (
                BoardBackgroundOption::Transparent,
                ColorTripletInput::from([0.0, 0.0, 0.0]),
            ),
            BoardBackgroundConfig::Color(color) => (
                BoardBackgroundOption::Color,
                ColorTripletInput::from(color.rgb()),
            ),
        };
        let fallback_pen = default_pen_fallback(&item.background);
        Self {
            id: item.id.clone(),
            name: item.name.clone(),
            background_kind,
            background_color,
            default_pen_color: OptionalTripletInput::from_option(
                item.default_pen_color.as_ref(),
                fallback_pen,
            ),
            auto_adjust_pen: item.auto_adjust_pen,
            persist: item.persist,
            pinned: item.pinned,
        }
    }

    pub fn to_config(&self, index: usize, errors: &mut Vec<FormError>) -> Option<BoardItemConfig> {
        let id = self.id.trim();
        let id = if id.is_empty() {
            format!("board-{}", index + 1)
        } else {
            id.to_string()
        };
        let name = self.name.trim();
        let name = if name.is_empty() {
            format!("Board {}", index + 1)
        } else {
            name.to_string()
        };

        let background = match self.background_kind {
            BoardBackgroundOption::Transparent => {
                BoardBackgroundConfig::Transparent("transparent".to_string())
            }
            BoardBackgroundOption::Color => {
                let field = format!("boards.items[{index}].background");
                let rgb = parse_triplet(&self.background_color, &field, errors)?;
                BoardBackgroundConfig::Color(BoardColorConfig::Rgb(rgb))
            }
        };

        let default_pen_color = self
            .default_pen_color
            .to_option(&format!("boards.items[{index}].default_pen_color"), errors);

        Some(BoardItemConfig {
            id,
            name,
            background,
            default_pen_color,
            auto_adjust_pen: self.auto_adjust_pen,
            persist: self.persist,
            pinned: self.pinned,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoardsDraft {
    pub max_count: String,
    pub auto_create: bool,
    pub show_board_badge: bool,
    pub persist_customizations: bool,
    pub default_board: String,
    pub items: Vec<BoardItemDraft>,
}

impl BoardsDraft {
    pub fn from_config(config: &Config) -> Self {
        let boards = config.resolved_boards();
        Self {
            max_count: boards.max_count.to_string(),
            auto_create: boards.auto_create,
            show_board_badge: boards.show_board_badge,
            persist_customizations: boards.persist_customizations,
            default_board: boards.default_board.clone(),
            items: boards
                .items
                .iter()
                .map(BoardItemDraft::from_config)
                .collect(),
        }
    }

    pub fn to_config(&self, errors: &mut Vec<FormError>) -> BoardsConfig {
        let mut config = BoardsConfig::default();
        parse_usize(&self.max_count, "boards.max_count", errors, |value| {
            config.max_count = value
        });
        config.auto_create = self.auto_create;
        config.show_board_badge = self.show_board_badge;
        config.persist_customizations = self.persist_customizations;

        let default_board = self.default_board.trim();
        if !default_board.is_empty() {
            config.default_board = default_board.to_string();
        }

        config.items = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| item.to_config(index, errors))
            .collect();
        config
    }

    pub fn effective_ids(&self) -> Vec<String> {
        self.items
            .iter()
            .enumerate()
            .map(|(index, item)| self.effective_id_for_value(&item.id, index))
            .collect()
    }

    pub fn effective_id_for_value(&self, id: &str, index: usize) -> String {
        let id = id.trim();
        if id.is_empty() {
            format!("board-{}", index + 1)
        } else {
            id.to_string()
        }
    }

    pub fn effective_id_for_index(&self, index: usize) -> Option<String> {
        self.items
            .get(index)
            .map(|item| self.effective_id_for_value(&item.id, index))
    }

    pub fn next_board_id(&self) -> String {
        let existing: HashSet<&str> = self.items.iter().map(|item| item.id.as_str()).collect();
        let mut index = 1;
        loop {
            let candidate = format!("board-{index}");
            if !existing.contains(candidate.as_str()) {
                return candidate;
            }
            index += 1;
        }
    }

    pub fn new_item(&self) -> BoardItemDraft {
        let id = self.next_board_id();
        let name = format!("Board {}", self.items.len() + 1);
        BoardItemDraft {
            id,
            name,
            background_kind: BoardBackgroundOption::Color,
            background_color: ColorTripletInput::from([0.992, 0.992, 0.992]),
            default_pen_color: OptionalTripletInput::from_option(
                Some(&BoardColorConfig::Rgb([0.0, 0.0, 0.0])),
                [0.0, 0.0, 0.0],
            ),
            auto_adjust_pen: true,
            persist: true,
            pinned: false,
        }
    }

    pub fn ensure_default_exists(&mut self) {
        if self.items.is_empty() {
            self.default_board.clear();
            return;
        }

        let default_id = self.default_board.trim();
        if default_id.is_empty() {
            if let Some((index, item)) = self
                .items
                .iter()
                .enumerate()
                .find(|(_, item)| item.id.trim().is_empty())
            {
                self.default_board = self.effective_id_for_value(&item.id, index);
                return;
            }
        } else {
            let has_default =
                self.items.iter().enumerate().any(|(index, item)| {
                    self.effective_id_for_value(&item.id, index) == default_id
                });
            if has_default {
                return;
            }
        }

        self.default_board = self.effective_id_for_value(&self.items[0].id, 0);
    }
}

fn default_pen_fallback(background: &BoardBackgroundConfig) -> [f64; 3] {
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

fn parse_triplet(
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

fn parse_usize<F>(value: &str, field: &'static str, errors: &mut Vec<FormError>, apply: F)
where
    F: FnOnce(usize),
{
    match value.trim().parse::<usize>() {
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}
