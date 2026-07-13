//! The GTK top strip.
//!
//! Reproduces the built-in strip's reading order — drag grip | pens |
//! shapes | shapes-picker | annotations | quick colors + chip | history |
//! Clear | pin/overflow/minimize — from the same `ui::toolbar::model`
//! ordering/visibility logic and the same `plan_top_strip` width
//! degradation, so the two frontends stay behaviorally identical.
//!
//! The bar content is rebuilt only when its *structure* changes (item
//! order/visibility, icon mode, plan, scale, palette); per-state changes
//! (active tool, colors, undo availability, popover open flags) run
//! through stored updater closures so open popovers and hover states
//! survive snapshot churn.

mod controls;
mod drag;
mod popovers;
mod strip;
#[cfg(test)]
mod tests;

pub(super) use drag::{FrameCoalescedDrag, clamp_drag_offsets, drag_frame_position};

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::backend::wayland::{TopStripPlan, plan_top_strip, top_toolbar_size};
use crate::config::{Action, ToolbarLayoutMode, action_label, action_short_label};
use crate::input::Tool;
use crate::label_format::format_binding_label;
use crate::toolbar_icons;
use crate::ui::toolbar::bindings::{tool_label, tool_tooltip_label};
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot, model};

use super::super::icons::{IconWidget, tool_icon_painter};
use super::super::widgets::{
    FeedbackSender, SwatchButton, add_shortcut_badge, icon_button, install_shortcut_focus_policy,
    send_event, set_active_class, sized_button, swatch_with_shortcut, text_button,
};
use super::super::{GtkToolbarDragPhase, GtkToolbarFeedback, GtkToolbarKind};

// Spec-unit design tokens mirrored from the built-in layout
// (`layout/spec/top.rs`); the plan/natural-width math reuses the builtin
// functions directly, these only size the GTK widgets.
const GAP: f64 = 5.0;
const START_X: f64 = 19.0;
const HANDLE_SIZE: f64 = 18.0;
const ICON_BUTTON: f64 = 46.0;
const ICON_SIZE: f64 = 28.0;
const TEXT_BUTTON_W: f64 = 60.0;
const TEXT_BUTTON_H: f64 = 36.0;
const PIN_BUTTON_SIZE: f64 = 24.0;
const PIN_BUTTON_GAP: f64 = 6.0;
const PIN_MARGIN_RIGHT: f64 = 15.0;
const DIVIDER_SPAN: f64 = 7.0;
const SWATCH_SIZE: f64 = 22.0;
const SWATCH_GAP: f64 = 4.0;
const CHIP_SIZE: f64 = 28.0;
const COMPACT_BUTTON: f64 = 26.0;
const COMPACT_GAP: f64 = 1.0;
const COMPACT_CHROME: f64 = 18.0;
const COMPACT_MARGIN_RIGHT: f64 = 8.0;
const COMPACT_START_X: f64 = 4.0;
const MINIMIZED_SIZE: (f64, f64) = (64.0, 24.0);
const BASE_MARGIN: (i32, i32) = (12, 12);
const END_MARGIN: (f64, f64) = (12.0, 0.0);

use super::Updater;

/// Snapshot inputs the shapes-popover grid renders from: active tool,
/// override, fill flag, polygon sides.
type ShapesContentKey = (Tool, Option<Tool>, bool, u8);

/// Snapshot inputs the overflow grid renders from: active tool, override,
/// text/note/highlight active flags.
type OverflowContentKey = (Tool, Option<Tool>, bool, bool, bool);

/// Inputs that force a rebuild of the bar's widget structure.
#[derive(PartialEq)]
struct StructureKey {
    minimized: bool,
    use_icons: bool,
    layout_mode: ToolbarLayoutMode,
    scale_milli: i64,
    items: crate::config::ResolvedToolbarItems,
    quick_colors: crate::config::QuickColorPalette,
    binding_hints: crate::ui::toolbar::ToolbarBindingHints,
    plan: TopStripPlan,
    ring_row: bool,
}

