//! GTK toolbar windows: owns the top strip and side palette, the shared
//! stylesheet, and output pinning.

mod capture_suppression;
mod drag;
mod sections;
mod side_bar;
mod top_bar;

use gtk4::prelude::*;
use gtk4_layer_shell::{KeyboardMode, LayerShell};

use super::widgets::FeedbackSender;
use super::{GtkToolbarFeedback, GtkToolbarKind, GtkToolbarUpdate};
use crate::ui::toolbar::ToolbarSnapshot;

/// Closure applying one control's state from a fresh snapshot.
pub(super) type Updater = Box<dyn Fn(&ToolbarSnapshot)>;

const CAPTURE_SURFACE_CONTENT_CLASS: &str = "wayscriber-capture-surface-content";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ToolbarSurfacePresentation {
    window_visible: bool,
    capture_transparent: bool,
    visual_hidden: bool,
    input_enabled: bool,
}

fn toolbar_surface_presentation(
    configured_visible: bool,
    capture_suppressed: bool,
    drag_visual_hidden: bool,
    mapped_before_capture: bool,
) -> ToolbarSurfacePresentation {
    let window_visible = configured_visible || (capture_suppressed && mapped_before_capture);
    ToolbarSurfacePresentation {
        window_visible,
        capture_transparent: capture_suppressed,
        visual_hidden: drag_visual_hidden,
        input_enabled: window_visible && !capture_suppressed,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureUpdatePlan {
    Apply,
    ApplyAndAcknowledge(u64),
    Hold,
}

#[derive(Debug, Default)]
struct CaptureUpdateTracker {
    generation: Option<u64>,
}

impl CaptureUpdateTracker {
    fn plan(
        &mut self,
        capture_suppressed: bool,
        generation: Option<u64>,
    ) -> Result<CaptureUpdatePlan, String> {
        if !capture_suppressed {
            if let Some(generation) = generation {
                return Err(format!(
                    "capture suppression generation {generation} requested while suppression is inactive"
                ));
            }
            self.generation = None;
            return Ok(CaptureUpdatePlan::Apply);
        }

        match generation {
            Some(generation) if self.generation == Some(generation) => Ok(CaptureUpdatePlan::Hold),
            Some(generation) => {
                self.generation = Some(generation);
                Ok(CaptureUpdatePlan::ApplyAndAcknowledge(generation))
            }
            None if self.generation.is_some() => Ok(CaptureUpdatePlan::Hold),
            None => Ok(CaptureUpdatePlan::Apply),
        }
    }
}

fn drag_visual_should_be_hidden(
    backend_preview: Option<GtkToolbarKind>,
    kind: GtkToolbarKind,
    local_drag_active: bool,
    local_sequence: u64,
    backend_sequence: u64,
) -> bool {
    local_drag_active || local_sequence > backend_sequence || backend_preview == Some(kind)
}

/// Keep the layer surface mapped but hide its visual child while a drag preview
/// is rendered by the main overlay. Child visibility does not queue a resize,
/// so the parked window retains its configured dimensions.
pub(super) fn set_visual_hidden(
    window: &gtk4::Window,
    visual: &gtk4::Box,
    kind: GtkToolbarKind,
    hidden: bool,
) {
    let child_visible = !hidden;
    if visual.is_child_visible() == child_visible {
        return;
    }
    crate::toolbar_gtk::drag_debug_log(format!(
        "{kind:?} visual child -> {} (window remains mapped for drag input)",
        if hidden { "hidden" } else { "visible" }
    ));
    visual.set_child_visible(child_visible);
    visual.trigger_tooltip_query();
    window.queue_draw();
}

/// A mapped native surface whose ordinary content can be replaced by a real,
/// fully transparent render node during capture.
///
/// GTK omits widgets with opacity zero from the render tree. If every child is
/// omitted, the Wayland backend can issue only a state-only surface commit;
/// there is then no replacement buffer for the compositor to acknowledge.
/// The transparent texture keeps one render node in the snapshot without
/// changing any output pixel, so the capture barrier can wait for an actual
/// transparent buffer submission and presentation.
#[derive(Clone)]
pub(super) struct CaptureSurfaceContent {
    root: gtk4::Overlay,
    proof: gtk4::Picture,
    proof_serial: std::rc::Rc<std::cell::Cell<u8>>,
}

impl CaptureSurfaceContent {
    fn empty() -> Self {
        // Keep non-zero RGB data behind a zero alpha channel. Renderers cannot
        // infer an empty node from the texture format or pixels, while normal
        // source-over composition still contributes exactly zero opacity.
        let bytes = gtk4::glib::Bytes::from_static(&[0xff, 0x00, 0xff, 0x00]);
        let texture =
            gtk4::gdk::MemoryTexture::new(1, 1, gtk4::gdk::MemoryFormat::R8g8b8a8, &bytes, 4);
        let proof = gtk4::Picture::for_paintable(&texture);
        proof.set_content_fit(gtk4::ContentFit::Fill);
        proof.set_halign(gtk4::Align::Fill);
        proof.set_valign(gtk4::Align::Fill);
        proof.set_hexpand(true);
        proof.set_vexpand(true);
        proof.set_can_target(false);
        proof.set_visible(false);

        let root = gtk4::Overlay::new();
        root.add_css_class(CAPTURE_SURFACE_CONTENT_CLASS);
        root.add_overlay(&proof);

        Self {
            root,
            proof,
            proof_serial: std::rc::Rc::new(std::cell::Cell::new(0)),
        }
    }

    fn new<W>(content: &W) -> Self
    where
        W: IsA<gtk4::Widget>,
    {
        let surface = Self::empty();
        surface.set_content(content);
        surface
    }

    fn set_content<W>(&self, content: &W)
    where
        W: IsA<gtk4::Widget>,
    {
        self.root.set_child(Some(content));
    }

    fn take_content(&self) -> Option<gtk4::Widget> {
        let content = self.root.child()?;
        self.root.set_child(None::<&gtk4::Widget>);
        Some(content)
    }

    fn widget(&self) -> &gtk4::Overlay {
        &self.root
    }

    fn set_transparent(&self, transparent: bool) {
        let opacity = if transparent { 0.0 } else { 1.0 };
        if let Some(content) = self.root.child()
            && (content.opacity() - opacity).abs() > f64::EPSILON
        {
            content.set_opacity(opacity);
        }
        self.proof.set_visible(transparent);
        self.root.queue_draw();
    }

    /// Replace the invisible proof texture so GTK cannot coalesce this
    /// surface's dedicated proof render with the transparency change that
    /// another native surface already presented.
    fn refresh_transparent_proof(&self) {
        let serial = self.proof_serial.get().wrapping_add(1);
        self.proof_serial.set(serial);
        // Both variants remain fully transparent. Alternating hidden RGB and
        // allocating a new texture changes the render node identity without
        // contributing a visible pixel to the capture.
        let bytes = if serial & 1 == 0 {
            [0xff, 0x00, 0xff, 0x00]
        } else {
            [0x00, 0xff, 0xff, 0x00]
        };
        let bytes = gtk4::glib::Bytes::from_owned(bytes);
        let texture =
            gtk4::gdk::MemoryTexture::new(1, 1, gtk4::gdk::MemoryFormat::R8g8b8a8, &bytes, 4);
        self.proof.set_paintable(Some(&texture));
        self.root.queue_draw();
    }

    fn content_opacity(&self) -> Option<f64> {
        self.root.child().map(|content| content.opacity())
    }

    fn proof_visible(&self) -> bool {
        self.proof.get_visible()
    }

    fn is_wrapper(widget: &gtk4::Widget) -> bool {
        widget.has_css_class(CAPTURE_SURFACE_CONTENT_CLASS)
    }

    #[cfg(test)]
    fn proof_serial(&self) -> u8 {
        self.proof_serial.get()
    }
}

#[derive(Clone)]
struct CaptureProofTarget {
    name: &'static str,
    widget: gtk4::Widget,
    content: CaptureSurfaceContent,
    lifetime: CaptureProofLifetime,
    on_withdrawn: Option<std::rc::Rc<dyn Fn()>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptureProofLifetime {
    Required,
    WhileMapped,
}

impl CaptureProofTarget {
    fn new<W>(name: &'static str, widget: &W, content: &CaptureSurfaceContent) -> Self
    where
        W: IsA<gtk4::Widget>,
    {
        Self {
            name,
            widget: widget.clone().upcast(),
            content: content.clone(),
            lifetime: CaptureProofLifetime::Required,
            on_withdrawn: None,
        }
    }

    fn new_withdrawable<W>(name: &'static str, widget: &W, content: &CaptureSurfaceContent) -> Self
    where
        W: IsA<gtk4::Widget>,
    {
        Self {
            name,
            widget: widget.clone().upcast(),
            content: content.clone(),
            lifetime: CaptureProofLifetime::WhileMapped,
            on_withdrawn: None,
        }
    }

    fn new_withdrawable_with_callback<W, F>(
        name: &'static str,
        widget: &W,
        content: &CaptureSurfaceContent,
        on_withdrawn: F,
    ) -> Self
    where
        W: IsA<gtk4::Widget>,
        F: Fn() + 'static,
    {
        Self {
            name,
            widget: widget.clone().upcast(),
            content: content.clone(),
            lifetime: CaptureProofLifetime::WhileMapped,
            on_withdrawn: Some(std::rc::Rc::new(on_withdrawn)),
        }
    }

    fn is_withdrawn(&self) -> bool {
        self.lifetime == CaptureProofLifetime::WhileMapped && !widget_native_is_mapped(&self.widget)
    }

    fn required_but_unmapped(&self) -> bool {
        self.lifetime == CaptureProofLifetime::Required && !widget_native_is_mapped(&self.widget)
    }

    fn mark_withdrawn(&self) {
        if let Some(on_withdrawn) = self.on_withdrawn.as_ref() {
            on_withdrawn();
        }
    }

    fn refresh_transparent_proof(&self) {
        self.content.refresh_transparent_proof();
    }
}

pub(super) struct CaptureSurfaceLog<'a> {
    name: &'a str,
    configured_visible: bool,
    mapped_before_capture: bool,
    presentation: ToolbarSurfacePresentation,
    window: &'a gtk4::Window,
    visual: &'a gtk4::Box,
    capture_surface: &'a CaptureSurfaceContent,
}

fn log_capture_surface_state(generation: u64, state: CaptureSurfaceLog<'_>) {
    let CaptureSurfaceLog {
        name,
        configured_visible,
        mapped_before_capture,
        presentation,
        window,
        visual,
        capture_surface,
    } = state;
    let surface_mapped = window.surface().is_some_and(|surface| surface.is_mapped());
    log::info!(
        "capture.preflight id={generation} component=gtk surface={name} phase=applied configured_visible={configured_visible} mapped_before_capture={mapped_before_capture} planned_window_visible={} planned_capture_transparent={} planned_visual_hidden={} planned_input_enabled={} window_visible={} window_mapped={} surface_mapped={surface_mapped} visual_child_visible={} can_target={} size={}x{} window_opacity={:.3} content_opacity={:.3?} transparent_proof_visible={}",
        presentation.window_visible,
        presentation.capture_transparent,
        presentation.visual_hidden,
        presentation.input_enabled,
        window.is_visible(),
        window.is_mapped(),
        visual.is_child_visible(),
        window.can_target(),
        window.width(),
        window.height(),
        window.opacity(),
        capture_surface.content_opacity(),
        capture_surface.proof_visible(),
    );
}

fn set_surface_input_enabled(window: &gtk4::Window, enabled: bool) {
    window.set_can_target(enabled);
    window.set_keyboard_mode(if enabled {
        KeyboardMode::OnDemand
    } else {
        KeyboardMode::None
    });
    if !enabled {
        gtk4::prelude::GtkWindowExt::set_focus(window, gtk4::Widget::NONE);
    }
    if let Some(surface) = window.surface() {
        if enabled {
            surface.set_input_region(None);
        } else {
            let empty = gtk4::cairo::Region::create();
            surface.set_input_region(Some(&empty));
        }
    }
    window.queue_draw();
}

/// Run a callback only after GTK has painted the pending surface changes.
/// The drag preview lives on another Wayland connection, so sending its start
/// before this point can briefly show both the parked bar and its preview.
pub(super) fn after_next_surface_paint<W, F>(widget: &W, callback: F)
where
    W: IsA<gtk4::Widget>,
    F: FnOnce() + 'static,
{
    after_next_surface_paint_counter(widget, move |_| callback());
}

fn after_next_surface_paint_counter<W, F>(widget: &W, callback: F)
where
    W: IsA<gtk4::Widget>,
    F: FnOnce(Option<i64>) + 'static,
{
    let Some(frame_clock) = widget.frame_clock() else {
        callback(None);
        return;
    };
    let callback = std::rc::Rc::new(std::cell::RefCell::new(Some(callback)));
    let handler = std::rc::Rc::new(std::cell::RefCell::new(None));
    let callback_slot = callback.clone();
    let handler_slot = handler.clone();
    let callback_clock = frame_clock.clone();
    let handler_id = frame_clock.connect_after_paint(move |clock| {
        if let Some(handler_id) = handler_slot.borrow_mut().take() {
            callback_clock.disconnect(handler_id);
        }
        if let Some(callback) = callback_slot.borrow_mut().take() {
            callback(Some(clock.frame_counter()));
        }
    });
    *handler.borrow_mut() = Some(handler_id);
    widget.queue_draw();
    frame_clock.request_phase(gtk4::gdk::FrameClockPhase::PAINT);
}

pub(super) struct Windows {
    top: top_bar::TopBar,
    side: side_bar::SideBar,
    tooltip_capture: capture_suppression::TooltipCapture,
    css_provider: gtk4::CssProvider,
    css_scale_milli: i64,
    pinned_output: Option<String>,
    feedback: FeedbackSender,
    capture_updates: CaptureUpdateTracker,
}

impl Windows {
    pub(super) fn new(feedback: FeedbackSender) -> Self {
        let css_provider = gtk4::CssProvider::new();
        css_provider.load_from_string(&super::css::stylesheet(1.0));
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &css_provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        Self {
            top: top_bar::TopBar::new(feedback.clone()),
            side: side_bar::SideBar::new(feedback.clone()),
            tooltip_capture: capture_suppression::TooltipCapture::new(),
            css_provider,
            css_scale_milli: 1000,
            pinned_output: None,
            feedback,
            capture_updates: CaptureUpdateTracker::default(),
        }
    }

    pub(super) async fn apply(&mut self, update: &GtkToolbarUpdate) -> Result<(), String> {
        self.feedback
            .set_rebind_state(update.rebind_modifier, update.rebind_modifier_active);
        let capture_plan = self.capture_updates.plan(
            update.capture_suppressed,
            update.capture_suppression_generation,
        )?;
        if capture_plan == CaptureUpdatePlan::Hold {
            log::info!(
                "capture.preflight id={:?} component=gtk phase=update-held active_generation={:?}",
                update.capture_suppression_generation,
                self.capture_updates.generation
            );
            return Ok(());
        }
        self.refresh_css(update);
        self.pin_to_output(update);
        let defer_capture_input = matches!(capture_plan, CaptureUpdatePlan::ApplyAndAcknowledge(_));
        self.tooltip_capture
            .set_suppressed(update.capture_suppressed, defer_capture_input);
        let top_mapped = self.top.apply(update, defer_capture_input);
        let side_mapped = self.side.apply(update, defer_capture_input);
        self.refresh_popup_capture_sources();
        if let CaptureUpdatePlan::ApplyAndAcknowledge(generation) = capture_plan {
            let proof = async {
                self.wait_for_capture_paints(generation, top_mapped, side_mapped)
                    .await?;

                // Once every surface known at entry is transparent, close
                // compositor input admission. GDK defers input-region changes
                // until a surface commit, so force and presentation-confirm a
                // fresh transparent frame before the display roundtrip and
                // low-priority GTK-main-context settle loop.
                let display = gtk4::gdk::Display::default().ok_or_else(|| {
                    "GTK display disappeared during capture suppression".to_string()
                })?;
                capture_suppression::commit_input_regions_before_popup_quiescence(
                    generation,
                    || self.disable_capture_input(top_mapped, side_mapped),
                    || self.wait_for_capture_paints(generation, top_mapped, side_mapped),
                    || display.sync(),
                )
                .await?;

                let roots = self.popup_capture_roots();
                self.tooltip_capture
                    .wait_for_popover_quiescence(generation, &roots, |targets, deadline| {
                        capture_suppression::wait_for_presented_transparency_until(
                            generation, targets, deadline,
                        )
                    })
                    .await?;

                // The GTK bars use a separate Wayland connection. A
                // compositor frame or presentation timestamp proves the
                // opacity-zero commit was processed; this final roundtrip
                // orders every settled GTK request before the backend submits
                // its hidden frame on another connection.
                display.sync();
                Ok::<(), String>(())
            }
            .await;
            match proof {
                Ok(()) => {
                    log::info!(
                        "capture.preflight id={generation} component=gtk phase=display-sync-complete"
                    );
                    self.feedback
                        .send(GtkToolbarFeedback::CaptureSuppressionReady { generation })
                        .map_err(|()| {
                            format!(
                                "could not publish GTK capture suppression generation {generation}"
                            )
                        })?;
                    log::info!(
                        "capture.preflight id={generation} component=gtk phase=ack-published"
                    );
                }
                Err(error) => {
                    log::warn!(
                        "capture.preflight id={generation} component=gtk phase=proof-failed error={error}"
                    );
                    self.feedback
                        .send(GtkToolbarFeedback::CaptureSuppressionFailed {
                            generation,
                            error,
                        })
                        .map_err(|()| {
                            format!(
                                "could not publish GTK capture suppression failure for generation {generation}"
                            )
                        })?;
                }
            }
        }
        Ok(())
    }

    async fn wait_for_capture_paints(
        &self,
        generation: u64,
        top_mapped: bool,
        side_mapped: bool,
    ) -> Result<(), String> {
        let mut targets = Vec::new();
        if top_mapped {
            targets.push(self.top.capture_target());
        }
        if side_mapped {
            targets.push(self.side.capture_target());
        }
        targets.extend(self.top.capture_popover_targets());
        targets.extend(self.tooltip_capture.capture_popover_targets());
        if let Some(tooltip) = self.tooltip_capture.capture_target() {
            targets.push(tooltip);
        }

        capture_suppression::wait_for_presented_transparency(generation, targets).await?;
        self.tooltip_capture.mark_capture_popovers_proven();
        Ok(())
    }

    fn refresh_popup_capture_sources(&self) {
        for root in self.popup_capture_roots() {
            self.tooltip_capture.install_tree(&root);
        }
    }

    fn popup_capture_roots(&self) -> Vec<gtk4::Widget> {
        let mut roots = vec![
            self.top.window.clone().upcast::<gtk4::Widget>(),
            self.side.window.clone().upcast::<gtk4::Widget>(),
        ];
        roots.extend(self.top.tooltip_roots());
        roots
    }

    fn disable_capture_input(&self, top_mapped: bool, side_mapped: bool) {
        if top_mapped {
            set_surface_input_enabled(&self.top.window, false);
        }
        if side_mapped {
            set_surface_input_enabled(&self.side.window, false);
        }
        self.top.set_popovers_capture_input_enabled(false);
        self.tooltip_capture.set_input_enabled(false);
    }

    /// Regenerate the stylesheet when the toolbar scale changes.
    fn refresh_css(&mut self, update: &GtkToolbarUpdate) {
        let scale = update.snapshot.toolbar_scale;
        let scale = if scale.is_finite() {
            scale.clamp(0.5, 3.0)
        } else {
            1.0
        };
        let milli = (scale * 1000.0).round() as i64;
        if milli != self.css_scale_milli {
            self.css_provider
                .load_from_string(&super::css::stylesheet(scale));
            self.css_scale_milli = milli;
        }
    }

    /// Keep the bars on the same output as the annotation overlay.
    fn pin_to_output(&mut self, update: &GtkToolbarUpdate) {
        if self.pinned_output == update.output_name {
            return;
        }
        let monitor = update.output_name.as_deref().and_then(monitor_by_connector);
        if monitor.is_none() && update.output_name.is_some() {
            // GDK may not have seen the connector yet; leave the cache
            // unset so the next update retries the lookup.
            return;
        }
        self.pinned_output = update.output_name.clone();
        // None lets the compositor pick, matching a missing preference.
        self.top.window.set_monitor(monitor.as_ref());
        self.side.window.set_monitor(monitor.as_ref());
    }
}

fn widget_native_is_mapped(widget: &gtk4::Widget) -> bool {
    widget.is_mapped()
        && widget
            .native()
            .and_then(|native| native.surface())
            .is_some_and(|surface| surface.is_mapped())
}

fn monitor_by_connector(connector: &str) -> Option<gtk4::gdk::Monitor> {
    let display = gtk4::gdk::Display::default()?;
    let monitors = display.monitors();
    for index in 0..monitors.n_items() {
        let monitor = monitors
            .item(index)?
            .downcast::<gtk4::gdk::Monitor>()
            .ok()?;
        if monitor.connector().as_deref() == Some(connector) {
            return Some(monitor);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_visual_stays_hidden_until_the_backend_finishes_handoff() {
        assert!(drag_visual_should_be_hidden(
            None,
            GtkToolbarKind::Top,
            true,
            4,
            3,
        ));
        assert!(drag_visual_should_be_hidden(
            None,
            GtkToolbarKind::Top,
            false,
            4,
            3,
        ));
        assert!(drag_visual_should_be_hidden(
            Some(GtkToolbarKind::Top),
            GtkToolbarKind::Top,
            false,
            4,
            4,
        ));
        assert!(!drag_visual_should_be_hidden(
            None,
            GtkToolbarKind::Top,
            false,
            4,
            4,
        ));
    }

    #[test]
    fn another_bars_preview_does_not_hide_this_surface() {
        assert!(!drag_visual_should_be_hidden(
            Some(GtkToolbarKind::Side),
            GtkToolbarKind::Top,
            false,
            7,
            7,
        ));
    }

    #[test]
    fn capture_suppression_keeps_a_configured_toolbar_mapped_but_transparent() {
        assert_eq!(
            toolbar_surface_presentation(true, true, false, false),
            ToolbarSurfacePresentation {
                window_visible: true,
                capture_transparent: true,
                visual_hidden: false,
                input_enabled: false,
            }
        );
    }

    #[test]
    fn capture_suppression_does_not_map_a_configured_hidden_toolbar() {
        assert_eq!(
            toolbar_surface_presentation(false, true, false, false),
            ToolbarSurfacePresentation {
                window_visible: false,
                capture_transparent: true,
                visual_hidden: false,
                input_enabled: false,
            }
        );
    }

    #[test]
    fn capture_suppression_preserves_a_surface_that_was_still_mapped() {
        assert_eq!(
            toolbar_surface_presentation(false, true, false, true),
            ToolbarSurfacePresentation {
                window_visible: true,
                capture_transparent: true,
                visual_hidden: false,
                input_enabled: false,
            }
        );
    }

    #[test]
    fn drag_preview_keeps_the_mapped_surface_interactive() {
        assert_eq!(
            toolbar_surface_presentation(true, false, true, false),
            ToolbarSurfacePresentation {
                window_visible: true,
                capture_transparent: false,
                visual_hidden: true,
                input_enabled: true,
            }
        );
    }

    #[test]
    fn ending_capture_restores_pixels_and_input_without_remapping() {
        assert_eq!(
            toolbar_surface_presentation(true, false, false, false),
            ToolbarSurfacePresentation {
                window_visible: true,
                capture_transparent: false,
                visual_hidden: false,
                input_enabled: true,
            }
        );
    }

    #[test]
    fn duplicate_capture_generation_is_held_until_restoration() {
        let mut tracker = CaptureUpdateTracker::default();

        assert_eq!(
            tracker.plan(true, Some(17)),
            Ok(CaptureUpdatePlan::ApplyAndAcknowledge(17))
        );
        assert_eq!(tracker.plan(true, Some(17)), Ok(CaptureUpdatePlan::Hold));
        assert_eq!(tracker.plan(true, None), Ok(CaptureUpdatePlan::Hold));
        assert_eq!(tracker.plan(false, None), Ok(CaptureUpdatePlan::Apply));
        assert_eq!(tracker.generation, None);
    }

    #[test]
    fn a_new_capture_generation_can_replace_a_coalesced_restoration() {
        let mut tracker = CaptureUpdateTracker::default();
        assert_eq!(
            tracker.plan(true, Some(8)),
            Ok(CaptureUpdatePlan::ApplyAndAcknowledge(8))
        );
        assert_eq!(
            tracker.plan(true, Some(9)),
            Ok(CaptureUpdatePlan::ApplyAndAcknowledge(9))
        );
    }
}
