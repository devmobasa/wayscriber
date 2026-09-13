use super::{color_to_hex, parse_hex_color};
use crate::domain::{
    BOARD_GRID_MAX_SPACING, BOARD_GRID_MIN_SPACING, BoardBackground, BoardGrid, BoardGridKind,
    Color,
};
use crate::input::InputState;
use crate::input::boards::{BoardAppearance, BoardIdentityGeneration};
use crate::input::events::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppearanceField {
    Color,
    Pattern,
    Spacing,
}

/// Content width and the frame around the content origin, in sheet units.
const SHEET_WIDTH: f64 = 320.0;
const SHEET_MIN_WIDTH: f64 = 140.0;
const SHEET_PADDING: f64 = 12.0;
const SHEET_TOP: f64 = 70.0;
const SHEET_HEIGHT: f64 = 292.0;
const SHEET_SHADOW: f64 = 10.0;
/// The sheet is magnified up to this scale while it fits on the output.
const SHEET_MAX_SCALE: f64 = 2.0;
const SHEET_SCREEN_MARGIN: f64 = 24.0;
const SHEET_CLOSE_SIZE: f64 = 24.0;
const SHEET_COLOR_FIELD_WIDTH: f64 = 84.0;
const SHEET_HEADER_GAP: f64 = 8.0;
const SIZE_ROW_TOP: f64 = 60.0;
const SIZE_ROW_HEIGHT: f64 = 25.0;
const SIZE_LABEL_WIDTH: f64 = 92.0;
const SIZE_FIELD_WIDTH: f64 = 64.0;
const SIZE_CONTROL_GAP: f64 = 10.0;
const SLIDER_MIN_WIDTH: f64 = 60.0;
const SLIDER_THUMB_RADIUS: f64 = 7.0;
const BUTTON_TOP: f64 = 158.0;
const BUTTON_HEIGHT: f64 = 30.0;
const BUTTON_GAP: f64 = 8.0;

/// Where the paper sheet sits on the surface. Its controls are laid out in
/// sheet units from the content origin and magnified by `scale`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BoardAppearanceFrame {
    /// Content origin on the surface.
    pub(crate) x: f64,
    pub(crate) y: f64,
    /// Content width in sheet units.
    pub(crate) width: f64,
    pub(crate) scale: f64,
}

impl BoardAppearanceFrame {
    /// The frame around the content, in sheet units.
    pub(crate) fn outline(self) -> (f64, f64, f64, f64) {
        (
            -SHEET_PADDING,
            -SHEET_TOP,
            self.width + 2.0 * SHEET_PADDING,
            SHEET_HEIGHT,
        )
    }

    /// The frame on the surface, without its shadow.
    pub(crate) fn bounds(self) -> (f64, f64, f64, f64) {
        self.to_surface(self.outline())
    }

    pub(crate) fn to_surface(
        self,
        (x, y, width, height): (f64, f64, f64, f64),
    ) -> (f64, f64, f64, f64) {
        (
            self.x + x * self.scale,
            self.y + y * self.scale,
            width * self.scale,
            height * self.scale,
        )
    }

    /// A pointer position in sheet units.
    fn to_sheet(self, x: i32, y: i32) -> (f64, f64) {
        (
            (f64::from(x) - self.x) / self.scale,
            (f64::from(y) - self.y) / self.scale,
        )
    }
}

/// The paper sheet's header controls as `(x, y, width, height)` rectangles in
/// sheet units.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BoardAppearanceHeader {
    /// The title is clipped before this x coordinate.
    pub(crate) title_right: f64,
    pub(crate) color_field: (f64, f64, f64, f64),
    pub(crate) close: (f64, f64, f64, f64),
}

/// Size controls in sheet units, shared by painting and pointer input.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BoardAppearanceSizeRow {
    /// The label is clipped before this x coordinate.
    pub(crate) label_right: f64,
    /// Slider hit area; zero width when the sheet is too narrow for a slider.
    pub(crate) track: (f64, f64, f64, f64),
    /// Painted rail as `(start_x, end_x, center_y)`.
    pub(crate) rail: (f64, f64, f64),
    pub(crate) thumb_x: f64,
    pub(crate) thumb_radius: f64,
    pub(crate) field: (f64, f64, f64, f64),
}

/// Dialog buttons, with Cancel before the affirmative Apply.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BoardAppearanceButtons {
    pub(crate) cancel: (f64, f64, f64, f64),
    pub(crate) apply: (f64, f64, f64, f64),
}

fn rect_contains((x, y, width, height): (f64, f64, f64, f64), (px, py): (f64, f64)) -> bool {
    px >= x && px <= x + width && py >= y && py <= y + height
}

