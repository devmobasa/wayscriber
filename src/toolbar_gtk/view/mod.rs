//! GTK toolbar windows: owns the top strip and side palette, the shared
//! stylesheet, and output pinning.

mod sections;
mod side_bar;
mod top_bar;

use gtk4::prelude::*;
use gtk4_layer_shell::LayerShell;

use super::widgets::FeedbackSender;
use super::{GtkToolbarKind, GtkToolbarUpdate};
use crate::ui::toolbar::ToolbarSnapshot;

/// Closure applying one control's state from a fresh snapshot.
pub(super) type Updater = Box<dyn Fn(&ToolbarSnapshot)>;

fn drag_visual_should_be_hidden(
    backend_preview: Option<GtkToolbarKind>,
    kind: GtkToolbarKind,
    local_drag_active: bool,
    local_sequence: u64,
    backend_sequence: u64,
) -> bool {
    local_drag_active || local_sequence > backend_sequence || backend_preview == Some(kind)
}

/// Keep the gesture-owning layer surface mapped for stable GTK coordinates,
/// but unmap its visual child while the backend draws the inline preview.
/// Child visibility does not queue a resize, so the transparent window keeps
/// its configured dimensions and continues receiving the window-level drag.
pub(super) fn set_drag_visual_hidden(
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
    window.queue_draw();
}

/// Run a callback only after GTK has painted the pending surface changes.
/// The drag preview lives on another Wayland connection, so sending its start
/// before this point can briefly show both the parked bar and its preview.
pub(super) fn after_next_surface_paint<F>(window: &gtk4::Window, callback: F)
where
    F: FnOnce() + 'static,
{
    let Some(frame_clock) = window.frame_clock() else {
        callback();
        return;
    };
    let callback = std::rc::Rc::new(std::cell::RefCell::new(Some(callback)));
    let handler = std::rc::Rc::new(std::cell::RefCell::new(None));
    let callback_slot = callback.clone();
    let handler_slot = handler.clone();
    let callback_clock = frame_clock.clone();
    let handler_id = frame_clock.connect_after_paint(move |_| {
        if let Some(handler_id) = handler_slot.borrow_mut().take() {
            callback_clock.disconnect(handler_id);
        }
        if let Some(callback) = callback_slot.borrow_mut().take() {
            callback();
        }
    });
    *handler.borrow_mut() = Some(handler_id);
    window.queue_draw();
    frame_clock.request_phase(gtk4::gdk::FrameClockPhase::PAINT);
}

pub(super) struct Windows {
    top: top_bar::TopBar,
    side: side_bar::SideBar,
    css_provider: gtk4::CssProvider,
    css_scale_milli: i64,
    pinned_output: Option<String>,
    feedback: FeedbackSender,
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
        }
    }

    pub(super) fn apply(&mut self, update: &GtkToolbarUpdate) {
        self.feedback
            .set_rebind_state(update.rebind_modifier, update.rebind_modifier_active);
        self.refresh_css(update);
        self.pin_to_output(update);
        self.top.apply(update);
        self.side.apply(update);
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
}
