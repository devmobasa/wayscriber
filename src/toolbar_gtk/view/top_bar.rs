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
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::backend::wayland::{TopStripPlan, plan_top_strip};
use crate::config::{Action, ToolbarLayoutMode, action_label, action_short_label};
use crate::input::Tool;
use crate::label_format::format_binding_label;
use crate::toolbar_icons;
use crate::ui::toolbar::bindings::{tool_label, tool_tooltip_label};
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot, model};

use super::super::GtkToolbarFeedback;
use super::super::icons::{IconWidget, tool_icon_painter};
use super::super::widgets::{
    FeedbackSender, SwatchButton, icon_button, send_event, set_active_class, sized_button,
    text_button,
};

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

use super::Updater;

/// Everything needed to render one utility button: painter, short label,
/// tooltip, click event, and the optional active-state probe.
type UtilitySpec = (
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
    drag_active: Rc<Cell<bool>>,
    offsets: Rc<Cell<(f64, f64)>>,
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
        window.set_keyboard_mode(KeyboardMode::OnDemand);

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
            drag_active: Rc::new(Cell::new(false)),
            offsets: Rc::new(Cell::new((0.0, 0.0))),
        }
    }

    pub(in crate::toolbar_gtk) fn apply(
        &mut self,
        snapshot: &ToolbarSnapshot,
        visible: bool,
        offsets: (f64, f64),
    ) {
        if !visible {
            self.window.set_visible(false);
            return;
        }
        self.apply_offsets(offsets);

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
    /// flight (the drag owns the margins until it ends).
    fn apply_offsets(&self, offsets: (f64, f64)) {
        if self.drag_active.get() {
            return;
        }
        self.offsets.set(offsets);
        self.window
            .set_margin(Edge::Left, BASE_MARGIN.1 + offsets.0.round() as i32);
        self.window
            .set_margin(Edge::Top, BASE_MARGIN.0 + offsets.1.round() as i32);
    }

    fn rebuild(&mut self, snapshot: &ToolbarSnapshot, plan: &TopStripPlan) {
        while let Some(child) = self.root.first_child() {
            self.root.remove(&child);
        }
        self.updaters.borrow_mut().clear();
        self.shapes_popover = None;
        self.overflow_popover = None;

        if snapshot.top_minimized {
            self.build_minimized(snapshot);
        } else {
            self.build_strip(snapshot, plan);
        }
    }

    fn build_minimized(&mut self, snapshot: &ToolbarSnapshot) {
        let scale = effective_scale(snapshot);
        let restore = sized_button(MINIMIZED_SIZE.0 * scale, MINIMIZED_SIZE.1 * scale);
        restore.add_css_class("chrome");
        restore.set_tooltip_text(Some("Show toolbar"));
        let icon = IconWidget::new(
            toolbar_icons::draw_icon_chevron_down,
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
            let grip = IconWidget::new(toolbar_icons::draw_icon_grip_bars, sz(HANDLE_SIZE));
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
            for (index, entry) in snapshot
                .quick_colors
                .rendered_entries()
                .iter()
                .take(plan.swatch_count)
                .enumerate()
            {
                let entry_color = entry.color;
                let swatch = SwatchButton::new(
                    entry_color,
                    entry_color == snapshot.color,
                    sz(SWATCH_SIZE),
                    &entry.label,
                );
                let sender = self.feedback.clone();
                swatch.button.connect_clicked(move |_| {
                    send_event(&sender, ToolbarEvent::SetColor(entry_color));
                });
                let is_last = index + 1
                    == plan
                        .swatch_count
                        .min(snapshot.quick_colors.rendered_entries().len());
                append_gap(
                    &bar,
                    swatch.button.as_ref(),
                    if is_last { gap } else { SWATCH_GAP },
                );
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
    ) -> gtk4::Button {
        let tooltip = tool_tooltip(snapshot, tool);
        let button = if use_icons {
            icon_button(tool_icon_painter(tool), button_size, icon_size, &tooltip).button
        } else {
            text_button(tool_label(tool), button_size, &tooltip)
        };
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

    /// Shapes picker button: face shows the last-used shape; the popover
    /// carries the grid and per-tool option rows.
    fn shapes_picker_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
    ) -> gtk4::Button {
        let face_tool = model::current_shape_tool(snapshot.active_tool, snapshot.tool_override)
            .unwrap_or_else(model::default_shape_tool);
        let button = if use_icons {
            let icon = icon_button(
                tool_icon_painter(face_tool),
                button_size,
                icon_size,
                "Shapes",
            );
            let icon_handle = icon.icon.clone();
            self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                let face = model::current_shape_tool(snapshot.active_tool, snapshot.tool_override)
                    .unwrap_or_else(model::default_shape_tool);
                icon_handle.set_painter(tool_icon_painter(face));
            }));
            icon.button
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
        popover.set_autohide(true);
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
    ) -> Option<gtk4::Button> {
        let (painter, label, tooltip, event, active): UtilitySpec = match utility {
            model::TopUtilityButton::Text => (
                toolbar_icons::draw_icon_text,
                action_short_label(Action::EnterTextMode),
                action_tooltip(snapshot, Action::EnterTextMode),
                ToolbarEvent::EnterTextMode,
                Some(|snapshot| snapshot.text_active),
            ),
            model::TopUtilityButton::StickyNote => (
                toolbar_icons::draw_icon_note,
                action_short_label(Action::EnterStickyNoteMode),
                action_tooltip(snapshot, Action::EnterStickyNoteMode),
                ToolbarEvent::EnterStickyNoteMode,
                Some(|snapshot| snapshot.note_active),
            ),
            model::TopUtilityButton::Screenshot => (
                toolbar_icons::draw_icon_screenshot,
                "Shot",
                action_tooltip(snapshot, Action::CaptureSelection),
                ToolbarEvent::CaptureScreenshot,
                None,
            ),
            model::TopUtilityButton::Highlight => (
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
    ) -> gtk4::Button {
        let tooltip = action_tooltip(snapshot, action);
        let button = if use_icons {
            icon_button(painter, button_size, icon_size, &tooltip).button
        } else {
            text_button(action_short_label(action), button_size, &tooltip)
        };
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
                toolbar_icons::draw_icon_pin_filled
            } else {
                toolbar_icons::draw_icon_pin_outline
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
                toolbar_icons::draw_icon_pin_filled
            } else {
                toolbar_icons::draw_icon_pin_outline
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
        popover.set_autohide(true);
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
        let icon = IconWidget::new(toolbar_icons::draw_icon_dash, size * 0.6);
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
                popover.set_child(Some(&self.build_shapes_popover_content(
                    snapshot,
                    button_size,
                    icon_size,
                    use_icons,
                    scale,
                )));
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
                popover.set_child(Some(&self.build_overflow_popover_content(
                    snapshot,
                    plan,
                    button_size,
                    icon_size,
                    use_icons,
                    scale,
                )));
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
            fill.set_active(snapshot.fill_enabled);
            let sender = self.feedback.clone();
            let fill_enabled = snapshot.fill_enabled;
            fill.connect_toggled(move |check| {
                if check.is_active() != fill_enabled {
                    send_event(&sender, ToolbarEvent::ToggleFill(check.is_active()));
                }
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
            let (painter, label, tooltip, event, active): (
                super::super::icons::IconPainter,
                &str,
                String,
                ToolbarEvent,
                bool,
            ) = match utility {
                model::TopUtilityButton::Text => (
                    toolbar_icons::draw_icon_text,
                    action_short_label(Action::EnterTextMode),
                    action_label(Action::EnterTextMode).to_string(),
                    ToolbarEvent::EnterTextMode,
                    snapshot.text_active,
                ),
                model::TopUtilityButton::StickyNote => (
                    toolbar_icons::draw_icon_note,
                    action_short_label(Action::EnterStickyNoteMode),
                    action_label(Action::EnterStickyNoteMode).to_string(),
                    ToolbarEvent::EnterStickyNoteMode,
                    snapshot.note_active,
                ),
                model::TopUtilityButton::Screenshot => (
                    toolbar_icons::draw_icon_screenshot,
                    "Shot",
                    action_label(Action::CaptureSelection).to_string(),
                    ToolbarEvent::CaptureScreenshot,
                    false,
                ),
                model::TopUtilityButton::Highlight => (
                    toolbar_icons::draw_icon_highlight,
                    "Highlight",
                    action_label(Action::ToggleHighlightTool).to_string(),
                    ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active),
                    snapshot.any_highlight_active,
                ),
                model::TopUtilityButton::ClearCanvas | model::TopUtilityButton::IconMode => {
                    continue;
                }
            };
            let button = if use_icons {
                icon_button(painter, button_size, icon_size, &tooltip).button
            } else {
                text_button(label, button_size, &tooltip)
            };
            set_active_class(&button, active);
            let sender = self.feedback.clone();
            button.connect_clicked(move |_| {
                send_event(&sender, event.clone());
            });
            attach(&button);
        }
        grid
    }

    /// Drag-to-move: the grip moves the layer margins live and reports the
    /// offsets so the backend can clamp and persist them at drag end.
    fn attach_move_drag(&self, grip: &gtk4::DrawingArea) {
        let drag = gtk4::GestureDrag::new();
        let window = self.window.clone();
        let feedback = self.feedback.clone();
        let drag_active = self.drag_active.clone();
        let offsets = self.offsets.clone();
        let start = Rc::new(Cell::new((0.0f64, 0.0f64)));

        let begin_start = start.clone();
        let begin_offsets = offsets.clone();
        let begin_active = drag_active.clone();
        drag.connect_drag_begin(move |_, _, _| {
            begin_active.set(true);
            begin_start.set(begin_offsets.get());
        });

        let update_window = window.clone();
        let update_start = start.clone();
        let update_offsets = offsets.clone();
        let update_feedback = feedback.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let (sx, sy) = update_start.get();
            let x = (sx + dx).max(-(BASE_MARGIN.1 as f64));
            let y = (sy + dy).max(-(BASE_MARGIN.0 as f64));
            update_offsets.set((x, y));
            update_window.set_margin(Edge::Left, BASE_MARGIN.1 + x.round() as i32);
            update_window.set_margin(Edge::Top, BASE_MARGIN.0 + y.round() as i32);
            let _ = update_feedback.send(GtkToolbarFeedback::SetTopOffset { x, y, done: false });
        });

        drag.connect_drag_end(move |_, _, _| {
            drag_active.set(false);
            let (x, y) = offsets.get();
            let _ = feedback.send(GtkToolbarFeedback::SetTopOffset { x, y, done: true });
        });
        grip.add_controller(drag);
    }
}
