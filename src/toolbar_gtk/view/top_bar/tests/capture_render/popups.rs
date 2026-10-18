use super::support::*;
use crate::toolbar_gtk::view::top_bar::popovers::set_popover_capture_transparent;
use crate::toolbar_gtk::view::{
    CaptureProofTarget, CaptureSurfaceContent, capture_suppression::TooltipCapture,
};
use gtk4::prelude::*;

fn anchor() -> (gtk4::Window, gtk4::Button) {
    let anchor = gtk4::Button::with_label("Popup anchor");
    let window = gtk4::Window::builder()
        .default_width(320)
        .default_height(200)
        .decorated(false)
        .child(&anchor)
        .build();
    window.add_css_class("wayscriber-toolbar");
    window.present();
    wait_for_frame(&window);
    assert_renderer(&window);
    (window, anchor)
}

pub(super) fn popup(user: &gtk4::CssProvider, case: &str) {
    let values = crate::ui::theme::css::GtkStylesheetValues::new(1.0);
    let shadow = match case {
        "popup-none" => "none".to_string(),
        "popup-small" => "0 1px 2px #58c0e8".to_string(),
        "popup-default" => format!(
            "0 {}px {}px #58c0e8",
            values.shadow_offset_y, values.shadow_blur
        ),
        "popup-large" => "0 8px 24px #58c0e8".to_string(),
        _ => unreachable!(),
    };
    user.load_from_string(&painted_css(
        "popover, popover > contents, popover > arrow",
        "none",
    ));
    let (window, anchor) = anchor();
    let content = CaptureSurfaceContent::new(&gtk4::Label::new(Some("Popup content")));
    let popup = gtk4::Popover::new();
    popup.set_autohide(false);
    popup.set_child(Some(content.widget()));
    popup.set_parent(&anchor);
    popup.popup();
    wait_for_frame(&popup);
    let size = native_size(&popup);

    // Shadow overrides are deliberately ignored in normal and capture modes.
    // Assert the policy before capture too, including smaller/absent shadows:
    // changing extents at capture time would withdraw this native surface.
    user.load_from_string(
        &(painted_css("popover, popover > contents, popover > arrow", "none")
            + &format!("popover > contents {{ box-shadow: {shadow}; }}")),
    );
    wait_for_frame(&popup);
    assert_eq!(
        native_size(&popup),
        size,
        "stable application shadow policy"
    );
    let normal = settled_pixels(&popup);
    assert_fixture(&normal);

    for generation in 1..=2 {
        set_popover_capture_transparent(&popup, &content, true, false);
        prove(
            generation,
            vec![CaptureProofTarget::new_withdrawable(
                "pixel-popup",
                &popup,
                &content,
            )],
        );
        assert_clear(&popup, case);
        assert_eq!(native_size(&popup), size);
        assert!(content.proof_visible());
        set_popover_capture_transparent(&popup, &content, false, true);
        wait_for_frame(&popup);
        assert_restored(&popup, &normal, case);
        assert_eq!(native_size(&popup), size);
    }
    popup.popdown();
    popup.unparent();
    window.destroy();
}

