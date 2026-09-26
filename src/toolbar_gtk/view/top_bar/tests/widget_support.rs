//! GTK widget traversal, accessibility, and lifecycle helpers.

use super::super::TopBar;
use gtk4::prelude::*;
use std::ffi::CStr;
use std::ffi::CString;

pub(super) fn collect_semantic_widgets(root: &gtk4::Widget) -> Vec<gtk4::Widget> {
    fn visit(widget: &gtk4::Widget, widgets: &mut Vec<gtk4::Widget>) {
        if widget.widget_name().starts_with("top.") {
            widgets.push(widget.clone());
            return;
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            visit(&current, widgets);
        }
    }

    let mut widgets = Vec::new();
    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        visit(&current, &mut widgets);
    }
    widgets
}

fn find_control_surface(root: &gtk4::Widget) -> Option<gtk4::Widget> {
    if root.is::<gtk4::Button>() || root.is::<gtk4::CheckButton>() || root.is::<gtk4::DrawingArea>()
    {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        if let Some(surface) = find_control_surface(&current) {
            return Some(surface);
        }
    }
    None
}

pub(super) fn first_control_surface(root: &gtk4::Widget) -> gtk4::Widget {
    find_control_surface(root).unwrap_or_else(|| {
        panic!(
            "semantic widget has no control surface: {}",
            root.widget_name()
        )
    })
}

pub(super) fn assert_accessible_label(widget: &gtk4::Widget, expected: &str, id: &str) {
    let expected = CString::new(expected).expect("accessible label contains no NUL");
    // GTK returns a newly allocated diagnostic string on mismatch and null
    // when the live accessible property has the requested value.
    let mismatch = unsafe {
        gtk4::ffi::gtk_test_accessible_check_property(
            widget.as_ptr().cast(),
            gtk4::ffi::GTK_ACCESSIBLE_PROPERTY_LABEL,
            expected.as_ptr(),
        )
    };
    if mismatch.is_null() {
        return;
    }
    let message = unsafe { CStr::from_ptr(mismatch) }
        .to_string_lossy()
        .into_owned();
    unsafe { gtk4::glib::ffi::g_free(mismatch.cast()) };
    panic!("{id} accessible label: {message}");
}

/// True when `widget` carries a capture-phase click gesture, i.e. the controller
/// `install_click_modifier_capture` adds. A popover is its own GTK native, so
/// without one the toolbar window never sees the rebind chord for clicks inside.
pub(super) fn has_capture_phase_click_gesture(widget: &gtk4::Widget) -> bool {
    let controllers = widget.observe_controllers();
    (0..controllers.n_items()).any(|index| {
        controllers
            .item(index)
            .and_then(|object| object.downcast::<gtk4::GestureClick>().ok())
            .is_some_and(|gesture| gesture.propagation_phase() == gtk4::PropagationPhase::Capture)
    })
}

pub(super) fn detach_test_popovers(top: &mut TopBar) {
    top.shapes.clear();
    top.overflow.clear();
    top.canvas.clear();
    top.session.clear();
    top.settings.clear();
    top.layout.clear();
    top.feel.clear();
    top.arrow_style.clear();
}

pub(super) fn find_widget_named(root: &gtk4::Widget, name: &str) -> Option<gtk4::Widget> {
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        if let Some(found) = find_widget_named(&current, name) {
            return Some(found);
        }
    }
    None
}

pub(super) fn collect_descendants<W: IsA<gtk4::Widget>>(root: &gtk4::Widget, out: &mut Vec<W>) {
    if let Ok(widget) = root.clone().downcast::<W>() {
        out.push(widget);
    }
    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        collect_descendants(&current, out);
    }
}
