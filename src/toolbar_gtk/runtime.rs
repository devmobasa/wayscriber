//! GTK-thread runtime: owns `gtk4::init`, the GLib main loop, and the
//! toolbar windows. Nothing in here touches backend state; all traffic
//! goes through the bridge channels.

use gtk4::glib;
use std::future::Future;

use super::GtkToolbarUpdate;
use super::bridge::{BridgeHealth, FeedbackPublisher, LatestValueReceiver};

/// Flags the bridge as failed unless the shutdown was the clean
/// channel-close path. The update task is supervised separately so both task
/// panics and an unexpectedly exiting main loop restore the built-in bars.
struct FailureGuard {
    health: BridgeHealth,
    clean: std::cell::Cell<bool>,
}

fn spawn_monitored_local<F, C>(context: &glib::MainContext, future: F, completed: C)
where
    F: Future<Output = ()> + 'static,
    C: FnOnce(Result<(), glib::JoinError>) + 'static,
{
    let task = context.spawn_local(future);
    // GLib catches task panics and reports them only through JoinHandle. Keep a
    // detached supervisor on the same context so every terminal result reaches
    // bridge health instead of silently freezing the GTK frontend.
    drop(context.spawn_local(async move {
        completed(task.await);
    }));
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
    mut updates: LatestValueReceiver<GtkToolbarUpdate>,
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
    spawn_monitored_local(
        &glib::MainContext::default(),
        async move {
            let mut windows: Option<super::view::Windows> = None;
            while let Some(update) = updates.recv().await {
                windows
                    .get_or_insert_with(|| {
                        super::view::Windows::new(super::widgets::FeedbackSender::new(
                            feedback.clone(),
                        ))
                    })
                    .apply(&update);
            }
        },
        move |result| {
            match result {
                Ok(()) => {
                    // The backend dropped the bridge; shut the GTK side down with it.
                    loop_guard.clean.set(true);
                }
                Err(err) => {
                    loop_guard.health.fail(format!(
                        "GTK toolbar update loop failed ({err}); restoring built-in toolbars"
                    ));
                    // The supervised task already published the terminal state;
                    // suppress the two fallback reports after MainLoop::run.
                    loop_guard.clean.set(true);
                }
            }
            loop_handle.quit();
        },
    );
    main_loop.run();
    if !guard.clean.get() {
        health.fail("GTK toolbar main loop returned unexpectedly; restoring built-in toolbars");
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::os::fd::AsRawFd;
    use std::rc::Rc;

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

    #[test]
    fn panicking_local_task_reports_its_terminal_result() {
        let context = glib::MainContext::new();
        let _owner = context.acquire().expect("test owns main context");
        let completed = Rc::new(Cell::new(None));
        let observed = Rc::clone(&completed);

        spawn_monitored_local(
            &context,
            async { panic!("expected GTK update-loop panic") },
            move |result| observed.set(Some(result.is_err())),
        );
        while context.pending() {
            context.iteration(false);
        }

        assert_eq!(
            completed.get(),
            Some(true),
            "a detached GLib task hides its panic from bridge health"
        );
    }
}
