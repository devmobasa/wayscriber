//! GTK-thread runtime: owns `gtk4::init`, the GLib main loop, and the
//! toolbar windows. Nothing in here touches backend state; all traffic
//! goes through the bridge channels.

use gtk4::glib;

use super::GtkToolbarUpdate;
use super::bridge::{BridgeHealth, FeedbackPublisher};

/// Flags the bridge as failed unless the shutdown was the clean
/// channel-close path — covers panics anywhere on this thread and an
/// unexpectedly exiting main loop, so the backend falls back to the
/// built-in bars instead of running headless.
struct FailureGuard {
    health: BridgeHealth,
    clean: std::cell::Cell<bool>,
}

impl Drop for FailureGuard {
    fn drop(&mut self) {
        if !self.clean.get() {
            self.health
                .fail("GTK toolbar thread exited unexpectedly; restoring built-in toolbars");
        }
    }
}

pub(super) fn run(
    updates: async_channel::Receiver<GtkToolbarUpdate>,
    feedback: FeedbackPublisher,
    health: BridgeHealth,
) {
    let guard = std::rc::Rc::new(FailureGuard {
        health: health.clone(),
        clean: std::cell::Cell::new(false),
    });

    if health.stopping() {
        guard.clean.set(true);
        return;
    }

    if let Err(err) = gtk4::init() {
        guard.clean.set(true);
        health.fail(format!(
            "GTK toolbars unavailable: gtk4::init failed ({err}); restoring built-in toolbars"
        ));
        return;
    }
    if !gtk4_layer_shell::is_supported() {
        guard.clean.set(true);
        health.fail(
            "GTK toolbars unavailable: gtk4-layer-shell reports no compositor support; restoring built-in toolbars",
        );
        return;
    }

    if !health.mark_ready() {
        guard.clean.set(true);
        return;
    }

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
    if !guard.clean.get() {
        health.fail("GTK toolbar main loop returned unexpectedly; restoring built-in toolbars");
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;

    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;

    #[test]
    fn unexpected_runtime_exit_publishes_failure_before_wake() {
        let wake = RuntimeWakeSource::new().unwrap();
        let health = BridgeHealth::new(wake.handle());
        drop(FailureGuard {
            health: health.clone(),
            clean: std::cell::Cell::new(false),
        });

        let mut pollfd = libc::pollfd {
            fd: wake.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: pollfd and the source descriptor are valid for this non-blocking poll.
        let ready = unsafe { libc::poll(&mut pollfd, 1, 0) };
        assert_eq!(ready, 1);
        assert!(health.failed());
        assert_ne!(pollfd.revents & libc::POLLIN, 0);
    }

    #[test]
    fn clean_runtime_exit_does_not_publish_failure() {
        let wake = RuntimeWakeSource::new().unwrap();
        let health = BridgeHealth::new(wake.handle());
        drop(FailureGuard {
            health: health.clone(),
            clean: std::cell::Cell::new(true),
        });

        assert!(!health.failed());
        let mut pollfd = libc::pollfd {
            fd: wake.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: pollfd and the source descriptor are valid for this non-blocking poll.
        assert_eq!(unsafe { libc::poll(&mut pollfd, 1, 0) }, 0);
    }
}
