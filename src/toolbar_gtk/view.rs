//! GTK toolbar windows.
//!
//! Placeholder shell: proves the layer-shell windows and both bridge
//! directions end to end. The full top strip and side palette views
//! replace the placeholder content.

use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use super::GtkToolbarUpdate;
use crate::ui::toolbar::ToolbarEvent;

/// Matches the built-in bars so compositor rules (blur, shadows) keep
/// applying to whichever frontend is active.
const TOP_NAMESPACE: &str = "wayscriber-toolbar-top";

pub(super) fn install_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(BASE_CSS);
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

const BASE_CSS: &str = r#"
window.wayscriber-bar {
    background: transparent;
}
.wayscriber-bar box.bar {
    background-color: rgba(13, 13, 20, 0.92);
    border: 1px solid rgba(255, 255, 255, 0.10);
    border-radius: 12px;
    padding: 6px 12px;
}
"#;

pub(super) struct Windows {
    top: gtk4::Window,
    events: std::sync::mpsc::Sender<ToolbarEvent>,
}

impl Windows {
    pub(super) fn new(events: std::sync::mpsc::Sender<ToolbarEvent>) -> Self {
        let top = gtk4::Window::new();
        top.add_css_class("wayscriber-bar");
        top.init_layer_shell();
        top.set_layer(Layer::Overlay);
        top.set_namespace(Some(TOP_NAMESPACE));
        top.set_anchor(Edge::Top, true);
        top.set_anchor(Edge::Left, true);
        top.set_margin(Edge::Top, 12);
        top.set_margin(Edge::Left, 12);
        top.set_keyboard_mode(KeyboardMode::OnDemand);

        let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        bar.add_css_class("bar");
        let undo = gtk4::Button::from_icon_name("edit-undo-symbolic");
        undo.set_tooltip_text(Some("Undo"));
        let undo_events = events.clone();
        undo.connect_clicked(move |_| {
            let _ = undo_events.send(ToolbarEvent::Undo);
        });
        bar.append(&undo);
        top.set_child(Some(&bar));

        Self { top, events }
    }

    pub(super) fn apply(&mut self, update: &GtkToolbarUpdate) {
        let _ = &self.events;
        self.top.set_visible(update.top_visible);
    }
}
