//! The GTK side palette.
//!
//! Fixed chrome (drag grip · board chip · pin · minimize, then the pane
//! tabs) above a scrolled pane body whose sections come from
//! `view::sections`. Content structure rebuilds when the discrete inputs
//! change; continuous values flow through updaters so sliders, entries,
//! and scroll position survive snapshot churn.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::toolbar_icons;
use crate::ui::toolbar::model::{SideHeaderModel, ToolbarPresentationPayload};
use crate::ui::toolbar::snapshot::ToolContext;
use crate::ui::toolbar::{SidePane, ToolbarEvent, ToolbarSideSection, ToolbarSnapshot};

use super::super::GtkToolbarFeedback;
use super::super::icons::IconWidget;
use super::super::widgets::{
    FeedbackSender, install_shortcut_focus_policy, send_event, set_active_class, sized_button,
};
use super::{Updater, sections};

/// Board chip color dot, RGBA; `None` draws the empty outline.
type BoardDotColor = Rc<Cell<Option<(f64, f64, f64, f64)>>>;

const SIDE_WIDTH: f64 = 260.0;
const MINIMIZED_SIZE: (f64, f64) = (24.0, 64.0);
const BASE_MARGIN: (i32, i32) = (24, 24); // (top, left)

/// Discrete inputs that force a rebuild of the pane content.
#[derive(PartialEq)]
struct StructureKey {
    minimized: bool,
    pane: SidePane,
    scale_milli: i64,
    use_icons: bool,
    layout_mode: crate::config::ToolbarLayoutMode,
    items: crate::config::ResolvedToolbarItems,
    collapsed: BTreeSet<ToolbarSideSection>,
    tool_flags: (bool, bool, bool, bool, bool, bool),
    show_more_colors: bool,
    quick_colors: crate::config::QuickColorPalette,
    preset_slots: Vec<bool>,
    custom_section_enabled: bool,
    show_delay_sliders: bool,
    delay_actions_enabled: bool,
    show_actions_advanced: bool,
    show_zoom_actions: bool,
    customize_open: bool,
    customize_group: Option<crate::ui::toolbar::ToolbarItemCustomizeGroup>,
    recents: Vec<std::path::PathBuf>,
    pending_save_as: Option<std::path::PathBuf>,
    active_session: Option<std::path::PathBuf>,
    eraser_kind_targets: (bool, bool),
    polygon_active: bool,
    font_mono: bool,
    /// Drives the scoped section titles ("Color — Pen"): tool or text/note
    /// scope changes must rebuild even when nothing else did.
    title_scope: (crate::input::Tool, bool, bool, bool),
}

impl StructureKey {
    fn of(snapshot: &ToolbarSnapshot) -> Self {
        let tool_context = ToolContext::from_snapshot(snapshot);
        Self {
            minimized: snapshot.side_minimized,
            pane: snapshot.active_side_pane,
            scale_milli: (effective_scale(snapshot) * 1000.0).round() as i64,
            use_icons: snapshot.use_icons,
            layout_mode: snapshot.layout_mode,
            items: snapshot.resolved_toolbar_items.clone(),
            collapsed: snapshot.collapsed_side_sections.clone(),
            tool_flags: (
                tool_context.needs_color,
                tool_context.needs_thickness,
                tool_context.show_arrow_labels,
                tool_context.show_step_counter,
                tool_context.show_marker_opacity,
                tool_context.show_font_controls,
            ),
            show_more_colors: snapshot.show_more_colors,
            quick_colors: snapshot.quick_colors.clone(),
            preset_slots: snapshot.presets.iter().map(|slot| slot.is_some()).collect(),
            custom_section_enabled: snapshot.custom_section_enabled,
            show_delay_sliders: snapshot.show_delay_sliders,
            delay_actions_enabled: snapshot.delay_actions_enabled,
            show_actions_advanced: snapshot.show_actions_advanced,
            show_zoom_actions: snapshot.show_zoom_actions,
            customize_open: snapshot.customize_items_open,
            customize_group: snapshot.customize_items_group,
            recents: snapshot
                .recent_sessions
                .iter()
                .map(|recent| recent.path.clone())
                .collect(),
            pending_save_as: snapshot.pending_save_as_overwrite_path.clone(),
            active_session: snapshot.active_session_path.clone(),
            eraser_kind_targets: (
                snapshot.thickness_targets_eraser,
                snapshot.thickness_targets_marker,
            ),
            polygon_active: snapshot.active_tool == crate::input::Tool::RegularPolygon
                || snapshot.tool_override == Some(crate::input::Tool::RegularPolygon),
            font_mono: snapshot.font.family.eq_ignore_ascii_case("monospace"),
            title_scope: (
                snapshot.active_tool,
                snapshot.text_active,
                snapshot.note_active,
                snapshot.context_aware_ui,
            ),
        }
    }
}

