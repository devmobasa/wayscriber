//! The GTK top strip.
//!
//! Adapts the shared `TopToolbarSpec` into four detached pill islands —
//! tools (drag grip | pens | shapes | shapes-picker | annotations | quick
//! colors + chip), history (undo/redo + the overflow toggle anchoring Clear
//! and width-dropped items), chrome (pin/minimize), and the contextual
//! style pill (island D, `style_pill` module, from `StylePillSpec`) — as
//! GTK widgets. Width degradation uses the same shared plan as the
//! built-in frontend.
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
mod style_pill;
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
    FeedbackSender, SliderRow, SwatchButton, add_button_shortcut_hint, icon_button,
    install_click_modifier_capture, install_shortcut_focus_policy, send_event, set_active_class,
    sized_button, text_button,
};
use super::super::{GtkToolbarDragPhase, GtkToolbarFeedback, GtkToolbarKind};

// Spec-unit design tokens mirrored from the built-in layout
// (`layout/spec/top.rs`); the plan/natural-width math reuses the builtin
// functions directly, these only size the GTK widgets.
const GAP: f64 = 5.0;
/// Clear space between the detached pill islands
/// (`ToolbarLayoutSpec::TOP_ISLAND_GAP`).
const ISLAND_GAP: f64 = 10.0;
const COMPACT_ISLAND_GAP: f64 = 4.0;
const HANDLE_SIZE: f64 = 18.0;
const ICON_BUTTON: f64 = 46.0;
const ICON_SIZE: f64 = 28.0;
const TEXT_BUTTON_W: f64 = 60.0;
const TEXT_BUTTON_H: f64 = 36.0;
const PIN_BUTTON_SIZE: f64 = 24.0;
const PIN_BUTTON_GAP: f64 = 6.0;
const DIVIDER_SPAN: f64 = 7.0;
const SWATCH_SIZE: f64 = 22.0;
const SWATCH_GAP: f64 = 4.0;
const CHIP_SIZE: f64 = 28.0;
const COMPACT_BUTTON: f64 = 26.0;
const COMPACT_GAP: f64 = 1.0;
const COMPACT_CHROME: f64 = 18.0;
const MINIMIZED_SIZE: (f64, f64) = (64.0, 24.0);
/// Micro-mode chip size (`ToolbarLayoutSpec::TOP_MICRO_SIZE`).
const MICRO_SIZE: f64 = 44.0;
/// Style pill spec-unit tokens (`ToolbarLayoutSpec::TOP_STYLE_*`).
const STYLE_PILL_GAP: f64 = 6.0;
const STYLE_SLIDER_W: f64 = 110.0;
const STYLE_VALUE_W: f64 = 44.0;
const STYLE_ROW_H: f64 = 24.0;
/// `ToolbarLayoutSpec::TOP_STYLE_SEL_VALUE_W`.
const STYLE_SEL_VALUE_W: f64 = 64.0;
/// `ToolbarLayoutSpec::TOP_STYLE_STEP_W`.
const STYLE_STEP_W: f64 = 20.0;
/// Segment tab height (matches the Settings pane's segmented tabs).
const STYLE_TAB_H: f64 = 22.0;
/// Extra clear gap before a segmented control in the pill (M7-C3), on top of
/// the standard control gap, sharing `theme::toolbar::SEGMENT_LEADING_GAP`.
const STYLE_SEGMENT_LEAD: f64 = crate::ui::theme::toolbar::SEGMENT_LEADING_GAP;
const BASE_MARGIN: (i32, i32) = (12, 12);
const END_MARGIN: (f64, f64) = (12.0, 0.0);

use super::{CaptureProofTarget, CaptureSurfaceContent, Updater};

/// Snapshot inputs the shapes-popover grid renders from: active tool,
/// override, fill flag, polygon sides.
type ShapesContentKey = (Tool, Option<Tool>, bool, u8);

/// Snapshot inputs the overflow grid renders from: active tool, override,
/// text/note/highlight active flags, and the popover open flags (entry
/// active states).
type OverflowContentKey = (Tool, Option<Tool>, bool, bool, bool, bool, bool, bool);

