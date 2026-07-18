//! The GTK top strip.
//!
//! Adapts the shared `TopToolbarSpec` reading order — drag grip | pens |
//! shapes | shapes-picker | annotations | quick colors + chip | history |
//! Clear | pin/overflow/minimize — into GTK widgets. Width degradation uses
//! the same shared plan as the built-in frontend.
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

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::backend::wayland::{plan_top_strip, top_toolbar_size};
use crate::config::{Action, ToolbarLayoutMode, action_short_label};
use crate::input::Tool;
use crate::toolbar_icons::top_toolbar_icon_painter;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot, model};
use model::TopStripPlan;

use super::super::icons::IconWidget;
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

use super::{CaptureProofTarget, CaptureSurfaceContent, Updater};

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

fn top_default_width(snapshot: &ToolbarSnapshot) -> i32 {
    top_toolbar_size(snapshot).0.min(i32::MAX as u32) as i32
}

fn ring_row_active(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> bool {
    model::TopToolbarSpec::contextual_highlight_ring_visible(snapshot, plan)
}

/// Attach the renderer-neutral id to concrete widgets in test builds so the
/// contract suite can read the actual GTK adapter tree without widening the
/// production CSS surface.
fn set_semantic_widget_id(_widget: &impl IsA<gtk4::Widget>, _id: &str) {
    #[cfg(test)]
    _widget.as_ref().set_widget_name(_id);
}

fn set_control_widget_id(_widget: &impl IsA<gtk4::Widget>, _control: model::TopToolbarControl) {
    #[cfg(test)]
    set_semantic_widget_id(_widget, _control.id().render_id().as_ref());
}

fn set_prefixed_control_widget_id(
    _widget: &impl IsA<gtk4::Widget>,
    _prefix: &str,
    _control: model::TopToolbarControl,
) {
    #[cfg(test)]
    set_semantic_widget_id(_widget, &format!("{_prefix}{}", _control.id().render_id()));
}

pub(in crate::toolbar_gtk) struct TopBar {
    pub(in crate::toolbar_gtk) window: gtk4::Window,
    feedback: FeedbackSender,
    root: gtk4::Box,
    capture_surface: CaptureSurfaceContent,
    structure: Option<StructureKey>,
    updaters: Rc<RefCell<Vec<Updater>>>,
    shapes_popover: Option<gtk4::Popover>,
    shapes_capture_surface: Option<CaptureSurfaceContent>,
    overflow_popover: Option<gtk4::Popover>,
    overflow_capture_surface: Option<CaptureSurfaceContent>,
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
    move_drag: Option<gtk4::GestureDrag>,
    move_drag_cancel: Option<Rc<dyn Fn()>>,
    offsets: Rc<Cell<(f64, f64)>>,
    /// Base X in spec units from the backend (side palette pushes it).
    base_x: Rc<Cell<f64>>,
    /// Monotonic counter for outgoing drag offsets; stale echoes from the
    /// backend are ignored by comparing against it.
    offset_seq: Rc<Cell<u64>>,
    capture_suppressed: bool,
    mapped_before_capture: bool,
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

        Self::with_window(feedback, window)
    }

    /// Build an unpresented GTK widget tree without layer-shell side effects.
    /// This keeps adapter contract tests usable on any GTK display backend.
    #[cfg(test)]
    fn new_for_test(feedback: FeedbackSender) -> Self {
        let window = gtk4::Window::new();
        window.add_css_class("wayscriber-toolbar");
        Self::with_window(feedback, window)
    }

    fn with_window(feedback: FeedbackSender, window: gtk4::Window) -> Self {
        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        root.add_css_class("panel");
        let capture_surface = CaptureSurfaceContent::new(&root);
        window.set_child(Some(capture_surface.widget()));

        Self {
            window,
            feedback,
            root,
            capture_surface,
            structure: None,
            updaters: Rc::new(RefCell::new(Vec::new())),
            shapes_popover: None,
            shapes_capture_surface: None,
            overflow_popover: None,
            overflow_capture_surface: None,
            shapes_expected_open: Rc::new(Cell::new(false)),
            overflow_expected_open: Rc::new(Cell::new(false)),
            shapes_content_key: Cell::new(None),
            overflow_content_key: Cell::new(None),
            drag_active: Rc::new(Cell::new(false)),
            drag_blocked: Rc::new(Cell::new(false)),
            move_drag: None,
            move_drag_cancel: None,
            offsets: Rc::new(Cell::new((0.0, 0.0))),
            base_x: Rc::new(Cell::new(BASE_MARGIN.1 as f64)),
            offset_seq: Rc::new(Cell::new(0)),
            capture_suppressed: false,
            mapped_before_capture: false,
        }
    }

    pub(in crate::toolbar_gtk) fn apply(
        &mut self,
        update: &super::super::GtkToolbarUpdate,
        defer_capture_input: bool,
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
            update.top_visible,
            update.capture_suppressed,
            super::drag_visual_should_be_hidden(
                update.drag_preview,
                GtkToolbarKind::Top,
                self.drag_active.get(),
                self.offset_seq.get(),
                update.top_offset_seq,
            ),
            self.mapped_before_capture,
        );
        if !presentation.window_visible {
            // Suppress the dismissal echoes a hide-triggered popover close
            // would send, so an open picker survives a hide/show cycle
            // like the built-in bars.
            if update.capture_suppressed {
                self.set_popovers_capture_transparent(true, defer_capture_input);
            } else {
                self.hide_popovers_for_window_hide();
            }
            self.capture_surface
                .set_transparent(presentation.capture_transparent);
            super::set_surface_input_enabled(&self.window, false);
            self.window.set_visible(false);
            if let Some(generation) = update.capture_suppression_generation {
                super::log_capture_surface_state(
                    generation,
                    super::CaptureSurfaceLog {
                        name: "top",
                        configured_visible: update.top_visible,
                        mapped_before_capture: self.mapped_before_capture,
                        presentation,
                        window: &self.window,
                        visual: &self.root,
                        capture_surface: &self.capture_surface,
                    },
                );
            }
            return false;
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
        if update.capture_suppressed {
            self.set_popovers_capture_transparent(true, defer_capture_input);
        } else {
            self.set_popovers_capture_transparent(false, false);
            self.sync_popovers(snapshot, &plan);
        }
        self.window.set_visible(true);
        self.capture_surface
            .set_transparent(presentation.capture_transparent);
        super::set_visual_hidden(
            &self.window,
            &self.root,
            GtkToolbarKind::Top,
            presentation.visual_hidden,
        );
        super::set_surface_input_enabled(
            &self.window,
            presentation.input_enabled || defer_capture_input,
        );
        if let Some(generation) = update.capture_suppression_generation {
            super::log_capture_surface_state(
                generation,
                super::CaptureSurfaceLog {
                    name: "top",
                    configured_visible: update.top_visible,
                    mapped_before_capture: self.mapped_before_capture,
                    presentation,
                    window: &self.window,
                    visual: &self.root,
                    capture_surface: &self.capture_surface,
                },
            );
        }
        true
    }

    pub(in crate::toolbar_gtk::view) fn capture_target(&self) -> CaptureProofTarget {
        CaptureProofTarget::new("top", &self.window, &self.capture_surface)
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
        let (left, x) = super::drag::rounded_margin_and_offset(self.base_x.get(), offsets.0);
        let (top, y) = super::drag::rounded_margin_and_offset(BASE_MARGIN.0 as f64, offsets.1);
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
        self.shapes_capture_surface = None;
        if let Some(popover) = self.overflow_popover.take() {
            popover.unparent();
        }
        self.overflow_capture_surface = None;
        self.shapes_content_key.set(None);
        self.overflow_content_key.set(None);
        while let Some(child) = self.root.first_child() {
            self.root.remove(&child);
        }
        self.updaters.borrow_mut().clear();

        if snapshot.top_minimized {
            self.build_minimized(snapshot, plan);
        } else {
            self.build_strip(snapshot, plan);
        }
    }
}