fn effective_scale(snapshot: &ToolbarSnapshot) -> f64 {
    if snapshot.toolbar_scale.is_finite() {
        snapshot.toolbar_scale.clamp(0.5, 3.0)
    } else {
        1.0
    }
}

pub(in crate::toolbar_gtk) struct SideBar {
    pub(in crate::toolbar_gtk) window: gtk4::Window,
    feedback: FeedbackSender,
    root: gtk4::Box,
    structure: Option<StructureKey>,
    /// Chrome updaters survive content rebuilds; content updaters are
    /// replaced together with the pane body.
    chrome_updaters: Vec<Updater>,
    content_updaters: Vec<Updater>,
    scrolled: Option<gtk4::ScrolledWindow>,
    /// Per-pane scroll positions, saved on rebuild and restored when the
    /// same pane is built again — switching panes must not leak one
    /// pane's scroll into another (the backend keeps per-pane offsets
    /// too; this mirrors that behavior GTK-side).
    saved_scroll: std::rc::Rc<std::cell::RefCell<Vec<(SidePane, f64)>>>,
    drag_active: Rc<Cell<bool>>,
    offsets: Rc<Cell<(f64, f64)>>,
    /// Monotonic counter for outgoing drag offsets; stale echoes from the
    /// backend are ignored by comparing against it.
    offset_seq: Rc<Cell<u64>>,
}

impl SideBar {
    pub(in crate::toolbar_gtk) fn new(feedback: FeedbackSender) -> Self {
        let window = gtk4::Window::new();
        window.add_css_class("wayscriber-toolbar");
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_namespace(Some("wayscriber-toolbar-side"));
        window.set_anchor(Edge::Left, true);
        window.set_anchor(Edge::Top, true);
        window.set_margin(Edge::Top, BASE_MARGIN.0);
        window.set_margin(Edge::Left, BASE_MARGIN.1);
        // Stay focusable for the editable hex field, but relinquish focus
        // immediately after ordinary toolbar interaction.
        window.set_keyboard_mode(KeyboardMode::OnDemand);
        install_shortcut_focus_policy(&window);
        // Match the built-in bars: do not shift for other exclusive zones
        // (panels/bars) and do not reserve one.
        window.set_exclusive_zone(-1);

        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        root.add_css_class("panel");
        window.set_child(Some(&root));

        Self {
            window,
            feedback,
            root,
            structure: None,
            chrome_updaters: Vec::new(),
            content_updaters: Vec::new(),
            scrolled: None,
            saved_scroll: Rc::new(std::cell::RefCell::new(Vec::new())),
            drag_active: Rc::new(Cell::new(false)),
            offsets: Rc::new(Cell::new((0.0, 0.0))),
            offset_seq: Rc::new(Cell::new(0)),
        }
    }

    pub(in crate::toolbar_gtk) fn apply(&mut self, update: &super::super::GtkToolbarUpdate) {
        let snapshot = &update.snapshot;
        if !update.side_visible {
            self.window.set_visible(false);
            return;
        }
        self.apply_offsets(update.side_offset, update.side_offset_seq);

        let key = StructureKey::of(snapshot);
        if self.structure.as_ref() != Some(&key) {
            self.rebuild(snapshot);
            self.structure = Some(key);
        }
        for updater in self.chrome_updaters.iter().chain(&self.content_updaters) {
            updater(snapshot);
        }
        self.sync_viewport(snapshot);
        self.window.set_visible(true);
    }

    /// Mirror backend offsets into layer margins unless a local drag is in
    /// flight or the echo is older than what this bar already sent. Side
    /// offsets are (x, y) like the update carries them.
    fn apply_offsets(&self, offsets: (f64, f64), echo_seq: u64) {
        if self.drag_active.get() || echo_seq < self.offset_seq.get() {
            return;
        }
        self.offsets.set(offsets);
        self.window
            .set_margin(Edge::Left, BASE_MARGIN.1 + offsets.0.round() as i32);
        self.window
            .set_margin(Edge::Top, BASE_MARGIN.0 + offsets.1.round() as i32);
    }

    /// Cap the scrolled body so the palette never grows past the screen
    /// space the backend reports.
    fn sync_viewport(&self, snapshot: &ToolbarSnapshot) {
        let Some(scrolled) = &self.scrolled else {
            return;
        };
        let scale = effective_scale(snapshot);
        if let Some(viewport) = snapshot.side_viewport_max {
            // The viewport budget covers the whole palette; subtract the
            // fixed chrome above the scrolled body.
            let chrome = 76.0;
            let max = ((viewport - chrome).max(120.0) * scale).round() as i32;
            scrolled.set_max_content_height(max);
        }
    }

