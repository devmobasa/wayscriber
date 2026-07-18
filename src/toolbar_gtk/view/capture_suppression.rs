use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::prelude::*;

use super::{
    CaptureProofTarget, CaptureSurfaceContent, after_next_surface_paint_counter,
    widget_native_is_mapped,
};
use crate::toolbar_gtk::css::CAPTURE_TRANSPARENT_CLASS;

const CAPTURE_PAINT_TIMEOUT: Duration = Duration::from_millis(500);
const CAPTURE_PAINT_POLL_INTERVAL: Duration = Duration::from_millis(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturePresentationState {
    Pending,
    Presented,
    CompositorFrame,
}

/// Owns the one custom tooltip renderer shared by Wayscriber's GTK display.
///
/// GTK itself owns one private tooltip window per display. Installing the same
/// custom widget from every Wayscriber tooltip query gives capture suppression
/// a public-API handle to that native popup without depending on its private
/// `GtkTooltipWindow` type.
#[derive(Clone)]
pub(super) struct TooltipCapture {
    inner: Rc<TooltipCaptureInner>,
}

struct TooltipCaptureInner {
    content: CaptureSurfaceContent,
    label: gtk4::Label,
    installed_sources: RefCell<Vec<gtk4::glib::WeakRef<gtk4::Widget>>>,
    active_source: RefCell<Option<gtk4::glib::WeakRef<gtk4::Widget>>>,
    suppressed: Cell<bool>,
    mapped_before_capture: Cell<bool>,
}

impl TooltipCapture {
    pub(super) fn new() -> Self {
        let label = gtk4::Label::new(None);
        label.set_wrap(true);
        let content = CaptureSurfaceContent::new(&label);
        Self {
            inner: Rc::new(TooltipCaptureInner {
                content,
                label,
                installed_sources: RefCell::new(Vec::new()),
                active_source: RefCell::new(None),
                suppressed: Cell::new(false),
                mapped_before_capture: Cell::new(false),
            }),
        }
    }

    /// Install the custom query handler on every tooltipped descendant.
    /// Rebuilds can replace toolbar controls, so dead weak references are
    /// pruned and newly created controls are enrolled after every update.
    pub(super) fn install_tree(&self, root: &gtk4::Widget) {
        let mut pending = vec![root.clone()];
        while let Some(widget) = pending.pop() {
            let mut child = widget.first_child();
            while let Some(current) = child {
                child = current.next_sibling();
                pending.push(current);
            }
            if widget.has_tooltip() {
                self.install_source(&widget);
            }
        }
    }

    fn install_source(&self, source: &gtk4::Widget) {
        let already_installed = {
            let mut installed = self.inner.installed_sources.borrow_mut();
            installed.retain(|source| source.upgrade().is_some());
            installed
                .iter()
                .filter_map(gtk4::glib::WeakRef::upgrade)
                .any(|candidate| candidate == *source)
        };
        if already_installed {
            return;
        }

        self.inner
            .installed_sources
            .borrow_mut()
            .push(source.downgrade());
        let capture = self.clone();
        source.connect_query_tooltip(move |source, _, _, _, tooltip| {
            capture.query_tooltip(source, tooltip)
        });
    }

    fn query_tooltip(&self, source: &gtk4::Widget, tooltip: &gtk4::Tooltip) -> bool {
        let suppressed = self.inner.suppressed.get();
        if suppressed && (!self.inner.mapped_before_capture.get() || !self.active_source_is(source))
        {
            // Do not admit a new popup after capture suppression starts. A
            // tooltip that was already mapped is kept alive and made
            // transparent through the custom content below.
            return false;
        }

        if let Some(text) = source.tooltip_text() {
            self.inner.label.set_text(&text);
        } else if let Some(markup) = source.tooltip_markup() {
            self.inner.label.set_markup(&markup);
        } else {
            return false;
        }

        if !suppressed {
            self.inner.active_source.replace(Some(source.downgrade()));
        }
        self.inner.content.set_transparent(suppressed);
        tooltip.set_custom(Some(self.inner.content.widget()));
        self.sync_native_chrome();
        if !suppressed {
            self.set_input_enabled(true);
        }
        true
    }

    fn active_source_is(&self, source: &gtk4::Widget) -> bool {
        self.inner
            .active_source
            .borrow()
            .as_ref()
            .and_then(gtk4::glib::WeakRef::upgrade)
            .is_some_and(|active| active == *source)
    }

    /// Enter or leave capture mode without unmapping a tooltip that is
    /// already visible. Pending tooltip timers are rejected by the query
    /// handler while suppression is active.
    pub(super) fn set_suppressed(&self, suppressed: bool, defer_input: bool) {
        if suppressed && !self.inner.suppressed.get() {
            self.inner
                .mapped_before_capture
                .set(self.active_native_widget().is_some());
        }
        self.inner.suppressed.set(suppressed);
        self.inner.content.set_transparent(suppressed);
        self.sync_native_chrome();

        if !suppressed {
            self.set_input_enabled(true);
            self.inner.mapped_before_capture.set(false);
        } else if !defer_input {
            self.set_input_enabled(false);
        }
    }

    pub(super) fn set_input_enabled(&self, enabled: bool) {
        let Some(native) = self.native_widget() else {
            return;
        };
        native.set_can_target(enabled);
        if let Some(surface) = native.native().and_then(|native| native.surface()) {
            if enabled {
                surface.set_input_region(None);
            } else {
                let empty = gtk4::cairo::Region::create();
                surface.set_input_region(Some(&empty));
            }
        }
        native.queue_draw();
    }

    pub(super) fn capture_target(&self) -> Option<CaptureProofTarget> {
        self.inner
            .mapped_before_capture
            .get()
            .then(|| self.active_native_widget())
            .flatten()
            .map(|native| CaptureProofTarget::new("tooltip", &native, &self.inner.content))
    }

    fn active_native_widget(&self) -> Option<gtk4::Widget> {
        self.native_widget().filter(widget_native_is_mapped)
    }

    fn native_widget(&self) -> Option<gtk4::Widget> {
        let native = self.inner.content.widget().native()?;
        Some(native.upcast::<gtk4::Widget>())
    }

    fn sync_native_chrome(&self) {
        let Some(native) = self.native_widget() else {
            return;
        };
        if self.inner.suppressed.get() {
            native.add_css_class(CAPTURE_TRANSPARENT_CLASS);
        } else {
            native.remove_css_class(CAPTURE_TRANSPARENT_CLASS);
        }
        native.queue_draw();
    }

    #[cfg(test)]
    pub(super) fn installed_source_count(&self) -> usize {
        self.inner
            .installed_sources
            .borrow()
            .iter()
            .filter(|source| source.upgrade().is_some())
            .count()
    }

    #[cfg(test)]
    pub(super) fn transparent_content_state(&self) -> (Option<f64>, bool) {
        (
            self.inner.content.content_opacity(),
            self.inner.content.proof_visible(),
        )
    }
}

fn capture_presentation_state(timings: Option<(bool, i64)>) -> CapturePresentationState {
    match timings {
        Some((_, presentation_time)) if presentation_time > 0 => {
            CapturePresentationState::Presented
        }
        Some((true, _)) => CapturePresentationState::CompositorFrame,
        Some((false, _)) | None => CapturePresentationState::Pending,
    }
}

pub(super) async fn wait_for_presented_transparency(
    generation: u64,
    targets: Vec<CaptureProofTarget>,
) -> Result<(), String> {
    let started = Instant::now();
    log::info!(
        "capture.preflight id={generation} component=gtk phase=wait-mapped targets={}",
        targets
            .iter()
            .map(|target| target.name)
            .collect::<Vec<_>>()
            .join(",")
    );
    while targets
        .iter()
        .any(|target| !widget_native_is_mapped(&target.widget))
    {
        if started.elapsed() >= CAPTURE_PAINT_TIMEOUT {
            let states = targets
                .iter()
                .map(|target| {
                    format!(
                        "{}_mapped={}",
                        target.name,
                        widget_native_is_mapped(&target.widget)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "GTK capture suppression generation {generation} timed out waiting for normally visible toolbar surfaces to map ({states})"
            ));
        }
        gtk4::glib::timeout_future(CAPTURE_PAINT_POLL_INTERVAL).await;
    }

    // Popup surfaces inherit their parent's frame clock. Queueing every proof
    // together can therefore assign the same frame counter to multiple native
    // surfaces and let one surface's feedback satisfy another. Render and
    // confirm one native surface at a time so each counter has one source.
    let proof_started = Instant::now();
    for (ordinal, target) in targets.iter().enumerate() {
        wait_for_surface_presentation(
            generation,
            target,
            ordinal + 1,
            targets.len(),
            started,
            proof_started,
        )
        .await?;
    }

    log::info!(
        "capture.preflight id={generation} component=gtk phase=transparent-commit-confirmed elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

async fn wait_for_surface_presentation(
    generation: u64,
    target: &CaptureProofTarget,
    ordinal: usize,
    target_count: usize,
    started: Instant,
    proof_started: Instant,
) -> Result<(), String> {
    let name = target.name;
    let widget = &target.widget;
    let frame_clock = widget.frame_clock().ok_or_else(|| {
        format!(
            "GTK capture suppression generation {generation} found no frame clock for mapped {name}"
        )
    })?;
    let rendered_counter = Rc::new(Cell::new(None));
    let callback_counter = Rc::clone(&rendered_counter);

    log::info!(
        "capture.preflight id={generation} component=gtk surface={name} phase=proof-requested sequence={ordinal}/{target_count} elapsed_ms={}",
        started.elapsed().as_millis()
    );
    // The initial suppression update dirties every native at once. By the
    // time an earlier target is presented, GTK may already have rendered the
    // later targets; queue_draw() alone can then produce an after-paint phase
    // without a new Wayland buffer or presentation event. Give this target a
    // fresh but still alpha-zero texture before arming its dedicated frame.
    target.refresh_transparent_proof();
    after_next_surface_paint_counter(widget, move |counter| {
        callback_counter.set(counter);
    });

    while rendered_counter.get().is_none() {
        if proof_started.elapsed() >= CAPTURE_PAINT_TIMEOUT {
            return Err(format!(
                "GTK capture suppression generation {generation} timed out waiting for a dedicated transparent render from {name}"
            ));
        }
        gtk4::glib::timeout_future(CAPTURE_PAINT_POLL_INTERVAL).await;
    }

    let frame_counter = rendered_counter
        .get()
        .expect("dedicated transparent render counter recorded");
    log::info!(
        "capture.preflight id={generation} component=gtk surface={name} phase=rendered frame_counter={frame_counter} sequence={ordinal}/{target_count} elapsed_ms={}",
        started.elapsed().as_millis()
    );

    loop {
        let timings = frame_clock.timings(frame_counter);
        let state = capture_presentation_state(
            timings
                .as_ref()
                .map(|timings| (timings.is_complete(), timings.presentation_time())),
        );
        if state != CapturePresentationState::Pending {
            let complete = timings
                .as_ref()
                .is_some_and(|timings| timings.is_complete());
            let presentation_time = timings
                .as_ref()
                .map_or(0, |timings| timings.presentation_time());
            log::info!(
                "capture.preflight id={generation} component=gtk surface={name} phase=presentation status={state:?} frame_counter={frame_counter} complete={complete} presentation_time_us={presentation_time} sequence={ordinal}/{target_count} elapsed_ms={}",
                started.elapsed().as_millis()
            );
            return Ok(());
        }
        if proof_started.elapsed() >= CAPTURE_PAINT_TIMEOUT {
            return Err(format!(
                "generation {generation} received no compositor frame or presentation feedback for {name} after {} ms",
                proof_started.elapsed().as_millis()
            ));
        }
        gtk4::glib::timeout_future(CAPTURE_PAINT_POLL_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_paint_is_not_capture_safe_until_the_compositor_presents_it() {
        assert_eq!(
            capture_presentation_state(None),
            CapturePresentationState::Pending
        );
        assert_eq!(
            capture_presentation_state(Some((false, 0))),
            CapturePresentationState::Pending
        );
        assert_eq!(
            capture_presentation_state(Some((true, 0))),
            CapturePresentationState::CompositorFrame
        );
        assert_eq!(
            capture_presentation_state(Some((false, 42))),
            CapturePresentationState::Presented
        );
    }

    #[test]
    fn capture_aware_tooltips_reuse_one_renderer_and_reject_new_popups() {
        const CHILD_ENV: &str = "WAYSCRIBER_GTK_TOOLTIP_CAPTURE_CHILD";
        const TEST_NAME: &str = "toolbar_gtk::view::capture_suppression::tests::capture_aware_tooltips_reuse_one_renderer_and_reject_new_popups";

        if std::env::var_os(CHILD_ENV).is_none() {
            let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
                .arg(TEST_NAME)
                .arg("--exact")
                .arg("--test-threads=1")
                .env(CHILD_ENV, "1")
                .status()
                .expect("run isolated GTK tooltip capture test");
            assert!(status.success(), "isolated GTK tooltip capture test failed");
            return;
        }

        if let Err(error) = gtk4::init() {
            eprintln!("skipping GTK tooltip capture test: {error}");
            return;
        }

        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let button = gtk4::Button::new();
        button.set_tooltip_text(Some("Zoom"));
        root.append(&button);

        let capture = TooltipCapture::new();
        capture.install_tree(root.upcast_ref());
        capture.install_tree(root.upcast_ref());
        assert_eq!(capture.installed_source_count(), 1);

        let tooltip = gtk4::glib::Object::builder::<gtk4::Tooltip>().build();
        assert!(button.emit_by_name::<bool>("query-tooltip", &[&0i32, &0i32, &false, &tooltip]));
        assert_eq!(capture.inner.label.text().as_str(), "Zoom");
        assert_eq!(capture.transparent_content_state(), (Some(1.0), false));

        capture.set_suppressed(true, true);
        assert_eq!(capture.transparent_content_state(), (Some(0.0), true));
        let proof_before = capture.inner.content.proof.paintable();
        let serial_before = capture.inner.content.proof_serial();
        CaptureProofTarget::new("tooltip", &root, &capture.inner.content)
            .refresh_transparent_proof();
        assert_ne!(capture.inner.content.proof_serial(), serial_before);
        assert_ne!(capture.inner.content.proof.paintable(), proof_before);
        assert!(
            !capture.query_tooltip(button.upcast_ref(), &tooltip),
            "an unmapped tooltip must not appear after suppression starts"
        );

        // The mapped case uses the same query path but remains admitted so its
        // existing native popup can submit the transparent proof.
        capture.inner.mapped_before_capture.set(true);
        assert!(capture.query_tooltip(button.upcast_ref(), &tooltip));

        capture.set_suppressed(false, false);
        assert_eq!(capture.transparent_content_state(), (Some(1.0), false));
    }
}
