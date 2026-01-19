use crate::config::{BoardBackgroundConfig, BoardColorConfig, BoardItemConfig, BoardsConfig};
use crate::draw::{BoardPages, Color, Frame};

pub const BOARD_ID_TRANSPARENT: &str = "transparent";
pub const BOARD_ID_WHITEBOARD: &str = "whiteboard";
pub const BOARD_ID_BLACKBOARD: &str = "blackboard";

#[derive(Debug, Clone)]
pub enum BoardBackground {
    Transparent,
    Solid(Color),
}

impl BoardBackground {
    pub fn is_transparent(&self) -> bool {
        matches!(self, BoardBackground::Transparent)
    }
}

#[derive(Debug, Clone)]
pub struct BoardSpec {
    pub id: String,
    pub name: String,
    pub background: BoardBackground,
    pub default_pen_color: Option<Color>,
    pub auto_adjust_pen: bool,
    pub persist: bool,
    pub pinned: bool,
}

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

#[derive(Debug, Clone)]
pub struct BoardState {
    pub spec: BoardSpec,
    pub pages: BoardPages,
}

impl BoardState {
    pub fn new(spec: BoardSpec) -> Self {
        Self {
            spec,
            pages: BoardPages::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BoardManager {
    boards: Vec<BoardState>,
    active_index: usize,
    max_count: usize,
    auto_create: bool,
    show_badge: bool,
    persist_customizations: bool,
    default_board_id: String,
    template: BoardSpec,
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
            boards.push(BoardState::new(BoardSpec {
                id: BOARD_ID_TRANSPARENT.to_string(),
                name: "Overlay".to_string(),
                background: BoardBackground::Transparent,
                default_pen_color: None,
                auto_adjust_pen: false,
                persist: true,
                pinned: false,
            }));
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

    pub fn board_count(&self) -> usize {
        self.boards.len()
    }

    pub fn active_index(&self) -> usize {
        self.active_index
    }

    pub fn active_board(&self) -> &BoardState {
        &self.boards[self.active_index]
    }

    pub fn active_board_mut(&mut self) -> &mut BoardState {
        &mut self.boards[self.active_index]
    }

    pub fn active_board_id(&self) -> &str {
        &self.active_board().spec.id
    }

    pub fn has_board(&self, id: &str) -> bool {
        self.boards.iter().any(|board| board.spec.id == id)
    }

    pub fn active_board_name(&self) -> &str {
        &self.active_board().spec.name
    }

    pub fn active_background(&self) -> &BoardBackground {
        &self.active_board().spec.background
    }

    pub fn active_pages(&self) -> &BoardPages {
        &self.active_board().pages
    }

    pub fn active_pages_mut(&mut self) -> &mut BoardPages {
        &mut self.active_board_mut().pages
    }

    pub fn active_frame(&self) -> &Frame {
        self.active_pages().active_frame()
    }

    pub fn active_frame_mut(&mut self) -> &mut Frame {
        self.active_pages_mut().active_frame_mut()
    }

    pub fn show_badge(&self) -> bool {
        self.show_badge
    }

    pub fn max_count(&self) -> usize {
        self.max_count
    }

    pub fn persist_customizations(&self) -> bool {
        self.persist_customizations
    }

    #[allow(dead_code)]
    pub fn default_board_id(&self) -> &str {
        &self.default_board_id
    }

    pub fn page_count(&self) -> usize {
        self.active_pages().page_count()
    }

    pub fn active_page_index(&self) -> usize {
        self.active_pages().active_index()
    }

    pub fn next_page(&mut self) -> bool {
        self.active_pages_mut().next_page()
    }

    pub fn prev_page(&mut self) -> bool {
        self.active_pages_mut().prev_page()
    }

    pub fn new_page(&mut self) {
        self.active_pages_mut().new_page();
    }

    pub fn duplicate_page(&mut self) {
        self.active_pages_mut().duplicate_page();
    }

    pub fn insert_page(&mut self, page: crate::draw::Frame) {
        self.active_pages_mut().insert_page(page);
    }

    pub fn delete_page(&mut self) -> crate::draw::PageDeleteOutcome {
        self.active_pages_mut().delete_page()
    }

    pub fn switch_to_id(&mut self, id: &str) -> bool {
        if let Some(index) = self.boards.iter().position(|board| board.spec.id == id) {
            self.active_index = index;
            return true;
        }

        if !self.auto_create {
            return false;
        }

        if let Some(desired_slot) = parse_board_slot(id) {
            return self.switch_to_slot(desired_slot);
        }
        false
    }

    pub fn switch_to_slot(&mut self, slot: usize) -> bool {
        if slot >= self.max_count {
            return false;
        }

        if slot < self.boards.len() {
            self.active_index = slot;
            return true;
        }

        if !self.auto_create {
            return false;
        }

        while self.boards.len() <= slot {
            if self.boards.len() >= self.max_count {
                return false;
            }
            let new_spec = self.template_for_slot(self.boards.len());
            self.boards.push(BoardState::new(new_spec));
        }

        self.active_index = slot;
        true
    }

    pub fn next_board(&mut self) -> bool {
        if self.boards.is_empty() {
            return false;
        }
        let next = (self.active_index + 1) % self.boards.len();
        self.active_index = next;
        true
    }

    pub fn prev_board(&mut self) -> bool {
        if self.boards.is_empty() {
            return false;
        }
        let prev = if self.active_index == 0 {
            self.boards.len() - 1
        } else {
            self.active_index - 1
        };
        self.active_index = prev;
        true
    }

    #[allow(dead_code)]
    pub fn board_specs(&self) -> impl Iterator<Item = &BoardSpec> {
        self.boards.iter().map(|board| &board.spec)
    }

    pub fn board_states_mut(&mut self) -> &mut [BoardState] {
        &mut self.boards
    }

    pub fn board_states(&self) -> &[BoardState] {
        &self.boards
    }

    pub fn board_state_mut(&mut self, index: usize) -> Option<&mut BoardState> {
        self.boards.get_mut(index)
    }

    #[allow(dead_code)]
    pub fn board_state_by_id_mut(&mut self, id: &str) -> Option<&mut BoardState> {
        self.boards.iter_mut().find(|board| board.spec.id == id)
    }

    pub fn remove_active_board(&mut self) -> bool {
        if self.boards.len() <= 1 {
            return false;
        }
        let removed_id = self.boards[self.active_index].spec.id.clone();
        self.boards.remove(self.active_index);
        if self.active_index >= self.boards.len() {
            self.active_index = self.boards.len().saturating_sub(1);
        }
        if self.default_board_id == removed_id {
            self.default_board_id = self
                .boards
                .get(self.active_index)
                .map(|board| board.spec.id.clone())
                .unwrap_or_else(|| BOARD_ID_TRANSPARENT.to_string());
        }
        true
    }

    pub fn move_board(&mut self, from: usize, to: usize) -> bool {
        let len = self.boards.len();
        if from >= len || to >= len || from == to {
            return false;
        }
        let active_id = self.active_board_id().to_string();
        let board = self.boards.remove(from);
        self.boards.insert(to, board);
        if let Some(index) = self.boards.iter().position(|b| b.spec.id == active_id) {
            self.active_index = index;
        } else {
            self.active_index = self.active_index.min(self.boards.len().saturating_sub(1));
        }
        true
    }

    pub fn set_board_pages(&mut self, id: &str, pages: BoardPages) -> bool {
        if let Some(board) = self.ensure_board(id) {
            board.pages = pages;
            return true;
        }
        false
    }

    pub fn ensure_board(&mut self, id: &str) -> Option<&mut BoardState> {
        if let Some(index) = self.boards.iter().position(|board| board.spec.id == id) {
            return Some(&mut self.boards[index]);
        }

        if self.boards.len() >= self.max_count {
            return None;
        }

        let mut spec = if id == BOARD_ID_TRANSPARENT {
            BoardSpec {
                id: id.to_string(),
                name: "Overlay".to_string(),
                background: BoardBackground::Transparent,
                default_pen_color: None,
                auto_adjust_pen: false,
                persist: true,
                pinned: false,
            }
        } else {
            self.template.clone()
        };

        spec.id = id.to_string();
        spec.name = name_from_id(id);
        self.boards.push(BoardState::new(spec));
        let index = self.boards.len() - 1;
        Some(&mut self.boards[index])
    }

    pub fn create_board(&mut self) -> bool {
        if self.boards.len() >= self.max_count {
            return false;
        }
        let index = self.boards.len();
        let new_spec = self.template_for_slot(index);
        self.boards.push(BoardState::new(new_spec));
        self.active_index = index;
        true
    }

    /// Insert a board at the given index.
    /// Returns true if successful, false if the board limit is reached.
    pub fn insert_board(&mut self, index: usize, board: BoardState) -> bool {
        if self.boards.len() >= self.max_count {
            return false;
        }
        let insert_at = index.min(self.boards.len());
        self.boards.insert(insert_at, board);
        self.active_index = insert_at;
        true
    }

    /// Duplicate the active board.
    /// Returns the new board's id if successful, None if the board limit is reached.
    pub fn duplicate_active_board(&mut self) -> Option<String> {
        if self.boards.len() >= self.max_count {
            return None;
        }
        let active = &self.boards[self.active_index];
        let mut new_spec = active.spec.clone();
        let base_id = format!("{}-copy", active.spec.id);
        new_spec.id = self.unique_board_id(base_id);
        new_spec.name = format!("{} (copy)", active.spec.name);

        let mut new_board = BoardState::new(new_spec.clone());
        // Clone pages from the active board
        new_board.pages = active.pages.clone();

        let insert_at = self.active_index + 1;
        self.boards.insert(insert_at, new_board);
        self.active_index = insert_at;
        Some(new_spec.id)
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
                    background: match board.spec.background {
                        BoardBackground::Transparent => {
                            BoardBackgroundConfig::Transparent("transparent".to_string())
                        }
                        BoardBackground::Solid(color) => {
                            BoardBackgroundConfig::Color(BoardColorConfig::Rgb([
                                color.r, color.g, color.b,
                            ]))
                        }
                    },
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

    fn unique_board_id(&self, base: String) -> String {
        if !self.boards.iter().any(|board| board.spec.id == base) {
            return base;
        }
        let mut suffix = 2;
        loop {
            let candidate = format!("{base}-{suffix}");
            if !self.boards.iter().any(|board| board.spec.id == candidate) {
                return candidate;
            }
            suffix += 1;
        }
    }

    fn template_for_slot(&self, slot: usize) -> BoardSpec {
        let mut spec = self.template.clone();
        let index = slot + 1;
        let base_id = format!("board-{index}");
        spec.id = self.unique_board_id(base_id);
        spec.name = format!("Board {index}");
        spec
    }
}

fn board_background_from_config(config: &BoardBackgroundConfig) -> BoardBackground {
    match config {
        BoardBackgroundConfig::Transparent(_) => BoardBackground::Transparent,
        BoardBackgroundConfig::Color(color) => {
            BoardBackground::Solid(board_color_from_config(color))
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

fn parse_board_slot(id: &str) -> Option<usize> {
    let trimmed = id.strip_prefix("board-")?;
    let index: usize = trimmed.parse().ok()?;
    if index == 0 { None } else { Some(index - 1) }
}

fn name_from_id(id: &str) -> String {
    if let Some(slot) = parse_board_slot(id) {
        return format!("Board {}", slot + 1);
    }
    let normalized = id.replace('-', " ");
    let mut chars = normalized.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Board".to_string(),
    }
}
