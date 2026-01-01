use super::super::super::base::InputState;

impl InputState {
    pub fn set_properties_panel_focus(&mut self, focus: Option<usize>) {
        if let Some(panel) = self.shape_properties_panel.as_mut() {
            panel.keyboard_focus = focus;
        }
    }

    pub(in crate::input::state::core::properties) fn current_properties_focus_or_hover(
        &self,
    ) -> Option<usize> {
        self.shape_properties_panel
            .as_ref()
            .and_then(|panel| panel.keyboard_focus.or(panel.hover_index))
    }

    pub(crate) fn focus_next_properties_entry(&mut self) -> bool {
        self.advance_properties_focus(true)
    }

    pub(crate) fn focus_previous_properties_entry(&mut self) -> bool {
        self.advance_properties_focus(false)
    }

    pub(crate) fn focus_first_properties_entry(&mut self) -> bool {
        self.select_properties_edge_entry(true)
    }

    pub(crate) fn focus_last_properties_entry(&mut self) -> bool {
        self.select_properties_edge_entry(false)
    }

    fn select_properties_edge_entry(&mut self, start_front: bool) -> bool {
        let Some(panel) = self.shape_properties_panel.as_ref() else {
            return false;
        };
        if panel.entries.is_empty() {
            return false;
        }

        let mut index = if start_front {
            0
        } else {
            panel.entries.len().saturating_sub(1)
        };
        loop {
            let Some(entry) = panel.entries.get(index) else {
                return false;
            };
            if !entry.disabled {
                break;
            }
            if start_front {
                index += 1;
                if index >= panel.entries.len() {
                    return false;
                }
            } else if index == 0 {
                return false;
            } else {
                index -= 1;
            }
        }

        self.set_properties_panel_focus(Some(index));
        true
    }

    fn advance_properties_focus(&mut self, forward: bool) -> bool {
        let Some(panel) = self.shape_properties_panel.as_ref() else {
            return false;
        };
        if panel.entries.is_empty() {
            return false;
        }

        let index = match self.current_properties_focus_or_hover() {
            Some(index) => index,
            None => {
                return if forward {
                    self.select_properties_edge_entry(true)
                } else {
                    self.select_properties_edge_entry(false)
                };
            }
        };

        let mut next = if forward {
            index + 1
        } else {
            index.saturating_sub(1)
        };
        loop {
            let Some(entry) = panel.entries.get(next) else {
                return false;
            };
            if !entry.disabled {
                break;
            }
            if forward {
                next += 1;
            } else if next == 0 {
                return false;
            } else {
                next -= 1;
            }
        }

        self.set_properties_panel_focus(Some(next));
        true
    }
}