/// Start of a span of `length` kept within `low..high`, centered there when it
/// cannot fit.
fn fit_span(start: f64, length: f64, (low, high): (f64, f64)) -> f64 {
    if high - low < length {
        low + (high - low - length) / 2.0
    } else {
        start.clamp(low, high - length)
    }
}

/// Slider position in `0..=1` for a size. Sizes use a logarithmic scale so
/// small grids get as much travel as large ones.
fn size_to_slider(size: u16) -> f64 {
    let min = f64::from(BOARD_GRID_MIN_SPACING);
    let max = f64::from(BOARD_GRID_MAX_SPACING);
    (f64::from(size).clamp(min, max) / min).ln() / (max / min).ln()
}

fn slider_to_size(position: f64) -> u16 {
    let min = f64::from(BOARD_GRID_MIN_SPACING);
    let max = f64::from(BOARD_GRID_MAX_SPACING);
    (min * (max / min).powf(position.clamp(0.0, 1.0))).round() as u16
}

fn slider_size_at(track: (f64, f64, f64, f64), x: f64) -> u16 {
    let travel = (track.2 - 2.0 * SLIDER_THUMB_RADIUS).max(1.0);
    slider_to_size((x - track.0 - SLIDER_THUMB_RADIUS) / travel)
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
    /// The size text is selected, so the next digit replaces it.
    pub(crate) spacing_armed: bool,
    pub(crate) size_dragging: bool,
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
        if !self.size_is_valid() {
            return Some("Spacing must be a whole number from 8 to 200.");
        }
        None
    }

    fn spacing_value(&self) -> Option<u16> {
        self.spacing
            .parse::<u16>()
            .ok()
            .filter(|spacing| (BOARD_GRID_MIN_SPACING..=BOARD_GRID_MAX_SPACING).contains(spacing))
    }

    pub(crate) fn size_is_valid(&self) -> bool {
        self.spacing_value().is_some()
    }

    /// The size the slider and preview show: the typed value when valid,
    /// otherwise the original size while the field is mid-edit.
    pub(crate) fn size_value(&self) -> u16 {
        self.spacing_value().unwrap_or(self.original.grid.spacing())
    }

    fn set_size(&mut self, size: u16) {
        self.spacing = size
            .clamp(BOARD_GRID_MIN_SPACING, BOARD_GRID_MAX_SPACING)
            .to_string();
        self.error = None;
    }

    fn step_size(&mut self, delta: i32) {
        let next = (i32::from(self.size_value()) + delta).clamp(
            i32::from(BOARD_GRID_MIN_SPACING),
            i32::from(BOARD_GRID_MAX_SPACING),
        );
        self.set_size(next as u16);
        self.spacing_armed = true;
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
            BoardGrid::new(self.kind, i64::from(self.size_value())),
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
            spacing_armed: false,
            size_dragging: false,
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
        let shift = self.modifiers.shift;
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
                edit.spacing_armed = edit.focus == AppearanceField::Spacing;
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
            Key::Left | Key::Right | Key::Up | Key::Down
                if edit.focus == AppearanceField::Spacing =>
            {
                let step = if shift { 10 } else { 1 };
                edit.step_size(if matches!(key, Key::Right | Key::Up) {
                    step
                } else {
                    -step
                });
            }
            Key::Backspace | Key::Delete => match edit.focus {
                AppearanceField::Color => {
                    edit.color.pop();
                }
                AppearanceField::Spacing => {
                    if edit.spacing_armed {
                        edit.spacing.clear();
                        edit.spacing_armed = false;
                    } else {
                        edit.spacing.pop();
                    }
                }
                _ => {}
            },
            Key::Char(ch) => match edit.focus {
                AppearanceField::Color
                    if (ch.is_ascii_hexdigit() || ch == '#') && edit.color.len() < 7 =>
                {
                    edit.color.push(ch);
                }
                AppearanceField::Spacing if ch.is_ascii_digit() => {
                    if edit.spacing_armed {
                        edit.spacing.clear();
                        edit.spacing_armed = false;
                    }
                    if edit.spacing.len() < 3 {
                        edit.spacing.push(ch);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        self.needs_redraw = true;
        true
    }

    /// Where the sheet sits and how much it is magnified, shared by painting and
    /// pointer input. It is twice its base size when that fits on the output,
    /// stepping down to base size on small outputs. It leans toward the page
    /// column so the start of the board list stays visible.
    pub(crate) fn board_appearance_frame(&self) -> Option<BoardAppearanceFrame> {
        self.board_picker.appearance.as_ref()?;
        let layout = self.board_picker.layout.as_ref()?;
        let room_width = layout.screen_width - 2.0 * SHEET_SCREEN_MARGIN;
        let room_height = layout.screen_height - 2.0 * SHEET_SCREEN_MARGIN;
        let fit = (room_width / (SHEET_WIDTH + 2.0 * SHEET_PADDING))
            .min(room_height / (SHEET_HEIGHT + SHEET_SHADOW));
        // Quarter steps keep strokes close to whole pixels.
        let scale = ((fit * 4.0).floor() / 4.0).clamp(1.0, SHEET_MAX_SCALE);
        // Outputs too narrow even at base size narrow the content instead.
        let width = (room_width / scale - 2.0 * SHEET_PADDING).clamp(SHEET_MIN_WIDTH, SHEET_WIDTH);
        let frame_width = (width + 2.0 * SHEET_PADDING) * scale;
        let frame_height = SHEET_HEIGHT * scale;

        let picker_right = layout.origin_x + layout.width;
        let center_x = if layout.page_panel_enabled {
            (layout.page_panel_x + picker_right) / 2.0
        } else {
            layout.origin_x + layout.width / 2.0
        };
        let left = fit_span(
            center_x - frame_width / 2.0,
            frame_width,
            (layout.origin_x, picker_right),
        );
        let left = fit_span(
            left,
            frame_width,
            (
                SHEET_SCREEN_MARGIN,
                layout.screen_width - SHEET_SCREEN_MARGIN,
            ),
        );
        let top = fit_span(
            layout.origin_y + (layout.height - frame_height) / 2.0,
            frame_height,
            (
                SHEET_SCREEN_MARGIN,
                layout.screen_height - SHEET_SCREEN_MARGIN,
            ),
        );
        Some(BoardAppearanceFrame {
            x: left.round() + SHEET_PADDING * scale,
            y: top.round() + SHEET_TOP * scale,
            width,
            scale,
        })
    }

    /// Header controls in sheet units, shared by painting and clicks.
    pub(crate) fn board_appearance_header(&self) -> Option<BoardAppearanceHeader> {
        let width = self.board_appearance_frame()?.width;
        let close = (
            width + 4.0 - SHEET_CLOSE_SIZE,
            -62.0,
            SHEET_CLOSE_SIZE,
            SHEET_CLOSE_SIZE,
        );
        let field_x = close.0 - SHEET_HEADER_GAP - SHEET_COLOR_FIELD_WIDTH;
        Some(BoardAppearanceHeader {
            title_right: field_x - SHEET_HEADER_GAP,
            color_field: (field_x, -61.0, SHEET_COLOR_FIELD_WIDTH, 22.0),
            close,
        })
    }

    /// Size label, slider, and field, placed from the draft's current size.
    pub(crate) fn board_appearance_size_row(&self) -> Option<BoardAppearanceSizeRow> {
        let edit = self.board_picker.appearance.as_ref()?;
        let width = self.board_appearance_frame()?.width;
        let top = SIZE_ROW_TOP;
        let field = (
            width - SIZE_FIELD_WIDTH,
            top,
            SIZE_FIELD_WIDTH,
            SIZE_ROW_HEIGHT,
        );

        let track_x = SIZE_LABEL_WIDTH;
        let track_width = field.0 - SIZE_CONTROL_GAP - track_x;
        let (track, label_right) = if track_width >= SLIDER_MIN_WIDTH {
            ((track_x, top, track_width, SIZE_ROW_HEIGHT), track_x - 6.0)
        } else {
            (
                (track_x, top, 0.0, SIZE_ROW_HEIGHT),
                field.0 - SIZE_CONTROL_GAP,
            )
        };
        let rail_start = track.0 + SLIDER_THUMB_RADIUS;
        let rail_end = (track.0 + track.2 - SLIDER_THUMB_RADIUS).max(rail_start);

        Some(BoardAppearanceSizeRow {
            label_right,
            track,
            rail: (rail_start, rail_end, top + SIZE_ROW_HEIGHT / 2.0),
            thumb_x: rail_start + size_to_slider(edit.size_value()) * (rail_end - rail_start),
            thumb_radius: SLIDER_THUMB_RADIUS,
            field,
        })
    }

    pub(crate) fn board_appearance_buttons(&self) -> Option<BoardAppearanceButtons> {
        let width = self.board_appearance_frame()?.width;
        let button_width = (width - BUTTON_GAP) / 2.0;
        Some(BoardAppearanceButtons {
            cancel: (0.0, BUTTON_TOP, button_width, BUTTON_HEIGHT),
            apply: (
                button_width + BUTTON_GAP,
                BUTTON_TOP,
                button_width,
                BUTTON_HEIGHT,
            ),
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
        // The frame, its drop shadow below, and antialiasing on every side.
        let damage = self.board_appearance_frame().and_then(|frame| {
            let (left, top, width, height) = frame.bounds();
            let bottom = top + height + SHEET_SHADOW * frame.scale;
            let (x, y) = ((left - 4.0).floor(), (top - 4.0).floor());
            crate::util::Rect::new(
                x as i32,
                y as i32,
                ((left + width + 4.0).ceil() - x) as i32,
                ((bottom + 4.0).ceil() - y) as i32,
            )
        });
        if let Some(rect) = damage {
            self.dirty_tracker.mark_rect(rect);
        } else {
            self.dirty_tracker.mark_full();
        }
        self.needs_redraw = true;
    }

    /// A press on the size slider jumps the thumb there and starts a drag.
    pub(crate) fn board_appearance_press(&mut self, x: i32, y: i32) -> bool {
        let (Some(frame), Some(row)) = (
            self.board_appearance_frame(),
            self.board_appearance_size_row(),
        ) else {
            return false;
        };
        let point = frame.to_sheet(x, y);
        if row.track.2 <= 0.0 || !rect_contains(row.track, point) {
            return false;
        }

        let size = slider_size_at(row.track, point.0);
        if let Some(edit) = self.board_picker.appearance.as_mut() {
            edit.set_size(size);
            edit.focus = AppearanceField::Spacing;
            edit.spacing_armed = true;
            edit.size_dragging = true;
        }
        self.mark_board_appearance_region();
        true
    }

    pub(crate) fn board_appearance_drag_to(&mut self, x: i32, y: i32) -> bool {
        let dragging = self
            .board_picker
            .appearance
            .as_ref()
            .is_some_and(|edit| edit.size_dragging);
        if !dragging {
            return false;
        }
        let (Some(frame), Some(row)) = (
            self.board_appearance_frame(),
            self.board_appearance_size_row(),
        ) else {
            return false;
        };

        if let Some(edit) = self.board_picker.appearance.as_mut() {
            edit.set_size(slider_size_at(row.track, frame.to_sheet(x, y).0));
        }
        self.mark_board_appearance_region();
        true
    }

    /// The wheel steps the size over its row. While the sheet is open it also
    /// consumes wheel events over the picker so nothing behind it scrolls.
    pub(crate) fn board_appearance_wheel(&mut self, x: i32, y: i32, direction: i32) -> bool {
        let Some(frame) = self.board_appearance_frame() else {
            return false;
        };
        let point = frame.to_sheet(x, y);
        if !rect_contains(frame.outline(), point) && !self.board_picker_contains_point(x, y) {
            return false;
        }

        let size_row = (0.0, SIZE_ROW_TOP, frame.width, SIZE_ROW_HEIGHT);
        if direction != 0 && rect_contains(size_row, point) {
            let step = if self.modifiers.shift { 10 } else { 1 };
            if let Some(edit) = self.board_picker.appearance.as_mut() {
                edit.step_size(if direction > 0 { -step } else { step });
                edit.focus = AppearanceField::Spacing;
            }
            self.mark_board_appearance_region();
        }
        true
    }

    pub(crate) fn board_appearance_click(&mut self, x: i32, y: i32) -> bool {
        // A slider drag ends on release wherever the pointer is.
        if let Some(edit) = self.board_picker.appearance.as_mut()
            && edit.size_dragging
        {
            edit.size_dragging = false;
            self.mark_board_appearance_region();
            return true;
        }
        let Some(frame) = self.board_appearance_frame() else {
            return false;
        };
        self.mark_board_appearance_region();
        let point = frame.to_sheet(x, y);
        if !rect_contains(frame.outline(), point) {
            if let Some(color) = self.board_picker_palette_color_at(x, y) {
                return self.board_appearance_palette(color);
            }
            self.board_picker_cancel_edit();
            return false;
        }

        let header = self.board_appearance_header();
        let buttons = self.board_appearance_buttons();
        if header.is_some_and(|header| rect_contains(header.close, point))
            || buttons.is_some_and(|buttons| rect_contains(buttons.cancel, point))
        {
            self.board_picker_cancel_edit();
            return true;
        }
        if buttons.is_some_and(|buttons| rect_contains(buttons.apply, point)) {
            self.apply_board_appearance();
            return true;
        }

        let in_color_field = header.is_some_and(|header| rect_contains(header.color_field, point));
        let ((x, y), width) = (point, frame.width);
        let edit = self.board_picker.appearance.as_mut().unwrap();
        match y.floor() as i32 {
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
            // Slider presses start drags in `board_appearance_press`; the rest
            // of the row selects the size field for typing.
            60..=87 => {
                edit.focus = AppearanceField::Spacing;
                edit.spacing_armed = true;
            }
            _ => {}
        }
        self.needs_redraw = true;
        true
    }
}

#[cfg(test)]
mod tests;
