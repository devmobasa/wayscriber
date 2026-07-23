use super::{
    BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD, BoardBackground, BoardIdentityGeneration,
    BoardManager, BoardSpec, BoardState, board_color_from_config, board_color_to_config,
    runtime_contrast_pen_color,
};
use crate::config::{BoardBackgroundConfig, BoardItemConfig, BoardsConfig};
use crate::domain::Color;
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub(crate) enum BoardConfigChange {
    Structure,
    IdentitiesCreated(Vec<String>),
    IdentityDeleted(String),
    IdentityRestored(String),
    Name(String),
    Appearance(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PendingBoardRuntimeUiAction {
    TogglePin {
        board_id: String,
        board_identity_generation: BoardIdentityGeneration,
        pin_seed: bool,
    },
    IdentityDeleted {
        board_id: String,
    },
    IdentityAvailable {
        board_id: String,
        pin_seed: bool,
        pinned: bool,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PendingBoardConfigUpdate {
    pub(crate) snapshot: BoardsConfig,
    pub(crate) structure_changed: bool,
    pub(crate) created_ids: BTreeSet<String>,
    pub(crate) deleted_ids: BTreeSet<String>,
    pub(crate) changed_names: BTreeSet<String>,
    pub(crate) changed_appearances: BTreeSet<String>,
}

impl PendingBoardConfigUpdate {
    pub(crate) fn new(snapshot: BoardsConfig, change: BoardConfigChange) -> Self {
        let mut update = Self {
            snapshot,
            structure_changed: false,
            created_ids: BTreeSet::new(),
            deleted_ids: BTreeSet::new(),
            changed_names: BTreeSet::new(),
            changed_appearances: BTreeSet::new(),
        };
        update.record(change);
        update
    }

    pub(crate) fn merge(&mut self, snapshot: BoardsConfig, change: BoardConfigChange) {
        self.snapshot = snapshot;
        self.record(change);
    }

    pub(crate) fn into_snapshot(self) -> BoardsConfig {
        self.snapshot
    }

    fn record(&mut self, change: BoardConfigChange) {
        match change {
            BoardConfigChange::Structure => self.structure_changed = true,
            BoardConfigChange::IdentitiesCreated(ids) => {
                self.structure_changed = true;
                self.created_ids.extend(ids);
            }
            BoardConfigChange::IdentityDeleted(id) => {
                self.structure_changed = true;
                self.deleted_ids.insert(id);
            }
            BoardConfigChange::IdentityRestored(id) => {
                self.structure_changed = true;
                self.deleted_ids.remove(&id);
            }
            BoardConfigChange::Name(id) => {
                self.changed_names.insert(id);
            }
            BoardConfigChange::Appearance(id) => {
                self.changed_appearances.insert(id);
            }
        }
    }
}
use crate::domain::color::PALETTE_BLACK;

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
            BoardBackground::Solid(color) => Some(runtime_contrast_pen_color(color)),
            BoardBackground::Transparent => None,
        }
    }
}

impl BoardManager {
    pub fn from_config(config: BoardsConfig) -> Self {
        let pin_seeds = config
            .items
            .iter()
            .map(|item| (item.id.clone(), item.pinned))
            .collect();
        let mut boards: Vec<BoardState> = config
            .items
            .iter()
            .map(BoardSpec::from_config)
            .map(BoardState::new)
            .collect();

        if boards.is_empty() {
            boards.push(default_overlay_board());
        }
        let mut pin_seeds: std::collections::BTreeMap<String, bool> = pin_seeds;
        for board in &boards {
            pin_seeds
                .entry(board.spec.id.clone())
                .or_insert(board.spec.pinned);
        }

        let active_index = boards
            .iter()
            .position(|board| board.spec.id == config.default_board)
            .unwrap_or(0);

        let template = pick_template(&boards);

        Self {
            boards,
            pin_seeds,
            active_index,
            max_count: config.max_count,
            auto_create: config.auto_create,
            show_badge: config.show_board_badge,
            pan_enabled: config.pan_enabled,
            show_pan_badge: config.show_pan_badge,
            persist_customizations: config.persist_customizations,
            default_board_id: config.default_board,
            template,
            identity_generation: BoardIdentityGeneration::fresh(),
        }
    }

    pub fn to_config(&self) -> BoardsConfig {
        BoardsConfig {
            max_count: self.max_count,
            auto_create: self.auto_create,
            show_board_badge: self.show_badge,
            pan_enabled: self.pan_enabled,
            show_pan_badge: self.show_pan_badge,
            persist_customizations: self.persist_customizations,
            default_board: self.default_board_id.clone(),
            items: self
                .boards
                .iter()
                .map(|board| BoardItemConfig {
                    id: board.spec.id.clone(),
                    name: board.spec.name.clone(),
                    background: board_background_to_config(&board.spec.background),
                    default_pen_color: board.spec.default_pen_color.map(board_color_to_config),
                    auto_adjust_pen: board.spec.auto_adjust_pen,
                    persist: board.spec.persist,
                    pinned: self
                        .pin_seeds
                        .get(&board.spec.id)
                        .copied()
                        .unwrap_or(board.spec.pinned),
                })
                .collect(),
        }
    }

    pub(crate) fn sync_pin_seeds_from_config(&mut self, config: &BoardsConfig) {
        let configured = config
            .items
            .iter()
            .map(|item| (item.id.as_str(), item.pinned))
            .collect::<std::collections::BTreeMap<_, _>>();
        let live_ids = self
            .boards
            .iter()
            .map(|board| board.spec.id.as_str())
            .collect::<BTreeSet<_>>();
        self.pin_seeds
            .retain(|id, _| live_ids.contains(id.as_str()));
        for board in &self.boards {
            if let Some(pinned) = configured.get(board.spec.id.as_str()) {
                self.pin_seeds.insert(board.spec.id.clone(), *pinned);
            } else {
                self.pin_seeds
                    .entry(board.spec.id.clone())
                    .or_insert(board.spec.pinned);
            }
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
            BoardBackgroundConfig::Color(board_color_to_config(*color))
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
            default_pen_color: Some(PALETTE_BLACK),
            auto_adjust_pen: true,
            persist: true,
            pinned: false,
        });
    template.pinned = false;
    template
}
