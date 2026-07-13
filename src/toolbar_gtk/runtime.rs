//! GTK-thread runtime: owns `gtk4::init`, the GLib main loop, and the
//! toolbar windows. Nothing in here touches backend state; all traffic
//! goes through the bridge channels.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use gtk4::glib;

use super::enabled::{STATUS_FAILED, STATUS_READY};
use super::{GtkToolbarFeedback, GtkToolbarUpdate};

/// Flags the bridge as failed unless the shutdown was the clean
/// channel-close path — covers panics anywhere on this thread and an
/// unexpectedly exiting main loop, so the backend falls back to the
/// built-in bars instead of running headless.
struct FailureGuard {
    status: Arc<AtomicU8>,
    clean: std::cell::Cell<bool>,
}

impl Drop for FailureGuard {
    fn drop(&mut self) {
        if !self.clean.get() {
            log::warn!("GTK toolbar thread exited unexpectedly");
            self.status.store(STATUS_FAILED, Ordering::Release);
        }
    }
}

pub(super) fn run(
    updates: async_channel::Receiver<GtkToolbarUpdate>,
    feedback: std::sync::mpsc::Sender<GtkToolbarFeedback>,
    status: Arc<AtomicU8>,
) {
    let guard = std::rc::Rc::new(FailureGuard {
        status: status.clone(),
        clean: std::cell::Cell::new(false),
    });

    if let Err(err) = gtk4::init() {
        log::warn!("GTK toolbars unavailable: gtk4::init failed: {err}");
        guard.clean.set(true);
        status.store(STATUS_FAILED, Ordering::Release);
        return;
    }
    if !gtk4_layer_shell::is_supported() {
        log::warn!("GTK toolbars unavailable: gtk4-layer-shell reports no compositor support");
        guard.clean.set(true);
        status.store(STATUS_FAILED, Ordering::Release);
        return;
    }

    status.store(STATUS_READY, Ordering::Release);

    let main_loop = glib::MainLoop::new(None, false);
    let loop_handle = main_loop.clone();
    let loop_guard = guard.clone();
    glib::MainContext::default().spawn_local(async move {
        let mut windows: Option<super::view::Windows> = None;
        while let Ok(update) = updates.recv().await {
            windows
                .get_or_insert_with(|| {
                    super::view::Windows::new(super::widgets::FeedbackSender::new(feedback.clone()))
                })
                .apply(&update);
        }
        // The backend dropped the bridge; shut the GTK side down with it.
        loop_guard.clean.set(true);
        loop_handle.quit();
    });
    main_loop.run();
}
