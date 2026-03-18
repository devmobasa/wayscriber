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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor, Shape};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        )
    }

    fn open_rect_panel(state: &mut InputState) {
        let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
            fill: false,
            color: state.current_color,
            thick: state.current_thickness,
        });
        state.set_selection(vec![shape_id]);
        assert!(state.show_properties_panel());
    }

    #[test]
    fn current_properties_focus_prefers_keyboard_focus_over_hover() {
        let mut state = make_state();
        open_rect_panel(&mut state);
        let panel = state.shape_properties_panel.as_mut().expect("panel");
        panel.hover_index = Some(1);
        panel.keyboard_focus = Some(0);

        assert_eq!(state.current_properties_focus_or_hover(), Some(0));
    }

    #[test]
    fn focus_first_properties_entry_skips_disabled_entries() {
        let mut state = make_state();
        open_rect_panel(&mut state);
        let panel = state.shape_properties_panel.as_mut().expect("panel");
        panel.entries[0].disabled = true;
        panel.entries[1].disabled = false;

        assert!(state.focus_first_properties_entry());
        assert_eq!(state.properties_panel().and_then(|panel| panel.keyboard_focus), Some(1));
    }

    #[test]
    fn focus_last_properties_entry_skips_disabled_entries() {
        let mut state = make_state();
        open_rect_panel(&mut state);
        let last = state.properties_panel().expect("panel").entries.len() - 1;
        let panel = state.shape_properties_panel.as_mut().expect("panel");
        panel.entries[last].disabled = true;

        assert!(state.focus_last_properties_entry());
        assert_eq!(state.properties_panel().and_then(|panel| panel.keyboard_focus), Some(last - 1));
    }

    #[test]
    fn focus_next_properties_entry_uses_hover_when_keyboard_focus_is_missing() {
        let mut state = make_state();
        open_rect_panel(&mut state);
        let panel = state.shape_properties_panel.as_mut().expect("panel");
        panel.hover_index = Some(0);
        panel.keyboard_focus = None;
        panel.entries[1].disabled = true;

        assert!(state.focus_next_properties_entry());
        assert_eq!(state.properties_panel().and_then(|panel| panel.keyboard_focus), Some(2));
    }

    #[test]
    fn focus_previous_properties_entry_at_start_is_a_stable_no_op() {
        let mut state = make_state();
        open_rect_panel(&mut state);
        state.set_properties_panel_focus(Some(0));

        assert!(state.focus_previous_properties_entry());
        assert_eq!(state.properties_panel().and_then(|panel| panel.keyboard_focus), Some(0));
    }
}
