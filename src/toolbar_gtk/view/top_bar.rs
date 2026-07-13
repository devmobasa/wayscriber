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

/// Keep only the newest start-relative `GestureDrag` offset and apply it once
/// per compositor frame. The gesture-owning surface stays stationary, so the
/// offset remains in one stable coordinate space for the whole drag.
#[derive(Default)]
pub(super) struct FrameCoalescedDrag {
    next_generation: Cell<u64>,
    pending: RefCell<VecDeque<(u64, DragFrame)>>,
}

pub(super) struct DragFrame {
    pub(super) delta: (f64, f64),
    pub(super) phase: GtkToolbarDragPhase,
}

impl FrameCoalescedDrag {
    pub(super) fn begin(&self) -> u64 {
        let generation = self.next_generation.get().wrapping_add(1);
        self.next_generation.set(generation);
        generation
    }

    pub(super) fn update(&self, generation: u64, dx: f64, dy: f64) {
        let mut pending = self.pending.borrow_mut();
        if let Some((queued_generation, frame)) = pending.back_mut()
            && *queued_generation == generation
            && frame.phase != GtkToolbarDragPhase::End
        {
            frame.delta = (dx, dy);
            return;
        }
        pending.push_back((
            generation,
            DragFrame {
                delta: (dx, dy),
                phase: GtkToolbarDragPhase::Move,
            },
        ));
    }

    pub(super) fn end(&self, generation: u64, dx: f64, dy: f64) {
        let mut pending = self.pending.borrow_mut();
        if let Some((queued_generation, frame)) = pending.back_mut()
            && *queued_generation == generation
            && frame.phase != GtkToolbarDragPhase::End
        {
            frame.delta = (dx, dy);
            frame.phase = GtkToolbarDragPhase::End;
            return;
        }
        pending.push_back((
            generation,
            DragFrame {
                // Start-relative offsets are idempotent while the input
                // surface is parked, so replaying the final coordinate cannot
                // accumulate motion or produce a release jump.
                delta: (dx, dy),
                phase: GtkToolbarDragPhase::End,
            },
        ));
    }

    pub(super) fn take_frame(&self, generation: u64) -> Option<DragFrame> {
        let mut pending = self.pending.borrow_mut();
        let index = pending
            .iter()
            .position(|(queued_generation, _)| *queued_generation == generation)?;
        pending.remove(index).map(|(_, frame)| frame)
    }
}

pub(super) fn drag_frame_position(origin: (f64, f64), delta: (f64, f64)) -> (f64, f64) {
    (origin.0 + delta.0, origin.1 + delta.1)
}

/// Snapshot inputs the shapes-popover grid renders from: active tool,
/// override, fill flag, polygon sides.
type ShapesContentKey = (Tool, Option<Tool>, bool, u8);

/// Snapshot inputs the overflow grid renders from: active tool, override,
/// text/note/highlight active flags.
type OverflowContentKey = (Tool, Option<Tool>, bool, bool, bool);

