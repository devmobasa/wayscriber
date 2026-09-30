use super::*;
use std::future::{Future, poll_fn};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::task::Poll;

const CHILD_ENV: &str = "WAYSCRIBER_GTK_POPUP_PRESENTATION_CHILD";
const TEST_GENERATION: u64 = 1;
const FIXTURE_WAIT: Duration = Duration::from_secs(2);

#[test]
fn popup_capture_waits_for_its_own_render_after_a_shared_clock_paint() {
    let name = concat!(
        module_path!(),
        "::popup_capture_waits_for_its_own_render_after_a_shared_clock_paint"
    );
    if !run_in_private_display(name) {
        return;
    }

    gtk4::init().expect("private GTK display");
    let (window, anchor) = popup_parent();
    let content = CaptureSurfaceContent::new(&gtk4::Label::new(Some("Popup content")));
    let popover = gtk4::Popover::new();
    popover.set_autohide(false);
    popover.set_has_arrow(false);
    popover.set_child(Some(content.widget()));
    popover.set_parent(&anchor);
    window.present();

    gtk4::glib::MainContext::default().block_on(async {
        wait_for_native_mapping(window.upcast_ref()).await;
        require_gl_renderer(&window);

        popover.popup();
        wait_for_native_mapping(popover.upcast_ref()).await;
        wait_for_initial_popup_callback().await;

        control("arm");
        content.set_transparent(true);
        let target = CaptureProofTarget::new_withdrawable("canvas", &popover, &content);
        assert_popup_proof(&window, target).await;
    });

    content.set_transparent(false);
    popover.popdown();
    popover.unparent();
    window.destroy();
    eprintln!("EXECUTED: GTK native popup presentation regression");
}

#[test]
fn menu_capture_presents_its_empty_proof_and_restores_the_menu() {
    let name = concat!(
        module_path!(),
        "::menu_capture_presents_its_empty_proof_and_restores_the_menu"
    );
    if !run_in_private_display(name) {
        return;
    }

    gtk4::init().expect("private GTK display");
    let (window, anchor) = popup_parent();
    let model = gtk4::gio::Menu::new();
    model.append(Some("Copy"), Some("clipboard.copy"));
    let menu = gtk4::PopoverMenu::from_model(Some(&model));
    menu.set_autohide(false);
    menu.set_has_arrow(false);
    menu.set_parent(&anchor);
    let original_child = menu.child().expect("menu child");
    let original_model = menu.menu_model();
    let capture = TooltipCapture::new();
    window.present();

    gtk4::glib::MainContext::default().block_on(async {
        wait_for_native_mapping(window.upcast_ref()).await;
        require_gl_renderer(&window);

        menu.popup();
        wait_for_native_mapping(menu.upcast_ref()).await;
        wait_for_initial_popup_callback().await;

        control("arm");
        capture.set_suppressed(true);
        capture.install_tree(window.upcast_ref());
        let mut targets = capture.capture_popover_targets();
        assert_eq!(targets.len(), 1, "mapped native menu must be enrolled");
        let target = targets.pop().unwrap();
        assert_eq!(target.content.content_opacity(), None);
        assert_eq!(menu.child().as_ref(), Some(&original_child));

        assert_popup_proof(&window, target).await;
        capture.mark_capture_popovers_proven();
        assert!(capture.pending_capture_popover_targets().is_empty());
    });

    capture.set_suppressed(false);
    assert_eq!(menu.menu_model(), original_model);
    assert_eq!(menu.child().as_ref(), Some(&original_child));
    menu.popdown();
    menu.unparent();
    window.destroy();
    eprintln!("EXECUTED: GTK native menu presentation assertions");
}

fn run_in_private_display(test_name: &str) -> bool {
    if std::env::var_os(CHILD_ENV).is_some() {
        return true;
    }
    if std::env::var_os("WAYSCRIBER_REQUIRE_GTK_TESTS").is_none() {
        eprintln!("skipping native popup presentation test: run tools/test-gtk-widgets.sh");
        return false;
    }

    let test_name = test_name
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .expect("test name contains the crate prefix");
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/test-fixtures/gtk_popup_wayland.py"
    );
    let output = std::process::Command::new("python3")
        .arg(fixture)
        .arg(std::env::current_exe().expect("test binary"))
        .arg(test_name)
        .env(CHILD_ENV, "1")
        .output()
        .expect("run private Wayland popup fixture");
    std::io::stdout().write_all(&output.stdout).unwrap();
    std::io::stderr().write_all(&output.stderr).unwrap();

    assert!(
        output.status.success(),
        "native popup presentation test failed"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("test result: ok. 1 passed; 0 failed;"),
        "private fixture must execute exactly one passing test"
    );

    false
}

