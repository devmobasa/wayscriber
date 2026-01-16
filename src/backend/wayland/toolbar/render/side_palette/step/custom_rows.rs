use crate::backend::wayland::toolbar::events::{HitKind, delay_secs_from_t, delay_t_from_ms};
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};
use crate::ui_text::UiTextStyle;

use super::super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
use super::super::super::widgets::*;

pub(super) fn draw_custom_rows(
    ctx: &cairo::Context,
    hits: &mut Vec<HitRegion>,
    x: f64,
    y: f64,
    card_w: f64,
    snapshot: &ToolbarSnapshot,
    hover: Option<(f64, f64)>,
) {
    let label_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_LABEL,
    };
    let mut context = CustomRowContext {
        ctx,
        hits,
        x,
        card_w,
        snapshot,
        hover,
        label_style,
    };
    let undo_row_h = context.draw_row(y, true);
    let redo_y = y + undo_row_h + 8.0;
    context.draw_row(redo_y, false);
}

struct CustomRowContext<'a> {
    ctx: &'a cairo::Context,
    hits: &'a mut Vec<HitRegion>,
    x: f64,
    card_w: f64,
    snapshot: &'a ToolbarSnapshot,
    hover: Option<(f64, f64)>,
    label_style: UiTextStyle<'a>,
}

impl<'a> CustomRowContext<'a> {
    fn draw_row(&mut self, y: f64, is_undo: bool) -> f64 {
        let row_h = 26.0;
        let btn_w = if self.snapshot.use_icons { 42.0 } else { 90.0 };
        let steps_btn_w = 26.0;
        let gap = 6.0;
        let label = if is_undo { "Step Undo" } else { "Step Redo" };
        let steps = if is_undo {
            self.snapshot.custom_undo_steps
        } else {
            self.snapshot.custom_redo_steps
        };
        let delay_ms = if is_undo {
            self.snapshot.custom_undo_delay_ms
        } else {
            self.snapshot.custom_redo_delay_ms
        };

        let btn_hover = self
            .hover
            .map(|(hx, hy)| point_in_rect(hx, hy, self.x, y, btn_w, row_h))
            .unwrap_or(false);

        if self.snapshot.use_icons {
            let icon_size = 20.0;
            draw_button(self.ctx, self.x, y, btn_w, row_h, false, btn_hover);
            set_icon_color(self.ctx, btn_hover);
            if is_undo {
                toolbar_icons::draw_icon_step_undo(
                    self.ctx,
                    self.x + (btn_w - icon_size) / 2.0,
                    y + (row_h - icon_size) / 2.0,
                    icon_size,
                );
            } else {
                toolbar_icons::draw_icon_step_redo(
                    self.ctx,
                    self.x + (btn_w - icon_size) / 2.0,
                    y + (row_h - icon_size) / 2.0,
                    icon_size,
                );
            }
        } else {
            draw_button(self.ctx, self.x, y, btn_w, row_h, false, btn_hover);
            draw_label_left(
                self.ctx,
                self.label_style,
                self.x + 10.0,
                y,
                btn_w - 20.0,
                row_h,
                label,
            );
        }
        self.hits.push(HitRegion {
            rect: (self.x, y, btn_w, row_h),
            event: if is_undo {
                ToolbarEvent::CustomUndo
            } else {
                ToolbarEvent::CustomRedo
            },
            kind: HitKind::Click,
            tooltip: Some(if is_undo {
                "Step undo".to_string()
            } else {
                "Step redo".to_string()
            }),
        });

        let steps_x = self.x + btn_w + gap;
        let minus_hover = self
            .hover
            .map(|(hx, hy)| point_in_rect(hx, hy, steps_x, y, steps_btn_w, row_h))
            .unwrap_or(false);
        draw_button(self.ctx, steps_x, y, steps_btn_w, row_h, false, minus_hover);
        set_icon_color(self.ctx, minus_hover);
        toolbar_icons::draw_icon_minus(
            self.ctx,
            steps_x + (steps_btn_w - ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE) / 2.0,
            y + (row_h - ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE) / 2.0,
            ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE,
        );
        self.hits.push(HitRegion {
            rect: (steps_x, y, steps_btn_w, row_h),
            event: if is_undo {
                ToolbarEvent::SetCustomUndoSteps(steps.saturating_sub(1).max(1))
            } else {
                ToolbarEvent::SetCustomRedoSteps(steps.saturating_sub(1).max(1))
            },
            kind: HitKind::Click,
            tooltip: Some(if is_undo {
                "Decrease undo steps".to_string()
            } else {
                "Decrease redo steps".to_string()
            }),
        });

        let steps_val_x = steps_x + steps_btn_w + 4.0;
        draw_label_center(
            self.ctx,
            self.label_style,
            steps_val_x,
            y,
            54.0,
            row_h,
            &format!("{} steps", steps),
        );

        let steps_plus_x = steps_val_x + 58.0;
        let plus_hover = self
            .hover
            .map(|(hx, hy)| point_in_rect(hx, hy, steps_plus_x, y, steps_btn_w, row_h))
            .unwrap_or(false);
        draw_button(
            self.ctx,
            steps_plus_x,
            y,
            steps_btn_w,
            row_h,
            false,
            plus_hover,
        );
        set_icon_color(self.ctx, plus_hover);
        toolbar_icons::draw_icon_plus(
            self.ctx,
            steps_plus_x + (steps_btn_w - ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE) / 2.0,
            y + (row_h - ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE) / 2.0,
            ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE,
        );
        self.hits.push(HitRegion {
            rect: (steps_plus_x, y, steps_btn_w, row_h),
            event: if is_undo {
                ToolbarEvent::SetCustomUndoSteps(steps.saturating_add(1))
            } else {
                ToolbarEvent::SetCustomRedoSteps(steps.saturating_add(1))
            },
            kind: HitKind::Click,
            tooltip: Some(if is_undo {
                "Increase undo steps".to_string()
            } else {
                "Increase redo steps".to_string()
            }),
        });

        let slider_y = y + row_h + 8.0;
        let slider_h = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HEIGHT;
        let slider_r = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_KNOB_RADIUS;
        let slider_w = self.card_w - ToolbarLayoutSpec::SIDE_CARD_INSET * 2.0;
        self.ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
        draw_round_rect(self.ctx, self.x, slider_y, slider_w, slider_h, 3.0);
        let _ = self.ctx.fill();
        let t = delay_t_from_ms(delay_ms);
        let knob_x = self.x + t * (slider_w - slider_r * 2.0) + slider_r;
        self.ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
        self.ctx.arc(
            knob_x,
            slider_y + slider_h / 2.0,
            slider_r,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        let _ = self.ctx.fill();
        let hit_pad = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING;
        self.hits.push(HitRegion {
            rect: (
                self.x,
                slider_y - hit_pad,
                slider_w,
                slider_h + hit_pad * 2.0,
            ),
            event: if is_undo {
                ToolbarEvent::SetCustomUndoDelay(delay_secs_from_t(t))
            } else {
                ToolbarEvent::SetCustomRedoDelay(delay_secs_from_t(t))
            },
            kind: if is_undo {
                HitKind::DragCustomUndoDelay
            } else {
                HitKind::DragCustomRedoDelay
            },
            tooltip: Some(if is_undo {
                format!("Undo step delay: {:.1}s (drag)", delay_ms as f64 / 1000.0)
            } else {
                format!("Redo step delay: {:.1}s (drag)", delay_ms as f64 / 1000.0)
            }),
        });

        slider_y + slider_h + 10.0 - y
    }
}
