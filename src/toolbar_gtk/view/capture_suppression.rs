mod native_popovers;
mod quiescence;

use std::time::{Duration, Instant};

use gtk4::prelude::*;

use super::{
    CaptureProofTarget, CaptureProofWithdrawal, CaptureSurfaceContent,
    after_next_surface_paint_counter, widget_native_is_mapped,
};
use crate::toolbar_gtk::css::CAPTURE_TRANSPARENT_CLASS;
use crate::toolbar_gtk::view::drag::{TooltipIntent, ViewIntent, ViewIntentSender};
use native_popovers::NativePopoverCapture;

const CAPTURE_PAINT_TIMEOUT: Duration = Duration::from_millis(500);
const CAPTURE_PAINT_POLL_INTERVAL: Duration = Duration::from_millis(2);
const TOOLTIP_SOURCE_INSTALLED_CLASS: &str = "wayscriber-tooltip-capture-source";
const TOOLTIP_SOURCE_ACTIVE_CLASS: &str = "wayscriber-tooltip-capture-active";
const TOOLTIP_SUPPRESSED_CLASS: &str = "wayscriber-tooltip-capture-suppressed";
const TOOLTIP_MAPPED_BEFORE_CAPTURE_CLASS: &str = "wayscriber-tooltip-mapped-before-capture";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturePresentationState {
    Pending,
    Presented,
    CompositorFrame,
}

/// Owns capture proof surfaces for GTK-created native popups.
///
/// GTK itself owns one private tooltip window per display. Installing the same
/// custom widget from every Wayscriber tooltip query gives capture suppression
/// a public-API handle to that native popup without depending on its private
/// `GtkTooltipWindow` type. Visible `GtkPopover` descendants created internally
/// by widgets such as `GtkText` are discovered through the public widget tree.
/// Plain popovers receive the same proof wrapper; `GtkPopoverMenu` instead gets
/// a temporary public menu model so its class-owned direct child is untouched.
pub(super) struct TooltipCapture {
    content: CaptureSurfaceContent,
    label: gtk4::Label,
    installed_sources: Vec<(u64, gtk4::glib::WeakRef<gtk4::Widget>)>,
    next_source: u64,
    native_popovers: NativePopoverCapture,
    suppressed: bool,
    popup_input_enabled: bool,
    intents: ViewIntentSender,
}

impl TooltipCapture {
    pub(super) fn new(intents: ViewIntentSender) -> Self {
        let label = gtk4::Label::new(None);
        label.set_wrap(true);
        let content = CaptureSurfaceContent::new(&label);
        Self {
            content,
            label,
            installed_sources: Vec::new(),
            next_source: 0,
            native_popovers: NativePopoverCapture::default(),
            suppressed: false,
            popup_input_enabled: true,
            intents,
        }
    }

