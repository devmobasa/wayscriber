use std::cell::Cell;
use std::rc::Rc;
use std::{env, thread};

use gtk4::gdk::FrameClockPhase;
use gtk4::glib::MainContext;
use gtk4::graphene::Rect;
use gtk4::gsk::{CairoRenderer, GLRenderer};

use crate::toolbar_gtk::view::capture_suppression::wait_for_presented_transparency;
use crate::toolbar_gtk::view::{CaptureProofTarget, widget_native_is_mapped};
use gtk4::prelude::*;
use std::time::{Duration, Instant};

pub(super) fn prove(generation: u64, targets: Vec<CaptureProofTarget>) {
    MainContext::default()
        .block_on(wait_for_presented_transparency(generation, targets))
        .expect("transparent native buffer presentation");
}

pub(super) fn native_size(widget: &impl IsA<gtk4::Widget>) -> (i32, i32) {
    let surface = widget.as_ref().native().unwrap().surface().unwrap();
    assert!(surface.is_mapped(), "native surface remains mapped");
    (surface.width(), surface.height())
}

pub(super) fn assert_renderer(window: &gtk4::Window) {
    let renderer = window.renderer().unwrap();
    match env::var("GSK_RENDERER").unwrap().as_str() {
        "cairo" => assert!(renderer.is::<CairoRenderer>()),
        "gl" => assert!(renderer.is::<GLRenderer>()),
        "vulkan" => assert_eq!(renderer.type_().name(), "GskVulkanRenderer"),
        _ => unreachable!(),
    }
}

pub(super) fn painted_css(selector: &str, shadow: &str) -> String {
    format!(
        "{selector} {{ background-color: #23252e; background-image: linear-gradient(90deg, #23252e, #e86b18); border: 3px solid #11e811; outline: 2px solid #c72bd9; box-shadow: {shadow}; }}"
    )
}

pub(super) fn assert_fixture(pixels: &[[u8; 4]]) {
    assert!(
        pixels
            .iter()
            .filter(|pixel| **pixel == [17, 232, 17, 255])
            .count()
            > 10,
        "opaque green user border must override application CSS"
    );
}

pub(super) fn assert_animated(widget: &impl IsA<gtk4::Widget>) {
    let initial = pixels(widget);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        wait_for_frame(widget);
        if pixels(widget) != initial {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "user keyframe animation did not advance"
        );
    }
}

pub(super) fn assert_clear(widget: &impl IsA<gtk4::Widget>, label: &str) {
    let pixels = pixels(widget);
    let painted = pixels.iter().filter(|pixel| pixel[3] != 0).count();
    assert_eq!(painted, 0, "{label} leaked pixels, including surface edges");
}

pub(super) fn assert_restored(widget: &impl IsA<gtk4::Widget>, expected: &[[u8; 4]], label: &str) {
    // Removing the guard restores the user's transitions too. Only normal
    // restoration may settle; capture pixels are checked immediately after
    // the production presentation waiter, without an extra frame or delay.
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let actual = pixels(widget);
        let changed = actual.iter().zip(expected).filter(|(a, b)| a != b).count();
        if actual.len() == expected.len() && changed == 0 {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{label} failed to restore {changed} user CSS pixels and bounds {} vs {}",
            actual.len(),
            expected.len()
        );
        wait_for_frame(widget);
    }
}

pub(super) fn settled_pixels(widget: &impl IsA<gtk4::Widget>) -> Vec<[u8; 4]> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut previous = pixels(widget);
    let mut stable_since = Instant::now();

    // Popup layout can change after several identical frames. Require a
    // quiet interval before saving the normal pixels used after capture.
    loop {
        wait_for_frame(widget);
        let current = pixels(widget);
        let now = Instant::now();
        if current != previous {
            stable_since = now;
        } else if now.duration_since(stable_since) >= Duration::from_millis(200) {
            return current;
        }
        assert!(now < deadline, "normal fixture did not settle");
        previous = current;
    }
}

pub(super) fn pixels(widget: &impl IsA<gtk4::Widget>) -> Vec<[u8; 4]> {
    let widget = widget.as_ref();
    let deadline = Instant::now() + Duration::from_secs(3);
    let node = loop {
        let snapshot = gtk4::Snapshot::new();
        gtk4::WidgetPaintable::new(Some(widget)).snapshot(
            &snapshot,
            f64::from(widget.width()),
            f64::from(widget.height()),
        );
        if let Some(node) = snapshot.to_node() {
            break node;
        }

        // A newly mapped popup shares the parent's clock. Its first parent
        // paint can precede its own initial snapshot, especially with GL.
        assert!(Instant::now() < deadline, "nonempty native render node");
        wait_for_frame(widget);
    };
    // WidgetPaintable clips to the allocation. The margin checks transparent
    // render bounds only; native popup geometry is asserted separately.
    let viewport = Rect::new(
        -16.0,
        -16.0,
        widget.width() as f32 + 32.0,
        widget.height() as f32 + 32.0,
    );
    let texture = widget
        .native()
        .unwrap()
        .renderer()
        .unwrap()
        .render_texture(&node, Some(&viewport));
    let stride = texture.width() as usize * 4;
    let mut bytes = vec![0; stride * texture.height() as usize];
    texture.download(&mut bytes, stride);

    let (pixels, remainder) = bytes.as_chunks::<4>();
    assert!(remainder.is_empty(), "complete RGBA texture pixels");
    pixels.to_vec()
}

pub(super) fn wait_for_frame(widget: &impl IsA<gtk4::Widget>) {
    let context = MainContext::default();
    let deadline = Instant::now() + Duration::from_secs(3);
    let clock = loop {
        while context.pending() {
            context.iteration(false);
        }
        if let Some(clock) = widget.as_ref().frame_clock()
            && widget_native_is_mapped(widget.as_ref())
            && widget.as_ref().width() > 0
            && widget.as_ref().height() > 0
        {
            break clock;
        }
        assert!(Instant::now() < deadline, "GTK widget did not map");
        thread::sleep(Duration::from_millis(2));
    };
    let painted = Rc::new(Cell::new(false));
    let observed = painted.clone();
    let handler = clock.connect_after_paint(move |_| observed.set(true));
    widget.as_ref().queue_draw();
    clock.request_phase(FrameClockPhase::PAINT);
    while !painted.get() {
        while context.pending() {
            context.iteration(false);
        }
        assert!(Instant::now() < deadline, "GTK did not paint");
        thread::sleep(Duration::from_millis(2));
    }
    clock.disconnect(handler);
}
