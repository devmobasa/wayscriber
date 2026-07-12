//! GTK toolbar windows: owns the top strip and side palette, the shared
//! stylesheet, and output pinning.

mod sections;
mod side_bar;
mod top_bar;

use gtk4::prelude::*;
use gtk4_layer_shell::LayerShell;

use super::GtkToolbarUpdate;
use super::widgets::FeedbackSender;
use crate::ui::toolbar::ToolbarSnapshot;

/// Closure applying one control's state from a fresh snapshot.
pub(super) type Updater = Box<dyn Fn(&ToolbarSnapshot)>;

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