    /// Install the custom query handler on every tooltipped descendant.
    /// Rebuilds can replace toolbar controls, so dead weak references are
    /// pruned and newly created controls are enrolled after every update.
    pub(super) fn install_tree(&mut self, root: &gtk4::Widget) {
        let mut pending = vec![root.clone()];
        while let Some(widget) = pending.pop() {
            if self.suppressed
                && let Ok(popover) = widget.clone().downcast::<gtk4::Popover>()
            {
                let selected_for_capture = popover.is_visible() || popover.is_mapped();
                self.enroll_native_popover(&popover, selected_for_capture);
            }
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

    fn enroll_native_popover(&mut self, popover: &gtk4::Popover, selected_for_capture: bool) {
        self.native_popovers
            .enroll(popover, selected_for_capture, self.popup_input_enabled);
    }

    fn install_source(&mut self, source: &gtk4::Widget) {
        self.installed_sources
            .retain(|(_, source)| source.upgrade().is_some());
        let already_installed = self
            .installed_sources
            .iter()
            .filter_map(|(_, source)| source.upgrade())
            .any(|candidate| candidate == *source);
        if already_installed {
            return;
        }

        self.next_source = self.next_source.wrapping_add(1);
        let source_id = self.next_source;
        source.add_css_class(TOOLTIP_SOURCE_INSTALLED_CLASS);
        self.installed_sources.push((source_id, source.downgrade()));
        let content = self.content.clone();
        let label = self.label.clone();
        let intents = self.intents.clone();
        source.connect_query_tooltip(move |source, _, _, _, tooltip| {
            query_tooltip(&content, &label, &intents, source_id, source, tooltip)
        });
    }

    #[cfg(test)]
    fn query_tooltip(&mut self, source: &gtk4::Widget, tooltip: &gtk4::Tooltip) -> bool {
        let Some(source_id) = self.installed_sources.iter().find_map(|(id, candidate)| {
            candidate
                .upgrade()
                .filter(|candidate| candidate == source)
                .map(|_| *id)
        }) else {
            return false;
        };
        let shown = query_tooltip(
            &self.content,
            &self.label,
            &self.intents,
            source_id,
            source,
            tooltip,
        );
        if shown && !self.suppressed {
            self.handle_intent(TooltipIntent::Activated { source: source_id });
            self.set_input_enabled(true);
        }
        self.sync_native_chrome();
        shown
    }

    pub(super) fn handle_intent(&mut self, intent: TooltipIntent) {
        let TooltipIntent::Activated { source } = intent;
        for (id, candidate) in &self.installed_sources {
            let Some(candidate) = candidate.upgrade() else {
                continue;
            };
            candidate.remove_css_class(TOOLTIP_SOURCE_ACTIVE_CLASS);
            if *id == source {
                candidate.add_css_class(TOOLTIP_SOURCE_ACTIVE_CLASS);
            }
        }
    }

    /// Enter or leave capture mode without unmapping a tooltip that is
    /// already visible. Pending tooltip timers are rejected by the query
    /// handler while suppression is active.
    pub(super) fn set_suppressed(&mut self, suppressed: bool) {
        if suppressed && !self.suppressed && self.active_native_widget().is_some() {
            self.content
                .root
                .add_css_class(TOOLTIP_MAPPED_BEFORE_CAPTURE_CLASS);
        }
        self.suppressed = suppressed;
        self.popup_input_enabled = !suppressed;
        if suppressed {
            self.content.root.add_css_class(TOOLTIP_SUPPRESSED_CLASS);
        } else {
            self.content.root.remove_css_class(TOOLTIP_SUPPRESSED_CLASS);
        }
        self.content.set_transparent(suppressed);
        self.sync_native_chrome();
        self.native_popovers.set_suppressed(suppressed);

        if !suppressed {
            self.set_input_enabled(true);
            self.content
                .root
                .remove_css_class(TOOLTIP_MAPPED_BEFORE_CAPTURE_CLASS);
        } else {
            self.set_input_enabled(false);
        }
    }

    pub(super) fn set_input_enabled(&mut self, enabled: bool) {
        self.popup_input_enabled = enabled;
        if let Some(native) = self.native_widget() {
            set_native_widget_input_enabled(&native, enabled);
        }
        self.native_popovers.set_input_enabled(enabled);
    }

    pub(super) fn capture_target(&self) -> Option<CaptureProofTarget> {
        self.content
            .root
            .has_css_class(TOOLTIP_MAPPED_BEFORE_CAPTURE_CLASS)
            .then(|| self.active_native_widget())
            .flatten()
            .map(|native| {
                CaptureProofTarget::new_withdrawable_with_cleanup(
                    "tooltip",
                    &native,
                    &self.content,
                    CaptureProofWithdrawal::RemoveContentClass(TOOLTIP_MAPPED_BEFORE_CAPTURE_CLASS),
                )
            })
    }

    pub(super) fn capture_popover_targets(&mut self) -> Vec<CaptureProofTarget> {
        self.native_popovers.capture_targets()
    }

    pub(super) fn pending_capture_popover_targets(&mut self) -> Vec<CaptureProofTarget> {
        self.native_popovers.pending_capture_targets()
    }

    pub(super) fn mark_capture_popovers_proven(&mut self) {
        self.native_popovers.mark_proven();
    }

    pub(super) async fn wait_for_popover_quiescence<F, Fut>(
        &mut self,
        generation: u64,
        roots: &[gtk4::Widget],
        prove: F,
    ) -> Result<(), String>
    where
        F: FnMut(Vec<CaptureProofTarget>, Instant) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        quiescence::wait_for_popover_quiescence(generation, self, roots, prove).await
    }

    fn active_native_widget(&self) -> Option<gtk4::Widget> {
        self.native_widget().filter(widget_native_is_mapped)
    }

    fn native_widget(&self) -> Option<gtk4::Widget> {
        let native = self.content.widget().native()?;
        Some(native.upcast::<gtk4::Widget>())
    }

    fn sync_native_chrome(&self) {
        let Some(native) = self.native_widget() else {
            return;
        };
        if self.suppressed {
            native.add_css_class(CAPTURE_TRANSPARENT_CLASS);
        } else {
            native.remove_css_class(CAPTURE_TRANSPARENT_CLASS);
        }
        native.queue_draw();
    }

    #[cfg(test)]
    pub(super) fn installed_source_count(&self) -> usize {
        self.installed_sources
            .iter()
            .filter(|(_, source)| source.upgrade().is_some())
            .count()
    }

    #[cfg(test)]
    pub(super) fn transparent_content_state(&self) -> (Option<f64>, bool) {
        (self.content.content_opacity(), self.content.proof_visible())
    }
}

fn query_tooltip(
    content: &CaptureSurfaceContent,
    label: &gtk4::Label,
    intents: &ViewIntentSender,
    source_id: u64,
    source: &gtk4::Widget,
    tooltip: &gtk4::Tooltip,
) -> bool {
    let suppressed = content.root.has_css_class(TOOLTIP_SUPPRESSED_CLASS);
    if suppressed
        && (!content
            .root
            .has_css_class(TOOLTIP_MAPPED_BEFORE_CAPTURE_CLASS)
            || !source.has_css_class(TOOLTIP_SOURCE_ACTIVE_CLASS))
    {
        // Do not admit a new popup after capture suppression starts. A
        // tooltip that was already mapped is kept alive and made transparent.
        return false;
    }

    if let Some(text) = source.tooltip_text() {
        label.set_text(&text);
    } else if let Some(markup) = source.tooltip_markup() {
        label.set_markup(&markup);
    } else {
        return false;
    }

    if !suppressed {
        source.add_css_class(TOOLTIP_SOURCE_ACTIVE_CLASS);
        if !intents.send(ViewIntent::Tooltip(TooltipIntent::Activated {
            source: source_id,
        })) {
            return false;
        }
    }
    content.set_transparent(suppressed);
    tooltip.set_custom(Some(content.widget()));
    true
}

fn set_native_widget_input_enabled(widget: &gtk4::Widget, enabled: bool) {
    widget.set_can_target(enabled);
    if let Some(surface) = widget.native().and_then(|native| native.surface()) {
        if enabled {
            surface.set_input_region(None);
        } else {
            let empty = gtk4::cairo::Region::create();
            surface.set_input_region(Some(&empty));
        }
    }
    widget.queue_draw();
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
    wait_for_presented_transparency_until(
        generation,
        targets,
        Instant::now() + quiescence::CAPTURE_BARRIER_TIMEOUT,
    )
    .await
}

pub(super) async fn wait_for_presented_transparency_until(
    generation: u64,
    targets: Vec<CaptureProofTarget>,
    deadline: Instant,
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
        .any(CaptureProofTarget::required_but_unmapped)
    {
        if started.elapsed() >= CAPTURE_PAINT_TIMEOUT || Instant::now() >= deadline {
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
            deadline,
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
    deadline: Instant,
) -> Result<(), String> {
    let name = target.name;
    let widget = &target.widget;
    reject_withdrawn_target(generation, target, started)?;
    let frame_clock = widget.frame_clock().ok_or_else(|| {
        format!(
            "GTK capture suppression generation {generation} found no frame clock for mapped {name}"
        )
    })?;
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
    let rendered = after_next_surface_paint_counter(widget);
    tokio::pin!(rendered);

    let frame_counter = loop {
        tokio::select! {
            rendered = &mut rendered => {
                match rendered {
                    Some(counter) => break counter,
                    None => {
                        return Err(format!(
                            "GTK capture suppression generation {generation} lost the frame clock while rendering {name}"
                        ));
                    }
                }
            }
            _ = gtk4::glib::timeout_future(CAPTURE_PAINT_POLL_INTERVAL) => {
                reject_withdrawn_target(generation, target, started)?;
                if proof_started.elapsed() >= CAPTURE_PAINT_TIMEOUT || Instant::now() >= deadline {
                    return Err(format!(
                        "GTK capture suppression generation {generation} timed out waiting for a dedicated transparent render from {name}"
                    ));
                }
            }
        }
    };
    log::info!(
        "capture.preflight id={generation} component=gtk surface={name} phase=rendered frame_counter={frame_counter} sequence={ordinal}/{target_count} elapsed_ms={}",
        started.elapsed().as_millis()
    );

    loop {
        reject_withdrawn_target(generation, target, started)?;
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
        if proof_started.elapsed() >= CAPTURE_PAINT_TIMEOUT || Instant::now() >= deadline {
            return Err(format!(
                "generation {generation} received no compositor frame or presentation feedback for {name} after {} ms",
                proof_started.elapsed().as_millis()
            ));
        }
        gtk4::glib::timeout_future(CAPTURE_PAINT_POLL_INTERVAL).await;
    }
}

/// Unmapping is not proof of transparency: a compositor may animate the
/// popup's last visible buffer after GTK withdraws its surface. Only a target
/// that reaches compositor presentation may leave the barrier successfully.
fn reject_withdrawn_target(
    generation: u64,
    target: &CaptureProofTarget,
    started: Instant,
) -> Result<(), String> {
    if !target.is_withdrawn() {
        return Ok(());
    }

    target.mark_withdrawn();
    let error = format!(
        "GTK capture suppression generation {generation} cannot prove {} safe after it was withdrawn before transparent presentation",
        target.name
    );
    log::warn!(
        "capture.preflight id={generation} component=gtk surface={} phase=withdrawn-before-proof elapsed_ms={} error={error}",
        target.name,
        started.elapsed().as_millis()
    );
    Err(error)
}

#[cfg(test)]
mod tests {
    mod process;

    use std::sync::mpsc;

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
    fn capture_aware_gtk_popups_use_transparent_proof_surfaces() {
        const CHILD_ENV: &str = "WAYSCRIBER_GTK_POPUP_CAPTURE_CHILD";
        const TEST_NAME: &str = "toolbar_gtk::view::capture_suppression::tests::capture_aware_gtk_popups_use_transparent_proof_surfaces";

        if std::env::var_os(CHILD_ENV).is_none() {
            let status = process::run_isolated_gtk_test(CHILD_ENV, TEST_NAME);
            assert!(status.success(), "isolated GTK popup capture test failed");
            return;
        }

        if let Err(error) = gtk4::init() {
            eprintln!("skipping GTK popup capture test: {error}");
            return;
        }

        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let window = gtk4::Window::new();
        window.set_child(Some(&root));
        // GtkText creates its private popup against the nearest native. Realize
        // that native for the test without presenting or focusing a window.
        gtk4::prelude::WidgetExt::realize(&window);
        let button = gtk4::Button::new();
        button.set_tooltip_text(Some("Zoom"));
        root.append(&button);
        let entry = gtk4::Entry::new();
        root.append(&entry);
        let text = entry
            .first_child()
            .and_then(|child| child.downcast::<gtk4::Text>().ok())
            .expect("GtkEntry text delegate");
        text.activate_action("menu.popup", None)
            .expect("GtkText context-menu action");
        let context_popover = text
            .first_child()
            .and_then(|child| child.downcast::<gtk4::Popover>().ok())
            .expect("GtkText context popover");
        let context_menu = context_popover
            .clone()
            .downcast::<gtk4::PopoverMenu>()
            .expect("GtkText context popup is a GtkPopoverMenu");
        let original_context_child = context_popover.child().expect("GtkText popup child");
        let original_context_model = context_menu.menu_model().expect("GtkText menu model");

        let (intents, mut intent_rx, _failure_rx) = ViewIntentSender::channel();
        let mut capture = TooltipCapture::new(intents);
        capture.install_tree(root.upcast_ref());
        capture.install_tree(root.upcast_ref());
        assert_eq!(capture.installed_source_count(), 1);

        let tooltip = gtk4::glib::Object::builder::<gtk4::Tooltip>().build();
        assert!(button.emit_by_name::<bool>("query-tooltip", &[&0i32, &0i32, &false, &tooltip]));
        let activated = intent_rx
            .try_recv()
            .expect("the installed tooltip callback publishes its activation intent");
        assert!(matches!(activated, ViewIntent::Tooltip(_)));
        if let ViewIntent::Tooltip(activated) = activated {
            capture.handle_intent(activated);
        }
        assert_eq!(capture.label.text().as_str(), "Zoom");
        assert_eq!(capture.transparent_content_state(), (Some(1.0), false));

        capture.set_suppressed(true);
        capture.install_tree(root.upcast_ref());
        // A hidden realized toplevel cannot map its popup, so explicitly mark
        // and map the real GtkText-owned surface for this structural test. The
        // parent window remains hidden throughout.
        capture.enroll_native_popover(&context_popover, true);
        gtk4::prelude::WidgetExt::map(&context_popover);
        assert_eq!(
            context_popover.child().as_ref(),
            Some(&original_context_child),
            "capture suppression must preserve GtkPopoverMenu's class-owned child"
        );
        assert_eq!(capture.transparent_content_state(), (Some(0.0), true));
        assert!(
            context_popover.has_css_class(CAPTURE_TRANSPARENT_CLASS),
            "GTK-owned entry popovers must join capture suppression"
        );
        let popover_targets = capture.capture_popover_targets();
        assert_eq!(popover_targets.len(), 1);
        let popover_content = popover_targets[0].content.clone();
        assert_eq!(popover_content.content_opacity(), None);
        assert!(popover_content.proof_visible());
        assert_eq!(capture.pending_capture_popover_targets().len(), 1);
        capture.mark_capture_popovers_proven();
        assert!(capture.pending_capture_popover_targets().is_empty());

        // Exercise GtkPopoverMenu's class map/unmap hooks under
        // G_DEBUG=fatal-criticals. The reused native must also submit a fresh
        // proof after the remap.
        gtk4::prelude::WidgetExt::unmap(&context_popover);
        assert!(capture.capture_popover_targets().is_empty());
        gtk4::prelude::WidgetExt::map(&context_popover);
        assert_eq!(capture.pending_capture_popover_targets().len(), 1);
        assert_eq!(
            context_popover.child().as_ref(),
            Some(&original_context_child)
        );
        capture.mark_capture_popovers_proven();
        assert!(capture.pending_capture_popover_targets().is_empty());
        assert!(!context_popover.can_target());

        // A popup discovered by the final post-input rescan must receive its
        // own proof and remain non-interactive after input admission closes.
        let late_popover = gtk4::Popover::new();
        let late_child = gtk4::Label::new(Some("Late popup"));
        late_popover.set_child(Some(&late_child));
        capture.enroll_native_popover(&late_popover, true);
        assert_eq!(capture.capture_popover_targets().len(), 2);
        assert_eq!(capture.pending_capture_popover_targets().len(), 1);
        assert!(!late_popover.can_target());

        // GTK may reuse a plain popover while replacing its child. Re-enroll
        // that same native and restore the owner's latest child afterward.
        let replacement_late_child = gtk4::Label::new(Some("Replacement popup"));
        late_popover.set_child(Some(&replacement_late_child));
        capture.enroll_native_popover(&late_popover, true);
        assert!(
            late_popover
                .child()
                .as_ref()
                .is_some_and(CaptureSurfaceContent::is_wrapper)
        );
        let proof_before = capture.content.proof.paintable();
        let serial_before = capture.content.proof_serial();
        CaptureProofTarget::new("tooltip", &root, &capture.content).refresh_transparent_proof();
        assert_ne!(capture.content.proof_serial(), serial_before);
        assert_ne!(capture.content.proof.paintable(), proof_before);
        assert!(
            !capture.query_tooltip(button.upcast_ref(), &tooltip),
            "an unmapped tooltip must not appear after suppression starts"
        );

        // The mapped case uses the same query path but remains admitted so its
        // existing native popup can submit the transparent proof.
        capture
            .content
            .root
            .add_css_class(TOOLTIP_MAPPED_BEFORE_CAPTURE_CLASS);
        assert!(capture.query_tooltip(button.upcast_ref(), &tooltip));

        capture.set_suppressed(false);
        assert_eq!(capture.transparent_content_state(), (Some(1.0), false));
        assert!(!context_popover.has_css_class(CAPTURE_TRANSPARENT_CLASS));
        assert_eq!(popover_content.content_opacity(), None);
        assert!(!popover_content.proof_visible());
        assert!(context_popover.can_target());
        assert_eq!(context_popover.child(), Some(original_context_child));
        assert_eq!(context_menu.menu_model(), Some(original_context_model));
        assert_eq!(
            late_popover.child(),
            Some(replacement_late_child.upcast::<gtk4::Widget>())
        );
        assert!(capture.capture_popover_targets().is_empty());

        capture
            .content
            .root
            .add_css_class(TOOLTIP_MAPPED_BEFORE_CAPTURE_CLASS);
        let closed_target = CaptureProofTarget::new_withdrawable_with_cleanup(
            "closed-native-popover",
            &root,
            &capture.content,
            CaptureProofWithdrawal::RemoveContentClass(TOOLTIP_MAPPED_BEFORE_CAPTURE_CLASS),
        );
        let error = gtk4::glib::MainContext::default()
            .block_on(wait_for_presented_transparency_until(
                90,
                vec![closed_target],
                Instant::now() + Duration::from_millis(20),
            ))
            .expect_err("an unproven withdrawn popup must cancel capture");
        assert!(
            error.contains("withdrawn before transparent presentation"),
            "unexpected withdrawal error: {error}"
        );
        assert!(
            !capture
                .content
                .root
                .has_css_class(TOOLTIP_MAPPED_BEFORE_CAPTURE_CLASS)
        );

        // Exercise the production discovery path across a main-context yield.
        // The GtkText-owned popover does not exist during the first scan and
        // must join the barrier without a direct enroll_native_popover() call.
        let asynchronous_root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let asynchronous_window = gtk4::Window::new();
        asynchronous_window.set_child(Some(&asynchronous_root));
        gtk4::prelude::WidgetExt::realize(&asynchronous_window);
        let asynchronous_entry = gtk4::Entry::new();
        let second_asynchronous_entry = gtk4::Entry::new();
        asynchronous_root.append(&asynchronous_entry);
        asynchronous_root.append(&second_asynchronous_entry);
        let asynchronous_text = asynchronous_entry
            .first_child()
            .and_then(|child| child.downcast::<gtk4::Text>().ok())
            .expect("asynchronous GtkEntry text delegate");
        let second_asynchronous_text = second_asynchronous_entry
            .first_child()
            .and_then(|child| child.downcast::<gtk4::Text>().ok())
            .expect("second asynchronous GtkEntry text delegate");
        let (asynchronous_intents, _asynchronous_intent_rx, _asynchronous_failure_rx) =
            ViewIntentSender::channel();
        let mut asynchronous_capture = TooltipCapture::new(asynchronous_intents);
        asynchronous_capture.set_suppressed(true);
        asynchronous_capture.install_tree(asynchronous_root.upcast_ref());

        let (created_popovers_tx, created_popovers_rx) = mpsc::channel();
        let first_created_popover_tx = created_popovers_tx.clone();
        gtk4::glib::timeout_add_local_once(Duration::from_millis(10), move || {
            first_created_popover_tx
                .send(open_hidden_test_text_popover(&asynchronous_text))
                .expect("fixture popup observer remains connected");
        });

        let (proof_calls_tx, proof_calls_rx) = mpsc::channel();
        let mut first_proof = true;
        let roots = [asynchronous_root.clone().upcast::<gtk4::Widget>()];
        gtk4::glib::MainContext::default()
            .block_on(asynchronous_capture.wait_for_popover_quiescence(
                91,
                &roots,
                move |targets, _deadline| {
                    if first_proof {
                        first_proof = false;
                        let text = second_asynchronous_text.clone();
                        let popovers = created_popovers_tx.clone();
                        gtk4::glib::timeout_add_local_once(Duration::from_millis(10), move || {
                            popovers
                                .send(open_hidden_test_text_popover(&text))
                                .expect("fixture popup observer remains connected");
                        });
                    }
                    let proof_calls = proof_calls_tx.clone();
                    async move {
                        assert_eq!(targets.len(), 1);
                        assert_eq!(targets[0].name, "gtk-owned-popover");
                        // Real presentation proof yields the GTK main context.
                        // Mirror that so the second popup is created during
                        // the first proof round, after its discovery scan.
                        gtk4::glib::timeout_future(Duration::from_millis(20)).await;
                        proof_calls
                            .send(())
                            .expect("fixture proof observer remains connected");
                        Ok(())
                    }
                },
            ))
            .expect("asynchronous GTK-owned popup reaches quiescence");

        let proof_calls = proof_calls_rx.try_iter().count();
        let created_popovers = created_popovers_rx.try_iter().collect::<Vec<_>>();
        assert_eq!(proof_calls, 2);
        assert_eq!(created_popovers.len(), 2);
        for (popover, _) in &created_popovers {
            assert!(popover.has_css_class(CAPTURE_TRANSPARENT_CLASS));
            assert!(!popover.can_target());
        }
        assert!(
            asynchronous_capture
                .pending_capture_popover_targets()
                .is_empty()
        );
        asynchronous_capture.set_suppressed(false);
        for (popover, original_child) in &created_popovers {
            assert!(!popover.has_css_class(CAPTURE_TRANSPARENT_CLASS));
            assert!(popover.can_target());
            assert_eq!(popover.child().as_ref(), Some(original_child));
        }
    }

    fn open_hidden_test_text_popover(text: &gtk4::Text) -> (gtk4::Popover, gtk4::Widget) {
        text.activate_action("menu.popup", None)
            .expect("asynchronous GtkText context-menu action");
        let popover = text
            .first_child()
            .and_then(|child| child.downcast::<gtk4::Popover>().ok())
            .expect("asynchronous GtkText context popover");
        let original_child = popover.child().expect("GtkText popup child");
        // Keep the toplevel hidden so this test never foregrounds a window,
        // but mark the native popup mapped to exercise the production
        // visibility predicate and discovery path.
        gtk4::prelude::WidgetExt::map(&popover);
        (popover, original_child)
    }
}