pub(super) fn menu(user: &gtk4::CssProvider, case: &str) {
    let painted = !matches!(case, "menu-default" | "menu-long");
    let long = case == "menu-long";
    user.load_from_string(&menu_css(case));

    let (window, anchor) = anchor();
    let menu = menu_widget(&anchor, case == "menu-decorated", long);
    let original_child = menu.child();
    let original_parent = original_child.as_ref().unwrap().parent();
    assert!(
        original_parent.is_some(),
        "GTK owns the original menu child"
    );
    let scrolled = original_child
        .as_ref()
        .unwrap()
        .downcast_ref::<gtk4::ScrolledWindow>()
        .unwrap();

    menu.popup();
    wait_for_frame(&menu);
    let original_request = original_child.as_ref().unwrap().size_request();
    if long {
        assert_menu_scrollable(scrolled);
    }
    let original_max_content_height = scrolled.max_content_height();
    let original_policy = scrolled.policy();
    let original_propagation = (
        scrolled.propagates_natural_width(),
        scrolled.propagates_natural_height(),
    );

    let normal = settled_pixels(&menu);
    let size = native_size(&menu);
    assert!(
        normal.iter().any(|pixel| pixel[3] != 0),
        "visible menu fixture"
    );
    if painted {
        assert_fixture(&normal);
    }
    let original_model = menu.menu_model();

    let capture = TooltipCapture::new();
    capture.set_suppressed(true);
    capture.install_tree(window.upcast_ref());
    let targets = capture.capture_popover_targets();
    assert_eq!(targets.len(), 1);
    prove(1, targets);
    assert_clear(&menu, "GTK-owned menu intermediaries and proof");
    assert_eq!(native_size(&menu), size, "menu native size during proof");
    assert_eq!(menu.child(), original_child);
    assert_eq!(original_child.as_ref().unwrap().parent(), original_parent);

    capture.set_suppressed(false);
    wait_for_frame(&menu);
    assert_restored(&menu, &normal, "GTK-owned menu");
    assert_eq!(menu.menu_model(), original_model);
    assert_eq!(original_child.as_ref().unwrap().parent(), original_parent);
    assert_eq!(menu.child(), original_child);
    assert_eq!(
        original_child.as_ref().unwrap().size_request(),
        original_request
    );
    assert_eq!(native_size(&menu), size, "restored menu native size");
    assert_eq!(scrolled.max_content_height(), original_max_content_height);
    assert_eq!(scrolled.policy(), original_policy);
    assert_eq!(
        (
            scrolled.propagates_natural_width(),
            scrolled.propagates_natural_height()
        ),
        original_propagation
    );

    if long {
        // Compare the initial scene before exercising restored menu scrolling;
        // mid-scroll restoration is outside this fixture.
        assert_menu_scrolls(&menu, scrolled);
        assert_eq!(native_size(&menu), size);
    }

    menu.popdown();
    menu.unparent();
    window.destroy();
}

fn menu_css(case: &str) -> String {
    match case {
        "menu-default" | "menu-long" => String::new(),
        "menu" | "menu-decorated" => {
            // Preserve the adversarial fixtures that paint every GTK-owned
            // intermediary and explicitly zero the scrollbar's theme metrics.
            let mut css = painted_css("popover, popover *", "none")
                + "popover scrollbar, popover scrollbar * { min-width: 0; min-height: 0; padding: 0; margin: 0; }";
            if case == "menu-decorated" {
                css += "popover scrolledwindow { padding: 4px; margin: 2px; }";
            }
            css
        }
        _ => unreachable!("unknown menu fixture: {case}"),
    }
}

fn menu_widget(anchor: &gtk4::Button, decorated: bool, long: bool) -> gtk4::PopoverMenu {
    let model = gtk4::gio::Menu::new();
    if long {
        for item in 0..60 {
            model.append(Some(&format!("Copy item {item}")), Some("clipboard.copy"));
        }
    } else {
        model.append(Some("Copy"), Some("clipboard.copy"));
    }
    let menu = gtk4::PopoverMenu::from_model(Some(&model));
    menu.set_autohide(false);
    menu.set_parent(anchor);
    if long {
        // Bound the initial popup so compositor placement need not shrink it.
        menu.child()
            .unwrap()
            .downcast_ref::<gtk4::ScrolledWindow>()
            .unwrap()
            .set_max_content_height(240);
    }
    if decorated {
        let child = menu.child().unwrap();
        child.set_margin_start(3);
        child.set_margin_end(5);
        child.set_margin_top(4);
        child.set_margin_bottom(6);
        child.set_size_request(180, 100);
    }

    menu
}

fn assert_menu_scrollable(scrolled: &gtk4::ScrolledWindow) {
    let adjustment = scrolled.vadjustment();
    assert!(adjustment.page_size() > 0.0, "allocated GTK viewport");
    assert!(
        adjustment.upper() > adjustment.page_size(),
        "60-item fixture must overflow the allocated GTK viewport"
    );
}

fn assert_menu_scrolls(menu: &gtk4::PopoverMenu, scrolled: &gtk4::ScrolledWindow) {
    assert_menu_scrollable(scrolled);
    let adjustment = scrolled.vadjustment();
    adjustment.set_value((adjustment.upper() - adjustment.page_size()) / 2.0);
    assert!(adjustment.value() > 0.0, "long menu must actually scroll");
    wait_for_frame(menu);
}
