use super::{color_to_hex, parse_hex_color};
use crate::domain::{BoardBackground, BoardGrid, BoardGridKind, Color};
use crate::input::InputState;
use crate::input::boards::{BoardAppearance, BoardIdentityGeneration};
use crate::input::events::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppearanceField {
    Color,
    Pattern,
    Spacing,
}

/// A board-identity-bound draft. Only Apply writes to the board or session.
#[derive(Debug)]
pub(crate) struct BoardAppearanceEdit {
    id: String,
    generation: BoardIdentityGeneration,
    original: BoardAppearance,
    pub(crate) color: String,
    pub(crate) kind: BoardGridKind,
    pub(crate) spacing: String,
    color_dirty: bool,
    kind_dirty: bool,
    spacing_dirty: bool,
    pub(crate) focus: AppearanceField,
    pub(crate) error: Option<String>,
}

impl BoardAppearanceEdit {
    pub(super) fn set_color_text(&mut self, value: String) {
        self.color_dirty = self.color != value;
        self.color = value;
    }
    pub(crate) fn validation_error(&self) -> Option<&'static str> {
        if parse_hex_color(&self.color).is_none() {
            return Some("Use a color in #RRGGBB format.");
        }
        if !self
            .spacing
            .parse::<i64>()
            .is_ok_and(|s| (8..=200).contains(&s))
        {
            return Some("Spacing must be a whole number from 8 to 200.");
        }
        None
    }

    pub(crate) fn preview(&self) -> (Color, BoardGrid) {
        let base = match self.original.background {
            BoardBackground::Solid(color) => color,
            _ => crate::draw::WHITE,
        };
        (
            if self.color_dirty {
                parse_hex_color(&self.color).unwrap_or(base)
            } else {
                base
            },
            BoardGrid::new(
                self.kind,
                self.spacing
                    .parse()
                    .unwrap_or(i64::from(self.original.grid.spacing())),
            ),
        )
    }

    fn desired(&self, current: &BoardAppearance) -> Result<BoardAppearance, &'static str> {
        let color = parse_hex_color(&self.color).ok_or("Use a color in #RRGGBB format.")?;
        let spacing = self
            .spacing
            .parse::<i64>()
            .ok()
            .filter(|s| (8..=200).contains(s))
            .ok_or("Spacing must be a whole number from 8 to 200.")?;
        let mut desired = current.clone();
        fn conflict<T: PartialEq>(dirty: bool, current: &T, original: &T, desired: &T) -> bool {
            dirty && current != original && current != desired
        }
        if conflict(
            self.color_dirty,
            &current.background,
            &self.original.background,
            &BoardBackground::Solid(color),
        ) || conflict(
            self.kind_dirty,
            &current.grid.kind,
            &self.original.grid.kind,
            &self.kind,
        ) || conflict(
            self.spacing_dirty,
            &current.grid.spacing(),
            &self.original.grid.spacing(),
            &(spacing as u16),
        ) {
            return Err("Board appearance changed. Cancel and reopen to edit it.");
        }
        if self.color_dirty {
            desired.background = BoardBackground::Solid(color);
        }
        desired.grid = BoardGrid::new(
            if self.kind_dirty {
                self.kind
            } else {
                current.grid.kind
            },
            if self.spacing_dirty {
                spacing
            } else {
                i64::from(current.grid.spacing())
            },
        );
        Ok(desired)
    }
}

impl InputState {
    pub(crate) fn board_appearance_edit(&self) -> Option<&BoardAppearanceEdit> {
        self.board_picker.appearance.as_ref()
    }

    pub(crate) fn begin_board_appearance(&mut self, index: usize) -> bool {
        let Some(board) = self.boards.board_states().get(index) else {
            return false;
        };
        let BoardBackground::Solid(color) = board.spec.background else {
            return false;
        };
        self.board_picker.appearance = Some(BoardAppearanceEdit {
            id: board.spec.id.clone(),
            generation: self.boards.board_identity_generation(),
            original: BoardAppearance::from_spec(&board.spec),
            color: color_to_hex(color),
            kind: board.spec.grid.kind,
            spacing: board.spec.grid.spacing().to_string(),
            color_dirty: false,
            kind_dirty: false,
            spacing_dirty: false,
            focus: AppearanceField::Color,
            error: None,
        });
        true
    }

