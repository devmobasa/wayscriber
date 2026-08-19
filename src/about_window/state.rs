//! About-window state: focus, hover, and the actions the handlers trigger.

use log::{debug, warn};
use smithay_client_toolkit::seat::pointer::CursorIcon;
use wayland_client::Connection;

use super::content::{AboutAction, AboutContent, UpdateState};
use super::interaction::{self, Element};
use super::layout::Plan;
use super::{AboutWindowState, clipboard, icon, surface_size};

/// How the footer acknowledges an action that has no visible result of its own.
const COPIED_NOTICE: &str = "Copied to clipboard";
const OPENING_NOTICE: &str = "Opening in your browser";
const OPEN_FAILED_NOTICE: &str = "Could not open your browser — see logs";
const REPORTED_NOTICE: &str = "Diagnostics copied — opening browser";
const REPORT_OPEN_FAILED_NOTICE: &str = "Diagnostics copied — browser open failed";

impl AboutWindowState {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        registry_state: super::RegistryState,
        compositor_state: super::CompositorState,
        shm: super::Shm,
        output_state: super::OutputState,
        seat_state: super::SeatState,
        xdg_shell: super::XdgShell,
        window: super::Window,
        content: AboutContent,
        plan: Plan,
    ) -> Self {
        // Opening the dialog costs no network: the row reports whatever the
        // last background check wrote, and the user can ask for a fresh one.
        let update = UpdateState::from_cache(crate::update_check::cached_status());
        let elements = interaction::focus_order(&content, &update);
        let (width, height) = surface_size(&plan);

        Self {
            registry_state,
            compositor_state,
            shm,
            output_state,
            seat_state,
            xdg_shell,
            window,
            pool: None,
            width,
            height,
            scale: 1,
            configured: false,
            should_exit: false,
            needs_redraw: true,
            check_requested: false,
            helper_workers: Vec::new(),
            content,
            plan,
            update,
            elements,
            hover: None,
            focus: None,
            shift_held: false,
            notice: None,
            icon: icon::load(),
            themed_pointer: None,
        }
    }

    /// Logical size this dialog wants, used as the configure fallback.
    pub(super) fn preferred_size(&self) -> (u32, u32) {
        surface_size(&self.plan)
    }

    pub(super) fn element_at(&self, position: (f64, f64)) -> Option<Element> {
        interaction::element_at(&self.elements, &self.plan, position)
            .and_then(|index| self.elements.get(index).copied())
    }

    pub(super) fn update_hover(&mut self, position: (f64, f64)) {
        self.set_hover(self.element_at(position));
    }

    pub(super) fn set_hover(&mut self, next: Option<Element>) {
        if self.hover == next {
            return;
        }
        self.hover = next;
        // Moving to a different element is the acknowledgement that the notice
        // has been read.
        self.notice = None;
        self.needs_redraw = true;
    }

    /// Put keyboard focus on `element`, so a Tab after a click continues from
    /// where the pointer left off.
    pub(super) fn focus_element(&mut self, element: Element) {
        if self.focus == Some(element) {
            return;
        }
        self.focus = Some(element);
        self.needs_redraw = true;
    }

    /// Move keyboard focus by `delta` positions, wrapping at both ends.
    pub(super) fn move_focus(&mut self, delta: i32) {
        let current = self
            .focus
            .and_then(|element| interaction::index_of(&self.elements, element));
        self.focus = interaction::step_focus(current, self.elements.len(), delta)
            .and_then(|index| self.elements.get(index).copied());
        self.notice = None;
        self.needs_redraw = true;
    }

    /// Activate the focused element, if any.
    pub(super) fn activate_focus(&mut self) {
        if let Some(element) = self.focus {
            self.activate(element);
        }
    }

    pub(super) fn activate(&mut self, element: Element) {
        let Some(action) = interaction::action_for(element, &self.content, &self.update) else {
            return;
        };
        self.perform(action);
    }

    fn perform(&mut self, action: AboutAction) {
        match action {
            AboutAction::OpenUrl(url) => match clipboard::open_url(&url) {
                Ok(worker) => {
                    self.track_helper_worker(worker);
                    self.set_notice(OPENING_NOTICE);
                }
                Err(err) => {
                    warn!("About dialog refused or failed to open a URL: {err:#}");
                    self.set_notice(OPEN_FAILED_NOTICE);
                }
            },
            AboutAction::CopyText(text) => {
                if let Some(worker) = clipboard::copy_text_to_clipboard(&text) {
                    self.track_helper_worker(worker);
                }
                self.set_notice(COPIED_NOTICE);
            }
            // Copy as well as open: the URL carries the same diagnostics in its
            // fragment, but a browser that never launches, or a form that drops
            // the prefill, still leaves them one paste away.
            AboutAction::ReportBug { url, diagnostics } => {
                // Start the desktop-open worker before the independent clipboard
                // publication worker.
                let opened = clipboard::open_url(&url);
                if let Some(worker) = clipboard::copy_text_to_clipboard(&diagnostics) {
                    self.track_helper_worker(worker);
                }
                match opened {
                    Ok(worker) => {
                        self.track_helper_worker(worker);
                        self.set_notice(REPORTED_NOTICE);
                    }
                    Err(err) => {
                        warn!("About dialog failed to open the report URL: {err:#}");
                        self.set_notice(REPORT_OPEN_FAILED_NOTICE);
                    }
                }
            }
            AboutAction::CheckForUpdates => self.begin_update_check(),
            AboutAction::Close => self.should_exit = true,
        }
    }

    fn track_helper_worker(&mut self, worker: std::thread::JoinHandle<()>) {
        self.helper_workers.push(worker);
    }

    /// Finish in-flight open/copy workers before the process broker tears down.
    pub(super) fn join_helper_workers(&mut self) {
        for worker in self.helper_workers.drain(..) {
            let _ = worker.join();
        }
    }

    fn begin_update_check(&mut self) {
        self.set_update(UpdateState::Checking);
        self.notice = None;
        self.check_requested = true;
    }

    /// Perform the check synchronously. Called from the event loop, never from a
    /// protocol handler.
    pub(super) fn run_update_check(&mut self) {
        let next = match crate::update_check::check_now() {
            Ok(crate::update_check::CheckOutcome::Update(update)) => UpdateState::Available {
                update: Box::new(update),
                freshness: crate::update_check::Freshness {
                    checked_seconds_ago: Some(0),
                    last_attempt_failed: false,
                },
            },
            Ok(crate::update_check::CheckOutcome::UpToDate { .. }) => {
                UpdateState::UpToDate(crate::update_check::Freshness {
                    checked_seconds_ago: Some(0),
                    last_attempt_failed: false,
                })
            }
            Err(err) => {
                debug!("About dialog update check failed: {err}");
                UpdateState::Failed(err)
            }
        };
        self.set_update(next);
    }

    fn set_update(&mut self, update: UpdateState) {
        self.update = update;
        // The card leaves the focus order while a check runs, so the order has
        // to be rebuilt alongside the state it is derived from.
        self.elements = interaction::focus_order(&self.content, &self.update);
        self.needs_redraw = true;
    }

    fn set_notice(&mut self, notice: &str) {
        self.notice = Some(notice.to_string());
        self.needs_redraw = true;
    }

    pub(super) fn update_cursor(&self, conn: &Connection) {
        if let Some(pointer) = self.themed_pointer.as_ref() {
            let cursor = if self.hover.is_some() {
                CursorIcon::Pointer
            } else {
                CursorIcon::Default
            };
            if let Err(err) = pointer.set_cursor(conn, cursor) {
                debug!("Failed to set cursor icon: {err}");
            }
        }
    }
}
