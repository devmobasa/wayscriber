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

const SHEET_PADDING: f64 = 12.0;
const SHEET_COLUMN_MARGIN: f64 = 8.0;
const SHEET_MIN_DOCKED_WIDTH: f64 = 240.0;
const SHEET_CLOSE_SIZE: f64 = 24.0;
const SHEET_COLOR_FIELD_WIDTH: f64 = 84.0;
const SHEET_HEADER_GAP: f64 = 8.0;

/// The paper sheet's header controls as `(x, y, width, height)` rectangles.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BoardAppearanceHeader {
    /// The title is clipped before this x coordinate.
    pub(crate) title_right: f64,
    pub(crate) color_field: (f64, f64, f64, f64),
    pub(crate) close: (f64, f64, f64, f64),
}

fn rect_contains((x, y, width, height): (f64, f64, f64, f64), (px, py): (f64, f64)) -> bool {
    px >= x && px <= x + width && py >= y && py <= y + height
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
    pub(crate) focus: AppearanceField,
    pub(crate) error: Option<String>,
}

impl BoardAppearanceEdit {
    pub(super) fn set_color_text(&mut self, value: String) {
        self.color = value;
    }

    pub(crate) fn board_id(&self) -> &str {
        &self.id
    }
    // Compare the displayed color with its original displayed value, preserving
    // the exact configured float color when an edit is reverted.
    fn color_is_dirty(&self) -> bool {
        let BoardBackground::Solid(original) = self.original.background else {
            return false;
        };
        parse_hex_color(&self.color) != parse_hex_color(&color_to_hex(original))
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

    /// Whether the typed size equals a preset, so only that preset reads as selected.
    pub(crate) fn spacing_matches(&self, preset: u16) -> bool {
        self.spacing.trim().parse::<u16>().ok() == Some(preset)
    }

    pub(crate) fn preview(&self) -> (Color, BoardGrid) {
        let base = match self.original.background {
            BoardBackground::Solid(color) => color,
            _ => crate::draw::WHITE,
        };
        (
            if self.color_is_dirty() {
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
        let kind_dirty = self.kind != self.original.grid.kind;
        let spacing_dirty = spacing != i64::from(self.original.grid.spacing());
        let mut desired = current.clone();
        fn conflict<T: PartialEq>(dirty: bool, current: &T, original: &T, desired: &T) -> bool {
            dirty && current != original && current != desired
        }
        if conflict(
            self.color_is_dirty(),
            &current.background,
            &self.original.background,
            &BoardBackground::Solid(color),
        ) || conflict(
            kind_dirty,
            &current.grid.kind,
            &self.original.grid.kind,
            &self.kind,
        ) || conflict(
            spacing_dirty,
            &current.grid.spacing(),
            &self.original.grid.spacing(),
            &(spacing as u16),
        ) {
            return Err("Board appearance changed. Cancel and reopen to edit it.");
        }
        if self.color_is_dirty() {
            desired.background = BoardBackground::Solid(color);
        }
        desired.grid = BoardGrid::new(
            if kind_dirty {
                self.kind
            } else {
                current.grid.kind
            },
            if spacing_dirty {
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
            focus: AppearanceField::Color,
            error: None,
        });
        // The sheet dims the whole surface behind it.
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn apply_board_appearance(&mut self) -> bool {
        self.mark_board_appearance_region();
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
        self.mark_board_appearance_region();
        let Some(edit) = &mut self.board_picker.appearance else {
            return false;
        };
        edit.color = color_to_hex(color);
        edit.error = None;
        self.needs_redraw = true;
        true
    }

    pub(crate) fn board_appearance_key(&mut self, key: Key) -> bool {
        self.mark_board_appearance_region();
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
            }
            Key::Backspace | Key::Delete => match edit.focus {
                AppearanceField::Color => {
                    edit.color.pop();
                }
                AppearanceField::Spacing => {
                    edit.spacing.pop();
                }
                _ => {}
            },
            Key::Char(ch) => match edit.focus {
                AppearanceField::Color
                    if (ch.is_ascii_hexdigit() || ch == '#') && edit.color.len() < 7 =>
                {
                    edit.color.push(ch);
                }
                AppearanceField::Spacing if !ch.is_control() && edit.spacing.len() < 8 => {
                    edit.spacing.push(ch);
                }
                _ => {}
            },
            _ => {}
        }
        self.needs_redraw = true;
        true
    }

    /// Shared sheet geometry for painting and mouse input. The sheet floats over
    /// the page column when it fits there, so it leaves the board list readable;
    /// narrow pickers center it instead.
    pub(crate) fn board_appearance_rect(&self) -> Option<(f64, f64, f64)> {
        self.board_picker.appearance.as_ref()?;
        let layout = self.board_picker.layout.as_ref()?;
        let y = layout.origin_y + layout.height / 2.0 - 75.0;

        // Sheet padding plus a small margin on both sides of the page column.
        let column_width = layout.origin_x + layout.width - layout.page_panel_x;
        let docked_width = (column_width - 2.0 * (SHEET_PADDING + SHEET_COLUMN_MARGIN)).min(320.0);
        // A picker wider than the output is clipped at both edges, so keep the
        // sheet centered there to leave it fully on screen.
        if layout.page_panel_enabled
            && layout.origin_x >= 0.0
            && docked_width >= SHEET_MIN_DOCKED_WIDTH
        {
            let x = layout.page_panel_x + (column_width - docked_width) / 2.0;
            return Some((x, y, docked_width));
        }

        let width = (layout.width - 32.0).clamp(140.0, 320.0);
        Some((layout.origin_x + (layout.width - width) / 2.0, y, width))
    }

    /// Header controls in surface coordinates, shared by painting and clicks.
    pub(crate) fn board_appearance_header(&self) -> Option<BoardAppearanceHeader> {
        let (x, y, width) = self.board_appearance_rect()?;
        let close = (
            x + width + 4.0 - SHEET_CLOSE_SIZE,
            y - 62.0,
            SHEET_CLOSE_SIZE,
            SHEET_CLOSE_SIZE,
        );
        let field_x = close.0 - SHEET_HEADER_GAP - SHEET_COLOR_FIELD_WIDTH;
        Some(BoardAppearanceHeader {
            title_right: field_x - SHEET_HEADER_GAP,
            color_field: (field_x, y - 61.0, SHEET_COLOR_FIELD_WIDTH, 22.0),
            close,
        })
    }

    pub(in crate::input::state) fn mark_board_appearance_region(&mut self) {
        if self.board_picker.appearance.is_none() {
            return;
        }
        // The outer palette also highlights the draft color.
        if let Some(layout) = self.board_picker.layout
            && layout.palette_rows > 0
        {
            self.mark_board_picker_region(&layout);
        }
        // The frame plus its drop shadow below.
        if let Some((x, y, width)) = self.board_appearance_rect()
            && let Some(rect) = crate::util::Rect::new(
                (x - 15.0).floor() as i32,
                (y - 72.0).floor() as i32,
                width.ceil() as i32 + 31,
                308,
            )
        {
            self.dirty_tracker.mark_rect(rect);
        } else {
            self.dirty_tracker.mark_full();
        }
        self.needs_redraw = true;
    }

    pub(crate) fn board_appearance_click(&mut self, x: i32, y: i32) -> bool {
        let Some((left, top, width)) = self.board_appearance_rect() else {
            return false;
        };
        self.mark_board_appearance_region();
        let (screen_x, screen_y) = (x, y);
        let x = f64::from(x) - left;
        let y = f64::from(y) - top;
        if !(-12.0..width + 12.0).contains(&x) || !(-70.0..222.0).contains(&y) {
            if let Some(color) = self.board_picker_palette_color_at(screen_x, screen_y) {
                return self.board_appearance_palette(color);
            }
            self.board_picker_cancel_edit();
            return false;
        }
        let header = self.board_appearance_header();
        let point = (f64::from(screen_x), f64::from(screen_y));
        if header.is_some_and(|header| rect_contains(header.close, point)) {
            self.board_picker_cancel_edit();
            return true;
        }
        let in_color_field = header.is_some_and(|header| rect_contains(header.color_field, point));
        let edit = self.board_picker.appearance.as_mut().unwrap();
        match y as i32 {
            -65..=-35 => {
                if in_color_field {
                    edit.focus = AppearanceField::Color;
                }
            }
            -30..=-5 => {
                let index = (x / (width / 11.0)).floor() as usize;
                if let Some(color) = super::board_palette_colors().get(index) {
                    edit.color = color_to_hex(*color);
                }
            }
            0..=55 => {
                edit.kind = BoardGridKind::ALL
                    [((y / 28.0) as usize * 2 + (x / (width / 2.0)) as usize).min(3)];
                edit.focus = AppearanceField::Pattern;
            }
            60..=87 => {
                edit.focus = AppearanceField::Spacing;
                if x > width - 40.0 {
                    edit.spacing = "40".into();
                } else if x > width - 80.0 {
                    edit.spacing = "20".into();
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
