//! Actual GTK snapshots on the repository's isolated headless compositor.

use super::super::{FeedbackSender, ToolbarSnapshot, TopBar};
use super::gtk_contract::install_gtk_contract_metrics;
use super::widget_support::find_widget_named;
use crate::input::state::test_support::make_test_input_state;
use gtk4::prelude::*;
use std::time::{Duration, Instant};

#[test]
fn gtk_pen_feel_headless_render_at_fractional_scale() {
    // A window is allowed only on the dedicated test compositor, never on
    // the user's display. Ordinary library runs skip this render check.
    if std::env::var_os("WAYSCRIBER_REQUIRE_GTK_TESTS").is_none()
        || std::env::var("WAYLAND_DISPLAY").as_deref() != Ok("wayscriber-widget-tests")
    {
        return;
    }

    const CHILD_ENV: &str = "WAYSCRIBER_GTK_PEN_RENDER_CHILD";
    const TEST_NAME: &str = "toolbar_gtk::view::top_bar::tests::pen_feel_render::gtk_pen_feel_headless_render_at_fractional_scale";
    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([TEST_NAME, "--exact", "--test-threads=1", "--nocapture"])
            .env(CHILD_ENV, "1")
            .status()
            .expect("run isolated GTK render test");
        assert!(status.success(), "isolated GTK render test failed");
        return;
    }

    gtk4::init().expect("headless GTK initialization");
    install_gtk_contract_metrics();
    let mut state = make_test_input_state();
    state.set_pen_smoothing(3);
    let snapshot = ToolbarSnapshot::from_input(&state);
    let (tx, _) = std::sync::mpsc::channel();
    let top = TopBar::new_for_test(FeedbackSender::new(tx));

    for scale in [1.0, 5.0 / 3.0] {
        let provider = gtk4::CssProvider::new();
        provider.load_from_string(&crate::toolbar_gtk::css::stylesheet(scale));
        let display = gtk4::gdk::Display::default().unwrap();
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let (content, updaters) = top.build_pen_feel_content(&snapshot, scale);
        let window = gtk4::Window::builder().child(&content).build();
        window.add_css_class("wayscriber-toolbar");
        window.present();
        wait_for_frame(&content);
        assert!(content.width() > 0 && content.height() > 0);
        let dot = find_widget_named(content.upcast_ref(), "top.feel.smoothing.level-0")
            .expect("zero dot");
        let origin = dot
            .compute_point(&content, &gtk4::graphene::Point::new(0.0, 0.0))
            .expect("dot in panel");
        let controllers = dot.observe_controllers();
        let motion = (0..controllers.n_items())
            .find_map(|i| {
                controllers
                    .item(i)?
                    .downcast::<gtk4::EventControllerMotion>()
                    .ok()
            })
            .expect("dot hover controller");
        let resting = snapshot_pixels(&content);
        motion.emit_by_name::<()>("enter", &[&6.0f64, &10.0f64]);
        wait_for_frame(&content);
        let hovered = snapshot_pixels(&content);

        let mut widths = Vec::new();
        for level in 1..=6 {
            let bar = find_widget_named(
                content.upcast_ref(),
                &format!("top.feel.smoothing.level-{level}"),
            )
            .expect("smoothing bar");
            let fill = bar.first_child().expect("bar fill");
            widths.push(bar.width());
            assert!(
                f64::from(bar.width() - fill.width()) >= 2.0 * scale,
                "GTK bar {level} retains the gap at scale {scale}: slot={} fill={}",
                bar.width(),
                fill.width()
            );
        }
        assert!(
            widths.iter().max().unwrap() - widths.iter().min().unwrap() <= 1,
            "GTK bar widths at scale {scale}: {widths:?}"
        );
        let ring_x = (f64::from(origin.x()) + (6.0 + 7.5) * scale).round() as usize;
        let ring_y = (f64::from(origin.y()) + f64::from(dot.height()) / 2.0).round() as usize;
        assert_ne!(
            pixel(&resting, ring_x, ring_y),
            pixel(&hovered, ring_x, ring_y),
            "GTK hover ring at scale {scale}"
        );

        let mut off = snapshot.clone();
        off.pen_smoothing = 0;
        for updater in &updaters {
            updater(&off);
        }
        wait_for_frame(&content);
        let active_hovered = snapshot_pixels(&content);
        motion.emit_by_name::<()>("leave", &[]);
        wait_for_frame(&content);
        let active_resting = snapshot_pixels(&content);
        assert_eq!(
            pixel(&active_hovered, ring_x, ring_y),
            pixel(&active_resting, ring_x, ring_y),
            "GTK level zero has no hover ring at scale {scale}"
        );

        window.close();
        gtk4::style_context_remove_provider_for_display(&display, &provider);
    }
    eprintln!("EXECUTED: GTK Pen feel fractional-scale render assertions");
}

fn wait_for_frame(widget: &impl IsA<gtk4::Widget>) {
    let context = gtk4::glib::MainContext::default();
    let deadline = Instant::now() + Duration::from_secs(3);
    let clock = loop {
        while context.pending() {
            context.iteration(false);
        }
        if let Some(clock) = widget.as_ref().frame_clock() {
            break clock;
        }
        assert!(Instant::now() < deadline, "GTK widget has no frame clock");
        std::thread::sleep(Duration::from_millis(2));
    };
    let painted = std::rc::Rc::new(std::cell::Cell::new(false));
    let observed = painted.clone();
    let handler = clock.connect_after_paint(move |_| observed.set(true));
    widget.as_ref().queue_draw();
    while !painted.get() {
        while context.pending() {
            context.iteration(false);
        }
        assert!(
            Instant::now() < deadline,
            "GTK did not paint the requested frame"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    clock.disconnect(handler);
}

fn snapshot_pixels(widget: &impl IsA<gtk4::Widget>) -> cairo::ImageSurface {
    let widget = widget.as_ref();
    let snapshot = gtk4::Snapshot::new();
    gtk4::WidgetPaintable::new(Some(widget)).snapshot(
        &snapshot,
        f64::from(widget.width()),
        f64::from(widget.height()),
    );
    let node = snapshot.to_node().expect("GTK render node");
    let surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, widget.width(), widget.height())
            .unwrap();
    node.draw(&cairo::Context::new(&surface).unwrap());
    surface
}

fn pixel(surface: &cairo::ImageSurface, x: usize, y: usize) -> [u8; 4] {
    let stride = surface.stride() as usize;
    let mut pixel = [0; 4];
    surface
        .with_data(|bytes| {
            pixel.copy_from_slice(&bytes[y * stride + x * 4..y * stride + x * 4 + 4])
        })
        .unwrap();
    pixel
}
