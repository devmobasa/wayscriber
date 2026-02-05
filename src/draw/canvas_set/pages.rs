use super::super::Frame;

/// Collection of pages for a single board mode.
#[derive(Debug, Clone)]
pub struct BoardPages {
    pages: Vec<Frame>,
    active: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageDeleteOutcome {
    /// Page was removed from the board
    Removed,
    /// Last page was cleared (can't delete)
    Cleared,
    /// Deletion pending confirmation (first press)
    Pending,
}

impl Default for BoardPages {
    fn default() -> Self {
        Self::new()
    }
}

impl BoardPages {
    pub fn new() -> Self {
        Self {
            pages: vec![Frame::new()],
            active: 0,
        }
    }

    pub fn from_pages(mut pages: Vec<Frame>, active: usize) -> Self {
        if pages.is_empty() {
            pages.push(Frame::new());
        }
        let active = active.min(pages.len() - 1);
        Self { pages, active }
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active_frame(&self) -> &Frame {
        &self.pages[self.active]
    }

    pub fn active_frame_mut(&mut self) -> &mut Frame {
        &mut self.pages[self.active]
    }

    pub fn next_page(&mut self) -> bool {
        if self.active + 1 < self.pages.len() {
            self.active += 1;
            true
        } else {
            false
        }
    }

    pub fn prev_page(&mut self) -> bool {
        if self.active > 0 {
            self.active -= 1;
            true
        } else {
            false
        }
    }

    pub fn switch_to_page(&mut self, index: usize) -> bool {
        if index < self.pages.len() && index != self.active {
            self.active = index;
            true
        } else {
            false
        }
    }

    pub fn new_page(&mut self) {
        self.pages.push(Frame::new());
        self.active = self.pages.len() - 1;
    }

    pub fn duplicate_page(&mut self) {
        let cloned = self.active_frame().clone_without_history();
        self.pages.push(cloned);
        self.active = self.pages.len() - 1;
    }

    pub fn duplicate_page_at(&mut self, index: usize) -> Option<usize> {
        if index >= self.pages.len() {
            return None;
        }
        let cloned = self.pages[index].clone_without_history();
        let insert_at = (index + 1).min(self.pages.len());
        self.pages.insert(insert_at, cloned);
        self.active = insert_at;
        Some(insert_at)
    }

    /// Insert a page after the current position.
    pub fn insert_page(&mut self, page: Frame) {
        let insert_at = (self.active + 1).min(self.pages.len());
        self.pages.insert(insert_at, page);
        self.active = insert_at;
    }

    pub fn delete_page(&mut self) -> PageDeleteOutcome {
        if self.pages.len() == 1 {
            self.pages[0].clear();
            PageDeleteOutcome::Cleared
        } else {
            self.pages.remove(self.active);
            if self.active >= self.pages.len() {
                self.active = self.pages.len() - 1;
            }
            PageDeleteOutcome::Removed
        }
    }

    /// Delete a specific page by index.
    pub fn delete_page_at(&mut self, index: usize) -> PageDeleteOutcome {
        let len = self.pages.len();
        if index >= len {
            return PageDeleteOutcome::Pending;
        }
        if len == 1 {
            self.pages[0].clear();
            self.active = 0;
            return PageDeleteOutcome::Cleared;
        }
        self.pages.remove(index);
        if self.active == index {
            if self.active >= self.pages.len() {
                self.active = self.pages.len() - 1;
            }
        } else if self.active > index {
            self.active = self.active.saturating_sub(1);
        }
        PageDeleteOutcome::Removed
    }

    /// Remove a page and return it, keeping at least one page in the board.
    pub fn take_page(&mut self, index: usize) -> Option<Frame> {
        let len = self.pages.len();
        if index >= len {
            return None;
        }
        if len == 1 {
            let page = std::mem::take(&mut self.pages[0]);
            self.active = 0;
            return Some(page);
        }
        let page = self.pages.remove(index);
        if self.active == index {
            if self.active >= self.pages.len() {
                self.active = self.pages.len() - 1;
            }
        } else if self.active > index {
            self.active = self.active.saturating_sub(1);
        }
        Some(page)
    }

    /// Append a page and make it active. Returns the new index.
    pub fn push_page(&mut self, page: Frame) -> usize {
        self.pages.push(page);
        self.active = self.pages.len() - 1;
        self.active
    }

    /// Move a page from one index to another.
    pub fn move_page(&mut self, from: usize, to: usize) -> bool {
        let len = self.pages.len();
        if from >= len || to >= len {
            return false;
        }
        if from == to {
            return true;
        }
        let page = self.pages.remove(from);
        let insert_index = to.min(self.pages.len());
        self.pages.insert(insert_index, page);

        if self.active == from {
            self.active = insert_index;
        } else if from < self.active && insert_index >= self.active {
            self.active = self.active.saturating_sub(1);
        } else if from > self.active && insert_index <= self.active {
            self.active = (self.active + 1).min(self.pages.len().saturating_sub(1));
        }

        true
    }

    #[allow(dead_code)]
    pub fn trim_trailing_empty_pages(&mut self) {
        while self.pages.len() > 1
            && self
                .pages
                .last()
                .is_some_and(|frame| !frame.has_persistable_data())
        {
            self.pages.pop();
            if self.active >= self.pages.len() {
                self.active = self.pages.len() - 1;
            }
        }
    }

    pub fn pages(&self) -> &[Frame] {
        &self.pages
    }

    pub fn page_name(&self, index: usize) -> Option<&str> {
        self.pages.get(index).and_then(|page| page.page_name())
    }

    pub fn set_page_name(&mut self, index: usize, name: Option<String>) -> bool {
        let Some(page) = self.pages.get_mut(index) else {
            return false;
        };
        page.set_page_name(name);
        true
    }

    pub fn pages_mut(&mut self) -> &mut Vec<Frame> {
        &mut self.pages
    }
}
