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

/// Render the mapped GTK toplevel fully transparent during capture. Unlike
/// hiding its only child, changing toplevel opacity invalidates the complete
/// rendered surface while retaining its size and mapping.
fn set_capture_transparent<W>(widget: &W, transparent: bool)
where
    W: IsA<gtk4::Widget>,
{
    let opacity = if transparent { 0.0 } else { 1.0 };
    if (widget.opacity() - opacity).abs() <= f64::EPSILON {
        return;
    }
    widget.set_opacity(opacity);
    widget.queue_draw();
}

fn log_capture_surface_state(
    generation: u64,
    name: &str,
    configured_visible: bool,
    mapped_before_capture: bool,
    presentation: ToolbarSurfacePresentation,
    window: &gtk4::Window,
    visual: &gtk4::Box,
) {
    let surface_mapped = window.surface().is_some_and(|surface| surface.is_mapped());
    log::info!(
        "capture.preflight id={generation} component=gtk surface={name} phase=applied configured_visible={configured_visible} mapped_before_capture={mapped_before_capture} planned_window_visible={} planned_capture_transparent={} planned_visual_hidden={} planned_input_enabled={} window_visible={} window_mapped={} surface_mapped={surface_mapped} visual_child_visible={} can_target={} size={}x{} opacity={:.3}",
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
        let top_mapped = self.top.apply(update);
        let side_mapped = self.side.apply(update);
        if let CaptureUpdatePlan::ApplyAndAcknowledge(generation) = capture_plan {
            let proof = self
                .wait_for_capture_paints(generation, top_mapped, side_mapped)
                .await
                .and_then(|()| {
                    gtk4::gdk::Display::default().ok_or_else(|| {
                        "GTK display disappeared during capture suppression".to_string()
                    })
                });
            match proof {
                Ok(display) => {
                    // The GTK bars use a separate Wayland connection. A
                    // compositor frame or presentation timestamp proves the
                    // opacity-zero commit was processed; this roundtrip orders
                    // remaining GTK requests before the backend submits its
                    // hidden frame on another connection.
                    display.sync();
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
        let targets = [
            (
                "top",
                top_mapped,
                self.top.window.clone().upcast::<gtk4::Widget>(),
            ),
            (
                "side",
                side_mapped,
                self.side.window.clone().upcast::<gtk4::Widget>(),
            ),
        ]
        .into_iter()
        .filter_map(|(name, visible, window)| visible.then_some((name, window)))
        .collect::<Vec<_>>();

        let mut targets = targets;
        targets.extend(self.top.capture_popover_targets());

        capture_suppression::wait_for_presented_transparency(generation, targets).await
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
