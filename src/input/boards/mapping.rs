use super::{
    BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD, BoardBackground, BoardManager, BoardSpec, BoardState,
};
use crate::config::{BoardBackgroundConfig, BoardColorConfig, BoardItemConfig, BoardsConfig};
use crate::draw::Color;

impl BoardSpec {
    pub fn from_config(item: &BoardItemConfig) -> Self {
        Self {
            id: item.id.clone(),
            name: item.name.clone(),
            background: board_background_from_config(&item.background),
            default_pen_color: item.default_pen_color.as_ref().map(board_color_from_config),
            auto_adjust_pen: item.auto_adjust_pen,
            persist: item.persist,
            pinned: item.pinned,
        }
    }

    pub fn effective_pen_color(&self) -> Option<Color> {
        if let Some(color) = self.default_pen_color {
            return Some(color);
        }

        if !self.auto_adjust_pen {
            return None;
        }

        match self.background {
            BoardBackground::Solid(color) => Some(contrast_color(color)),
            BoardBackground::Transparent => None,
        }
    }
}

impl BoardManager {
    pub fn from_config(config: BoardsConfig) -> Self {
        let mut boards: Vec<BoardState> = config
            .items
            .iter()
            .map(BoardSpec::from_config)
            .map(BoardState::new)
            .collect();

        if boards.is_empty() {
            boards.push(default_overlay_board());
        }

        let active_index = boards
            .iter()
            .position(|board| board.spec.id == config.default_board)
            .unwrap_or(0);

        let template = pick_template(&boards);

        Self {
            boards,
            active_index,
            max_count: config.max_count,
            auto_create: config.auto_create,
            show_badge: config.show_board_badge,
            persist_customizations: config.persist_customizations,
            default_board_id: config.default_board,
            template,
        }
    }

    pub fn to_config(&self) -> BoardsConfig {
        BoardsConfig {
            max_count: self.max_count,
            auto_create: self.auto_create,
            show_board_badge: self.show_badge,
            persist_customizations: self.persist_customizations,
            default_board: self.default_board_id.clone(),
            items: self
                .boards
                .iter()
                .map(|board| BoardItemConfig {
                    id: board.spec.id.clone(),
                    name: board.spec.name.clone(),
                    background: board_background_to_config(&board.spec.background),
                    default_pen_color: board
                        .spec
                        .default_pen_color
                        .map(|color| BoardColorConfig::Rgb([color.r, color.g, color.b])),
                    auto_adjust_pen: board.spec.auto_adjust_pen,
                    persist: board.spec.persist,
                    pinned: board.spec.pinned,
                })
                .collect(),
        }
    }
}

fn default_overlay_board() -> BoardState {
    BoardState::new(BoardSpec {
        id: BOARD_ID_TRANSPARENT.to_string(),
        name: "Overlay".to_string(),
        background: BoardBackground::Transparent,
        default_pen_color: None,
        auto_adjust_pen: false,
        persist: true,
        pinned: false,
    })
}

fn board_background_from_config(config: &BoardBackgroundConfig) -> BoardBackground {
    match config {
        BoardBackgroundConfig::Transparent(_) => BoardBackground::Transparent,
        BoardBackgroundConfig::Color(color) => {
            BoardBackground::Solid(board_color_from_config(color))
        }
    }
}

fn board_background_to_config(background: &BoardBackground) -> BoardBackgroundConfig {
    match background {
        BoardBackground::Transparent => {
            BoardBackgroundConfig::Transparent("transparent".to_string())
        }
        BoardBackground::Solid(color) => {
            BoardBackgroundConfig::Color(BoardColorConfig::Rgb([color.r, color.g, color.b]))
        }
    }
}

fn board_color_from_config(config: &BoardColorConfig) -> Color {
    let rgb = config.rgb();
    Color {
        r: rgb[0],
        g: rgb[1],
        b: rgb[2],
        a: 1.0,
    }
}

fn contrast_color(background: Color) -> Color {
    let luminance = 0.2126 * background.r + 0.7152 * background.g + 0.0722 * background.b;
    if luminance > 0.5 {
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }
    } else {
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    }
}

fn pick_template(boards: &[BoardState]) -> BoardSpec {
    let mut template = boards
        .iter()
        .find(|board| !board.spec.background.is_transparent())
        .map(|board| board.spec.clone())
        .unwrap_or_else(|| BoardSpec {
            id: BOARD_ID_WHITEBOARD.to_string(),
            name: "Whiteboard".to_string(),
            background: BoardBackground::Solid(Color {
                r: 0.992,
                g: 0.992,
                b: 0.992,
                a: 1.0,
            }),
            default_pen_color: Some(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
            auto_adjust_pen: true,
            persist: true,
            pinned: false,
        });
    template.pinned = false;
    template
}
