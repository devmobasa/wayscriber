//! GTK colors-section unit tests.

use super::*;
use crate::toolbar_gtk::GtkToolbarFeedback;
use crate::toolbar_gtk::widgets::{emit_secondary_press, test_feedback_channel};
use crate::ui::toolbar::ToolbarBindingHints;

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
    let (feedback, mailbox) = test_feedback_channel();
    let mut updaters: Vec<crate::toolbar_gtk::view::Updater> = Vec::new();
    let ctx = SectionCtx {
        snapshot: &snapshot,
        feedback,
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
        mailbox.receive_one().expect("primary click event"),
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
        mailbox.receive_one().expect("secondary click event"),
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
fn hex_paste_request_marker_is_single_owner_and_edit_invalidates_it() {
    const CHILD_ENV: &str = "WAYSCRIBER_GTK_HEX_PASTE_MARKER_CHILD";
    const TEST_NAME: &str = "toolbar_gtk::view::sections::colors::tests::hex_paste_request_marker_is_single_owner_and_edit_invalidates_it";

    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg(TEST_NAME)
            .arg("--exact")
            .arg("--test-threads=1")
            .env(CHILD_ENV, "1")
            .status()
            .expect("run isolated GTK hex-paste marker test");
        assert!(
            status.success(),
            "isolated GTK hex-paste marker test failed"
        );
        return;
    }

    if let Err(error) = gtk4::init() {
        eprintln!("skipping GTK hex-paste marker test: {error}");
        return;
    }

    let entry = gtk4::Entry::new();
    let older = begin_hex_paste_request(&entry);
    assert!(older.widget().is_some(), "first request owns the marker");

    let newer = begin_hex_paste_request(&entry);
    assert!(
        older.widget().is_none(),
        "new request supersedes the old one"
    );
    assert!(newer.widget().is_some(), "new request owns the marker");

    invalidate_hex_paste_request(&entry);
    assert!(newer.widget().is_none(), "editing invalidates the request");
}
