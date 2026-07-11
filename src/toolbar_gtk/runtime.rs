//! GTK-thread runtime: owns `gtk4::init`, the GLib main loop, and the
//! toolbar windows. Nothing in here touches backend state; all traffic
//! goes through the bridge channels.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use gtk4::glib;

use super::enabled::{STATUS_FAILED, STATUS_READY};
use super::{GtkToolbarFeedback, GtkToolbarUpdate};

pub(super) fn run(
    updates: async_channel::Receiver<GtkToolbarUpdate>,
    feedback: std::sync::mpsc::Sender<GtkToolbarFeedback>,
    status: Arc<AtomicU8>,
) {
    if let Err(err) = gtk4::init() {
        log::warn!("GTK toolbars unavailable: gtk4::init failed: {err}");
        status.store(STATUS_FAILED, Ordering::Release);
        return;
    }
    if !gtk4_layer_shell::is_supported() {
        log::warn!("GTK toolbars unavailable: gtk4-layer-shell reports no compositor support");
        status.store(STATUS_FAILED, Ordering::Release);
        return;
    }

    status.store(STATUS_READY, Ordering::Release);

    let main_loop = glib::MainLoop::new(None, false);
    let loop_handle = main_loop.clone();
    glib::MainContext::default().spawn_local(async move {
        let mut windows: Option<super::view::Windows> = None;
        while let Ok(update) = updates.recv().await {
            windows
                .get_or_insert_with(|| super::view::Windows::new(feedback.clone()))
                .apply(&update);
        }
        // The backend dropped the bridge; shut the GTK side down with it.
        loop_handle.quit();
    });
    main_loop.run();
}
