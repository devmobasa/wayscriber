use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::board::BoardConfig;

/// Configurable multi-board settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BoardsConfig {
    /// Maximum number of boards allowed.
    #[serde(default = "default_boards_max_count")]
    pub max_count: usize,

    /// Auto-create boards when switching to a missing slot.
    #[serde(default = "default_boards_auto_create")]
    pub auto_create: bool,

    /// Show the board badge in the status bar.
    #[serde(default = "default_boards_show_badge")]
    pub show_board_badge: bool,

    /// Persist runtime edits (name/color) back to config.
    #[serde(default = "default_boards_persist_customizations")]
    pub persist_customizations: bool,

    /// Default board id on startup.
    #[serde(default = "default_boards_default_board")]
    pub default_board: String,

    /// Board definitions.
    #[serde(default)]
    pub items: Vec<BoardItemConfig>,
}

impl Default for BoardsConfig {
    fn default() -> Self {
        Self {
            max_count: default_boards_max_count(),
            auto_create: default_boards_auto_create(),
            show_board_badge: default_boards_show_badge(),
            persist_customizations: default_boards_persist_customizations(),
            default_board: default_boards_default_board(),
            items: Self::default_items(),
        }
    }
}

impl BoardsConfig {
    pub fn default_items() -> Vec<BoardItemConfig> {
        default_board_items()
    }

    pub fn from_legacy(legacy: &BoardConfig) -> Self {
        let mut items = vec![BoardItemConfig {
            id: "transparent".to_string(),
            name: "Overlay".to_string(),
            background: BoardBackgroundConfig::Transparent("transparent".to_string()),
            default_pen_color: None,
            auto_adjust_pen: false,
            persist: true,
            pinned: false,
        }];

        if legacy.enabled {
            items.push(BoardItemConfig {
                id: "whiteboard".to_string(),
                name: "Whiteboard".to_string(),
                background: BoardBackgroundConfig::Color(BoardColorConfig::Rgb(
                    legacy.whiteboard_color,
                )),
                default_pen_color: Some(BoardColorConfig::Rgb(legacy.whiteboard_pen_color)),
                auto_adjust_pen: legacy.auto_adjust_pen,
                persist: true,
                pinned: false,
            });
            items.push(BoardItemConfig {
                id: "blackboard".to_string(),
                name: "Blackboard".to_string(),
                background: BoardBackgroundConfig::Color(BoardColorConfig::Rgb(
                    legacy.blackboard_color,
                )),
                default_pen_color: Some(BoardColorConfig::Rgb(legacy.blackboard_pen_color)),
                auto_adjust_pen: legacy.auto_adjust_pen,
                persist: true,
                pinned: false,
            });
        }

        Self {
            max_count: default_boards_max_count(),
            auto_create: default_boards_auto_create(),
            show_board_badge: default_boards_show_badge(),
            persist_customizations: default_boards_persist_customizations(),
            default_board: legacy.default_mode.clone(),
            items,
        }
    }
}

/// Single board definition.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BoardItemConfig {
    /// Stable board id (used for keybindings and persistence).
    pub id: String,

    /// Human-friendly display name.
    pub name: String,

    /// Background style (transparent or solid RGB).
    #[serde(default = "default_board_background")]
    pub background: BoardBackgroundConfig,

    /// Default pen color when auto-adjust is enabled.
    #[serde(default)]
    pub default_pen_color: Option<BoardColorConfig>,

    /// Automatically switch pen color when entering this board.
    #[serde(default = "default_board_auto_adjust_pen")]
    pub auto_adjust_pen: bool,

    /// Persist this board in session saves.
    #[serde(default = "default_board_persist")]
    pub persist: bool,

    /// Pin this board to the top of the quick switch list.
    #[serde(default = "default_board_pinned")]
    pub pinned: bool,
}

/// Background specification: "transparent" or an RGB color.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum BoardBackgroundConfig {
    Transparent(String),
    Color(BoardColorConfig),
}

impl BoardBackgroundConfig {
    pub fn is_transparent(&self) -> bool {
        matches!(self, BoardBackgroundConfig::Transparent(_))
    }
}

/// RGB color input, either as an array or `{ rgb = [..] }`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum BoardColorConfig {
    Rgb([f64; 3]),
    RgbMap { rgb: [f64; 3] },
}

impl BoardColorConfig {
    pub fn rgb(&self) -> [f64; 3] {
        match self {
            BoardColorConfig::Rgb(rgb) => *rgb,
            BoardColorConfig::RgbMap { rgb } => *rgb,
        }
    }
}

fn default_boards_max_count() -> usize {
    9
}

fn default_boards_auto_create() -> bool {
    true
}

fn default_boards_show_badge() -> bool {
    true
}

fn default_boards_persist_customizations() -> bool {
    true
}

fn default_boards_default_board() -> String {
    "transparent".to_string()
}

fn default_board_background() -> BoardBackgroundConfig {
    BoardBackgroundConfig::Transparent("transparent".to_string())
}

fn default_board_auto_adjust_pen() -> bool {
    true
}

fn default_board_persist() -> bool {
    true
}

fn default_board_pinned() -> bool {
    false
}

fn default_board_items() -> Vec<BoardItemConfig> {
    vec![
        BoardItemConfig {
            id: "transparent".to_string(),
            name: "Overlay".to_string(),
            background: BoardBackgroundConfig::Transparent("transparent".to_string()),
            default_pen_color: None,
            auto_adjust_pen: false,
            persist: true,
            pinned: false,
        },
        BoardItemConfig {
            id: "whiteboard".to_string(),
            name: "Whiteboard".to_string(),
            background: BoardBackgroundConfig::Color(BoardColorConfig::Rgb([0.992, 0.992, 0.992])),
            default_pen_color: Some(BoardColorConfig::Rgb([0.0, 0.0, 0.0])),
            auto_adjust_pen: true,
            persist: true,
            pinned: false,
        },
        BoardItemConfig {
            id: "blackboard".to_string(),
            name: "Blackboard".to_string(),
            background: BoardBackgroundConfig::Color(BoardColorConfig::Rgb([0.067, 0.067, 0.067])),
            default_pen_color: Some(BoardColorConfig::Rgb([1.0, 1.0, 1.0])),
            auto_adjust_pen: true,
            persist: true,
            pinned: false,
        },
        BoardItemConfig {
            id: "blueprint".to_string(),
            name: "Blueprint".to_string(),
            background: BoardBackgroundConfig::Color(BoardColorConfig::Rgb([0.063, 0.125, 0.251])),
            default_pen_color: Some(BoardColorConfig::Rgb([0.902, 0.945, 1.0])),
            auto_adjust_pen: true,
            persist: true,
            pinned: false,
        },
        BoardItemConfig {
            id: "corkboard".to_string(),
            name: "Corkboard".to_string(),
            background: BoardBackgroundConfig::Color(BoardColorConfig::Rgb([0.420, 0.294, 0.165])),
            default_pen_color: Some(BoardColorConfig::Rgb([0.969, 0.890, 0.784])),
            auto_adjust_pen: true,
            persist: true,
            pinned: false,
        },
    ]
}
