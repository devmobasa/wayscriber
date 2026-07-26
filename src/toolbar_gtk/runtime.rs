//! GTK-thread runtime: owns `gtk4::init`, the GLib main loop, and the
//! toolbar windows. Nothing in here touches backend state; all traffic
//! goes through the bridge channels.

use std::fmt;
use std::future::Future;
use std::sync::mpsc::{self, TryRecvError};

use gtk4::glib;
use tokio::sync::oneshot;

use super::GtkToolbarUpdate;
use super::bridge::{FeedbackPublisher, LatestValueReceiver};

pub(super) enum RuntimeExit {
    Clean,
    Failed(String),
}

enum UpdateLoopExit {
    BackendClosed,
    ShutdownRequested,
    UpdateFailed(String),
}

enum MonitoredTaskError {
    Task(glib::JoinError),
    Supervisor(glib::JoinError),
    MainLoopExited,
}

impl fmt::Display for MonitoredTaskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Task(error) => write!(formatter, "local task failed: {error}"),
            Self::Supervisor(error) => write!(formatter, "task supervisor failed: {error}"),
            Self::MainLoopExited => formatter.write_str("GTK main loop exited before its task"),
        }
    }
}

/// Runs one GLib-local future without detaching either its task handle or its
/// supervisor. An unexpected main-loop exit explicitly destroys the task
/// source before consuming the supervisor handle.
fn run_monitored_local<F, T>(
    context: &glib::MainContext,
    main_loop: &glib::MainLoop,
    future: F,
) -> Result<T, MonitoredTaskError>
where
    F: Future<Output = T> + 'static,
    T: 'static,
{
    let task = context.spawn_local(future);
    let task_source = task.source().clone();
    let (result_tx, result_rx) = mpsc::channel();
    let loop_handle = main_loop.clone();
    let supervisor = context.spawn_local(async move {
        let result = task.await;
        let _ = result_tx.send(result);
        loop_handle.quit();
    });

    main_loop.run();
    let observed = result_rx.try_recv();
    let main_loop_exited = matches!(
        observed,
        Err(TryRecvError::Empty | TryRecvError::Disconnected)
    );
    if main_loop_exited {
        task_source.destroy();
        supervisor.abort();
        return Err(MonitoredTaskError::MainLoopExited);
    }

    if let Err(error) = context.block_on(supervisor) {
        return Err(MonitoredTaskError::Supervisor(error));
    }

    match observed {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(MonitoredTaskError::Task(error)),
        Err(TryRecvError::Empty | TryRecvError::Disconnected) => {
            Err(MonitoredTaskError::MainLoopExited)
        }
    }
}

async fn update_windows(
    mut updates: LatestValueReceiver<GtkToolbarUpdate>,
    feedback: FeedbackPublisher,
) -> UpdateLoopExit {
    let Some(update) = updates.recv().await else {
        return UpdateLoopExit::BackendClosed;
    };
    let (intents, mut intent_rx, mut intent_failures) =
        super::view::drag::ViewIntentSender::channel();
    let mut windows =
        super::view::Windows::new(super::widgets::FeedbackSender::new(feedback), intents);
    if let Err(err) = windows.apply(&update).await {
        return UpdateLoopExit::UpdateFailed(err.to_string());
    }
    loop {
        tokio::select! {
            biased;
            failure = intent_failures.recv() => {
                return UpdateLoopExit::UpdateFailed(failure.unwrap_or_else(|| {
                    "GTK view-intent failure channel closed unexpectedly".to_string()
                }));
            }
            intent = intent_rx.recv() => {
                let Some(intent) = intent else {
                    return UpdateLoopExit::UpdateFailed(
                        "GTK view-intent channel closed unexpectedly".to_string(),
                    );
                };
                if let Err(error) = windows.handle_intent(intent).await {
                    return UpdateLoopExit::UpdateFailed(error);
                }
            }
            next_update = updates.recv() => {
                let Some(next_update) = next_update else {
                    return UpdateLoopExit::BackendClosed;
                };
                if let Err(err) = windows.apply(&next_update).await {
                    return UpdateLoopExit::UpdateFailed(err.to_string());
                }
            }
        }
    }
}

pub(super) fn run(
    updates: LatestValueReceiver<GtkToolbarUpdate>,
    mut shutdown: oneshot::Receiver<()>,
    feedback: FeedbackPublisher,
) -> RuntimeExit {
    match shutdown.try_recv() {
        Ok(()) | Err(oneshot::error::TryRecvError::Closed) => return RuntimeExit::Clean,
        Err(oneshot::error::TryRecvError::Empty) => {}
    }

    if let Err(err) = gtk4::init() {
        return RuntimeExit::Failed(format!(
            "GTK toolbars unavailable: gtk4::init failed ({err}); restoring built-in toolbars"
        ));
    }
    if !gtk4_layer_shell::is_supported() {
        return RuntimeExit::Failed(
            "GTK toolbars unavailable: gtk4-layer-shell reports no compositor support; restoring built-in toolbars"
                .into(),
        );
    }

    let context = glib::MainContext::default();
    let main_loop = glib::MainLoop::new(Some(&context), false);
    let task = async move {
        tokio::select! {
            biased;
            _ = &mut shutdown => UpdateLoopExit::ShutdownRequested,
            exit = update_windows(updates, feedback) => exit,
        }
    };

    match run_monitored_local(&context, &main_loop, task) {
        Ok(UpdateLoopExit::BackendClosed | UpdateLoopExit::ShutdownRequested) => RuntimeExit::Clean,
        Ok(UpdateLoopExit::UpdateFailed(error)) => RuntimeExit::Failed(format!(
            "GTK toolbar update failed ({error}); restoring built-in toolbars"
        )),
        Err(error) => RuntimeExit::Failed(format!(
            "GTK toolbar runtime failed ({error}); restoring built-in toolbars"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DropSignal(mpsc::Sender<()>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }

    #[test]
    fn monitored_local_task_returns_its_value_and_consumes_supervisor() {
        let context = glib::MainContext::new();
        let main_loop = glib::MainLoop::new(Some(&context), false);

        assert!(matches!(
            run_monitored_local(&context, &main_loop, async { 42 }),
            Ok(42)
        ));
    }

    #[test]
    fn panicking_local_task_reports_its_terminal_result() {
        let context = glib::MainContext::new();
        let main_loop = glib::MainLoop::new(Some(&context), false);

        let result = run_monitored_local(&context, &main_loop, async {
            panic!("expected GTK update-loop panic");
        });
        assert!(matches!(result, Err(MonitoredTaskError::Task(_))));
    }

    #[test]
    fn unexpected_main_loop_exit_cancels_task_before_returning() {
        let context = glib::MainContext::new();
        let main_loop = glib::MainLoop::new(Some(&context), false);
        let quit = main_loop.clone();
        let (dropped_tx, dropped_rx) = mpsc::channel();

        let result = run_monitored_local(&context, &main_loop, async move {
            let _drop_signal = DropSignal(dropped_tx);
            quit.quit();
            std::future::pending::<()>().await;
        });
        assert!(matches!(result, Err(MonitoredTaskError::MainLoopExited)));
        assert_eq!(dropped_rx.try_recv(), Ok(()));
    }
}