/// Snapshot inputs the Canvas popover content renders from — the Boards /
/// Pages / Advanced / Zoom command sections plus the Step Undo/Redo config
/// are a pure function of these (per-button hidden overrides live in
/// `StructureKey.items`, which rebuilds the whole bar). Enabled states and
/// glyph faces that shift without a structure rebuild are captured so the
/// popover stays fresh.
///
/// The four delay-slider *values* (`custom_undo_delay_ms`,
/// `custom_redo_delay_ms`, `undo_all_delay_ms`, `redo_all_delay_ms`) are
/// deliberately NOT in this key: each slider emits continuously during a drag,
/// so keying the content on its value would rebuild the popover subtree on the
/// first backend echo, destroying the live gesture and resetting the scroll
/// position. They ride the persistent `canvas_updaters` instead (like the
/// strip's thickness/opacity/font-size sliders), which set the value in place
/// and are a no-op while the slider is mid-drag. The step *counts* stay in the
/// key: they change through discrete −/+ clicks, never a drag, so a rebuild is
/// harmless there.
#[derive(PartialEq)]
struct CanvasMenuContentKey {
    use_icons: bool,
    show_boards_section: bool,
    show_pages_section: bool,
    show_zoom_actions: bool,
    show_actions_advanced: bool,
    show_step_section: bool,
    board_count: usize,
    is_transparent: bool,
    page_index: usize,
    page_count: usize,
    zoom_active: bool,
    zoom_locked: bool,
    frozen_active: bool,
    undo_available: bool,
    redo_available: bool,
    delay_actions_enabled: bool,
    custom_section_enabled: bool,
    show_delay_sliders: bool,
    custom_undo_steps: usize,
    custom_redo_steps: usize,
}

impl CanvasMenuContentKey {
    fn of(snapshot: &ToolbarSnapshot) -> Self {
        Self {
            use_icons: snapshot.use_icons,
            show_boards_section: snapshot.show_boards_section,
            show_pages_section: snapshot.show_pages_section,
            show_zoom_actions: snapshot.show_zoom_actions,
            show_actions_advanced: snapshot.show_actions_advanced,
            show_step_section: snapshot.show_step_section,
            board_count: snapshot.board_count,
            is_transparent: snapshot.is_transparent,
            page_index: snapshot.page_index,
            page_count: snapshot.page_count,
            zoom_active: snapshot.zoom_active,
            zoom_locked: snapshot.zoom_locked,
            frozen_active: snapshot.frozen_active,
            undo_available: snapshot.undo_available,
            redo_available: snapshot.redo_available,
            delay_actions_enabled: snapshot.delay_actions_enabled,
            custom_section_enabled: snapshot.custom_section_enabled,
            show_delay_sliders: snapshot.show_delay_sliders,
            custom_undo_steps: snapshot.custom_undo_steps,
            custom_redo_steps: snapshot.custom_redo_steps,
        }
    }
}

/// Snapshot inputs the Session popover content renders from — the session
/// model is a pure function of these (plus structural inputs already in
/// `StructureKey`). `use_icons` is captured directly: the pane's action
/// buttons render icon+label in icon mode, and `StructureKey.use_icons`
/// (`use_icons || plan.compact`) can mask a flip while the plan is compact.
#[derive(PartialEq)]
struct SessionMenuContentKey {
    active_session_name: Option<String>,
    active_session_path: Option<std::path::PathBuf>,
    recent_sessions: Vec<crate::ui::toolbar::SessionRecentSnapshot>,
    pending_save_as_overwrite_path: Option<std::path::PathBuf>,
    use_icons: bool,
}

impl SessionMenuContentKey {
    fn of(snapshot: &ToolbarSnapshot) -> Self {
        Self {
            active_session_name: snapshot.active_session_name.clone(),
            active_session_path: snapshot.active_session_path.clone(),
            recent_sessions: snapshot.recent_sessions.clone(),
            pending_save_as_overwrite_path: snapshot.pending_save_as_overwrite_path.clone(),
            use_icons: snapshot.use_icons,
        }
    }
}

