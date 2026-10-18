use super::{FeedbackSender, TopBar, support::*};
use crate::toolbar_gtk::GtkToolbarUpdate;
use crate::ui::toolbar::{ToolbarBindingHints, ToolbarSnapshot};
use gtk4::prelude::*;

pub(super) fn apply_paths(user: &gtk4::CssProvider) {
    user.load_from_string(&painted_css("window", "0 2px 6px #58c0e8"));
    let state = crate::input::state::test_support::make_test_input_state();
    let mut update = GtkToolbarUpdate {
        snapshot: ToolbarSnapshot::from_input_with_bindings(
            &state,
            ToolbarBindingHints::from_input_state(&state),
        ),
        top_visible: false,
        top_offset: (0.0, 0.0),
        top_offset_seq: 0,
        top_base_x: 0.0,
        output_name: None,
        rebind_modifier: Default::default(),
        rebind_modifier_active: false,
        modal_engaged: false,
        drag_preview: None,
        capture_suppressed: false,
        capture_suppression_generation: None,
    };
    let (tx, _) = std::sync::mpsc::channel();
    let mut top = TopBar::new_for_test(FeedbackSender::new(tx));
    top.window.set_decorated(false);

    hidden_capture_and_cancel(&mut top, &mut update);
    let (normal, size) = visible_capture_and_restore(&mut top, &mut update);
    idle_hide_and_reveal(&mut top, &mut update, &normal, size);

    update.top_visible = false;
    assert!(!top.apply(&update));
    assert!(!top.window.is_visible());

    super::super::widget_support::detach_test_popovers(&mut top);
    top.window.destroy();
}

fn hidden_capture_and_cancel(top: &mut TopBar, update: &mut GtkToolbarUpdate) {
    assert!(!top.apply(update));

    update.capture_suppressed = true;
    update.capture_suppression_generation = Some(1);
    assert!(!top.apply(update));
    assert!(
        !top.window.is_visible(),
        "capture cannot map hidden toolbar"
    );

    update.capture_suppressed = false;
    update.capture_suppression_generation = None;
    assert!(!top.apply(update));
    assert!(
        !top.window.is_visible(),
        "cancel keeps hidden toolbar hidden"
    );
}

fn visible_capture_and_restore(
    top: &mut TopBar,
    update: &mut GtkToolbarUpdate,
) -> (Vec<[u8; 4]>, (i32, i32)) {
    update.top_visible = true;
    assert!(top.apply(update));
    wait_for_frame(&top.window);
    assert!(top.apply(update));
    wait_for_frame(&top.window);

    // Weston may assign initial keyboard focus to the first slider after the
    // map callback. Capture releases that focus; compare an unfocused normal
    // toolbar so its Cairo focus ring is not part of the styling fixture.
    gtk4::prelude::GtkWindowExt::set_focus(&top.window, gtk4::Widget::NONE);
    assert!(gtk4::prelude::GtkWindowExt::focus(&top.window).is_none());

    assert_renderer(&top.window);
    let normal = settled_pixels(&top.window);
    assert_fixture(&normal);
    let size = native_size(&top.window);

    for generation in 2..=3 {
        update.capture_suppressed = true;
        update.capture_suppression_generation = Some(generation);
        assert!(top.apply(update));
        assert!(top.drag_blocked.get());
        assert!(top.root.is_child_visible());
        assert_eq!(top.root.opacity(), 0.0);

        prove(generation, vec![top.capture_target()]);
        assert_clear(&top.window, "apply capture");
        assert_eq!(native_size(&top.window), size);

        update.capture_suppressed = false;
        update.capture_suppression_generation = None;
        assert!(top.apply(update));
        assert!(!top.drag_blocked.get());
        assert!(top.root.is_child_visible());
        assert_eq!(top.root.opacity(), 1.0);
        wait_for_frame(&top.window);
        assert_restored(&top.window, &normal, "apply cancel/complete");
    }

    (normal, size)
}

fn idle_hide_and_reveal(
    top: &mut TopBar,
    update: &mut GtkToolbarUpdate,
    normal: &[[u8; 4]],
    size: (i32, i32),
) {
    update.snapshot.top_fade = 0.0;
    assert!(top.apply(update));
    wait_for_frame(&top.window);
    assert_clear(&top.window, "apply idle-hidden strip");
    assert_eq!(native_size(&top.window), size);

    update.snapshot.top_fade = 1.0;
    assert!(top.apply(update));
    wait_for_frame(&top.window);
    assert_restored(&top.window, normal, "apply idle reveal");
}
