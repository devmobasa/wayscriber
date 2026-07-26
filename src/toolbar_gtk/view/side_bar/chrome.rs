//! Side-palette header controls.

use super::*;
use crate::ui::theme::{Rgba, set_color};

/// Empty board-dot outline: muted blue-gray with no theme token (mirrors
/// the built-in `side_palette::header`).
/// TODO(theme-consolidation): hoist the mirrored pair into `theme::toolbar`.
const COLOR_BOARD_CHIP_EMPTY_DOT: Rgba = (0.62, 0.68, 0.76, 0.7);

impl SideBar {
    pub(super) fn board_chip(&mut self, _snapshot: &ToolbarSnapshot, scale: f64) -> gtk4::Button {
        let chip = gtk4::Button::new();
        chip.add_css_class("board-chip");
        chip.set_hexpand(true);
        chip.set_size_request(-1, (22.0 * scale).round() as i32);
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, (4.0 * scale).round() as i32);
        let dot = gtk4::DrawingArea::new();
        let dot_size = (14.0 * scale).round() as i32;
        dot.set_content_width(dot_size);
        dot.set_content_height(dot_size);
        dot.set_valign(gtk4::Align::Center);
        install_board_dot_draw(&dot, None);
        row.append(&dot);
        let board_icon = IconWidget::new(toolbar_icons::draw_icon_board, 10.0 * scale);
        row.append(&board_icon.area);
        let label = gtk4::Label::new(None);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        label.set_xalign(0.0);
        label.set_hexpand(true);
        row.append(&label);
        let chevron = IconWidget::new(toolbar_icons::draw_icon_chevron_right, 12.0 * scale);
        row.append(&chevron.area);
        chip.set_child(Some(&row));
        let sender = self.feedback.clone();
        chip.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::ToggleBoardPicker);
        });
        let chip_handle = chip.clone();
        self.chrome_updaters.push(Box::new(move |snapshot| {
            let header_model = SideHeaderModel::from_snapshot(snapshot);
            let (text, color) = match &header_model.board_chip.presentation.payload {
                ToolbarPresentationPayload::BoardChip(board) => (
                    board.label.clone(),
                    board.color.map(|c| (c.r, c.g, c.b, c.a)),
                ),
                ToolbarPresentationPayload::None => {
                    (header_model.board_chip.presentation.label.to_string(), None)
                }
            };
            label.set_text(&text);
            if let Some(tooltip) = header_model.board_chip.presentation.tooltip.as_string() {
                chip_handle.set_tooltip_text(Some(&tooltip));
            }
            install_board_dot_draw(&dot, color);
            dot.queue_draw();
        }));
        chip
    }

    pub(super) fn pin_button(&mut self, snapshot: &ToolbarSnapshot, size: f64) -> gtk4::Button {
        let button = sized_button(size, size);
        button.add_css_class("chrome");
        let icon = IconWidget::new(
            if snapshot.side_pinned {
                toolbar_icons::draw_icon_pin
            } else {
                toolbar_icons::draw_icon_unpin
            },
            size * 0.62,
        );
        button.set_child(Some(&icon.area));
        sync_pin_presentation(&button, snapshot.side_pinned);
        let sender = self.feedback.clone();
        button.connect_clicked(move |button| {
            send_event(
                &sender,
                ToolbarEvent::PinSideToolbar(!button.has_css_class("pinned")),
            );
        });
        let handle = button.clone();
        self.chrome_updaters.push(Box::new(move |snapshot| {
            icon.set_painter(if snapshot.side_pinned {
                toolbar_icons::draw_icon_pin
            } else {
                toolbar_icons::draw_icon_unpin
            });
            sync_pin_presentation(&handle, snapshot.side_pinned);
        }));
        button
    }

    pub(super) fn minimize_button(&mut self, size: f64) -> gtk4::Button {
        let button = sized_button(size, size);
        button.add_css_class("chrome");
        button.add_css_class("minimize");
        button.set_tooltip_text(Some("Minimize (leaves a restore tab)"));
        let icon = IconWidget::new(toolbar_icons::draw_icon_side_minimize, size * 0.6);
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::SetSideMinimized(true));
        });
        button
    }
}

fn install_board_dot_draw(dot: &gtk4::DrawingArea, color: Option<(f64, f64, f64, f64)>) {
    dot.set_draw_func(move |_, ctx, width, height| {
        let size = width.min(height) as f64;
        super::super::super::widgets::rounded_rect_path(ctx, 0.5, 0.5, size - 1.0, size - 1.0, 3.0);
        match color {
            Some((r, g, b, a)) => {
                ctx.set_source_rgba(r, g, b, a);
                let _ = ctx.fill();
            }
            None => {
                set_color(ctx, COLOR_BOARD_CHIP_EMPTY_DOT);
                ctx.set_line_width(1.0);
                let _ = ctx.stroke();
            }
        }
    });
}

fn sync_pin_presentation(button: &gtk4::Button, pinned: bool) {
    if pinned {
        button.add_css_class("pinned");
        button.set_tooltip_text(Some("Pinned: opens at startup (click to disable)"));
    } else {
        button.remove_css_class("pinned");
        button.set_tooltip_text(Some("Pin: click to open at startup"));
    }
}