impl StructureKey {
    fn of(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> Self {
        Self {
            minimized: snapshot.top_minimized,
            use_icons: snapshot.use_icons || plan.compact,
            layout_mode: snapshot.layout_mode,
            scale_milli: (effective_scale(snapshot) * 1000.0).round() as i64,
            items: snapshot.resolved_toolbar_items.clone(),
            quick_colors: snapshot.quick_colors.clone(),
            binding_hints: snapshot.binding_hints.clone(),
            plan: plan.clone(),
            ring_row: ring_row_active(snapshot, plan),
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

pub(super) fn rounded_margin_and_offset(base: f64, offset: f64) -> (i32, f64) {
    let margin = (base + offset).round() as i32;
    (margin, margin as f64 - base)
}

fn top_default_width(snapshot: &ToolbarSnapshot) -> i32 {
    top_toolbar_size(snapshot).0.min(i32::MAX as u32) as i32
}

fn ring_row_active(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> bool {
    let is_simple = snapshot.layout_mode == ToolbarLayoutMode::Simple;
    (snapshot.use_icons || plan.compact)
        && !is_simple
        && snapshot.highlight_tool_active
        && model::top_highlight_ring_visible(snapshot)
        && model::visible_top_utility_buttons(snapshot, is_simple, snapshot.use_icons)
            .contains(&model::TopUtilityButton::Highlight)
        && !plan
            .dropped_utilities
            .contains(&model::TopUtilityButton::Highlight)
}

/// Tooltip for a tool button: label plus binding and/or drag hint,
/// mirroring the built-in `tool_tooltip`.
fn tool_tooltip(snapshot: &ToolbarSnapshot, tool: Tool) -> String {
    let label = tool_tooltip_label(tool);
    let default_hint = model::default_drag_hint(tool);
    let binding = match (snapshot.binding_hints.for_tool(tool), default_hint) {
        (Some(binding), Some(fallback)) => Some(format!("{binding}, {fallback}")),
        (Some(binding), None) => Some(binding.to_string()),
        (None, Some(fallback)) => Some(fallback.to_string()),
        (None, None) => None,
    };
    format_binding_label(label, binding.as_deref())
}

fn action_tooltip(snapshot: &ToolbarSnapshot, action: Action) -> String {
    format_binding_label(
        action_label(action),
        snapshot.binding_hints.binding_for_action(action),
    )
}

pub(in crate::toolbar_gtk) struct TopBar {
    pub(in crate::toolbar_gtk) window: gtk4::Window,
    feedback: FeedbackSender,
    root: gtk4::Box,
    structure: Option<StructureKey>,
    updaters: Rc<RefCell<Vec<Updater>>>,
    shapes_popover: Option<gtk4::Popover>,
    overflow_popover: Option<gtk4::Popover>,
    /// Popover open state as last driven by the snapshot; lets the
    /// `closed` handlers distinguish user dismissal from state sync.
    shapes_expected_open: Rc<Cell<bool>>,
    overflow_expected_open: Rc<Cell<bool>>,
    /// Discriminants of the currently built popover contents; skips the
    /// per-snapshot rebuild that would reset hover and in-flight presses.
    shapes_content_key: Cell<Option<ShapesContentKey>>,
    overflow_content_key: Cell<Option<OverflowContentKey>>,
    drag_active: Rc<Cell<bool>>,
    drag_blocked: Rc<Cell<bool>>,
    offsets: Rc<Cell<(f64, f64)>>,
    /// Base X in spec units from the backend (side palette pushes it).
    base_x: Rc<Cell<f64>>,
    /// Monotonic counter for outgoing drag offsets; stale echoes from the
    /// backend are ignored by comparing against it.
    offset_seq: Rc<Cell<u64>>,
}

impl TopBar {
    pub(in crate::toolbar_gtk) fn new(feedback: FeedbackSender) -> Self {
        let window = gtk4::Window::new();
        window.add_css_class("wayscriber-toolbar");
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_namespace(Some("wayscriber-toolbar-top"));
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Left, true);
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
            updaters: Rc::new(RefCell::new(Vec::new())),
            shapes_popover: None,
            overflow_popover: None,
            shapes_expected_open: Rc::new(Cell::new(false)),
            overflow_expected_open: Rc::new(Cell::new(false)),
            shapes_content_key: Cell::new(None),
            overflow_content_key: Cell::new(None),
            drag_active: Rc::new(Cell::new(false)),
            drag_blocked: Rc::new(Cell::new(false)),
            offsets: Rc::new(Cell::new((0.0, 0.0))),
            base_x: Rc::new(Cell::new(BASE_MARGIN.1 as f64)),
            offset_seq: Rc::new(Cell::new(0)),
        }
    }

    pub(in crate::toolbar_gtk) fn apply(&mut self, update: &super::super::GtkToolbarUpdate) {
        let snapshot = &update.snapshot;
        self.drag_blocked.set(update.modal_engaged);
        let panel_opacity = if update.drag_preview == Some(GtkToolbarKind::Top) {
            0.0
        } else {
            1.0
        };
        if (self.root.opacity() - panel_opacity).abs() > f64::EPSILON {
            crate::toolbar_gtk::drag_debug_log(format!(
                "top panel opacity -> {panel_opacity:.1} (window remains mapped for drag input)"
            ));
            self.root.set_opacity(panel_opacity);
        }
        if !update.top_visible {
            // Suppress the dismissal echoes a hide-triggered popover close
            // would send, so an open picker survives a hide/show cycle
            // like the built-in bars.
            self.shapes_expected_open.set(false);
            self.overflow_expected_open.set(false);
            self.window.set_visible(false);
            return;
        }
        self.base_x.set(update.top_base_x);
        self.apply_offsets(update.top_offset, update.top_offset_seq);

        let plan = plan_top_strip(snapshot);
        let key = StructureKey::of(snapshot, &plan);
        if self.structure.as_ref() != Some(&key) {
            self.rebuild(snapshot, &plan);
            self.structure = Some(key);
        }
        for updater in self.updaters.borrow().iter() {
            updater(snapshot);
        }
        self.sync_popovers(snapshot, &plan);
        self.window.set_visible(true);
    }

    /// Mirror backend offsets into layer margins unless a local drag is in
    /// flight or the echo is older than what this bar already sent.
    fn apply_offsets(&self, offsets: (f64, f64), echo_seq: u64) {
        if self.drag_active.get() || echo_seq < self.offset_seq.get() {
            crate::toolbar_gtk::drag_debug_log(format!(
                "top echo rejected echo_seq={echo_seq} local_seq={} active={} backend=({:.3},{:.3}) local=({:.3},{:.3})",
                self.offset_seq.get(),
                self.drag_active.get(),
                offsets.0,
                offsets.1,
                self.offsets.get().0,
                self.offsets.get().1,
            ));
            return;
        }
        let (left, x) = rounded_margin_and_offset(self.base_x.get(), offsets.0);
        let (top, y) = rounded_margin_and_offset(BASE_MARGIN.0 as f64, offsets.1);
        self.offsets.set((x, y));
        self.window.set_margin(Edge::Left, left);
        self.window.set_margin(Edge::Top, top);
        crate::toolbar_gtk::drag_debug_log(format!(
            "top echo applied echo_seq={echo_seq} backend=({:.3},{:.3}) stored=({x:.3},{y:.3}) margin=({left},{top}) size={}x{}",
            offsets.0,
            offsets.1,
            self.window.width(),
            self.window.height(),
        ));
    }

    fn rebuild(&mut self, snapshot: &ToolbarSnapshot, plan: &TopStripPlan) {
        // Popovers are parented to bar buttons; unparent them before the
        // buttons go away or GTK complains and leaks the popover widgets.
        self.shapes_expected_open.set(false);
        self.overflow_expected_open.set(false);
        if let Some(popover) = self.shapes_popover.take() {
            popover.unparent();
        }
        if let Some(popover) = self.overflow_popover.take() {
            popover.unparent();
        }
        self.shapes_content_key.set(None);
        self.overflow_content_key.set(None);
        while let Some(child) = self.root.first_child() {
            self.root.remove(&child);
        }
        self.updaters.borrow_mut().clear();

        if snapshot.top_minimized {
            self.build_minimized(snapshot);
        } else {
            self.build_strip(snapshot, plan);
        }
    }
}