fn popup_parent() -> (gtk4::Window, gtk4::Button) {
    let window = gtk4::Window::new();
    window.set_default_size(320, 200);
    let anchor = gtk4::Button::with_label("Canvas");
    window.set_child(Some(&anchor));

    (window, anchor)
}

fn require_gl_renderer(window: &gtk4::Window) {
    assert!(
        window
            .renderer()
            .expect("window renderer")
            .is::<gtk4::gsk::GLRenderer>()
    );
}

async fn wait_for_native_mapping(widget: &gtk4::Widget) {
    let deadline = Instant::now() + FIXTURE_WAIT;
    loop {
        if widget_native_is_mapped(widget) && widget.width() > 0 && widget.height() > 0 {
            return;
        }

        assert!(
            Instant::now() < deadline,
            "native surface did not map with nonzero bounds"
        );
        gtk4::glib::timeout_future(CAPTURE_PAINT_POLL_INTERVAL).await;
    }
}

async fn wait_for_initial_popup_callback() {
    let deadline = Instant::now() + FIXTURE_WAIT;
    while control("status").popup_callbacks_delivered == 0 {
        assert!(
            Instant::now() < deadline,
            "initial popup frame did not complete"
        );

        gtk4::glib::timeout_future(CAPTURE_PAINT_POLL_INTERVAL).await;
    }
}

async fn assert_popup_proof(window: &gtk4::Window, target: CaptureProofTarget) {
    let popup = target.widget.clone();
    let clock = popup.frame_clock().expect("popup frame clock");
    assert_eq!(window.frame_clock().as_ref(), Some(&clock));
    wait_for_held_popup_callback().await;

    let held_commits = control("status").popup_commits;
    let shared_paint = Rc::new(Cell::new(None));
    let paint_observation = Rc::clone(&shared_paint);
    let handler = clock.connect_after_paint(move |clock| {
        paint_observation.set(Some(clock.frame_counter()));
    });
    let proof = wait_for_presented_transparency(TEST_GENERATION, vec![target]);
    let mut proof = std::pin::pin!(proof);

    // Arm the real capture wait before asking the shared clock to paint.
    // A separately spawned task could start after that paint and miss the race.
    assert!(
        poll_fn(|cx| Poll::Ready(proof.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    clock.request_phase(gtk4::gdk::FrameClockPhase::PAINT);

    let deadline = Instant::now() + Duration::from_millis(250);
    while shared_paint.get().is_none() {
        assert!(Instant::now() < deadline, "shared clock did not paint");
        gtk4::glib::timeout_future(CAPTURE_PAINT_POLL_INTERVAL).await;
    }
    clock.disconnect(handler);

    assert!(
        poll_fn(|cx| Poll::Ready(proof.as_mut().poll(cx)))
            .await
            .is_pending(),
        "a parent paint cannot admit the popup"
    );
    let status = control("status");
    assert!(status.held);
    assert_eq!(status.popup_commits, held_commits);

    control("release");
    proof.await.expect(
        "the fresh popup render must complete capture after the previous callback is released",
    );

    let status = control("status");
    assert!(status.popup_commits > held_commits);
    assert!(status.popup_callbacks_delivered >= 2);
    assert!(widget_native_is_mapped(&popup));
}

async fn wait_for_held_popup_callback() {
    let deadline = Instant::now() + FIXTURE_WAIT;
    while !control("status").held {
        assert!(
            Instant::now() < deadline,
            "popup never submitted the held callback"
        );

        gtk4::glib::timeout_future(CAPTURE_PAINT_POLL_INTERVAL).await;
    }
}

#[derive(Debug, serde::Deserialize)]
struct ProxyStatus {
    held: bool,
    popup_commits: u64,
    popup_callbacks_delivered: u64,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum ProxyResponse {
    Status(ProxyStatus),
    Error { error: String },
}

fn control(command: &str) -> ProxyStatus {
    let path = std::env::var_os("WAYSCRIBER_GTK_WAYLAND_CONTROL").expect("private proxy control");
    let mut socket = UnixStream::connect(path).expect("connect proxy control");
    socket.set_read_timeout(Some(FIXTURE_WAIT)).unwrap();

    writeln!(socket, "{command}").unwrap();
    let mut response = String::new();
    BufReader::new(socket).read_line(&mut response).unwrap();

    match serde_json::from_str(&response).expect("proxy response") {
        ProxyResponse::Status(status) => status,
        ProxyResponse::Error { error } => panic!("proxy error: {error}"),
    }
}