    fn rebuild(&mut self, snapshot: &ToolbarSnapshot) {
        // Preserve the outgoing pane's scroll position, keyed by pane so
        // it is only restored into the same pane.
        if let (Some(scrolled), Some(key)) = (&self.scrolled, &self.structure) {
            let value = scrolled.vadjustment().value();
            let mut saved = self.saved_scroll.borrow_mut();
            saved.retain(|(pane, _)| *pane != key.pane);
            saved.push((key.pane, value));
        }
        while let Some(child) = self.root.first_child() {
            self.root.remove(&child);
        }
        self.chrome_updaters.clear();
        self.content_updaters.clear();
        self.scrolled = None;

        if snapshot.side_minimized {
            self.build_minimized(snapshot);
        } else {
            self.build_palette(snapshot);
        }
    }

    fn build_minimized(&mut self, snapshot: &ToolbarSnapshot) {
        let scale = effective_scale(snapshot);
        self.root.add_css_class("minimized");
        // Drop the palette's 260px default width so the tab shrinks.
        self.window.set_default_size(
            (MINIMIZED_SIZE.0 * scale).round() as i32,
            (MINIMIZED_SIZE.1 * scale).round() as i32,
        );
        let restore = sized_button(MINIMIZED_SIZE.0 * scale, MINIMIZED_SIZE.1 * scale);
        restore.add_css_class("chrome");
        restore.set_tooltip_text(Some("Show toolbar"));
        let icon = IconWidget::new(
            toolbar_icons::draw_icon_chevron_right,
            (MINIMIZED_SIZE.0 * 0.75 * scale).min(18.0 * scale),
        );
        restore.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        restore.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::SetSideMinimized(false));
        });
        self.root.append(&restore);
    }

    fn build_palette(&mut self, snapshot: &ToolbarSnapshot) {
        self.root.remove_css_class("minimized");
        let scale = effective_scale(snapshot);
        let px = |value: f64| (value * scale).round() as i32;
        self.window
            .set_default_size((SIDE_WIDTH * scale).round() as i32, -1);

        // ===== Header band: grip · board chip · pin · minimize =====
        let band = gtk4::Box::new(gtk4::Orientation::Horizontal, px(6.0));
        band.add_css_class("header-band");

        let grip = IconWidget::new(toolbar_icons::draw_icon_grip_bars, 18.0 * scale);
        grip.area.set_can_target(true);
        grip.area.add_css_class("drag-handle");
        grip.area.set_tooltip_text(Some("Drag toolbar"));
        grip.area.set_valign(gtk4::Align::Center);
        grip.area.set_cursor_from_name(Some("grab"));
        self.attach_move_drag(&grip.area);
        band.append(&grip.area);

        let chip = self.board_chip(snapshot, scale);
        band.append(&chip);

        band.append(&self.pin_button(snapshot, 22.0 * scale));
        band.append(&self.minimize_button(22.0 * scale));
        self.root.append(&band);

        // ===== Pane navigation =====
        let nav = gtk4::Box::new(gtk4::Orientation::Horizontal, px(4.0));
        nav.set_homogeneous(true);
        nav.set_margin_top(px(6.0));
        for pane in SidePane::ALL {
            let tab = gtk4::Button::with_label(pane.label());
            tab.add_css_class("tab");
            tab.set_size_request(-1, px(26.0));
            tab.set_tooltip_text(Some(&format!("{} pane", pane.label())));
            let sender = self.feedback.clone();
            tab.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::SetSidePane(pane));
            });
            let handle = tab.clone();
            self.chrome_updaters.push(Box::new(move |snapshot| {
                set_active_class(&handle, snapshot.active_side_pane == pane);
            }));
            nav.append(&tab);
        }
        self.root.append(&nav);

        // ===== Scrolled pane body =====
        let mut content_updaters = Vec::new();
        let content = sections::build_pane_content(
            snapshot,
            self.feedback.clone(),
            scale,
            &mut content_updaters,
        );
        content.set_margin_top(px(8.0));
        self.content_updaters = content_updaters;

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scrolled.set_propagate_natural_height(true);
        scrolled.set_child(Some(&content));
        self.root.append(&scrolled);

        // Restore this pane's scroll position once the new content is
        // laid out; one-shot so later size changes never yank the user's
        // scroll.
        let adjustment = scrolled.vadjustment();
        let pane = snapshot.active_side_pane;
        let pending = std::cell::Cell::new(
            self.saved_scroll
                .borrow()
                .iter()
                .find(|(saved_pane, _)| *saved_pane == pane)
                .map(|(_, value)| *value)
                .unwrap_or(0.0),
        );
        adjustment.connect_changed(move |adjustment| {
            let target = pending.get();
            let reachable = adjustment.upper() - adjustment.page_size();
            if target > 0.0 && reachable >= target {
                adjustment.set_value(target);
                pending.set(0.0);
            }
        });
        self.scrolled = Some(scrolled);
    }

    fn board_chip(&mut self, _snapshot: &ToolbarSnapshot, scale: f64) -> gtk4::Button {
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
                    super::super::widgets::rounded_rect_path(
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
                    super::super::widgets::rounded_rect_path(
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

    fn pin_button(&mut self, snapshot: &ToolbarSnapshot, size: f64) -> gtk4::Button {
        let button = sized_button(size, size);
        button.add_css_class("chrome");
        let icon = IconWidget::new(
            if snapshot.side_pinned {
                toolbar_icons::draw_icon_pin_filled
            } else {
                toolbar_icons::draw_icon_pin_outline
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
                toolbar_icons::draw_icon_pin_filled
            } else {
                toolbar_icons::draw_icon_pin_outline
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

    fn minimize_button(&mut self, size: f64) -> gtk4::Button {
        let button = sized_button(size, size);
        button.add_css_class("chrome");
        button.add_css_class("minimize");
        button.set_tooltip_text(Some("Minimize (leaves a restore tab)"));
        let icon = IconWidget::new(toolbar_icons::draw_icon_dash, size * 0.6);
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::SetSideMinimized(true));
        });
        button
    }

    /// See `top_bar::attach_move_drag`: coalesce surface-local residual
    /// motion to one layer-margin update per GTK frame.
    fn attach_move_drag(&self, grip: &gtk4::DrawingArea) {
        let drag = gtk4::GestureDrag::new();
        let window = self.window.clone();
        let feedback = self.feedback.clone();
        let drag_active = self.drag_active.clone();
        let offsets = self.offsets.clone();
        let seq = self.offset_seq.clone();
        let pending = Rc::new(super::top_bar::FrameCoalescedDrag::default());
        let active_generation = Rc::new(Cell::new(0));

        let begin_active = drag_active.clone();
        let begin_pending = pending.clone();
        let begin_generation = active_generation.clone();
        let frame_window = window.clone();
        let frame_offsets = offsets.clone();
        let frame_feedback = feedback.clone();
        let frame_seq = seq.clone();
        drag.connect_drag_begin(move |gesture, _, _| {
            begin_active.set(true);
            let generation = begin_pending.begin();
            begin_generation.set(generation);
            let Some(grip) = gesture.widget() else {
                begin_active.set(false);
                return;
            };

            let pending = begin_pending.clone();
            let window = frame_window.clone();
            let offsets = frame_offsets.clone();
            let feedback = frame_feedback.clone();
            let seq = frame_seq.clone();
            let drag_active = begin_active.clone();
            let active_generation = begin_generation.clone();
            grip.add_tick_callback(move |_, _| {
                let Some(frame) = pending.take_frame(generation) else {
                    return gtk4::glib::ControlFlow::Continue;
                };
                let (cx, cy) = offsets.get();
                let (x, y) = super::top_bar::clamp_drag_offsets(
                    &window,
                    (cx + frame.delta.0, cy + frame.delta.1),
                    (BASE_MARGIN.1 as f64, BASE_MARGIN.0 as f64),
                );
                offsets.set((x, y));
                window.set_margin(Edge::Left, BASE_MARGIN.1 + x.round() as i32);
                window.set_margin(Edge::Top, BASE_MARGIN.0 + y.round() as i32);
                seq.set(seq.get() + 1);
                let _ = feedback.send(GtkToolbarFeedback::SetSideOffset {
                    x,
                    y,
                    seq: seq.get(),
                    done: frame.done,
                });
                if frame.done {
                    if active_generation.get() == generation {
                        drag_active.set(false);
                    }
                    gtk4::glib::ControlFlow::Break
                } else {
                    gtk4::glib::ControlFlow::Continue
                }
            });
        });

        let update_pending = pending.clone();
        let update_generation = active_generation.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            update_pending.update(update_generation.get(), dx, dy);
        });

        drag.connect_drag_end(move |_, dx, dy| {
            pending.end(active_generation.get(), dx, dy);
        });
        grip.add_controller(drag);
    }
}