    pub(crate) fn apply_board_appearance(&mut self) -> bool {
        let Some(edit) = &self.board_picker.appearance else {
            return false;
        };
        let index = self
            .boards
            .board_states()
            .iter()
            .position(|b| b.spec.id == edit.id);
        if self.boards.board_identity_generation() != edit.generation || index.is_none() {
            self.board_picker.appearance.as_mut().unwrap().error =
                Some("Board identity changed. Cancel and reopen to edit it.".into());
            self.needs_redraw = true;
            return false;
        }
        let index = index.unwrap();
        let current = BoardAppearance::from_spec(&self.boards.board_states()[index].spec);
        let proposed = if current.background.is_transparent() {
            Err("Paper patterns require a solid board.")
        } else {
            edit.desired(&current)
        };
        let desired = match proposed {
            Ok(desired) => desired,
            Err(error) => {
                self.board_picker.appearance.as_mut().unwrap().error = Some(error.into());
                self.needs_redraw = true;
                return false;
            }
        };
        self.apply_board_appearance_value(index, desired);
        self.board_picker_clear_edit();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn board_appearance_palette(&mut self, color: Color) -> bool {
        let Some(edit) = &mut self.board_picker.appearance else {
            return false;
        };
        edit.color = color_to_hex(color);
        edit.color_dirty = true;
        edit.error = None;
        self.needs_redraw = true;
        true
    }

    pub(crate) fn board_appearance_key(&mut self, key: Key) -> bool {
        let Some(edit) = &mut self.board_picker.appearance else {
            return false;
        };
        match key {
            Key::Escape => self.board_picker_cancel_edit(),
            Key::Return => {
                self.apply_board_appearance();
            }
            Key::F2 => return false,
            Key::Tab => {
                edit.focus = match edit.focus {
                    AppearanceField::Color => AppearanceField::Pattern,
                    AppearanceField::Pattern => AppearanceField::Spacing,
                    AppearanceField::Spacing => AppearanceField::Color,
                };
            }
            Key::Left | Key::Right | Key::Up | Key::Down
                if edit.focus == AppearanceField::Pattern =>
            {
                let index = BoardGridKind::ALL
                    .iter()
                    .position(|kind| *kind == edit.kind)
                    .unwrap_or(0);
                let step = if matches!(key, Key::Left | Key::Up) {
                    3
                } else {
                    1
                };
                edit.kind = BoardGridKind::ALL[(index + step) % 4];
                edit.kind_dirty = true;
            }
            Key::Backspace | Key::Delete => match edit.focus {
                AppearanceField::Color => {
                    edit.color.pop();
                    edit.color_dirty = true;
                }
                AppearanceField::Spacing => {
                    edit.spacing.pop();
                    edit.spacing_dirty = true;
                }
                _ => {}
            },
            Key::Char(ch) => match edit.focus {
                AppearanceField::Color
                    if (ch.is_ascii_hexdigit() || ch == '#') && edit.color.len() < 7 =>
                {
                    edit.color.push(ch);
                    edit.color_dirty = true;
                }
                AppearanceField::Spacing if !ch.is_control() && edit.spacing.len() < 8 => {
                    edit.spacing.push(ch);
                    edit.spacing_dirty = true;
                }
                _ => {}
            },
            _ => {}
        }
        self.needs_redraw = true;
        true
    }

    /// Shared sheet geometry for painting and mouse input, centered on the picker.
    pub(crate) fn board_appearance_rect(&self) -> Option<(f64, f64, f64)> {
        self.board_picker.appearance.as_ref()?;
        let layout = self.board_picker.layout.as_ref()?;
        let width = (layout.width - 32.0).clamp(140.0, 320.0);
        Some((
            layout.origin_x + (layout.width - width) / 2.0,
            layout.origin_y + layout.height / 2.0 - 75.0,
            width,
        ))
    }

    pub(crate) fn board_appearance_click(&mut self, x: i32, y: i32) -> bool {
        let Some((left, top, width)) = self.board_appearance_rect() else {
            return false;
        };
        let x = f64::from(x) - left;
        let y = f64::from(y) - top;
        if !(-12.0..width + 12.0).contains(&x) || !(-70.0..222.0).contains(&y) {
            self.board_picker_cancel_edit();
            return false;
        }
        let edit = self.board_picker.appearance.as_mut().unwrap();
        match y as i32 {
            -65..=-35 => edit.focus = AppearanceField::Color,
            -30..=-5 => {
                let index = (x / (width / 11.0)).floor() as usize;
                if let Some(color) = super::board_palette_colors().get(index) {
                    edit.color = color_to_hex(*color);
                    edit.color_dirty = true;
                }
            }
            0..=55 => {
                edit.kind = BoardGridKind::ALL
                    [((y / 28.0) as usize * 2 + (x / (width / 2.0)) as usize).min(3)];
                edit.kind_dirty = true;
                edit.focus = AppearanceField::Pattern;
            }
            60..=87 => {
                edit.focus = AppearanceField::Spacing;
                if x > width - 40.0 {
                    edit.spacing = "40".into();
                    edit.spacing_dirty = true;
                } else if x > width - 80.0 {
                    edit.spacing = "20".into();
                    edit.spacing_dirty = true;
                }
            }
            160..=187 => {
                if x < width / 2.0 {
                    self.apply_board_appearance();
                } else {
                    self.board_picker_cancel_edit();
                }
            }
            _ => {}
        }
        self.needs_redraw = true;
        true
    }
}

#[cfg(test)]
mod tests;
