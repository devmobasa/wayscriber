use super::super::Frame;

/// Collection of pages for a single board mode.
#[derive(Debug, Clone)]
pub struct BoardPages {
    pages: Vec<Frame>,
    active: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageDeleteOutcome {
    Removed,
    Cleared,
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

    pub fn new_page(&mut self) {
        self.pages.push(Frame::new());
        self.active = self.pages.len() - 1;
    }

    pub fn duplicate_page(&mut self) {
        let cloned = self.active_frame().clone_without_history();
        self.pages.push(cloned);
        self.active = self.pages.len() - 1;
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

    pub fn pages_mut(&mut self) -> &mut Vec<Frame> {
        &mut self.pages
    }
}
