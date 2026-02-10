use super::{BOARD_ID_TRANSPARENT, BoardBackground, BoardManager, BoardSpec, BoardState};

impl BoardManager {
    pub fn switch_to_id(&mut self, id: &str) -> bool {
        if let Some(index) = self.boards.iter().position(|board| board.spec.id == id) {
            self.active_index = index;
            return true;
        }

        if !self.auto_create {
            return false;
        }

        let Some(desired_slot) = parse_board_slot(id) else {
            return false;
        };
        self.switch_to_slot(desired_slot)
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

pub(super) fn parse_board_slot(id: &str) -> Option<usize> {
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