/// Snapshot inputs the Settings popover content renders from — the settings
/// model reads these toggle/customization fields on top of the structural
/// inputs already captured by `StructureKey`.
#[derive(PartialEq)]
struct SettingsMenuContentKey {
    /// Drives the "Icon buttons" checkbox state and its baked
    /// `ToggleIconMode(!use_icons)` event (plus icon-vs-text button
    /// rendering). Captured directly because `StructureKey.use_icons` is
    /// `use_icons || plan.compact`, which masks a flip while the plan is
    /// compact — without this field the stored event goes stale.
    use_icons: bool,
    context_aware_ui: bool,
    show_text_controls: bool,
    show_status_bar: bool,
    show_status_board_badge: bool,
    show_status_page_badge: bool,
    show_floating_badge_always: bool,
    show_preset_toasts: bool,
    show_presets: bool,
    show_actions_section: bool,
    show_zoom_actions: bool,
    show_actions_advanced: bool,
    show_boards_section: bool,
    show_pages_section: bool,
    show_step_section: bool,
    customize_items_open: bool,
    customize_items_group: Option<crate::ui::toolbar::ToolbarItemCustomizeGroup>,
    layout_mode: ToolbarLayoutMode,
}

impl SettingsMenuContentKey {
    fn of(snapshot: &ToolbarSnapshot) -> Self {
        Self {
            use_icons: snapshot.use_icons,
            context_aware_ui: snapshot.context_aware_ui,
            show_text_controls: snapshot.show_text_controls,
            show_status_bar: snapshot.show_status_bar,
            show_status_board_badge: snapshot.show_status_board_badge,
            show_status_page_badge: snapshot.show_status_page_badge,
            show_floating_badge_always: snapshot.show_floating_badge_always,
            show_preset_toasts: snapshot.show_preset_toasts,
            show_presets: snapshot.show_presets,
            show_actions_section: snapshot.show_actions_section,
            show_zoom_actions: snapshot.show_zoom_actions,
            show_actions_advanced: snapshot.show_actions_advanced,
            show_boards_section: snapshot.show_boards_section,
            show_pages_section: snapshot.show_pages_section,
            show_step_section: snapshot.show_step_section,
            customize_items_open: snapshot.customize_items_open,
            customize_items_group: snapshot.customize_items_group,
            layout_mode: snapshot.layout_mode,
        }
    }
}

/// Canvas/Session/Settings popover content width and internal-scroll cap, in spec
/// units — the same theme tokens the builtin `view/top/menus.rs` popovers
/// build from, so the frontends cannot drift.
const MENU_CONTENT_W: f64 = crate::ui::theme::toolbar::MENU_CONTENT_W;
/// The Canvas popover content column, shared with the builtin canvas nodes via
/// the theme token. It is wider than [`MENU_CONTENT_W`] so the five-action rows
/// and Step Undo/Redo controls have breathing room.
const CANVAS_MENU_CONTENT_W: f64 = crate::ui::theme::toolbar::CANVAS_MENU_CONTENT_W;
const MENU_MAX_CONTENT_H: f64 = crate::ui::theme::toolbar::MENU_MAX_CONTENT_H;

/// Inputs that force a rebuild of the bar's widget structure.
#[derive(PartialEq)]
struct StructureKey {
    minimized: bool,
    micro: bool,
    use_icons: bool,
    layout_mode: ToolbarLayoutMode,
    scale_milli: i64,
    items: crate::config::ResolvedToolbarItems,
    quick_colors: crate::config::QuickColorPalette,
    binding_hints: crate::ui::toolbar::ToolbarBindingHints,
    plan: TopStripPlan,
    ring_row: bool,
    /// Ordered style-pill control ids: captures the pill's morph state
    /// (including swatch count, reset presence, and segment kind) so a
    /// tool change rebuilds the pill while value churn stays in updaters.
    style_pill: Vec<String>,
    /// Presets-island structure: the display toggle, slot count, and the
    /// saved slots. A change here (toggled visibility, a saved/cleared slot)
    /// rebuilds the island; the applied-slot highlight rides an updater.
    show_presets: bool,
    preset_slot_count: usize,
    presets: Vec<Option<crate::ui::toolbar::PresetSlotSnapshot>>,
}

