//! Private-display pixel tests. Each case has a fresh GTK process so failures
//! in native popup proof cannot hide window, wrapper, or menu regressions.

mod lifecycle;
mod popups;
mod subprocess;
mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::{env, process};

use gtk4::gdk::Display;

use crate::toolbar_gtk::css;

use super::super::{FeedbackSender, TopBar};
use crate::toolbar_gtk::view::install_capture_css_provider;
use gtk4::prelude::*;
use subprocess::{CHILD_ENV, Outcome};
use support::*;

#[test]
fn capture_roots_override_user_css_and_restore_rendered_pixels() {
    if env::var_os("WAYSCRIBER_REQUIRE_GTK_TESTS").is_none()
        || env::var("WAYLAND_DISPLAY").as_deref() != Ok("wayscriber-widget-tests")
    {
        return;
    }

    if env::var_os(CHILD_ENV).is_none() {
        let name = concat!(
            module_path!(),
            "::capture_roots_override_user_css_and_restore_rendered_pixels"
        );
        let name = name.strip_prefix("wayscriber::").unwrap();
        subprocess::run_cases(name);
        return;
    }

    gtk4::init().expect("private GTK display");
    // Production GTK defaults allow animations. A 400ms CSS transition must
    // not outlive the first acknowledged transparent frame.
    gtk4::Settings::default()
        .unwrap()
        .set_gtk_enable_animations(true);
    gtk4::Settings::default()
        .unwrap()
        .set_gtk_font_name(Some("Sans 11"));
    gtk4::Settings::default()
        .unwrap()
        .set_gtk_xft_dpi(96 * 1024);
    let display = Display::default().unwrap();
    let application = gtk4::CssProvider::new();
    application.load_from_string(&css::stylesheet(1.0));
    gtk4::style_context_add_provider_for_display(
        &display,
        &application,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let capture = install_capture_css_provider(1.0);
    let user = gtk4::CssProvider::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let observed = errors.clone();
    user.connect_parsing_error(move |_, _, error| observed.borrow_mut().push(error.to_string()));
    gtk4::style_context_add_provider_for_display(
        &display,
        &user,
        gtk4::STYLE_PROVIDER_PRIORITY_USER,
    );

    let case = env::var(CHILD_ENV).unwrap();
    eprintln!(
        "capture case: {case}, renderer: {}",
        env::var("GSK_RENDERER").unwrap()
    );
    let renderer = env::var("GSK_RENDERER").unwrap();
    let mut outcome = Outcome::Passed;
    match case.as_str() {
        "renderer-probe" => {
            let window = gtk4::Window::builder()
                .default_width(32)
                .default_height(32)
                .build();
            window.present();
            wait_for_frame(&window);
            let renderer = window.renderer().unwrap();
            eprintln!("optional Vulkan probe selected {}", renderer.type_().name());
            if renderer.type_().name() == "GskVulkanRenderer" {
                assert_renderer(&window);
            } else {
                outcome = Outcome::VulkanUnavailable;
            }
            window.destroy();
        }
        case if case.starts_with("menu") => popups::menu(&user, case),
        "lifecycle" => lifecycle::apply_paths(&user),
        case if case.starts_with("popup-") => popups::popup(&user, case),
        case => roots(&user, case),
    }
    assert!(
        errors.borrow().is_empty(),
        "user CSS fixture parse errors: {:?}",
        errors.borrow()
    );
    for provider in [&user, &capture, &application] {
        gtk4::style_context_remove_provider_for_display(&display, provider);
    }

    subprocess::complete(&renderer, &case, &outcome);
    if outcome == Outcome::VulkanUnavailable {
        process::exit(77);
    }
}

fn roots(user: &gtk4::CssProvider, case: &str) {
    let (tx, _) = mpsc::channel();
    let top = TopBar::new_for_test(FeedbackSender::new(tx));
    top.root.set_size_request(160, 64);
    top.window.set_decorated(false);
    let selector = match case {
        "window" | "transition" | "animation" | "border-image" => "window",
        "wrapper" | "wrapper-minimum" => "overlay.wayscriber-capture-surface-content",
        "picture" | "proof-geometry" => "picture",
        "universal" => "*",
        _ => unreachable!(),
    };
    let transition = if case == "transition" {
        "transition: background-color 400ms linear, box-shadow 400ms linear;"
    } else {
        ""
    };
    let image = if case == "border-image" {
        "border-color: transparent; border-image-source: linear-gradient(90deg, #11e811, #11e811); border-image-slice: 1;"
    } else {
        ""
    };
    let metrics = if case == "wrapper-minimum" {
        "min-width: 360px; min-height: 120px; padding: 5px; margin: 2px;"
    } else if case == "proof-geometry" {
        "border-width: 9px; padding: 8px; margin: 3px;"
    } else {
        ""
    };
    user.load_from_string(&format!(
        "{}{selector} {{ {transition} {image} {metrics} }}",
        painted_css(selector, "0 2px 6px #58c0e8")
    ));
    if case == "animation" {
        let mut css = painted_css("window", "none");
        css.push_str(
            r#"
            @keyframes capture-fixture {
                from { background-color: #23252e; }
                to { background-color: #e86b18; }
            }
            window { background-image: none; animation: capture-fixture 400ms linear infinite alternate; }
        "#,
        );
        user.load_from_string(&css);
    }
    top.window.present();
    wait_for_frame(&top.window);
    assert_renderer(&top.window);
    if case == "animation" {
        assert_fixture(&pixels(&top.window));
        assert_animated(&top.window);
        let size = native_size(&top.window);
        for generation in 1..=2 {
            top.set_capture_transparent(true);
            prove(generation, vec![top.capture_target()]);
            assert_clear(&top.window, "active keyframe animation");
            assert_eq!(native_size(&top.window), size);
            top.set_capture_transparent(false);
            wait_for_frame(&top.window);
            assert_fixture(&pixels(&top.window));
            assert_animated(&top.window);
        }
        top.window.destroy();
        return;
    }
    // The proof Picture is normally invisible. Reveal it without capture CSS
    // to prove this fixture paints it before testing the guarded presentation.
    if matches!(case, "picture" | "proof-geometry" | "universal") {
        top.capture_surface.set_transparent(true);
        top.capture_surface
            .widget()
            .remove_css_class(css::CAPTURE_TRANSPARENT_CLASS);
        wait_for_frame(&top.window);
        assert_fixture(&pixels(&top.capture_surface.proof));
        top.capture_surface.set_transparent(false);
        wait_for_frame(&top.window);
    }
    let normal = settled_pixels(&top.window);
    if !matches!(case, "picture" | "proof-geometry") {
        assert_fixture(&normal);
    }
    let size = native_size(&top.window);

    for generation in 1..=2 {
        top.set_capture_transparent(true);
        prove(generation, vec![top.capture_target()]);
        // No transition-duration settling occurs after the proof waiter.
        assert_clear(&top.window, case);
        assert_clear(top.capture_surface.widget(), "proof wrapper");
        assert!(top.capture_surface.proof_visible());
        assert_eq!(native_size(&top.window), size);

        top.set_capture_transparent(false);
        wait_for_frame(&top.window);
        assert_restored(&top.window, &normal, case);
    }
    top.window.destroy();
}