/// Everything needed to render one utility button: painter, short label,
/// tooltip, click event, and the optional active-state probe.
type UtilitySpec = (
    Action,
    super::super::icons::IconPainter,
    &'static str,
    String,
    ToolbarEvent,
    Option<fn(&ToolbarSnapshot) -> bool>,
);

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

    fn build_minimized(&mut self, snapshot: &ToolbarSnapshot) {
        let scale = effective_scale(snapshot);
        // A GTK toplevel never shrinks on its own; reset the default size
        // or the tab keeps the full strip's width. The panel padding is
        // dropped so the tab hugs the 64x24 builtin footprint.
        self.window.set_default_size(
            (MINIMIZED_SIZE.0 * scale).round() as i32,
            (MINIMIZED_SIZE.1 * scale).round() as i32,
        );
        self.root.add_css_class("minimized");
        let restore = sized_button(MINIMIZED_SIZE.0 * scale, MINIMIZED_SIZE.1 * scale);
        restore.add_css_class("chrome");
        restore.set_tooltip_text(Some("Show toolbar"));
        let icon = IconWidget::new(
            toolbar_icons::draw_icon_restore,
            (MINIMIZED_SIZE.1 * 0.75 * scale).min(18.0 * scale),
        );
        restore.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        restore.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::SetTopMinimized(false));
        });
        self.root.append(&restore);
    }

    // The running spec-unit `x` mirrors the builtin builder walk; the last
    // increments are intentionally kept even where nothing reads them so
    // the two walks stay line-for-line comparable.
    #[allow(unused_assignments)]
    fn build_strip(&mut self, snapshot: &ToolbarSnapshot, plan: &TopStripPlan) {
        self.root.remove_css_class("minimized");
        // GTK toplevels retain their previous default width across widget-tree
        // rebuilds. Reset it from the shared natural-size calculation so a
        // narrower layout (notably `simple`) does not keep the regular strip's
        // empty trailing area. Height remains content-driven for GTK popovers.
        self.window
            .set_default_size(top_default_width(snapshot), -1);
        let scale = effective_scale(snapshot);
        let is_simple = snapshot.layout_mode == ToolbarLayoutMode::Simple;
        let use_icons = snapshot.use_icons || plan.compact;
        let gap = if plan.compact { COMPACT_GAP } else { GAP };
        let (btn_w, btn_h) = if plan.compact {
            (COMPACT_BUTTON, COMPACT_BUTTON)
        } else if use_icons {
            (ICON_BUTTON, ICON_BUTTON)
        } else {
            (TEXT_BUTTON_W, TEXT_BUTTON_H)
        };
        let sz = |value: f64| value * scale;
        let px = |value: f64| (value * scale).round() as i32;

        let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        bar.set_margin_start(px(if plan.compact {
            COMPACT_START_X
        } else {
            START_X
        }));
        bar.set_margin_end(0);
        self.root.append(&bar);

        // Running spec-unit x, mirroring the builtin builder walk; used to
        // align the contextual ring row under the Highlight button.
        let mut x = if plan.compact {
            COMPACT_START_X
        } else {
            START_X
        };
        let mut highlight_x: Option<f64> = None;

        let append_gap = |bar: &gtk4::Box, widget: &gtk4::Widget, gap_units: f64| {
            widget.set_margin_end(px(gap_units).max(0));
            bar.append(widget);
        };
        let push_divider = |bar: &gtk4::Box, x: &mut f64| {
            let span = if plan.compact { 3.0 } else { DIVIDER_SPAN };
            let divider = gtk4::Separator::new(gtk4::Orientation::Vertical);
            divider.set_margin_top(px(6.0));
            divider.set_margin_bottom(px(6.0));
            divider.set_margin_start(px((span - 1.0) / 2.0));
            divider.set_margin_end(px((span - 1.0) / 2.0) + px(gap));
            bar.append(&divider);
            *x += span + gap;
        };

        // --- Drag grip -----------------------------------------------------
        if model::toolbar_item_visible(snapshot, crate::config::toolbar_item_ids::TOP_CHROME_DRAG) {
            let grip = IconWidget::new(toolbar_icons::draw_icon_drag, sz(HANDLE_SIZE));
            grip.area.set_can_target(true);
            grip.area.add_css_class("drag-handle");
            grip.area.set_tooltip_text(Some("Drag toolbar"));
            grip.area.set_valign(gtk4::Align::Center);
            grip.area.set_cursor_from_name(Some("grab"));
            self.attach_move_drag(&grip.area);
            append_gap(&bar, grip.area.as_ref(), gap);
            x += HANDLE_SIZE + gap;
        }

        // --- Tool groups: pens | shapes --------------------------------------
        let mut previous_group: Option<model::TopToolGroup> = None;
        let mut tool_drawn = false;
        for tool in model::visible_top_tool_buttons(is_simple, snapshot) {
            if plan.dropped_tools.contains(&tool) {
                continue;
            }
            let group = model::top_tool_group(tool);
            if let Some(previous) = previous_group
                && previous != group
            {
                push_divider(&bar, &mut x);
            }
            previous_group = Some(group);
            let button = self.tool_button(
                snapshot,
                tool,
                (sz(btn_w), sz(btn_h)),
                sz(ICON_SIZE),
                use_icons,
                !plan.compact,
            );
            append_gap(&bar, button.as_ref(), gap);
            x += btn_w + gap;
            tool_drawn = true;
        }

        // --- Shapes picker ----------------------------------------------------
        if model::top_shape_picker_visible(snapshot) {
            if previous_group == Some(model::TopToolGroup::Pens) {
                push_divider(&bar, &mut x);
            }
            let button = self.shapes_picker_button(
                snapshot,
                (sz(btn_w), sz(btn_h)),
                sz(ICON_SIZE),
                use_icons,
            );
            append_gap(&bar, button.as_ref(), gap);
            x += btn_w + gap;
            tool_drawn = true;
        }

        // --- Annotation utilities (Clear pulled out below) --------------------
        let utilities: Vec<model::TopUtilityButton> =
            model::visible_top_utility_buttons(snapshot, is_simple, snapshot.use_icons)
                .into_iter()
                .filter(|button| *button != model::TopUtilityButton::ClearCanvas)
                .filter(|button| !plan.dropped_utilities.contains(button))
                .collect();
        let clear_visible =
            model::visible_top_utility_buttons(snapshot, is_simple, snapshot.use_icons)
                .contains(&model::TopUtilityButton::ClearCanvas);
        if !utilities.is_empty() && tool_drawn {
            push_divider(&bar, &mut x);
        }
        for utility in utilities {
            if utility == model::TopUtilityButton::Highlight {
                highlight_x = Some(x);
            }
            if let Some(button) = self.utility_button(
                snapshot,
                utility,
                (sz(btn_w), sz(btn_h)),
                sz(ICON_SIZE),
                use_icons,
                !plan.compact,
            ) {
                append_gap(&bar, button.as_ref(), gap);
                x += btn_w + gap;
            }
        }

        // --- Quick colors + current-color chip --------------------------------
        if model::toolbar_item_visible(
            snapshot,
            crate::config::toolbar_item_ids::TOP_GROUP_QUICK_COLORS,
        ) {
            push_divider(&bar, &mut x);
            let show_swatch_badge_row = !plan.compact
                && snapshot
                    .quick_colors
                    .rendered_entries()
                    .iter()
                    .take(plan.swatch_count)
                    .enumerate()
                    .any(|(index, _)| snapshot.binding_hints.quick_color_badge(index).is_some());
            for (index, entry) in snapshot
                .quick_colors
                .rendered_entries()
                .iter()
                .take(plan.swatch_count)
                .enumerate()
            {
                let entry_color = entry.color;
                let action = crate::config::QuickColorPalette::action_for_index(index);
                let binding =
                    action.and_then(|action| snapshot.binding_hints.binding_for_action(action));
                let tooltip = format_binding_label(&entry.label, binding);
                let swatch = SwatchButton::new(
                    entry_color,
                    entry_color == snapshot.color,
                    sz(SWATCH_SIZE),
                    &tooltip,
                );
                let sender = self.feedback.clone();
                swatch.button.connect_clicked(move |_| {
                    send_event(
                        &sender,
                        ToolbarEvent::SetQuickColor {
                            color: entry_color,
                            action,
                        },
                    );
                });
                let is_last = index + 1
                    == plan
                        .swatch_count
                        .min(snapshot.quick_colors.rendered_entries().len());
                let badge = (!plan.compact)
                    .then(|| snapshot.binding_hints.quick_color_badge(index))
                    .flatten();
                let swatch_root = if !show_swatch_badge_row {
                    swatch.button.clone().upcast()
                } else {
                    swatch_with_shortcut(&swatch.button, badge, sz(SWATCH_SIZE), sz(10.0))
                };
                append_gap(&bar, &swatch_root, if is_last { gap } else { SWATCH_GAP });
                x += SWATCH_SIZE + if is_last { gap } else { SWATCH_GAP };
                self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                    swatch.set_selected(entry_color == snapshot.color);
                }));
            }
            let chip = SwatchButton::new(snapshot.color, true, sz(CHIP_SIZE), "Color picker");
            let sender = self.feedback.clone();
            chip.button.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::OpenColorPickerPopup);
            });
            append_gap(&bar, chip.button.as_ref(), gap);
            x += CHIP_SIZE + gap;
            self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                chip.set_color(snapshot.color);
            }));
        }

        // --- History -----------------------------------------------------------
        let undo_visible = model::toolbar_item_visible(
            snapshot,
            crate::config::toolbar_item_ids::TOP_UTILITY_UNDO,
        );
        let redo_visible = model::toolbar_item_visible(
            snapshot,
            crate::config::toolbar_item_ids::TOP_UTILITY_REDO,
        );
        if undo_visible || redo_visible {
            push_divider(&bar, &mut x);
        }
        if undo_visible {
            let button = self.history_button(
                snapshot,
                toolbar_icons::draw_icon_undo,
                Action::Undo,
                ToolbarEvent::Undo,
                |snapshot| snapshot.undo_available,
                (sz(btn_w), sz(btn_h)),
                sz(ICON_SIZE),
                use_icons,
                !plan.compact,
            );
            append_gap(&bar, button.as_ref(), gap);
            x += btn_w + gap;
        }
        if redo_visible {
            let button = self.history_button(
                snapshot,
                toolbar_icons::draw_icon_redo,
                Action::Redo,
                ToolbarEvent::Redo,
                |snapshot| snapshot.redo_available,
                (sz(btn_w), sz(btn_h)),
                sz(ICON_SIZE),
                use_icons,
                !plan.compact,
            );
            append_gap(&bar, button.as_ref(), gap);
            x += btn_w + gap;
        }

        // --- Destructive Clear, isolated by a double gap -------------------------
        if clear_visible {
            let button = if use_icons {
                let icon = icon_button(
                    toolbar_icons::draw_icon_clear,
                    (sz(btn_w), sz(btn_h)),
                    sz(ICON_SIZE),
                    &action_tooltip(snapshot, Action::ClearCanvas),
                );
                icon.button
            } else {
                text_button(
                    action_short_label(Action::ClearCanvas),
                    (sz(btn_w), sz(btn_h)),
                    &action_tooltip(snapshot, Action::ClearCanvas),
                )
            };
            if !plan.compact {
                add_shortcut_badge(
                    &button,
                    snapshot.binding_hints.badge_for_action(Action::ClearCanvas),
                );
            }
            button.add_css_class("destructive");
            button.set_margin_start(px(gap));
            let sender = self.feedback.clone();
            button.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::ClearCanvas);
            });
            append_gap(&bar, button.as_ref(), gap);
        }

        // --- Right-aligned chrome -----------------------------------------------
        let chrome_size = if plan.compact {
            COMPACT_CHROME
        } else {
            PIN_BUTTON_SIZE
        };
        let chrome_gap = if plan.compact {
            COMPACT_GAP
        } else {
            PIN_BUTTON_GAP
        };
        let chrome = gtk4::Box::new(gtk4::Orientation::Horizontal, px(chrome_gap));
        chrome.set_margin_end(px(if plan.compact {
            COMPACT_MARGIN_RIGHT
        } else {
            PIN_MARGIN_RIGHT
        }));
        chrome.set_valign(gtk4::Align::Center);
        if model::toolbar_item_visible(snapshot, crate::config::toolbar_item_ids::TOP_CHROME_PIN) {
            chrome.append(&self.pin_button(snapshot, sz(chrome_size)));
        }
        if plan.show_overflow {
            chrome.append(&self.overflow_button(snapshot, plan, sz(chrome_size), use_icons));
        }
        if model::toolbar_item_visible(snapshot, crate::config::toolbar_item_ids::TOP_CHROME_CLOSE)
        {
            chrome.append(&self.minimize_button(sz(chrome_size)));
        }
        bar.append(&chrome);

        // --- Contextual highlight ring row ----------------------------------------
        if ring_row_active(snapshot, plan)
            && let Some(ring_x) = highlight_x
        {
            let ring = gtk4::CheckButton::with_label("Ring");
            ring.add_css_class("mini");
            ring.set_tooltip_text(Some("Highlight ring"));
            ring.set_active(snapshot.highlight_tool_ring_enabled);
            ring.set_halign(gtk4::Align::Start);
            ring.set_margin_start(px(ring_x));
            ring.set_margin_top(px(2.0));
            let sender = self.feedback.clone();
            let syncing = Rc::new(Cell::new(false));
            let toggle_sync = syncing.clone();
            ring.connect_toggled(move |check| {
                if !toggle_sync.get() {
                    send_event(
                        &sender,
                        ToolbarEvent::ToggleHighlightToolRing(check.is_active()),
                    );
                }
            });
            let ring_handle = ring.clone();
            self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                if ring_handle.is_active() != snapshot.highlight_tool_ring_enabled {
                    syncing.set(true);
                    ring_handle.set_active(snapshot.highlight_tool_ring_enabled);
                    syncing.set(false);
                }
            }));
            self.root.append(&ring);
        }
    }

    fn tool_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        tool: Tool,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        show_badge: bool,
    ) -> gtk4::Button {
        let tooltip = tool_tooltip(snapshot, tool);
        let button = if use_icons {
            icon_button(tool_icon_painter(tool), button_size, icon_size, &tooltip).button
        } else {
            text_button(tool_label(tool), button_size, &tooltip)
        };
        if show_badge {
            add_shortcut_badge(&button, snapshot.binding_hints.badge_for_tool(tool));
        }
        let sender = self.feedback.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::SelectTool(tool));
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            let active = snapshot.active_tool == tool || snapshot.tool_override == Some(tool);
            set_active_class(&handle, active);
        }));
        button
    }

    /// Shapes picker button: the family icon opens the grid and per-tool
    /// option rows; individual shapes keep their own icons inside the popover.
    fn shapes_picker_button(
        &mut self,
        _snapshot: &ToolbarSnapshot,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
    ) -> gtk4::Button {
        let button = if use_icons {
            icon_button(
                toolbar_icons::draw_icon_shape_picker,
                button_size,
                icon_size,
                "Shapes",
            )
            .button
        } else {
            text_button("Shapes", button_size, "Shapes")
        };
        let sender = self.feedback.clone();
        let expected = self.shapes_expected_open.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::ToggleShapePicker(!expected.get()));
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            let active = snapshot.shape_picker_open
                || model::current_shape_tool(snapshot.active_tool, snapshot.tool_override)
                    .is_some();
            set_active_class(&handle, active);
        }));

        let popover = gtk4::Popover::new();
        popover.set_parent(&button);
        popover.set_position(gtk4::PositionType::Bottom);
        // No autohide grab: the built-in dismissal policy already closes
        // the picker through the snapshot round-trip (any other toolbar
        // event or a canvas press), so an outside click both dismisses AND
        // activates the control under it — one click, like the builtin.
        popover.set_autohide(false);
        attach_escape_dismiss(
            &popover,
            &self.feedback,
            ToolbarEvent::ToggleShapePicker(false),
        );
        let sender = self.feedback.clone();
        let expected = self.shapes_expected_open.clone();
        popover.connect_closed(move |_| {
            if expected.get() {
                send_event(&sender, ToolbarEvent::ToggleShapePicker(false));
            }
        });
        self.shapes_popover = Some(popover);
        button
    }

    fn utility_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        utility: model::TopUtilityButton,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        show_badge: bool,
    ) -> Option<gtk4::Button> {
        let (action, painter, label, tooltip, event, active): UtilitySpec = match utility {
            model::TopUtilityButton::Text => (
                Action::EnterTextMode,
                toolbar_icons::draw_icon_text,
                action_short_label(Action::EnterTextMode),
                action_tooltip(snapshot, Action::EnterTextMode),
                ToolbarEvent::EnterTextMode,
                Some(|snapshot| snapshot.text_active),
            ),
            model::TopUtilityButton::StickyNote => (
                Action::EnterStickyNoteMode,
                toolbar_icons::draw_icon_note,
                action_short_label(Action::EnterStickyNoteMode),
                action_tooltip(snapshot, Action::EnterStickyNoteMode),
                ToolbarEvent::EnterStickyNoteMode,
                Some(|snapshot| snapshot.note_active),
            ),
            model::TopUtilityButton::Screenshot => (
                Action::CaptureSelection,
                toolbar_icons::draw_icon_screenshot,
                "Shot",
                action_tooltip(snapshot, Action::CaptureSelection),
                ToolbarEvent::CaptureScreenshot,
                None,
            ),
            model::TopUtilityButton::Highlight => (
                Action::ToggleHighlightTool,
                toolbar_icons::draw_icon_highlight,
                "Highlight",
                action_tooltip(snapshot, Action::ToggleHighlightTool),
                // The click handler recomputes the toggle from live state.
                ToolbarEvent::ToggleAllHighlight(true),
                Some(|snapshot| snapshot.any_highlight_active),
            ),
            model::TopUtilityButton::ClearCanvas | model::TopUtilityButton::IconMode => {
                return None;
            }
        };
        let button = if use_icons {
            icon_button(painter, button_size, icon_size, &tooltip).button
        } else {
            text_button(label, button_size, &tooltip)
        };
        if show_badge {
            add_shortcut_badge(&button, snapshot.binding_hints.badge_for_action(action));
        }
        let sender = self.feedback.clone();
        if utility == model::TopUtilityButton::Highlight {
            // Highlight toggles off the *current* state rather than firing a
            // fixed event.
            let active_state = Rc::new(Cell::new(snapshot.any_highlight_active));
            let click_state = active_state.clone();
            button.connect_clicked(move |_| {
                send_event(
                    &sender,
                    ToolbarEvent::ToggleAllHighlight(!click_state.get()),
                );
            });
            let handle = button.clone();
            self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                active_state.set(snapshot.any_highlight_active);
                set_active_class(&handle, snapshot.any_highlight_active);
            }));
        } else {
            button.connect_clicked(move |_| {
                send_event(&sender, event.clone());
            });
            if let Some(is_active) = active {
                let handle = button.clone();
                self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                    set_active_class(&handle, is_active(snapshot));
                }));
            }
        }
        Some(button)
    }

    #[allow(clippy::too_many_arguments)]
    fn history_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        painter: super::super::icons::IconPainter,
        action: Action,
        event: ToolbarEvent,
        available: fn(&ToolbarSnapshot) -> bool,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        show_badge: bool,
    ) -> gtk4::Button {
        let tooltip = action_tooltip(snapshot, action);
        let button = if use_icons {
            icon_button(painter, button_size, icon_size, &tooltip).button
        } else {
            text_button(action_short_label(action), button_size, &tooltip)
        };
        if show_badge {
            add_shortcut_badge(&button, snapshot.binding_hints.badge_for_action(action));
        }
        let sender = self.feedback.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            handle.set_sensitive(available(snapshot));
        }));
        button
    }

    fn pin_button(&mut self, snapshot: &ToolbarSnapshot, size: f64) -> gtk4::Button {
        let button = sized_button(size, size);
        button.add_css_class("chrome");
        let icon = IconWidget::new(
            if snapshot.top_pinned {
                toolbar_icons::draw_icon_pin
            } else {
                toolbar_icons::draw_icon_unpin
            },
            size * 0.62,
        );
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        let pinned = Rc::new(Cell::new(snapshot.top_pinned));
        let click_pinned = pinned.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::PinTopToolbar(!click_pinned.get()));
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            pinned.set(snapshot.top_pinned);
            icon.set_painter(if snapshot.top_pinned {
                toolbar_icons::draw_icon_pin
            } else {
                toolbar_icons::draw_icon_unpin
            });
            if snapshot.top_pinned {
                handle.add_css_class("pinned");
                handle.set_tooltip_text(Some("Pinned: opens at startup (click to disable)"));
            } else {
                handle.remove_css_class("pinned");
                handle.set_tooltip_text(Some("Pin: click to open at startup"));
            }
        }));
        button
    }

    fn overflow_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        _plan: &TopStripPlan,
        size: f64,
        _use_icons: bool,
    ) -> gtk4::Button {
        let button = sized_button(size, size);
        button.add_css_class("chrome");
        button.set_tooltip_text(Some("More tools"));
        let icon = IconWidget::new(toolbar_icons::draw_icon_more, size * 0.7);
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        let expected = self.overflow_expected_open.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::ToggleTopOverflow(!expected.get()));
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            set_active_class(&handle, snapshot.top_overflow_open);
        }));
        let _ = snapshot;

        let popover = gtk4::Popover::new();
        popover.set_parent(&button);
        popover.set_position(gtk4::PositionType::Bottom);
        // See the shapes popover: dismissal stays with the backend policy.
        popover.set_autohide(false);
        attach_escape_dismiss(
            &popover,
            &self.feedback,
            ToolbarEvent::ToggleTopOverflow(false),
        );
        let sender = self.feedback.clone();
        let expected = self.overflow_expected_open.clone();
        popover.connect_closed(move |_| {
            if expected.get() {
                send_event(&sender, ToolbarEvent::ToggleTopOverflow(false));
            }
        });
        self.overflow_popover = Some(popover);
        button
    }

    fn minimize_button(&mut self, size: f64) -> gtk4::Button {
        let button = sized_button(size, size);
        button.add_css_class("chrome");
        button.add_css_class("minimize");
        button.set_tooltip_text(Some("Minimize (leaves a restore tab)"));
        let icon = IconWidget::new(toolbar_icons::draw_icon_minimize, size * 0.6);
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::SetTopMinimized(true));
        });
        button
    }

    /// Keep the popovers' contents and open state in line with the
    /// snapshot. Contents are rebuilt each pass (they are small and this
    /// keeps the option rows context-correct); open state only changes
    /// when the snapshot flag differs from what the popover shows.
    fn sync_popovers(&mut self, snapshot: &ToolbarSnapshot, plan: &TopStripPlan) {
        let scale = effective_scale(snapshot);
        let use_icons = snapshot.use_icons || plan.compact;
        let (btn_w, btn_h) = if plan.compact {
            (COMPACT_BUTTON, COMPACT_BUTTON)
        } else if use_icons {
            (ICON_BUTTON, ICON_BUTTON)
        } else {
            (TEXT_BUTTON_W, TEXT_BUTTON_H)
        };
        let button_size = (btn_w * scale, btn_h * scale);
        let icon_size = ICON_SIZE * scale;

        if let Some(popover) = self.shapes_popover.clone() {
            let open = snapshot.shape_picker_open && model::top_shape_picker_visible(snapshot);
            if open {
                // Only rebuild the grid when its inputs changed; a rebuild
                // resets hover and cancels an in-flight press.
                let content_key = (
                    snapshot.active_tool,
                    snapshot.tool_override,
                    snapshot.fill_enabled,
                    snapshot.polygon_sides,
                );
                if self.shapes_content_key.get() != Some(content_key) {
                    popover.set_child(Some(&self.build_shapes_popover_content(
                        snapshot,
                        button_size,
                        icon_size,
                        use_icons,
                        scale,
                    )));
                    self.shapes_content_key.set(Some(content_key));
                }
            }
            self.shapes_expected_open.set(open);
            if open && !popover.is_visible() {
                popover.popup();
            } else if !open && popover.is_visible() {
                popover.popdown();
            }
        }

        if let Some(popover) = self.overflow_popover.clone() {
            let open = snapshot.top_overflow_open
                && plan.dropped_tools.len() + plan.dropped_utilities.len() > 0;
            if open {
                let content_key = (
                    snapshot.active_tool,
                    snapshot.tool_override,
                    snapshot.text_active,
                    snapshot.note_active,
                    snapshot.any_highlight_active,
                );
                if self.overflow_content_key.get() != Some(content_key) {
                    popover.set_child(Some(&self.build_overflow_popover_content(
                        snapshot,
                        plan,
                        button_size,
                        icon_size,
                        use_icons,
                        scale,
                    )));
                    self.overflow_content_key.set(Some(content_key));
                }
            }
            self.overflow_expected_open.set(open);
            if open && !popover.is_visible() {
                popover.popup();
            } else if !open && popover.is_visible() {
                popover.popdown();
            }
        } else {
            self.overflow_expected_open.set(false);
        }
    }

    fn build_shapes_popover_content(
        &self,
        snapshot: &ToolbarSnapshot,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        scale: f64,
    ) -> gtk4::Box {
        let is_simple = snapshot.layout_mode == ToolbarLayoutMode::Simple;
        let gap = (GAP * scale).round() as i32;
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, gap);

        for row in model::visible_shape_picker_rows(snapshot, is_simple) {
            let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, gap);
            for tool in row {
                if !model::tool_visible(snapshot, tool) {
                    continue;
                }
                let tooltip = tool_tooltip(snapshot, tool);
                let button = if use_icons {
                    icon_button(tool_icon_painter(tool), button_size, icon_size, &tooltip).button
                } else {
                    text_button(tool_label(tool), button_size, &tooltip)
                };
                add_shortcut_badge(&button, snapshot.binding_hints.badge_for_tool(tool));
                set_active_class(
                    &button,
                    snapshot.active_tool == tool || snapshot.tool_override == Some(tool),
                );
                let sender = self.feedback.clone();
                button.connect_clicked(move |_| {
                    send_event(&sender, ToolbarEvent::SelectTool(tool));
                });
                row_box.append(&button);
            }
            content.append(&row_box);
        }

        // Option rows: Fill and polygon sides live inside the popover, so
        // using them must not close it (GTK popovers keep inside clicks).
        let fill_tool_active =
            model::fill_tool_active(snapshot.active_tool, snapshot.tool_override);
        if fill_tool_active && model::top_fill_visible(snapshot) {
            let fill = gtk4::CheckButton::with_label(action_short_label(Action::ToggleFill));
            fill.set_tooltip_text(Some(&action_tooltip(snapshot, Action::ToggleFill)));
            // set_active runs before connect_toggled, so every later
            // toggle is user input and forwards unconditionally.
            fill.set_active(snapshot.fill_enabled);
            let sender = self.feedback.clone();
            fill.connect_toggled(move |check| {
                send_event(&sender, ToolbarEvent::ToggleFill(check.is_active()));
            });
            content.append(&fill);
        }
        if snapshot.active_tool == Tool::RegularPolygon
            || snapshot.tool_override == Some(Tool::RegularPolygon)
        {
            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, gap);
            let side_button = (24.0 * scale, 24.0 * scale);
            let minus = text_button("−", side_button, "Fewer sides");
            let sender = self.feedback.clone();
            minus.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::NudgePolygonSides(-1));
            });
            let label = gtk4::Label::new(Some(&format!("{} sides", snapshot.polygon_sides)));
            label.set_hexpand(true);
            let plus = text_button("+", side_button, "More sides");
            let sender = self.feedback.clone();
            plus.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::NudgePolygonSides(1));
            });
            row.append(&minus);
            row.append(&label);
            row.append(&plus);
            row.set_size_request((160.0 * scale).round() as i32, -1);
            content.append(&row);
        }
        content
    }

    #[allow(clippy::too_many_arguments)]
    fn build_overflow_popover_content(
        &self,
        snapshot: &ToolbarSnapshot,
        plan: &TopStripPlan,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        scale: f64,
    ) -> gtk4::Grid {
        let gap = (GAP * scale).round() as i32;
        let grid = gtk4::Grid::new();
        grid.set_row_spacing(gap as u32);
        grid.set_column_spacing(gap as u32);
        let dropped_count = plan.dropped_tools.len() + plan.dropped_utilities.len();
        let cols = dropped_count.clamp(1, 5) as i32;
        let mut index = 0i32;
        let mut attach = |widget: &gtk4::Button| {
            grid.attach(widget, index % cols, index / cols, 1, 1);
            index += 1;
        };
        for tool in &plan.dropped_tools {
            let tool = *tool;
            let tooltip = tool_tooltip(snapshot, tool);
            let button = if use_icons {
                icon_button(tool_icon_painter(tool), button_size, icon_size, &tooltip).button
            } else {
                text_button(tool_label(tool), button_size, &tooltip)
            };
            add_shortcut_badge(&button, snapshot.binding_hints.badge_for_tool(tool));
            set_active_class(
                &button,
                snapshot.active_tool == tool || snapshot.tool_override == Some(tool),
            );
            let sender = self.feedback.clone();
            button.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::SelectTool(tool));
            });
            attach(&button);
        }
        for utility in &plan.dropped_utilities {
            let (action, painter, label, event, active): (
                Action,
                super::super::icons::IconPainter,
                &str,
                ToolbarEvent,
                bool,
            ) = match utility {
                model::TopUtilityButton::Text => (
                    Action::EnterTextMode,
                    toolbar_icons::draw_icon_text,
                    action_short_label(Action::EnterTextMode),
                    ToolbarEvent::EnterTextMode,
                    snapshot.text_active,
                ),
                model::TopUtilityButton::StickyNote => (
                    Action::EnterStickyNoteMode,
                    toolbar_icons::draw_icon_note,
                    action_short_label(Action::EnterStickyNoteMode),
                    ToolbarEvent::EnterStickyNoteMode,
                    snapshot.note_active,
                ),
                model::TopUtilityButton::Screenshot => (
                    Action::CaptureSelection,
                    toolbar_icons::draw_icon_screenshot,
                    "Shot",
                    ToolbarEvent::CaptureScreenshot,
                    false,
                ),
                model::TopUtilityButton::Highlight => (
                    Action::ToggleHighlightTool,
                    toolbar_icons::draw_icon_highlight,
                    "Highlight",
                    ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active),
                    snapshot.any_highlight_active,
                ),
                model::TopUtilityButton::ClearCanvas | model::TopUtilityButton::IconMode => {
                    continue;
                }
            };
            let tooltip = action_tooltip(snapshot, action);
            let button = if use_icons {
                icon_button(painter, button_size, icon_size, &tooltip).button
            } else {
                text_button(label, button_size, &tooltip)
            };
            add_shortcut_badge(&button, snapshot.binding_hints.badge_for_action(action));
            set_active_class(&button, active);
            let sender = self.feedback.clone();
            button.connect_clicked(move |_| {
                send_event(&sender, event.clone());
            });
            attach(&button);
        }
        grid
    }

    /// Drag-to-move: park the GTK input surface at its origin and let the main
    /// overlay render the moving preview. Moving this surface during the
    /// gesture changes GTK's local coordinate space and makes fast drags lag,
    /// overshoot, or reverse. The backend moves the transparent surface only
    /// after the gesture ends, then reveals it after the handoff delay.
    fn attach_move_drag(&self, grip: &gtk4::DrawingArea) {
        let drag = gtk4::GestureDrag::new();
        let window = self.window.clone();
        let feedback = self.feedback.clone();
        let drag_active = self.drag_active.clone();
        let offsets = self.offsets.clone();
        let base_x = self.base_x.clone();
        let seq = self.offset_seq.clone();
        let pending = Rc::new(FrameCoalescedDrag::default());
        let active_generation = Rc::new(Cell::new(0));
        let drag_origin = Rc::new(Cell::new((0.0, 0.0)));

        let begin_active = drag_active.clone();
        let begin_blocked = self.drag_blocked.clone();
        let begin_pending = pending.clone();
        let begin_generation = active_generation.clone();
        let begin_origin = drag_origin.clone();
        let frame_window = window.clone();
        let frame_offsets = offsets.clone();
        let frame_feedback = feedback.clone();
        let frame_base = base_x.clone();
        let frame_seq = seq.clone();
        drag.connect_drag_begin(move |gesture, _, _| {
            if begin_blocked.get() {
                gesture.set_state(gtk4::EventSequenceState::Denied);
                return;
            }
            let Some(grip) = gesture.widget() else {
                return;
            };
            begin_active.set(true);
            let generation = begin_pending.begin();
            begin_generation.set(generation);
            begin_origin.set(frame_offsets.get());
            frame_seq.set(frame_seq.get() + 1);
            let origin = begin_origin.get();
            let _ = frame_feedback.send(GtkToolbarFeedback::SetTopOffset {
                x: origin.0,
                y: origin.1,
                surface_size: crate::toolbar_gtk::GtkToolbarSurfaceSize::from_window(
                    &frame_window,
                ),
                seq: frame_seq.get(),
                phase: GtkToolbarDragPhase::Start,
            });

            let pending = begin_pending.clone();
            let window = frame_window.clone();
            let offsets = frame_offsets.clone();
            let feedback = frame_feedback.clone();
            let base_x = frame_base.clone();
            let seq = frame_seq.clone();
            let drag_active = begin_active.clone();
            let active_generation = begin_generation.clone();
            let drag_origin = begin_origin.clone();
            grip.add_tick_callback(move |_, _| {
                let Some(frame) = pending.take_frame(generation) else {
                    return gtk4::glib::ControlFlow::Continue;
                };
                let base = base_x.get();
                let (cx, cy) = offsets.get();
                let (mut x, mut y) = drag_frame_position(drag_origin.get(), frame.delta);
                (x, y) =
                    clamp_drag_offsets(&window, (x, y), (base, BASE_MARGIN.0 as f64), END_MARGIN);
                offsets.set((x, y));
                seq.set(seq.get() + 1);
                crate::toolbar_gtk::drag_debug_log(format!(
                    "top frame generation={generation} seq={} phase={:?} delta=({:.3},{:.3}) origin=({:.3},{:.3}) before=({cx:.3},{cy:.3}) preview=({x:.3},{y:.3}) parked_margin=({}, {}) size={}x{}",
                    seq.get(),
                    frame.phase,
                    frame.delta.0,
                    frame.delta.1,
                    drag_origin.get().0,
                    drag_origin.get().1,
                    window.margin(Edge::Left),
                    window.margin(Edge::Top),
                    window.width(),
                    window.height(),
                ));
                let _ = feedback.send(GtkToolbarFeedback::SetTopOffset {
                    x,
                    y,
                    surface_size: crate::toolbar_gtk::GtkToolbarSurfaceSize::from_window(&window),
                    seq: seq.get(),
                    phase: frame.phase,
                });
                if frame.phase.is_end() {
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
            let generation = update_generation.get();
            crate::toolbar_gtk::drag_debug_log(format!(
                "top raw generation={generation} start_relative=({dx:.3},{dy:.3})",
            ));
            update_pending.update(generation, dx, dy);
        });

        drag.connect_drag_end(move |_, dx, dy| {
            crate::toolbar_gtk::drag_debug_log(format!(
                "top end generation={} delta=({dx:.3},{dy:.3})",
                active_generation.get(),
            ));
            pending.end(active_generation.get(), dx, dy);
        });
        grip.add_controller(drag);
    }
}

/// Without an autohide grab, Escape needs explicit wiring to dismiss an
/// open popover through the backend state.
pub(super) fn attach_escape_dismiss(
    popover: &gtk4::Popover,
    feedback: &FeedbackSender,
    dismiss: ToolbarEvent,
) {
    let key = gtk4::EventControllerKey::new();
    let sender = feedback.clone();
    key.connect_key_pressed(move |_, keyval, _, _| {
        if keyval == gtk4::gdk::Key::Escape {
            send_event(&sender, dismiss.clone());
            return gtk4::glib::Propagation::Stop;
        }
        gtk4::glib::Propagation::Proceed
    });
    popover.add_controller(key);
}

/// Keep a dragged bar inside the same start/end margins enforced by the
/// backend when it persists the final offsets.
pub(super) fn clamp_drag_offsets(
    window: &gtk4::Window,
    (x, y): (f64, f64),
    (base_x, base_y): (f64, f64),
    (end_x, end_y): (f64, f64),
) -> (f64, f64) {
    if let Some(surface) = window.surface()
        && let Some(display) = gtk4::gdk::Display::default()
        && let Some(monitor) = display.monitor_at_surface(&surface)
    {
        let geometry = monitor.geometry();
        let (x, _, _) = crate::backend::wayland::clamp_floating_axis_offset(
            x,
            geometry.width() as f64,
            window.width() as f64,
            base_x,
            end_x,
        );
        let (y, _, _) = crate::backend::wayland::clamp_floating_axis_offset(
            y,
            geometry.height() as f64,
            window.height() as f64,
            base_y,
            end_y,
        );
        return (x, y);
    }
    (x.max(-base_x), y.max(-base_y))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::config::KeyBinding;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarBindingHints;

    #[test]
    fn drag_updates_are_coalesced_to_the_latest_start_relative_offset() {
        let drag = FrameCoalescedDrag::default();
        let first = drag.begin();
        drag.update(first, 2.0, 3.0);
        drag.update(first, 5.0, 7.0);

        let frame = drag.take_frame(first).expect("latest motion is pending");
        assert_eq!(frame.delta, (5.0, 7.0));
        assert_eq!(frame.phase, GtkToolbarDragPhase::Move);
        assert!(drag.take_frame(first).is_none());

        drag.end(first, 5.0, 7.0);
        let frame = drag.take_frame(first).expect("drag end is pending");
        assert_eq!(frame.delta, (5.0, 7.0));
        assert_eq!(frame.phase, GtkToolbarDragPhase::End);
    }

    #[test]
    fn rounded_offset_matches_the_integer_layer_margin() {
        assert_eq!(rounded_margin_and_offset(12.0, 3.6), (16, 4.0));
        assert_eq!(rounded_margin_and_offset(24.0, -24.0), (0, -24.0));
        assert_eq!(rounded_margin_and_offset(100.25, 4.4), (105, 4.75));
    }

    #[test]
    fn rapid_start_relative_updates_do_not_accumulate() {
        let origin = (100.0, 200.0);
        let first = drag_frame_position(origin, (25.0, 40.0));
        let second = drag_frame_position(origin, (80.0, 90.0));

        assert_eq!(first, (125.0, 240.0));
        assert_eq!(second, (180.0, 290.0));
    }

    #[test]
    fn consecutive_drags_keep_separate_final_frames() {
        let drag = FrameCoalescedDrag::default();
        let first = drag.begin();
        drag.update(first, 4.0, 6.0);
        drag.end(first, 4.0, 6.0);
        let second = drag.begin();
        drag.update(second, 1.0, 2.0);

        let first_frame = drag.take_frame(first).expect("first drag end is retained");
        assert_eq!(first_frame.delta, (4.0, 6.0));
        assert_eq!(first_frame.phase, GtkToolbarDragPhase::End);

        let second_frame = drag
            .take_frame(second)
            .expect("second drag motion is retained");
        assert_eq!(second_frame.delta, (1.0, 2.0));
        assert_eq!(second_frame.phase, GtkToolbarDragPhase::Move);
    }

    #[test]
    fn top_structure_rebuilds_when_current_shortcuts_change() {
        let mut state = make_test_input_state();
        let initial = ToolbarSnapshot::from_input_with_bindings(
            &state,
            ToolbarBindingHints::from_input_state(&state),
        );
        let initial_plan = plan_top_strip(&initial);
        let initial_key = StructureKey::of(&initial, &initial_plan);

        state.set_action_bindings(HashMap::from([(
            Action::SelectPenTool,
            vec![KeyBinding::parse("9").expect("binding")],
        )]));
        let changed = ToolbarSnapshot::from_input_with_bindings(
            &state,
            ToolbarBindingHints::from_input_state(&state),
        );
        let changed_plan = plan_top_strip(&changed);
        let changed_key = StructureKey::of(&changed, &changed_plan);

        assert!(initial_key != changed_key);
        assert_eq!(changed.binding_hints.badge_for_tool(Tool::Pen), Some("9"));
    }

    #[test]
    fn simple_layout_requests_its_smaller_natural_width() {
        let mut state = make_test_input_state();
        state.toolbar_use_icons = true;
        let regular = ToolbarSnapshot::from_input_with_bindings(
            &state,
            ToolbarBindingHints::from_input_state(&state),
        );

        state.toolbar_layout_mode = ToolbarLayoutMode::Simple;
        let simple = ToolbarSnapshot::from_input_with_bindings(
            &state,
            ToolbarBindingHints::from_input_state(&state),
        );

        let regular_width = top_default_width(&regular);
        let simple_width = top_default_width(&simple);
        assert!(simple_width < regular_width);
        assert_eq!(simple_width, top_toolbar_size(&simple).0 as i32);
    }

    #[test]
    fn degraded_layout_requests_the_selected_plan_width() {
        let state = make_test_input_state();
        let mut snapshot = ToolbarSnapshot::from_input_with_bindings(
            &state,
            ToolbarBindingHints::from_input_state(&state),
        );
        snapshot.top_viewport_max = Some(700.0);

        let plan = plan_top_strip(&snapshot);
        assert!(plan.compact || plan.show_overflow || plan.swatch_count < 8);
        assert!(top_default_width(&snapshot) <= 700);
    }
}