impl StructureKey {
    fn of(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> Self {
        Self {
            minimized: snapshot.top_minimized,
            micro: snapshot.top_micro_active(),
            use_icons: snapshot.use_icons || plan.compact,
            layout_mode: snapshot.layout_mode,
            scale_milli: (effective_scale(snapshot) * 1000.0).round() as i64,
            items: snapshot.resolved_toolbar_items.clone(),
            quick_colors: snapshot.quick_colors.clone(),
            binding_hints: snapshot.binding_hints.clone(),
            plan: plan.clone(),
            ring_row: ring_row_active(snapshot, plan),
            style_pill: model::StylePillSpec::build(snapshot, plan)
                .controls()
                .iter()
                .map(|control| control.id().into_owned())
                .collect(),
            show_presets: snapshot.show_presets,
            preset_slot_count: snapshot.preset_slot_count,
            presets: snapshot.presets.clone(),
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

/// Test-only name for an island pill container: `island.<key>`, derived from
/// the shared spec island key. Deliberately *not* `top.`-prefixed so the
/// contract suite's `collect_semantic_widgets` keeps descending into the
/// pill boxes (it stops only at `top.`-named semantic widgets) while island
/// membership stays assertable via the widget ancestry.
fn set_island_widget_id(_widget: &impl IsA<gtk4::Widget>, _island: model::TopToolbarIsland) {
    #[cfg(test)]
    set_semantic_widget_id(_widget, &format!("island.{}", _island.key()));
}

pub(in crate::toolbar_gtk) struct TopBar {
    pub(in crate::toolbar_gtk) window: gtk4::Window,
    feedback: FeedbackSender,
    root: gtk4::Box,
    capture_surface: CaptureSurfaceContent,
    structure: Option<StructureKey>,
    updaters: Rc<RefCell<Vec<Updater>>>,
    /// Persistent value-updaters for the open Canvas popover's content. Unlike
    /// the Session/Settings popovers (whose whole subtree rebuilds on any
    /// modelled change), the Canvas popover hosts the continuously-dragged
    /// delay sliders: their live values ride these updaters so a drag never
    /// triggers a subtree rebuild. Repopulated whenever the popover content is
    /// (re)built; run every `apply`.
    canvas_updaters: Rc<RefCell<Vec<Updater>>>,
    shapes_popover: Option<gtk4::Popover>,
    shapes_capture_surface: Option<CaptureSurfaceContent>,
    overflow_popover: Option<gtk4::Popover>,
    overflow_capture_surface: Option<CaptureSurfaceContent>,
    canvas_popover: Option<gtk4::Popover>,
    canvas_capture_surface: Option<CaptureSurfaceContent>,
    session_popover: Option<gtk4::Popover>,
    session_capture_surface: Option<CaptureSurfaceContent>,
    settings_popover: Option<gtk4::Popover>,
    settings_capture_surface: Option<CaptureSurfaceContent>,
    /// Popover open state as last driven by the snapshot; lets the
    /// `closed` handlers distinguish user dismissal from state sync.
    shapes_expected_open: Rc<Cell<bool>>,
    overflow_expected_open: Rc<Cell<bool>>,
    canvas_expected_open: Rc<Cell<bool>>,
    session_expected_open: Rc<Cell<bool>>,
    settings_expected_open: Rc<Cell<bool>>,
    /// Discriminants of the currently built popover contents; skips the
    /// per-snapshot rebuild that would reset hover and in-flight presses.
    shapes_content_key: Cell<Option<ShapesContentKey>>,
    overflow_content_key: Cell<Option<OverflowContentKey>>,
    canvas_content_key: RefCell<Option<CanvasMenuContentKey>>,
    session_content_key: RefCell<Option<SessionMenuContentKey>>,
    settings_content_key: RefCell<Option<SettingsMenuContentKey>>,
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
        // Known divergence from the built-in strip: the built-in top surface
        // restricts its input region to the island pills so the inter-island
        // gaps click through to the canvas. This GTK window never carves
        // per-island input regions — the gaps swallow clicks here. Accepted
        // for now; fixing it would require wl_surface input-region surgery
        // on the GTK window.

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

        // Report top-window hover to the backend: GTK runs on its own
        // Wayland connection, so this is the only way the backend's
        // top-strip idle fade can restore on pointer approach.
        let hover = gtk4::EventControllerMotion::new();
        let enter_feedback = feedback.clone();
        hover.connect_enter(move |_, _, _| {
            let _ = enter_feedback.send(GtkToolbarFeedback::TopHover { hovered: true });
        });
        let leave_feedback = feedback.clone();
        hover.connect_leave(move |_| {
            let _ = leave_feedback.send(GtkToolbarFeedback::TopHover { hovered: false });
        });
        window.add_controller(hover);

        Self {
            window,
            feedback,
            root,
            capture_surface,
            structure: None,
            updaters: Rc::new(RefCell::new(Vec::new())),
            canvas_updaters: Rc::new(RefCell::new(Vec::new())),
            shapes_popover: None,
            shapes_capture_surface: None,
            overflow_popover: None,
            overflow_capture_surface: None,
            canvas_popover: None,
            canvas_capture_surface: None,
            session_popover: None,
            session_capture_surface: None,
            settings_popover: None,
            settings_capture_surface: None,
            shapes_expected_open: Rc::new(Cell::new(false)),
            overflow_expected_open: Rc::new(Cell::new(false)),
            canvas_expected_open: Rc::new(Cell::new(false)),
            session_expected_open: Rc::new(Cell::new(false)),
            settings_expected_open: Rc::new(Cell::new(false)),
            shapes_content_key: Cell::new(None),
            overflow_content_key: Cell::new(None),
            canvas_content_key: RefCell::new(None),
            session_content_key: RefCell::new(None),
            settings_content_key: RefCell::new(None),
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
                self.set_popovers_capture_transparent(true);
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
            self.set_popovers_capture_transparent(true);
        } else {
            self.set_popovers_capture_transparent(false);
            self.sync_popovers(snapshot, &plan);
        }
        // The Canvas popover's delay sliders live off the content key, so a
        // delay drag drives them through these persistent updaters instead of
        // a subtree rebuild (set_value is a no-op mid-drag, so an in-flight
        // gesture survives). Run after `sync_popovers` so a fresh rebuild's
        // updaters are the ones invoked.
        for updater in self.canvas_updaters.borrow().iter() {
            updater(snapshot);
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
        super::set_surface_input_enabled(&self.window, presentation.input_enabled);
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
        self.canvas_expected_open.set(false);
        self.session_expected_open.set(false);
        self.settings_expected_open.set(false);
        if let Some(popover) = self.shapes_popover.take() {
            popover.unparent();
        }
        self.shapes_capture_surface = None;
        if let Some(popover) = self.overflow_popover.take() {
            popover.unparent();
        }
        self.overflow_capture_surface = None;
        if let Some(popover) = self.canvas_popover.take() {
            popover.unparent();
        }
        self.canvas_capture_surface = None;
        if let Some(popover) = self.session_popover.take() {
            popover.unparent();
        }
        self.session_capture_surface = None;
        if let Some(popover) = self.settings_popover.take() {
            popover.unparent();
        }
        self.settings_capture_surface = None;
        self.shapes_content_key.set(None);
        self.overflow_content_key.set(None);
        *self.canvas_content_key.borrow_mut() = None;
        *self.session_content_key.borrow_mut() = None;
        *self.settings_content_key.borrow_mut() = None;
        while let Some(child) = self.root.first_child() {
            self.root.remove(&child);
        }
        self.updaters.borrow_mut().clear();
        // The Canvas popover was just unparented above, so its persistent
        // value-updaters (which capture those now-dead widgets) must go too;
        // a fresh open repopulates them.
        self.canvas_updaters.borrow_mut().clear();

        if snapshot.top_minimized {
            self.build_minimized(snapshot, plan);
        } else if snapshot.top_micro_active() {
            self.build_micro(snapshot, plan);
        } else {
            self.build_strip(snapshot, plan);
        }
    }
}
