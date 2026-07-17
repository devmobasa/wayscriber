//! The GTK side palette.
//!
//! This module owns the side-window lifecycle. Widget assembly, chrome,
//! structure tracking, and drag adaptation live in focused child modules.

mod chrome;
mod drag;
mod palette;
mod structure;

#[cfg(test)]
mod tests;

use std::cell::Cell;
use std::collections::BTreeSet;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::toolbar_icons;
use crate::ui::toolbar::model::{SideHeaderModel, ToolbarPresentationPayload};
use crate::ui::toolbar::snapshot::ToolContext;
use crate::ui::toolbar::{SidePane, ToolbarEvent, ToolbarSideSection, ToolbarSnapshot};

use super::super::icons::IconWidget;
use super::super::widgets::{
    FeedbackSender, install_shortcut_focus_policy, send_event, set_active_class, sized_button,
};
use super::super::{GtkToolbarDragPhase, GtkToolbarFeedback, GtkToolbarKind};
use super::{Updater, sections};
use structure::{StructureKey, effective_scale};

const SIDE_WIDTH: f64 = 260.0;
const MINIMIZED_SIZE: (f64, f64) = (24.0, 64.0);
const BASE_MARGIN: (i32, i32) = (24, 24); // (top, left)
const END_MARGIN: (f64, f64) = (0.0, 24.0);

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
    drag_blocked: Rc<Cell<bool>>,
    move_drag: Option<gtk4::GestureDrag>,
    move_drag_cancel: Option<Rc<dyn Fn()>>,
    offsets: Rc<Cell<(f64, f64)>>,
    /// Monotonic counter for outgoing drag offsets; stale echoes from the
    /// backend are ignored by comparing against it.
    offset_seq: Rc<Cell<u64>>,
    capture_suppressed: bool,
    mapped_before_capture: bool,
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
        install_shortcut_focus_policy(&window, &feedback);
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
            drag_blocked: Rc::new(Cell::new(false)),
            move_drag: None,
            move_drag_cancel: None,
            offsets: Rc::new(Cell::new((0.0, 0.0))),
            offset_seq: Rc::new(Cell::new(0)),
            capture_suppressed: false,
            mapped_before_capture: false,
        }
    }

    pub(in crate::toolbar_gtk) fn apply(
        &mut self,
        update: &super::super::GtkToolbarUpdate,
    ) -> bool {
        let snapshot = &update.snapshot;
        let entering_capture_suppression = update.capture_suppressed && !self.capture_suppressed;
        if entering_capture_suppression {
            self.mapped_before_capture = self.window.is_visible() || self.window.is_mapped();
        } else if !update.capture_suppressed {
            self.mapped_before_capture = false;
        }
        self.capture_suppressed = update.capture_suppressed;
        self.drag_blocked
            .set(update.modal_engaged || update.capture_suppressed);
        if entering_capture_suppression && let Some(cancel) = self.move_drag_cancel.as_ref() {
            cancel();
        }
        let presentation = super::toolbar_surface_presentation(
            update.side_visible,
            update.capture_suppressed,
            super::drag_visual_should_be_hidden(
                update.drag_preview,
                GtkToolbarKind::Side,
                self.drag_active.get(),
                self.offset_seq.get(),
                update.side_offset_seq,
            ),
            self.mapped_before_capture,
        );
        if !presentation.window_visible {
            super::set_capture_transparent(&self.window, presentation.capture_transparent);
            super::set_surface_input_enabled(&self.window, false);
            self.window.set_visible(false);
            if let Some(generation) = update.capture_suppression_generation {
                super::log_capture_surface_state(
                    generation,
                    "side",
                    update.side_visible,
                    self.mapped_before_capture,
                    presentation,
                    &self.window,
                    &self.root,
                );
            }
            return false;
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
        super::set_capture_transparent(&self.window, presentation.capture_transparent);
        super::set_visual_hidden(
            &self.window,
            &self.root,
            GtkToolbarKind::Side,
            presentation.visual_hidden,
        );
        super::set_surface_input_enabled(&self.window, presentation.input_enabled);
        if let Some(generation) = update.capture_suppression_generation {
            super::log_capture_surface_state(
                generation,
                "side",
                update.side_visible,
                self.mapped_before_capture,
                presentation,
                &self.window,
                &self.root,
            );
        }
        true
    }

    /// Mirror backend offsets into layer margins unless a local drag is in
    /// flight or the echo is older than what this bar already sent. Side
    /// offsets are (x, y) like the update carries them.
    fn apply_offsets(&self, offsets: (f64, f64), echo_seq: u64) {
        if self.drag_active.get() || echo_seq < self.offset_seq.get() {
            crate::toolbar_gtk::drag_debug_log(format!(
                "side echo rejected echo_seq={echo_seq} local_seq={} active={} backend=({:.3},{:.3}) local=({:.3},{:.3})",
                self.offset_seq.get(),
                self.drag_active.get(),
                offsets.0,
                offsets.1,
                self.offsets.get().0,
                self.offsets.get().1,
            ));
            return;
        }
        let (left, x) = super::drag::rounded_margin_and_offset(BASE_MARGIN.1 as f64, offsets.0);
        let (top, y) = super::drag::rounded_margin_and_offset(BASE_MARGIN.0 as f64, offsets.1);
        self.offsets.set((x, y));
        self.window.set_margin(Edge::Left, left);
        self.window.set_margin(Edge::Top, top);
        crate::toolbar_gtk::drag_debug_log(format!(
            "side echo applied echo_seq={echo_seq} backend=({:.3},{:.3}) stored=({x:.3},{y:.3}) margin=({left},{top}) size={}x{}",
            offsets.0,
            offsets.1,
            self.window.width(),
            self.window.height(),
        ));
    }
}
