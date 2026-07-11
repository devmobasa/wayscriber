//! GTK toolbar windows: owns the top strip (and, in time, the side
//! palette), the shared stylesheet, and output pinning.

mod top_bar;

use gtk4::prelude::*;
use gtk4_layer_shell::LayerShell;

use super::GtkToolbarUpdate;
use super::widgets::FeedbackSender;

pub(super) struct Windows {
    top: top_bar::TopBar,
    css_provider: gtk4::CssProvider,
    css_scale_milli: i64,
    pinned_output: Option<String>,
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
            top: top_bar::TopBar::new(feedback),
            css_provider,
            css_scale_milli: 1000,
            pinned_output: None,
        }
    }

    pub(super) fn apply(&mut self, update: &GtkToolbarUpdate) {
        self.refresh_css(update);
        self.pin_to_output(update);
        self.top
            .apply(&update.snapshot, update.top_visible, update.top_offset);
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
        self.pinned_output = update.output_name.clone();
        let monitor = update.output_name.as_deref().and_then(monitor_by_connector);
        // None lets the compositor pick, matching a missing preference.
        self.top.window.set_monitor(monitor.as_ref());
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
