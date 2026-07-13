//! Side-palette header controls.

use super::*;

/// Board chip color dot, RGBA; `None` draws the empty outline.
type BoardDotColor = Rc<Cell<Option<(f64, f64, f64, f64)>>>;

impl SideBar {
    pub(super) fn board_chip(&mut self, _snapshot: &ToolbarSnapshot, scale: f64) -> gtk4::Button {
        let chip = gtk4::Button::new();
        chip.add_css_class("board-chip");
        chip.set_hexpand(true);
        chip.set_size_request(-1, (22.0 * scale).round() as i32);
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, (4.0 * scale).round() as i32);
        let dot_color: BoardDotColor = Rc::new(Cell::new(None));
        let dot = gtk4::DrawingArea::new();
        let dot_size = (14.0 * scale).round() as i32;
        dot.set_content_width(dot_size);
        dot.set_content_height(dot_size);
        dot.set_valign(gtk4::Align::Center);
        let draw_color = dot_color.clone();
        dot.set_draw_func(move |_, ctx, width, height| {
            let size = width.min(height) as f64;
            match draw_color.get() {
                Some((r, g, b, a)) => {
                    super::super::super::widgets::rounded_rect_path(
                        ctx,
                        0.5,
                        0.5,
                        size - 1.0,
                        size - 1.0,
                        3.0,
                    );
                    ctx.set_source_rgba(r, g, b, a);
                    let _ = ctx.fill();
                }
                None => {
                    super::super::super::widgets::rounded_rect_path(
                        ctx,
                        0.5,
                        0.5,
                        size - 1.0,
                        size - 1.0,
                        3.0,
                    );
                    ctx.set_source_rgba(0.62, 0.68, 0.76, 0.7);
                    ctx.set_line_width(1.0);
                    let _ = ctx.stroke();
                }
            }
        });
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
            if dot_color.get() != color {
                dot_color.set(color);
                dot.queue_draw();
            }
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
        let sender = self.feedback.clone();
        let pinned = Rc::new(Cell::new(snapshot.side_pinned));
        let click_pinned = pinned.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::PinSideToolbar(!click_pinned.get()));
        });
        let handle = button.clone();
        self.chrome_updaters.push(Box::new(move |snapshot| {
            pinned.set(snapshot.side_pinned);
            icon.set_painter(if snapshot.side_pinned {
                toolbar_icons::draw_icon_pin
            } else {
                toolbar_icons::draw_icon_unpin
            });
            if snapshot.side_pinned {
                handle.add_css_class("pinned");
                handle.set_tooltip_text(Some("Pinned: opens at startup (click to disable)"));
            } else {
                handle.remove_css_class("pinned");
                handle.set_tooltip_text(Some("Pin: click to open at startup"));
            }
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
