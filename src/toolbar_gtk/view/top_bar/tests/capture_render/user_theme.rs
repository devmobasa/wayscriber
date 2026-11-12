use std::sync::mpsc;

use gtk4::gdk::RGBA;

use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::ToolbarSnapshot;

use super::super::super::TopStripPlan;
use super::super::widget_support::{collect_descendants, detach_test_popovers};
use super::support::*;
use super::{FeedbackSender, TopBar};
use gtk4::prelude::*;

const THEME_WINDOW_PIXEL: [u8; 4] = [250, 250, 250, 255];
const THEME_FOREGROUND: &str = "rgb(18, 52, 86)";

/// A full GTK theme imported from `~/.config/gtk-4.0/gtk.css` paints every
/// window and button at user priority. The toolbar keeps its own look anyway.
pub(super) fn strip(user: &gtk4::CssProvider) {
    user.load_from_string(&format!(
        "window {{ background-color: #fafafa; color: {THEME_FOREGROUND}; }}
         button {{ background-color: #ebebed; color: {THEME_FOREGROUND}; }}"
    ));
    let snapshot = ToolbarSnapshot::from_input(&make_test_input_state());
    let (tx, _) = mpsc::channel();
    let mut top = TopBar::new_for_test(FeedbackSender::new(tx));
    top.build_strip(&snapshot, &TopStripPlan::unconstrained());
    for updater in top.updaters.borrow().iter() {
        updater(&snapshot);
    }

    top.window.set_decorated(false);
    top.window.present();
    wait_for_frame(&top.window);
    assert_renderer(&top.window);

    let themed = pixels(&top.window)
        .iter()
        .filter(|pixel| **pixel == THEME_WINDOW_PIXEL)
        .count();
    assert_eq!(themed, 0, "the theme painted the toolbar window");

    let theme = RGBA::parse(THEME_FOREGROUND).expect("fixture color");
    let mut areas = Vec::<gtk4::DrawingArea>::new();
    collect_descendants(top.root.upcast_ref(), &mut areas);
    let glyphs: Vec<_> = areas
        .iter()
        .filter(|area| area.ancestor(gtk4::Button::static_type()).is_some())
        .collect();
    assert!(glyphs.len() > 10, "the strip shows its button glyphs");
    for glyph in glyphs {
        let color = glyph.color();
        let channels = [
            color.red() - theme.red(),
            color.green() - theme.green(),
            color.blue() - theme.blue(),
        ];
        assert!(
            channels.iter().any(|delta| delta.abs() > 0.01),
            "a toolbar glyph took the theme's foreground"
        );
    }

    detach_test_popovers(&mut top);
    top.window.destroy();
}
