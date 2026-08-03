//! GTK colors-section unit tests.

use super::*;
use crate::toolbar_gtk::GtkToolbarFeedback;
use crate::toolbar_gtk::widgets::emit_secondary_press;
use crate::ui::toolbar::ToolbarBindingHints;
use std::sync::mpsc::channel;

/// Both palette gestures on one GTK swatch: the primary click selects the
/// slot, the secondary press opens its recolor popup. Widget construction
/// needs a GTK display, so the body runs in an isolated child process the
/// same way the top-strip widget contract test does.
#[test]
fn side_palette_swatch_selects_on_primary_and_recolors_on_secondary() {
    const CHILD_ENV: &str = "WAYSCRIBER_GTK_SIDE_SWATCH_CHILD";
    const TEST_NAME: &str = "toolbar_gtk::view::sections::colors::tests::side_palette_swatch_selects_on_primary_and_recolors_on_secondary";

    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg(TEST_NAME)
            .arg("--exact")
            .arg("--test-threads=1")
            .env(CHILD_ENV, "1")
            .status()
            .expect("run isolated GTK side-swatch test");
        assert!(status.success(), "isolated GTK side-swatch test failed");
        return;
    }

    if let Err(error) = gtk4::init() {
        eprintln!("skipping GTK side-swatch test: {error}");
        return;
    }

    let state = crate::input::state::test_support::make_test_input_state();
    let snapshot = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let (tx, rx) = channel::<GtkToolbarFeedback>();
    let mut updaters: Vec<crate::toolbar_gtk::view::Updater> = Vec::new();
    let ctx = SectionCtx {
        snapshot: &snapshot,
        theme: crate::ui::theme::Theme::dark(),
        feedback: FeedbackSender::new(tx),
        scale: 1.0,
        use_icons: true,
        updaters: &mut updaters,
    };

    // The compact row reorders the palette, so take its first swatch and
    // assert both gestures name the slot it actually draws.
    let swatches = compact_palette_swatches(&snapshot.quick_colors);
    let (color, _, action, index) = swatches.first().expect("compact swatch").clone();
    let mut tracked = Vec::new();
    let row = swatch_row(&ctx, &swatches[..1], None, &mut tracked);
    let button = row
        .first_child()
        .expect("swatch widget")
        .downcast::<gtk4::Button>()
        .expect("swatch button");

    button.emit_clicked();
    assert_eq!(
        rx.recv().expect("primary click event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::SetQuickColor {
                color,
                action,
                index,
            },
            rebind_requested: false,
        }
    );

    emit_secondary_press(button.upcast_ref());
    assert_eq!(
        rx.recv().expect("secondary click event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::EditQuickColor { index },
            rebind_requested: false,
        }
    );

    assert!(
        button
            .tooltip_text()
            .is_some_and(|text| text.contains("right-click to recolor")),
        "the swatch advertises the recolor gesture"
    );
}

#[test]
fn newer_hex_paste_request_supersedes_pending_callback() {
    let requests = HexPasteRequests::default();
    let older = requests.begin();
    let newer = requests.begin();

    assert!(!requests.is_current(older));
    assert!(requests.is_current(newer));
}

#[test]
fn editing_hex_text_invalidates_pending_paste_callback() {
    let requests = HexPasteRequests::default();
    let pending = requests.begin();

    requests.invalidate();

    assert!(!requests.is_current(pending));
}
